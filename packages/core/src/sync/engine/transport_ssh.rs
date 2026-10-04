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
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

use git2::transport::{Service, SmartSubtransport, SmartSubtransportStream};

use super::transport_http::{current_context, record_failure};
use super::EngineError;

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
    stdout: ChildStdout,
    stderr: Arc<Mutex<Vec<u8>>>,
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
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::other("нет stdout"))?;
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
                        sink.extend_from_slice(&chunk[..read]);
                    }
                }
            });
        }
        Ok(Channel {
            child,
            stdin,
            stdout,
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
        let read = channel.stdout.read(buf)?;
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
