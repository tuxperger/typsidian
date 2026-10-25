//! Движок git на native — libgit2 без встроенных транспортов (research §1, §2).
//!
//! Локальные операции — прямые вызовы libgit2: на 5 000 файлов они укладываются
//! в доли секунды (SC-002). Сетевые уходят в `spawn_blocking`: libgit2
//! синхронен, а ждать сервер на потоке асинхронного рантайма значило бы
//! остановить всё остальное приложение (принцип X).
//!
//! HTTPS и SSH — свои транспорты ([`super::transport_http`],
//! [`super::transport_ssh`]), регистрируемые один раз на процесс. Данные доступа
//! и отмена передаются им через контекст потока на время одной операции и
//! нигде не сохраняются.

use std::path::{Path, PathBuf};

use git2::{
    BranchType, Delta, DiffOptions, ErrorCode, FetchOptions, Oid, PushOptions, RemoteCallbacks,
    Repository, RepositoryState, ResetType, Signature, Sort, Time,
};

use super::transport_http::{self, TransportContext, NETWORK_IDLE_LIMIT};
use super::*;
use crate::sync::Author;

/// Движок одной истории: рабочая копия vault или голое зеркало.
/// Самый большой файл истории, который движок читает в память (T077): файл
/// больше — отказ с названным пределом, а не исчерпание памяти.
pub const BLOB_LIMIT_MB: u32 = 256;

#[derive(Debug, Clone)]
pub struct NativeGitEngine {
    root: PathBuf,
    bare: bool,
    /// Предел простоя сетевого обмена — см. [`NETWORK_IDLE_LIMIT`].
    idle_limit: std::time::Duration,
}

impl NativeGitEngine {
    /// История в папке vault.
    pub fn open(root: impl Into<PathBuf>) -> Self {
        register_transports();
        Self {
            root: root.into(),
            bare: false,
            idle_limit: NETWORK_IDLE_LIMIT,
        }
    }

    /// Голое зеркало удалённого репозитория в каталоге устройства (FR-033).
    pub fn mirror(path: impl Into<PathBuf>) -> Self {
        register_transports();
        Self {
            root: path.into(),
            bare: true,
            idle_limit: NETWORK_IDLE_LIMIT,
        }
    }

    /// Другой предел простоя сети. Умолчание выбрано под живого человека за
    /// экраном; тесту «сервер молчит» ждать двадцать секунд незачем.
    pub fn with_idle_limit(mut self, idle_limit: std::time::Duration) -> Self {
        self.idle_limit = idle_limit;
        self
    }

    fn repo(&self) -> Result<Repository, EngineError> {
        let repo = if self.bare {
            Repository::open_bare(&self.root)
        } else {
            Repository::open(&self.root)
        };
        repo.map_err(|error| match error.code() {
            ErrorCode::NotFound => EngineError::NotARepository,
            _ => io(error),
        })
    }
}

fn register_transports() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // SAFETY: регистрация глобальна для libgit2 и делается ровно один раз,
        // до первой сетевой операции этого процесса.
        unsafe {
            let _ = git2::transport::register("https", |remote| {
                git2::transport::Transport::smart(remote, true, transport_http::HttpSubtransport)
            });
            let _ = git2::transport::register("http", |remote| {
                git2::transport::Transport::smart(remote, true, transport_http::HttpSubtransport)
            });
            #[cfg(not(target_os = "android"))]
            let _ = git2::transport::register("ssh", |remote| {
                git2::transport::Transport::smart(
                    remote,
                    false,
                    super::transport_ssh::SshSubtransport::default(),
                )
            });
        }
    });
}

fn io(error: git2::Error) -> EngineError {
    EngineError::Io {
        message: error.message().to_string(),
    }
}

fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn head_tree(repo: &Repository) -> Option<git2::Tree<'_>> {
    repo.head().ok()?.peel_to_tree().ok()
}

fn deltas(diff: &git2::Diff<'_>) -> Vec<RawChange> {
    diff.deltas()
        .filter_map(|delta| {
            let status = match delta.status() {
                Delta::Added | Delta::Untracked => RawStatus::Added,
                Delta::Deleted => RawStatus::Deleted,
                Delta::Modified | Delta::Typechange => RawStatus::Modified,
                _ => return None,
            };
            let file = if status == RawStatus::Deleted {
                delta.old_file()
            } else {
                delta.new_file()
            };
            let path = slash(file.path()?);
            let oid = |id: Oid| (!id.is_zero()).then(|| id.to_string());
            Some(RawChange {
                path,
                status,
                old_oid: oid(delta.old_file().id()),
                new_oid: oid(delta.new_file().id()),
            })
        })
        .collect()
}

fn commit_entry(commit: &git2::Commit<'_>) -> LogEntry {
    let author = commit.author();
    LogEntry {
        id: commit.id().to_string(),
        message: commit.message().unwrap_or("").to_string(),
        author: Author {
            name: author.name().unwrap_or("").to_string(),
            email: author.email().unwrap_or("").to_string(),
        },
        time: commit.time().seconds() * 1000,
        parents: commit.parent_ids().map(|id| id.to_string()).collect(),
    }
}

fn resolve<'r>(repo: &'r Repository, rev: &str) -> Result<git2::Commit<'r>, EngineError> {
    repo.revparse_single(rev)
        .and_then(|object| object.peel_to_commit())
        .map_err(|error| match error.code() {
            ErrorCode::NotFound | ErrorCode::UnbornBranch => EngineError::NotFound,
            _ => io(error),
        })
}

fn current_branch(repo: &Repository) -> Option<String> {
    let head = repo.find_reference("HEAD").ok()?;
    head.symbolic_target()
        .ok()
        .flatten()
        .and_then(|target| target.strip_prefix("refs/heads/"))
        .map(str::to_string)
}

/// Сетевая операция в отдельном потоке с контекстом транспорта.
async fn network<T: Send + 'static>(
    auth: &Auth,
    cancel: &CancelToken,
    idle_limit: std::time::Duration,
    operation: impl FnOnce() -> Result<T, git2::Error> + Send + 'static,
) -> Result<T, EngineError> {
    let context = TransportContext {
        auth: auth.clone(),
        cancel: cancel.clone(),
        idle_limit,
    };
    let cancel = cancel.clone();
    tokio::task::spawn_blocking(move || transport_http::with_context(context, operation))
        .await
        .map_err(|_| EngineError::Io {
            message: "сетевая операция прервана".to_string(),
        })?
        .map_err(|(error, recorded)| {
            if cancel.is_cancelled() {
                return EngineError::Cancelled;
            }
            recorded.unwrap_or_else(|| classify(&error))
        })
}

fn classify(error: &git2::Error) -> EngineError {
    let message = error.message().to_ascii_lowercase();
    if error.code() == ErrorCode::NotFastForward
        || message.contains("non-fast-forward")
        || message.contains("fetch first")
    {
        EngineError::RejectedNonFastForward
    } else if error.code() == ErrorCode::Auth || message.contains("authentication") {
        EngineError::AuthRejected
    } else if message.contains("unsupported url protocol") {
        EngineError::UnsupportedScheme {
            scheme: message.split('\'').nth(1).unwrap_or("unknown").to_string(),
        }
    } else if error.code() == ErrorCode::NotFound {
        EngineError::NotFound
    } else {
        EngineError::Io {
            message: error.message().to_string(),
        }
    }
}

fn callbacks<'a>(cancel: CancelToken) -> RemoteCallbacks<'a> {
    let mut callbacks = RemoteCallbacks::new();
    let transfer_cancel = cancel.clone();
    // На сервере получение ограничено объёмом и временем (фича 038): диск и
    // потоки общие для всех аккаунтов.
    let limited = super::server_policy::on_server();
    let started = std::time::Instant::now();
    callbacks.transfer_progress(move |progress| {
        if transfer_cancel.is_cancelled() {
            return false;
        }
        !limited
            || (progress.received_bytes() <= super::server_policy::SERVER_FETCH_LIMIT_BYTES
                && started.elapsed() <= super::server_policy::SERVER_EXCHANGE_LIMIT)
    });
    callbacks.push_transfer_progress(move |_, _, _| {});
    callbacks
}

/// Безымянный удалённый ровно с этим адресом. libgit2 применяет к адресу
/// `url.*.insteadOf` из конфигурации git, и проверенный адрес тихо стал бы другим —
/// каталогом, `localhost` или ssh (фича 038). Переписанный адрес — отказ.
fn exact_remote<'r>(repo: &'r Repository, url: &str) -> Result<git2::Remote<'r>, git2::Error> {
    let remote = repo.remote_anonymous(url)?;
    let same = remote.url().ok() == Some(url)
        && remote
            .pushurl()
            .ok()
            .flatten()
            .is_none_or(|push| push == url);
    if !same {
        return Err(git2::Error::from_str(
            "адрес обмена переписан настройками git (url.*.insteadOf)",
        ));
    }
    Ok(remote)
}

impl GitEngine for NativeGitEngine {
    async fn probe(&self) -> Result<RepoProbe, EngineError> {
        if self.bare {
            return Ok(match Repository::open_bare(&self.root) {
                Ok(_) => RepoProbe::Mirror,
                Err(_) => RepoProbe::Absent,
            });
        }
        match Repository::discover(&self.root) {
            Ok(repo) => {
                let Some(workdir) = repo.workdir() else {
                    return Ok(RepoProbe::Absent);
                };
                let workdir = std::fs::canonicalize(workdir).unwrap_or(workdir.to_path_buf());
                let root = std::fs::canonicalize(&self.root).unwrap_or(self.root.clone());
                if workdir == root {
                    Ok(RepoProbe::Local)
                } else {
                    let prefix = root.strip_prefix(&workdir).map(slash).unwrap_or_default();
                    Ok(RepoProbe::ForeignParent {
                        repo_root: workdir.to_string_lossy().into_owned(),
                        prefix,
                    })
                }
            }
            Err(error) if error.code() == ErrorCode::NotFound => Ok(RepoProbe::Absent),
            Err(error) => Err(io(error)),
        }
    }

    async fn init(&self, branch: &str) -> Result<(), EngineError> {
        let mut options = git2::RepositoryInitOptions::new();
        options.initial_head(branch).bare(self.bare);
        Repository::init_opts(&self.root, &options).map_err(io)?;
        Ok(())
    }

    async fn head(&self) -> Result<HeadInfo, EngineError> {
        let repo = self.repo()?;
        let oid = repo
            .head()
            .ok()
            .and_then(|head| head.target())
            .map(|oid| oid.to_string());
        let in_progress = match repo.state() {
            RepositoryState::Clean => None,
            RepositoryState::Merge => Some(InProgressOp::Merge),
            RepositoryState::Rebase
            | RepositoryState::RebaseInteractive
            | RepositoryState::RebaseMerge
            | RepositoryState::ApplyMailbox
            | RepositoryState::ApplyMailboxOrRebase => Some(InProgressOp::Rebase),
            RepositoryState::CherryPick | RepositoryState::CherryPickSequence => {
                Some(InProgressOp::CherryPick)
            }
            RepositoryState::Revert | RepositoryState::RevertSequence => Some(InProgressOp::Revert),
            RepositoryState::Bisect => Some(InProgressOp::Bisect),
        };
        Ok(HeadInfo {
            branch: current_branch(&repo),
            oid,
            in_progress,
        })
    }

    async fn status(&self) -> Result<Vec<RawChange>, EngineError> {
        let repo = self.repo()?;
        let tree = head_tree(&repo);
        let mut options = DiffOptions::new();
        options
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_typechange(true);
        let diff = repo
            .diff_tree_to_workdir_with_index(tree.as_ref(), Some(&mut options))
            .map_err(io)?;
        Ok(deltas(&diff))
    }

    async fn commit(
        &self,
        paths: &[String],
        message: &str,
        author: &Author,
        time: i64,
        merge_parent: Option<&str>,
    ) -> Result<String, EngineError> {
        let repo = self.repo()?;
        let mut index = repo.index().map_err(io)?;
        for path in paths {
            let relative = Path::new(path);
            if self.root.join(relative).is_file() {
                index.add_path(relative).map_err(io)?;
            } else {
                // Удалённого файла в индексе может и не быть — это не ошибка.
                let _ = index.remove_path(relative);
            }
        }
        index.write().map_err(io)?;
        let tree_id = index.write_tree().map_err(io)?;
        let tree = repo.find_tree(tree_id).map_err(io)?;
        let signature =
            Signature::new(&author.name, &author.email, &Time::new(time / 1000, 0)).map_err(io)?;
        let mut parents = Vec::new();
        if let Ok(head) = repo.head().and_then(|head| head.peel_to_commit()) {
            parents.push(head);
        }
        if let Some(merge) = merge_parent {
            parents.push(resolve(&repo, merge)?);
        }
        let parent_refs: Vec<&git2::Commit<'_>> = parents.iter().collect();
        let oid = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parent_refs,
            )
            .map_err(io)?;
        Ok(oid.to_string())
    }

    async fn log(
        &self,
        start: Option<&str>,
        offset: u32,
        limit: u32,
    ) -> Result<LogPage, EngineError> {
        let repo = self.repo()?;
        let start = match start {
            Some(rev) => match resolve(&repo, rev) {
                Ok(commit) => commit.id(),
                Err(EngineError::NotFound) => {
                    return Ok(LogPage {
                        entries: Vec::new(),
                        next: None,
                    })
                }
                Err(error) => return Err(error),
            },
            None => match repo.head().ok().and_then(|head| head.target()) {
                Some(oid) => oid,
                None => {
                    return Ok(LogPage {
                        entries: Vec::new(),
                        next: None,
                    })
                }
            },
        };
        let mut walk = repo.revwalk().map_err(io)?;
        walk.set_sorting(Sort::TIME).map_err(io)?;
        walk.push(start).map_err(io)?;
        let mut entries = Vec::new();
        let mut more = false;
        for (index, oid) in walk.skip(offset as usize).enumerate() {
            if index as u32 >= limit {
                more = true;
                break;
            }
            let commit = repo.find_commit(oid.map_err(io)?).map_err(io)?;
            entries.push(commit_entry(&commit));
        }
        Ok(LogPage {
            next: more.then_some(offset + entries.len() as u32),
            entries,
        })
    }

    async fn read_blob(&self, rev: &str, path: &str) -> Result<Option<Vec<u8>>, EngineError> {
        let repo = self.repo()?;
        let commit = match resolve(&repo, rev) {
            Ok(commit) => commit,
            Err(EngineError::NotFound) => return Ok(None),
            Err(error) => return Err(error),
        };
        let tree = commit.tree().map_err(io)?;
        let entry = match tree.get_path(Path::new(path)) {
            Ok(entry) => entry,
            Err(error) if error.code() == ErrorCode::NotFound => return Ok(None),
            Err(error) => return Err(io(error)),
        };
        // Размер — из заголовка объекта, до чтения содержимого в память (T077).
        let size = repo
            .odb()
            .and_then(|odb| odb.read_header(entry.id()))
            .map(|(size, _)| size)
            .unwrap_or(0);
        if size > BLOB_LIMIT_MB as usize * 1024 * 1024 {
            return Err(EngineError::TooLarge {
                limit_mb: BLOB_LIMIT_MB,
            });
        }
        let blob = repo.find_blob(entry.id()).map_err(io)?;
        Ok(Some(blob.content().to_vec()))
    }

    async fn tree_diff(&self, from: Option<&str>, to: &str) -> Result<Vec<RawChange>, EngineError> {
        let repo = self.repo()?;
        let new = resolve(&repo, to)?.tree().map_err(io)?;
        let old = match from {
            Some(from) => Some(resolve(&repo, from)?.tree().map_err(io)?),
            None => None,
        };
        let diff = repo
            .diff_tree_to_tree(old.as_ref(), Some(&new), None)
            .map_err(io)?;
        Ok(deltas(&diff))
    }

    async fn merge_base(&self, a: &str, b: &str) -> Result<Option<String>, EngineError> {
        let repo = self.repo()?;
        let a = resolve(&repo, a)?.id();
        let b = resolve(&repo, b)?.id();
        match repo.merge_base(a, b) {
            Ok(oid) => Ok(Some(oid.to_string())),
            Err(error) if error.code() == ErrorCode::NotFound => Ok(None),
            Err(error) => Err(io(error)),
        }
    }

    async fn set_head(&self, oid: &str) -> Result<(), EngineError> {
        let repo = self.repo()?;
        let commit = resolve(&repo, oid)?;
        if repo.head().is_err() {
            // Снимков ещё не было: ветки нет, её создаёт первая перемотка.
            let branch = current_branch(&repo).unwrap_or_else(|| "main".to_string());
            repo.reference(
                &format!("refs/heads/{branch}"),
                commit.id(),
                true,
                "typsidian: pull",
            )
            .map_err(io)?;
        }
        // Mixed: ветка и индекс — на снимок, рабочие файлы не трогаются (их
        // уже записала сессия).
        repo.reset(commit.as_object(), ResetType::Mixed, None)
            .map_err(io)
    }

    async fn remote_head(&self, branch: &str) -> Result<Option<String>, EngineError> {
        let repo = self.repo()?;
        let name = if self.bare {
            format!("refs/heads/{branch}")
        } else {
            format!("refs/remotes/origin/{branch}")
        };
        Ok(repo
            .find_reference(&name)
            .ok()
            .and_then(|reference| reference.target())
            .map(|oid| oid.to_string()))
    }

    async fn ahead_behind(&self, local: &str, upstream: &str) -> Result<(u32, u32), EngineError> {
        let repo = self.repo()?;
        let local = resolve(&repo, local)?.id();
        let upstream = resolve(&repo, upstream)?.id();
        let (ahead, behind) = repo.graph_ahead_behind(local, upstream).map_err(io)?;
        Ok((ahead as u32, behind as u32))
    }

    async fn fetch(
        &self,
        remote: &RemoteSpec,
        auth: &Auth,
        cancel: &CancelToken,
    ) -> Result<Option<String>, EngineError> {
        let root = self.root.clone();
        let bare = self.bare;
        let url = remote.url.clone();
        let branch = remote.branch.clone();
        let token = cancel.clone();
        network(auth, cancel, self.idle_limit, move || {
            let repo = if bare {
                Repository::open_bare(&root)?
            } else {
                Repository::open(&root)?
            };
            let tracking = if bare {
                format!("refs/heads/{branch}")
            } else {
                format!("refs/remotes/origin/{branch}")
            };
            let refspec = format!("+refs/heads/{branch}:{tracking}");
            let mut remote = exact_remote(&repo, &url)?;
            let mut options = FetchOptions::new();
            options.remote_callbacks(callbacks(token));
            remote.fetch(&[refspec.as_str()], Some(&mut options), None)?;
            // Ветки на сервере ещё нет — это не ошибка, а пустой сервер.
            let tip = remote
                .list()?
                .iter()
                .find(|head| head.name() == format!("refs/heads/{branch}"))
                .map(|head| head.oid());
            match tip {
                Some(oid) => {
                    repo.reference(&tracking, oid, true, "typsidian: fetch")?;
                    Ok(Some(oid.to_string()))
                }
                None => Ok(None),
            }
        })
        .await
    }

    async fn push(
        &self,
        remote: &RemoteSpec,
        auth: &Auth,
        cancel: &CancelToken,
    ) -> Result<(), EngineError> {
        let root = self.root.clone();
        let url = remote.url.clone();
        let branch = remote.branch.clone();
        let token = cancel.clone();
        let rejected = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
        let rejected_inner = rejected.clone();
        network(auth, cancel, self.idle_limit, move || {
            let repo = Repository::open(&root)?;
            let head = repo
                .head()?
                .target()
                .ok_or_else(|| git2::Error::from_str("в истории нет снимков"))?;
            let mut remote = exact_remote(&repo, &url)?;
            let mut callbacks = callbacks(token);
            let sink = rejected_inner.clone();
            callbacks.push_update_reference(move |_, status| {
                if let Some(status) = status {
                    *sink.lock().unwrap() = Some(status.to_string());
                }
                Ok(())
            });
            let mut options = PushOptions::new();
            options.remote_callbacks(callbacks);
            let spec = format!("refs/heads/{branch}:refs/heads/{branch}");
            remote.push(&[spec.as_str()], Some(&mut options))?;
            if let Some(status) = rejected_inner.lock().unwrap().clone() {
                return Err(git2::Error::new(
                    ErrorCode::NotFastForward,
                    git2::ErrorClass::Reference,
                    status,
                ));
            }
            repo.reference(
                &format!("refs/remotes/origin/{branch}"),
                head,
                true,
                "typsidian: push",
            )?;
            Ok(())
        })
        .await?;
        let _ = rejected;
        Ok(())
    }

    async fn clone_into(
        &self,
        remote: &RemoteSpec,
        auth: &Auth,
        bare: bool,
        cancel: &CancelToken,
    ) -> Result<(), EngineError> {
        let root = self.root.clone();
        let url = remote.url.clone();
        let branch = remote.branch.clone();
        let token = cancel.clone();
        network(auth, cancel, self.idle_limit, move || {
            let mut options = FetchOptions::new();
            options.remote_callbacks(callbacks(token));
            let mut builder = git2::build::RepoBuilder::new();
            builder.bare(bare).fetch_options(options);
            let repo = builder.clone(&url, &root)?;
            // `HEAD` сервера может указывать на другую ветку (у `git init
            // --bare` это `master`), а обмен идёт веткой vault: переключаемся
            // на неё, если она на сервере есть.
            if bare {
                // Голый клон кладёт ветки сервера в `refs/remotes/origin/*`;
                // зеркалу нужна своя ветка обмена — её обновляет `fetch`.
                let wanted = format!("refs/heads/{branch}");
                if repo.find_reference(&wanted).is_err() {
                    if let Ok(oid) = repo.refname_to_id(&format!("refs/remotes/origin/{branch}")) {
                        repo.reference(&wanted, oid, true, "typsidian: mirror")?;
                    }
                }
                if repo.find_reference(&wanted).is_ok() {
                    repo.set_head(&wanted)?;
                }
            } else if let Ok(tracking) =
                repo.find_reference(&format!("refs/remotes/origin/{branch}"))
            {
                let target = tracking.peel_to_commit()?;
                let already = current_branch(&repo).as_deref() == Some(branch.as_str())
                    && repo.head().is_ok();
                if !already {
                    let mut local = repo.branch(&branch, &target, true)?;
                    let _ = local.set_upstream(Some(&format!("origin/{branch}")));
                    repo.set_head(&format!("refs/heads/{branch}"))?;
                    let mut checkout = git2::build::CheckoutBuilder::new();
                    checkout.force();
                    repo.checkout_head(Some(&mut checkout))?;
                }
            }
            Ok(())
        })
        .await
    }

    async fn set_remote_url(&self, url: Option<&str>) -> Result<(), EngineError> {
        let repo = self.repo()?;
        match url {
            Some(url) => {
                if repo.find_remote("origin").is_ok() {
                    repo.remote_set_url("origin", url).map_err(io)?;
                } else {
                    repo.remote("origin", url).map_err(io)?;
                }
                // Ветка следит за `origin` — тогда и внешний `git status`
                // показывает то же «впереди/позади», что панель.
                if let Some(branch) = current_branch(&repo) {
                    if let Ok(mut local) = repo.find_branch(&branch, BranchType::Local) {
                        let _ = local.set_upstream(Some(&format!("origin/{branch}")));
                    }
                }
            }
            None => match repo.remote_delete("origin") {
                Ok(()) => {}
                Err(error) if error.code() == ErrorCode::NotFound => {}
                Err(error) => return Err(io(error)),
            },
        }
        Ok(())
    }

    async fn global_author(&self) -> Option<Author> {
        // Автор администратора сервера не становится автором снимков пользователей.
        if super::server_policy::on_server() {
            return None;
        }
        let config = git2::Config::open_default().ok()?;
        Some(Author {
            name: config.get_string("user.name").ok()?,
            email: config.get_string("user.email").ok()?,
        })
    }
}
