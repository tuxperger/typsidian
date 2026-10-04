//! Движок git в памяти для тестов политики на обоих таргетах (T011).
//!
//! Не притворяется git целиком: снимок — полная копия дерева, «объекты» —
//! содержимое файлов. Этого достаточно, чтобы проверить решения сессии —
//! снимки выбранных путей, историю с переименованиями, получение со слиянием,
//! отказы сети — одинаково под `cargo test` и `wasm-bindgen-test`. Настоящий
//! libgit2 проверяется своими интеграционными тестами под native.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use super::*;
use crate::sync::Author;

type Tree = BTreeMap<String, Vec<u8>>;

/// Рабочие файлы vault глазами движка: чтение всех файлов и запись одного
/// (`None` — удалить).
pub type ReadWorkdir = Rc<dyn Fn() -> Tree>;
pub type WriteWorkdir = Rc<dyn Fn(&str, Option<&[u8]>)>;

#[derive(Debug, Clone)]
struct Commit {
    id: String,
    tree: Tree,
    message: String,
    author: Author,
    time: i64,
    parents: Vec<String>,
}

/// Хранилище снимков — общее у «сервера» и клонов.
#[derive(Debug, Default)]
pub struct FakeRepo {
    commits: BTreeMap<String, Commit>,
    branches: BTreeMap<String, String>,
    counter: u64,
}

impl FakeRepo {
    fn add(
        &mut self,
        tree: Tree,
        message: &str,
        author: &Author,
        time: i64,
        parents: Vec<String>,
    ) -> String {
        // Счётчик общий на процесс: у двух клонов одного «сервера» снимки
        // обязаны различаться, даже сделанные в одну и ту же миллисекунду.
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        self.counter += 1;
        let id = format!(
            "{:040x}",
            NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        );
        self.commits.insert(
            id.clone(),
            Commit {
                id: id.clone(),
                tree,
                message: message.to_string(),
                author: author.clone(),
                time,
                parents,
            },
        );
        id
    }

    fn ancestors(&self, start: &str) -> Vec<String> {
        // Обход в ширину с сортировкой по времени — как журнал git.
        let mut seen = std::collections::BTreeSet::new();
        let mut queue = vec![start.to_string()];
        let mut order = Vec::new();
        while let Some(id) = queue.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            if let Some(commit) = self.commits.get(&id) {
                order.push(commit.clone());
                queue.extend(commit.parents.iter().cloned());
            }
        }
        order.sort_by(|a, b| b.time.cmp(&a.time).then(b.id.cmp(&a.id)));
        order.into_iter().map(|commit| commit.id).collect()
    }

    /// Пересадка снимков с другого хранилища (обмен с «сервером»).
    fn import_from(&mut self, other: &FakeRepo, tip: &str) {
        for id in other.ancestors(tip) {
            if let Some(commit) = other.commits.get(&id) {
                self.commits.entry(id).or_insert_with(|| commit.clone());
            }
        }
    }
}

/// Режим сбоя сети.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeNetwork {
    Online,
    Offline,
    AuthRejected,
}

pub struct FakeGitEngine {
    repo: Rc<RefCell<Option<FakeRepo>>>,
    head_branch: RefCell<String>,
    remote: Option<Rc<RefCell<FakeRepo>>>,
    remote_tracking: RefCell<BTreeMap<String, String>>,
    remote_url: RefCell<Option<String>>,
    network: Rc<RefCell<FakeNetwork>>,
    read_workdir: ReadWorkdir,
    write_workdir: WriteWorkdir,
    bare: bool,
    pub global_author: Option<Author>,
}

impl FakeGitEngine {
    pub fn new(read_workdir: ReadWorkdir, write_workdir: WriteWorkdir) -> Self {
        Self {
            repo: Rc::new(RefCell::new(None)),
            head_branch: RefCell::new("main".to_string()),
            remote: None,
            remote_tracking: RefCell::new(BTreeMap::new()),
            remote_url: RefCell::new(None),
            network: Rc::new(RefCell::new(FakeNetwork::Online)),
            read_workdir,
            write_workdir,
            bare: false,
            global_author: None,
        }
    }

    /// Голое зеркало без рабочих файлов.
    pub fn bare() -> Self {
        let mut engine = Self::new(Rc::new(Tree::new), Rc::new(|_, _| {}));
        engine.bare = true;
        engine
    }

    /// «Сервер», с которым движок обменивается снимками.
    pub fn with_remote(mut self, remote: Rc<RefCell<FakeRepo>>) -> Self {
        self.remote = Some(remote);
        self
    }

    pub fn network(&self) -> Rc<RefCell<FakeNetwork>> {
        self.network.clone()
    }

    pub fn set_network(&self, network: FakeNetwork) {
        *self.network.borrow_mut() = network;
    }

    /// Пустой «сервер».
    pub fn new_remote() -> Rc<RefCell<FakeRepo>> {
        Rc::new(RefCell::new(FakeRepo::default()))
    }

    fn workdir(&self) -> Tree {
        (self.read_workdir)()
            .into_iter()
            .filter(|(path, _)| !is_ignored(path))
            .collect()
    }

    fn head_oid(&self) -> Option<String> {
        let repo = self.repo.borrow();
        let branch = self.head_branch.borrow();
        repo.as_ref()?.branches.get(branch.as_str()).cloned()
    }

    fn head_tree(&self) -> Tree {
        let Some(oid) = self.head_oid() else {
            return Tree::new();
        };
        self.tree_of(&oid).unwrap_or_default()
    }

    fn resolve(&self, rev: &str) -> Option<String> {
        if rev == "HEAD" {
            return self.head_oid();
        }
        let repo = self.repo.borrow();
        let repo = repo.as_ref()?;
        if repo.commits.contains_key(rev) {
            return Some(rev.to_string());
        }
        repo.branches.get(rev).cloned()
    }

    fn tree_of(&self, rev: &str) -> Option<Tree> {
        let oid = self.resolve(rev)?;
        let repo = self.repo.borrow();
        repo.as_ref()?
            .commits
            .get(&oid)
            .map(|commit| commit.tree.clone())
    }

    fn check_network(&self) -> Result<(), EngineError> {
        match *self.network.borrow() {
            FakeNetwork::Online => Ok(()),
            FakeNetwork::Offline => Err(EngineError::Offline),
            FakeNetwork::AuthRejected => Err(EngineError::AuthRejected),
        }
    }
}

/// Правила исключения истории в объёме тестов: то же, что пишет блок
/// `.gitignore` приложения.
fn is_ignored(path: &str) -> bool {
    path.starts_with(".git/")
        || path.starts_with(".typsidian/cache.sqlite")
        || path.starts_with(".typsidian/tmp/")
        || path.starts_with(".stfolder")
        || path == ".stignore"
        || crate::sync::conflict_copy::is_conflict_copy(path.rsplit('/').next().unwrap_or(path))
}

fn diff_trees(from: &Tree, to: &Tree, oids: bool) -> Vec<RawChange> {
    let oid = |bytes: &Vec<u8>| Some(format!("{:x}", fxhash(bytes)));
    let mut changes = Vec::new();
    for (path, new) in to {
        match from.get(path) {
            None => changes.push(RawChange {
                path: path.clone(),
                status: RawStatus::Added,
                old_oid: None,
                new_oid: if oids { oid(new) } else { None },
            }),
            Some(old) if old != new => changes.push(RawChange {
                path: path.clone(),
                status: RawStatus::Modified,
                old_oid: oid(old),
                new_oid: if oids { oid(new) } else { None },
            }),
            _ => {}
        }
    }
    for (path, old) in from {
        if !to.contains_key(path) {
            changes.push(RawChange {
                path: path.clone(),
                status: RawStatus::Deleted,
                old_oid: oid(old),
                new_oid: None,
            });
        }
    }
    changes
}

fn fxhash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ *byte as u64).wrapping_mul(0x100000001b3)
    })
}

impl GitEngine for FakeGitEngine {
    async fn probe(&self) -> Result<RepoProbe, EngineError> {
        Ok(match (self.repo.borrow().is_some(), self.bare) {
            (false, _) => RepoProbe::Absent,
            (true, true) => RepoProbe::Mirror,
            (true, false) => RepoProbe::Local,
        })
    }

    async fn init(&self, branch: &str) -> Result<(), EngineError> {
        *self.repo.borrow_mut() = Some(FakeRepo::default());
        *self.head_branch.borrow_mut() = branch.to_string();
        Ok(())
    }

    async fn head(&self) -> Result<HeadInfo, EngineError> {
        if self.repo.borrow().is_none() {
            return Err(EngineError::NotARepository);
        }
        Ok(HeadInfo {
            branch: Some(self.head_branch.borrow().clone()),
            oid: self.head_oid(),
            in_progress: None,
        })
    }

    async fn status(&self) -> Result<Vec<RawChange>, EngineError> {
        if self.repo.borrow().is_none() {
            return Err(EngineError::NotARepository);
        }
        Ok(diff_trees(&self.head_tree(), &self.workdir(), false))
    }

    async fn commit(
        &self,
        paths: &[String],
        message: &str,
        author: &Author,
        time: i64,
        merge_parent: Option<&str>,
    ) -> Result<String, EngineError> {
        let mut tree = self.head_tree();
        let workdir = self.workdir();
        for path in paths {
            match workdir.get(path) {
                Some(bytes) => {
                    tree.insert(path.clone(), bytes.clone());
                }
                None => {
                    tree.remove(path);
                }
            }
        }
        let mut parents: Vec<String> = self.head_oid().into_iter().collect();
        parents.extend(merge_parent.map(str::to_string));
        let branch = self.head_branch.borrow().clone();
        let mut repo = self.repo.borrow_mut();
        let repo = repo.as_mut().ok_or(EngineError::NotARepository)?;
        let id = repo.add(tree, message, author, time, parents);
        repo.branches.insert(branch, id.clone());
        Ok(id)
    }

    async fn log(
        &self,
        start: Option<&str>,
        offset: u32,
        limit: u32,
    ) -> Result<LogPage, EngineError> {
        let Some(start) = self.resolve(start.unwrap_or("HEAD")) else {
            return Ok(LogPage {
                entries: Vec::new(),
                next: None,
            });
        };
        let repo = self.repo.borrow();
        let repo = repo.as_ref().ok_or(EngineError::NotARepository)?;
        let all = repo.ancestors(&start);
        let entries: Vec<LogEntry> = all
            .iter()
            .skip(offset as usize)
            .take(limit as usize)
            .filter_map(|id| repo.commits.get(id))
            .map(|commit| LogEntry {
                id: commit.id.clone(),
                message: commit.message.clone(),
                author: commit.author.clone(),
                time: commit.time,
                parents: commit.parents.clone(),
            })
            .collect();
        let consumed = offset as usize + entries.len();
        Ok(LogPage {
            entries,
            next: (consumed < all.len()).then_some(consumed as u32),
        })
    }

    async fn read_blob(&self, rev: &str, path: &str) -> Result<Option<Vec<u8>>, EngineError> {
        Ok(self.tree_of(rev).and_then(|tree| tree.get(path).cloned()))
    }

    async fn tree_diff(&self, from: Option<&str>, to: &str) -> Result<Vec<RawChange>, EngineError> {
        let old = from.and_then(|from| self.tree_of(from)).unwrap_or_default();
        let new = self.tree_of(to).ok_or(EngineError::NotFound)?;
        Ok(diff_trees(&old, &new, true))
    }

    async fn merge_base(&self, a: &str, b: &str) -> Result<Option<String>, EngineError> {
        let repo = self.repo.borrow();
        let repo = repo.as_ref().ok_or(EngineError::NotARepository)?;
        let ours: std::collections::BTreeSet<String> = repo.ancestors(a).into_iter().collect();
        Ok(repo.ancestors(b).into_iter().find(|id| ours.contains(id)))
    }

    async fn set_head(&self, oid: &str) -> Result<(), EngineError> {
        let branch = self.head_branch.borrow().clone();
        let mut repo = self.repo.borrow_mut();
        let repo = repo.as_mut().ok_or(EngineError::NotARepository)?;
        repo.branches.insert(branch, oid.to_string());
        Ok(())
    }

    async fn remote_head(&self, branch: &str) -> Result<Option<String>, EngineError> {
        Ok(self.remote_tracking.borrow().get(branch).cloned())
    }

    async fn ahead_behind(&self, local: &str, upstream: &str) -> Result<(u32, u32), EngineError> {
        let repo = self.repo.borrow();
        let repo = repo.as_ref().ok_or(EngineError::NotARepository)?;
        let ours: std::collections::BTreeSet<String> = repo.ancestors(local).into_iter().collect();
        let theirs: std::collections::BTreeSet<String> =
            repo.ancestors(upstream).into_iter().collect();
        Ok((
            ours.difference(&theirs).count() as u32,
            theirs.difference(&ours).count() as u32,
        ))
    }

    async fn fetch(
        &self,
        remote: &RemoteSpec,
        _auth: &Auth,
        cancel: &CancelToken,
    ) -> Result<Option<String>, EngineError> {
        self.check_network()?;
        if cancel.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        let server = self.remote.as_ref().ok_or(EngineError::NotFound)?;
        let server = server.borrow();
        let Some(tip) = server.branches.get(&remote.branch).cloned() else {
            self.remote_tracking.borrow_mut().remove(&remote.branch);
            return Ok(None);
        };
        let mut repo = self.repo.borrow_mut();
        let repo = repo.as_mut().ok_or(EngineError::NotARepository)?;
        repo.import_from(&server, &tip);
        if self.bare {
            repo.branches.insert(remote.branch.clone(), tip.clone());
        }
        self.remote_tracking
            .borrow_mut()
            .insert(remote.branch.clone(), tip.clone());
        Ok(Some(tip))
    }

    async fn push(
        &self,
        remote: &RemoteSpec,
        _auth: &Auth,
        cancel: &CancelToken,
    ) -> Result<(), EngineError> {
        self.check_network()?;
        if cancel.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        let head = self.head_oid().ok_or(EngineError::NotFound)?;
        let server = self.remote.as_ref().ok_or(EngineError::NotFound)?;
        let mut server = server.borrow_mut();
        if let Some(remote_tip) = server.branches.get(&remote.branch).cloned() {
            let repo = self.repo.borrow();
            let repo = repo.as_ref().ok_or(EngineError::NotARepository)?;
            if !repo.ancestors(&head).contains(&remote_tip) {
                return Err(EngineError::RejectedNonFastForward);
            }
        }
        let repo = self.repo.borrow();
        let repo = repo.as_ref().ok_or(EngineError::NotARepository)?;
        server.import_from(repo, &head);
        server.branches.insert(remote.branch.clone(), head.clone());
        self.remote_tracking
            .borrow_mut()
            .insert(remote.branch.clone(), head);
        Ok(())
    }

    async fn clone_into(
        &self,
        remote: &RemoteSpec,
        auth: &Auth,
        bare: bool,
        cancel: &CancelToken,
    ) -> Result<(), EngineError> {
        if self.repo.borrow().is_some() {
            return Err(EngineError::Io {
                message: "не пусто".into(),
            });
        }
        *self.repo.borrow_mut() = Some(FakeRepo::default());
        *self.head_branch.borrow_mut() = remote.branch.clone();
        let tip = self.fetch(remote, auth, cancel).await?;
        if let Some(tip) = tip {
            self.set_head(&tip).await?;
            if !bare {
                for (path, bytes) in self.tree_of(&tip).unwrap_or_default() {
                    (self.write_workdir)(&path, Some(&bytes));
                }
            }
        }
        Ok(())
    }

    async fn set_remote_url(&self, url: Option<&str>) -> Result<(), EngineError> {
        *self.remote_url.borrow_mut() = url.map(str::to_string);
        Ok(())
    }

    async fn global_author(&self) -> Option<Author> {
        self.global_author.clone()
    }
}
