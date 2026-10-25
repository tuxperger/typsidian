//! SSH для libgit2 через системный клиент `ssh` (research §2; поправка
//! конституции 1.2.0, «Внешние процессы»).
//!
//! Процесс запускается только с аргументами, заданными здесь: `BatchMode` (никаких
//! вопросов в терминале, которого у приложения нет), тайм-аут соединения, адрес
//! и команда протокола git. Его stdin/stdout — исключительно канал протокола.
//! Ключи, агент, `known_hosts` и `~/.ssh/config` — пользователя; приложение их
//! не читает и не хранит.
//!
//! Только рабочий стол: на Android системного `ssh` нет, и адрес `ssh://`
//! отклоняется при подключении (FR-014a).

use std::io::{Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use git2::transport::{Service, SmartSubtransport, SmartSubtransportStream};

use super::transport_http::{current_context, record_failure};
use super::EngineError;

/// Сколько последних байт `stderr` процесса `ssh` хранится (T077).
const STDERR_LIMIT: usize = 64 * 1024;

/// Разобранный адрес `ssh://[user@]host[:port]/path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshTarget {
    pub user: Option<String>,
    pub host: String,
    pub port: Option<u16>,
    /// Путь для команды на сервере: `/repo.git` или `~/repo.git`.
    pub path: String,
}

/// Разбирает адрес и отвергает всё, что могло бы стать опцией `ssh` или
/// командой оболочки на сервере.
pub fn parse_target(url: &str) -> Option<SshTarget> {
    let rest = url.strip_prefix("ssh://")?;
    let (authority, path) = rest.split_once('/')?;
    let (user, hostport) = match authority.rsplit_once('@') {
        Some((user, host)) => (Some(user.to_string()), host),
        None => (None, authority),
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((host, port)) => (host, Some(port.parse().ok()?)),
        None => (hostport, None),
    };
    let token_ok = |value: &str| {
        !value.is_empty()
            && !value.starts_with('-')
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    };
    if !token_ok(host) || user.as_deref().is_some_and(|user| !token_ok(user)) {
        return None;
    }
    let path = format!("/{path}");
    // `ssh://host/~/repo` — путь от домашнего каталога, как у git.
    let path = path
        .strip_prefix("/~")
        .map(|rest| format!("~{rest}"))
        .unwrap_or(path);
    if path.chars().any(|c| c.is_control()) || path.len() < 2 {
        return None;
    }
    Some(SshTarget {
        user,
        host: host.to_string(),
        port,
        path,
    })
}

/// Путь в одинарных кавычках для команды на сервере.
fn quote(path: &str) -> String {
    format!("'{}'", path.replace('\'', "'\\''"))
}

struct Channel {
    child: Child,
    stdin: ChildStdin,
    /// Вывод `ssh` читает свой поток (T080): чтение не блокируется насмерть, и
    /// зависший процесс прерывается отменой и пределом простоя.
    stdout: Receiver<Vec<u8>>,
    /// Остаток куска, не поместившийся в буфер прошлого чтения.
    pending: Vec<u8>,
    stderr: Arc<Mutex<Vec<u8>>>,
}

/// Как часто чтение просыпается проверить отмену и предел простоя.
const READ_TICK: Duration = Duration::from_millis(100);

/// Почему чтение прервано.
#[derive(Debug, PartialEq, Eq)]
enum Interrupted {
    Cancelled,
    Idle,
}

/// Следующий кусок вывода: ждёт тактами, просыпаясь проверить отмену и предел
/// простоя. `Ok(None)` — вывод закончился.
fn next_chunk(
    stdout: &Receiver<Vec<u8>>,
    cancel: &dyn Fn() -> bool,
    idle_limit: Duration,
) -> Result<Option<Vec<u8>>, Interrupted> {
    let started = Instant::now();
    loop {
        if cancel() {
            return Err(Interrupted::Cancelled);
        }
        match stdout.recv_timeout(READ_TICK) {
            Ok(chunk) => return Ok(Some(chunk)),
            Err(RecvTimeoutError::Disconnected) => return Ok(None),
            Err(RecvTimeoutError::Timeout) if started.elapsed() >= idle_limit => {
                return Err(Interrupted::Idle)
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Подтранспорт: один процесс `ssh` на обмен — объявление ссылок и сам
/// пакет идут по одному каналу (не-RPC протокол git).
#[derive(Default)]
pub struct SshSubtransport {
    channel: Mutex<Option<Arc<Mutex<Channel>>>>,
}

impl SshSubtransport {
    fn spawn(target: &SshTarget, command: &str) -> std::io::Result<Channel> {
        let mut ssh = Command::new("ssh");
        ssh.args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "ServerAliveInterval=15",
        ]);
        if let Some(port) = target.port {
            ssh.arg("-p").arg(port.to_string());
        }
        let destination = match &target.user {
            Some(user) => format!("{user}@{}", target.host),
            None => target.host.clone(),
        };
        ssh.arg(destination)
            .arg(format!("{command} {}", quote(&target.path)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = ssh.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| std::io::Error::other("нет stdin"))?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::other("нет stdout"))?;
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut chunk = [0u8; 16 * 1024];
            // Конец вывода или его ошибка закрывают канал — читатель видит конец.
            while let Ok(read) = stdout.read(&mut chunk) {
                if read == 0 || sender.send(chunk[..read].to_vec()).is_err() {
                    break;
                }
            }
        });
        let stderr_buffer = Arc::new(Mutex::new(Vec::new()));
        if let Some(mut stderr) = child.stderr.take() {
            let sink = stderr_buffer.clone();
            std::thread::spawn(move || {
                let mut chunk = [0u8; 1024];
                while let Ok(read) = stderr.read(&mut chunk) {
                    if read == 0 {
                        break;
                    }
                    if let Ok(mut sink) = sink.lock() {
                        keep_tail(&mut sink, &chunk[..read], STDERR_LIMIT);
                    }
                }
            });
        }
        Ok(Channel {
            child,
            stdin,
            stdout: receiver,
            pending: Vec::new(),
            stderr: stderr_buffer,
        })
    }
}

impl SmartSubtransport for SshSubtransport {
    fn action(
        &self,
        url: &str,
        action: Service,
    ) -> Result<Box<dyn SmartSubtransportStream>, git2::Error> {
        // На сервере ssh шёл бы ключами и агентом администратора (фича 038).
        if super::server_policy::on_server() {
            record_failure(EngineError::UnsupportedScheme {
                scheme: "ssh".to_string(),
            });
            return Err(git2::Error::from_str("ssh на сервере недоступен"));
        }
        let target = parse_target(url).ok_or_else(|| {
            record_failure(EngineError::Io {
                message: "адрес ssh не разобран".to_string(),
            });
            git2::Error::from_str("адрес ssh не разобран")
        })?;
        let command = match action {
            Service::UploadPackLs | Service::UploadPack => "git-upload-pack",
            Service::ReceivePackLs | Service::ReceivePack => "git-receive-pack",
        };
        let mut slot = self
            .channel
            .lock()
            .map_err(|_| git2::Error::from_str("ssh"))?;
        let channel = match (&*slot, action) {
            // Второй шаг обмена идёт по уже открытому каналу.
            (Some(channel), Service::UploadPack | Service::ReceivePack) => channel.clone(),
            _ => {
                let channel = Self::spawn(&target, command).map_err(|_| {
                    record_failure(EngineError::UnsupportedScheme {
                        scheme: "ssh".to_string(),
                    });
                    git2::Error::from_str("не удалось запустить ssh")
                })?;
                let channel = Arc::new(Mutex::new(channel));
                *slot = Some(channel.clone());
                channel
            }
        };
        Ok(Box::new(SshStream { channel }))
    }

    fn close(&self) -> Result<(), git2::Error> {
        if let Ok(mut slot) = self.channel.lock() {
            *slot = None;
        }
        Ok(())
    }
}

struct SshStream {
    channel: Arc<Mutex<Channel>>,
}

/// Дописывает вывод, храня не больше `limit` последних байт: причину отказа
/// `ssh` пишет последней строкой (T077), и болтливый сервер память не занимает.
fn keep_tail(buffer: &mut Vec<u8>, chunk: &[u8], limit: usize) {
    buffer.extend_from_slice(chunk);
    if buffer.len() > limit {
        let excess = buffer.len() - limit;
        buffer.drain(..excess);
    }
}

/// Причина отказа по выводу `ssh` в stderr.
fn classify_stderr(stderr: &str) -> EngineError {
    let lower = stderr.to_ascii_lowercase();
    if lower.contains("permission denied") || lower.contains("host key verification failed") {
        EngineError::AuthRejected
    } else if lower.contains("could not resolve")
        || lower.contains("network is unreachable")
        || lower.contains("no route to host")
        || lower.contains("connection refused")
    {
        EngineError::Offline
    } else if lower.contains("timed out") {
        EngineError::Timeout
    } else if lower.contains("does not appear to be a git repository")
        || lower.contains("not found")
    {
        EngineError::NotFound
    } else {
        EngineError::Io {
            message: stderr.lines().last().unwrap_or("ssh").to_string(),
        }
    }
}

impl Read for SshStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if current_context().is_some_and(|context| context.cancel.is_cancelled()) {
            record_failure(EngineError::Cancelled);
            return Err(std::io::Error::other("отменено"));
        }
        let mut channel = self
            .channel
            .lock()
            .map_err(|_| std::io::Error::other("ssh"))?;
        if channel.pending.is_empty() {
            let context = current_context();
            let cancel = || {
                context
                    .as_ref()
                    .is_some_and(|context| context.cancel.is_cancelled())
            };
            let idle_limit = context
                .as_ref()
                .map_or(super::transport_http::NETWORK_IDLE_LIMIT, |context| {
                    context.idle_limit
                });
            match next_chunk(&channel.stdout, &cancel, idle_limit) {
                Ok(Some(chunk)) => channel.pending = chunk,
                Ok(None) => {}
                Err(interrupted) => {
                    // Зависший `ssh` завершается — по отмене и по простою.
                    let _ = channel.child.kill();
                    record_failure(match interrupted {
                        Interrupted::Cancelled => EngineError::Cancelled,
                        Interrupted::Idle => EngineError::Timeout,
                    });
                    return Err(std::io::Error::other("ssh прерван"));
                }
            }
        }
        let read = channel.pending.len().min(buf.len());
        buf[..read].copy_from_slice(&channel.pending[..read]);
        channel.pending.drain(..read);
        if read == 0 {
            // Канал закрыт до конца протокола — причина в stderr.
            std::thread::sleep(std::time::Duration::from_millis(50));
            let stderr = String::from_utf8_lossy(&channel.stderr.lock().unwrap()).to_string();
            if !stderr.trim().is_empty() {
                record_failure(classify_stderr(&stderr));
            }
        }
        Ok(read)
    }
}

impl Write for SshStream {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let mut channel = self
            .channel
            .lock()
            .map_err(|_| std::io::Error::other("ssh"))?;
        channel.stdin.write(data)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut channel = self
            .channel
            .lock()
            .map_err(|_| std::io::Error::other("ssh"))?;
        channel.stdin.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_are_parsed() {
        assert_eq!(
            parse_target("ssh://git@github.com/me/notes.git"),
            Some(SshTarget {
                user: Some("git".into()),
                host: "github.com".into(),
                port: None,
                path: "/me/notes.git".into(),
            })
        );
        let home = parse_target("ssh://me@box:2222/~/notes.git").unwrap();
        assert_eq!(home.port, Some(2222));
        assert_eq!(home.path, "~/notes.git");
    }

    #[test]
    fn option_injection_is_rejected() {
        assert!(parse_target("ssh://-oProxyCommand=evil/x").is_none());
        assert!(parse_target("ssh://-l@host/x").is_none());
        assert!(parse_target("ssh://ho st/x").is_none());
    }

    #[test]
    fn paths_are_quoted_for_the_server_shell() {
        assert_eq!(quote("/a'b"), "'/a'\\''b'");
    }

    #[test]
    fn a_silent_ssh_is_given_up_on_and_cancellation_is_heard() {
        let (sender, receiver) = std::sync::mpsc::channel::<Vec<u8>>();
        // Молчит дольше предела простоя — чтение прерывается по времени.
        let started = Instant::now();
        assert_eq!(
            next_chunk(&receiver, &|| false, Duration::from_millis(300)),
            Err(Interrupted::Idle)
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        // Отмена слышна в пределах такта, даже пока процесс молчит.
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let started = Instant::now();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(150));
                cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
            });
            assert_eq!(
                next_chunk(
                    &receiver,
                    &|| cancelled.load(std::sync::atomic::Ordering::SeqCst),
                    Duration::from_secs(60)
                ),
                Err(Interrupted::Cancelled)
            );
        });
        assert!(started.elapsed() < Duration::from_secs(2));
        // Данные приходят — отдаются; конец вывода — конец.
        sender.send(b"0008NAK\n".to_vec()).unwrap();
        assert_eq!(
            next_chunk(&receiver, &|| false, Duration::from_secs(1)),
            Ok(Some(b"0008NAK\n".to_vec()))
        );
        drop(sender);
        assert_eq!(
            next_chunk(&receiver, &|| false, Duration::from_secs(1)),
            Ok(None)
        );
    }

    #[test]
    fn stderr_keeps_only_its_tail() {
        let mut buffer = Vec::new();
        keep_tail(&mut buffer, b"0123456789", 4);
        assert_eq!(buffer, b"6789");
        keep_tail(&mut buffer, b"ab", 4);
        assert_eq!(buffer, b"89ab");
    }

    #[test]
    fn stderr_is_classified() {
        assert_eq!(
            classify_stderr("git@github.com: Permission denied (publickey)."),
            EngineError::AuthRejected
        );
        assert_eq!(
            classify_stderr("ssh: Could not resolve hostname x"),
            EngineError::Offline
        );
    }
}
