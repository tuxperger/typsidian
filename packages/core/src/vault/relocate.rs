//! Перенос заметки, доски или папки между папками vault (фича 023).
//!
//! Отдельная операция, а не вызов `rename` из интерфейса (research.md §1):
//! `rename` проверяет имя заметки и для папки оставляет индекс под старыми
//! путями, а перенос папки циклом по заметкам вынес бы правила допустимости и
//! порядок обновления состояния за пределы ядра (принцип I).
//!
//! Здесь живёт всё, что не зависит от таргета: типы, чистые проверки и расчёты
//! (`check_move`, `path_changes`, `plan_steps`, `broken_by_move`,
//! `move_targets`) и обобщённые над `VaultBackend` помощники. Сам перенос
//! файлов — в `native.rs` (`std::fs::rename`) и `web.rs` (копия с удалением
//! источника), потому что у File System Access API переноса папок нет
//! (research.md §6).

use super::backend::VaultBackend;
use super::events::VaultEvent;
use super::index::Note;
use super::sections::{normalize, SectionMark};
use super::service_dir;
use super::vault_settings::{move_sections, read_vault_settings};
use super::{classify, FileKind};
use crate::boards::placement::{note_assets_folder, AssetsPlacement};
use crate::boards::snapshot::assets_folder_name;
use crate::graph::{extract_links, resolve_link_target};
use crate::templates::TextSpan;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Что и куда переносят. Пути — относительно корня vault, корень — пустой путь.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct MoveRequest {
    /// Переносимый элемент: заметка, доска или папка.
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    pub from: PathBuf,
    /// Цель; пустой путь — корень vault.
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    pub to_folder: PathBuf,
}

/// Папка, которая может принять элемент: строка списка «Переместить в…» и
/// элемент множества допустимых целей жеста.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct MoveTarget {
    /// Путь папки; пустой — корень vault.
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    pub path: PathBuf,
    /// Имя папки; для корня — имя vault.
    pub name: String,
    /// Путь родителя для показа справа от имени; у корня и папок верхнего
    /// уровня пусто. Разделитель всегда `/`.
    pub parent: String,
    /// Папка помечена разделом (фича 006).
    pub is_section: bool,
    /// Где в `name` совпал фильтр (кодовые единицы UTF-16); `None` — совпало
    /// в пути или фильтра нет.
    pub name_match: Option<TextSpan>,
}

/// Пара «было → стало» для одной заметки, доски или папки.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct PathChange {
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    pub from: PathBuf,
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    pub to: PathBuf,
}

/// Содержимое предупреждения о ссылках (FR-016, FR-017).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct MoveImpact {
    /// Заметки (по прежним путям), у которых после переноса хотя бы одна
    /// сейчас рабочая ссылка перестанет находить адресата; по пути. Пусто —
    /// предупреждение не показывается.
    pub broken: Vec<Note>,
}

/// Причина отказа до любых изменений на диске.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum MoveRefusal {
    /// Элемент уже лежит в цели — пустое действие, интерфейс молчит (FR-005).
    #[error("элемент уже лежит в этой папке")]
    SamePlace,
    /// Цель — сама папка или её потомок (FR-003).
    #[error("папку нельзя перенести в саму себя")]
    IntoItself,
    /// В цели занято имя элемента или его папки снимков (FR-004, FR-018a).
    #[error("в целевой папке уже есть «{name}»")]
    NameConflict { name: String },
    /// Элемента больше нет.
    #[error("элемент не найден")]
    NotFound,
    /// Цели больше нет.
    #[error("целевая папка не найдена")]
    TargetNotFound,
    /// Vault недоступен или открыт только для чтения (FR-006).
    #[error("нет прав на запись")]
    AccessDenied,
    /// Элемент или цель — служебная папка vault.
    #[error("служебная папка vault не участвует в переносе")]
    ServiceFolder,
}

/// Ошибка операции переноса. `Io` означает, что на диске ничего не изменилось
/// (native: откат обратным `rename`; web: копия в цели удалена).
///
/// Отказ вложен полем `reason`, а не новтайп-вариантом: оба перечисления
/// размечены одним полем `kind`, и вложение без обёртки склеило бы их.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum MoveError {
    #[error("{reason}")]
    Refused { reason: MoveRefusal },
    #[error("не удалось перенести")]
    Io,
}

impl From<MoveRefusal> for MoveError {
    fn from(reason: MoveRefusal) -> Self {
        MoveError::Refused { reason }
    }
}

/// Исход переноса, не ставшего ошибкой.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum MoveOutcome {
    /// Перенесено целиком. `undo` — готовый обратный запрос (research.md §7).
    Done {
        moved: Vec<PathChange>,
        folders: Vec<PathChange>,
        undo: MoveRequest,
    },
    /// Отменено до удаления источника; на диске ничего не изменилось.
    Cancelled,
    /// Цель получила всё, но часть источника удалить не удалось (только web);
    /// `left` — что осталось на прежнем месте (FR-007).
    Partial {
        moved: Vec<PathChange>,
        folders: Vec<PathChange>,
        #[cfg_attr(target_arch = "wasm32", tsify(type = "string[]"))]
        left: Vec<PathBuf>,
    },
}

/// `Sync` там, где он нужен: на native ссылка на управление живёт через
/// `await` внутри команды Tauri, чьё будущее обязано быть `Send`; на web
/// управление — колбэк JS, и `Sync` у него быть не может.
#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSync: Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: ?Sized + Sync> MaybeSync for T {}
#[cfg(target_arch = "wasm32")]
pub trait MaybeSync {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> MaybeSync for T {}

/// Ход и отмена массового переноса (принцип X).
pub trait MoveControl: MaybeSync {
    /// Сколько файлов из скольких перенесено.
    fn progress(&self, done: u32, total: u32);
    /// Просили ли отменить. Спрашивается между файлами, до удаления источника.
    fn cancelled(&self) -> bool;
}

/// Управление, которому сообщать и отменять нечего: native переносит одним
/// системным вызовом на шаг.
pub struct NoControl;

impl MoveControl for NoControl {
    fn progress(&self, _done: u32, _total: u32) {}
    fn cancelled(&self) -> bool {
        false
    }
}

/// План переноса: что переезжает и что при этом меняет путь. Не пересекает
/// границу ядра; строится backend после проверок и до первого изменения диска.
#[derive(Debug, Clone)]
pub struct MovePlan {
    /// Нормализованный путь элемента.
    pub from: PathBuf,
    /// Новое место элемента.
    pub new_path: PathBuf,
    /// Папка ли элемент — по диску, а не по индексу.
    pub is_folder: bool,
    /// Пары «откуда → куда» верхнего уровня (research.md §2).
    pub steps: Vec<(PathBuf, PathBuf)>,
    /// Пары по каждой переезжающей заметке и доске.
    pub notes: Vec<PathChange>,
    /// Пары по каждой переезжающей папке (пусто, если переносят файл).
    pub folders: Vec<PathChange>,
}

impl MovePlan {
    pub fn new(
        indexed: &[Note],
        from: PathBuf,
        new_path: PathBuf,
        is_folder: bool,
        steps: Vec<(PathBuf, PathBuf)>,
    ) -> Self {
        let (notes, folders) = path_changes(indexed, &from, &new_path);
        Self {
            folders: if is_folder { folders } else { Vec::new() },
            from,
            new_path,
            is_folder,
            steps,
            notes,
        }
    }

    /// Исход «перенесено» с обратным запросом.
    pub fn done(&self) -> MoveOutcome {
        MoveOutcome::Done {
            moved: self.notes.clone(),
            folders: self.folders.clone(),
            undo: undo_request(&self.from, &self.new_path),
        }
    }
}

// ---------------------------------------------------------------------------
// Чистые проверки и расчёты
// ---------------------------------------------------------------------------

/// Путь `path`, лежащий под `from`, на новом месте `to`.
///
/// Отдельная функция из-за пустого хвоста: `to.join("")` дописывает разделитель
/// (`B/план.typ/`), и строковое представление пути разошлось бы с настоящим.
pub fn rebase(from: &Path, to: &Path, path: &Path) -> PathBuf {
    match path.strip_prefix(from) {
        Ok(tail) if !tail.as_os_str().is_empty() => to.join(tail),
        _ => to.to_path_buf(),
    }
}

/// Приводит запрос к сравнимому виду: без `./`, без хвостового разделителя.
///
/// Непригодный `from` (пустой, абсолютный, с `..`) — `NotFound`, непригодная
/// цель — `TargetNotFound`: перейти по такому пути нельзя, а значит и перенести.
pub fn normalize_request(request: &MoveRequest) -> Result<(PathBuf, PathBuf), MoveRefusal> {
    let from = normalize(&request.from)
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or(MoveRefusal::NotFound)?;
    let to_folder = normalize(&request.to_folder).ok_or(MoveRefusal::TargetNotFound)?;
    Ok((from, to_folder))
}

/// Проверки, не требующие диска (FR-003, FR-005), и новое место элемента.
pub fn check_move(from: &Path, to_folder: &Path) -> Result<PathBuf, MoveRefusal> {
    let request = MoveRequest {
        from: from.to_path_buf(),
        to_folder: to_folder.to_path_buf(),
    };
    let (from, to_folder) = normalize_request(&request)?;
    if service_dir::is_inside_service_dir(&from) || service_dir::is_inside_service_dir(&to_folder) {
        return Err(MoveRefusal::ServiceFolder);
    }
    // Сравнение по компонентам: `Работа` не предок `Работа-архив`.
    if to_folder.starts_with(&from) {
        return Err(MoveRefusal::IntoItself);
    }
    if from.parent().unwrap_or(Path::new("")) == to_folder {
        return Err(MoveRefusal::SamePlace);
    }
    let name = from.file_name().ok_or(MoveRefusal::NotFound)?;
    Ok(to_folder.join(name))
}

/// Пары переезда: по заметкам и доскам, и по папкам.
///
/// `from` — файл, если он есть среди `notes`; иначе папка, и тогда переезжает
/// всё под ней. Папки — сама `from` и каждая вложенная, в которой есть заметка
/// или доска: ровно те, что видит дерево. Для файла, которого в `notes` нет
/// (картинка), список папок всё равно содержит `from`; вызывающий, зная вид
/// элемента по диску, его сбрасывает.
pub fn path_changes(notes: &[Note], from: &Path, to: &Path) -> (Vec<PathChange>, Vec<PathChange>) {
    if notes.iter().any(|note| note.path == from) {
        return (
            vec![PathChange {
                from: from.to_path_buf(),
                to: to.to_path_buf(),
            }],
            Vec::new(),
        );
    }
    let mut moved = Vec::new();
    let mut folders: BTreeSet<PathBuf> = BTreeSet::new();
    folders.insert(from.to_path_buf());
    for note in notes {
        let Ok(tail) = note.path.strip_prefix(from) else {
            continue;
        };
        moved.push(PathChange {
            from: note.path.clone(),
            to: to.join(tail),
        });
        let mut dir = note.path.parent();
        while let Some(current) = dir {
            if !current.starts_with(from) {
                break;
            }
            folders.insert(current.to_path_buf());
            dir = current.parent();
        }
    }
    moved.sort_by(|a, b| a.from.cmp(&b.from));
    let folders = folders
        .into_iter()
        .map(|old| PathChange {
            to: rebase(from, to, &old),
            from: old,
        })
        .collect();
    (moved, folders)
}

/// Собственные папки снимков заметки, которые поедут вместе с ней (FR-018a,
/// research.md §2): прежняя `<заметка>.assets` рядом и, при расположении
/// «рядом с заметкой», `assets/<заметка>/`. Пути берутся из `boards`, а не
/// собираются здесь заново.
///
/// Общая папка корня vault или раздела не возвращается никогда: она принадлежит
/// многим заметкам. Для папки и для доски результат пуст — у доски снимков нет,
/// а всё, что лежит в папке, уезжает вместе с ней.
///
/// Возвращает **кандидатов**: есть ли они на диске, знает backend.
pub fn snapshot_folders_of(
    from: &Path,
    is_folder: bool,
    placement: AssetsPlacement,
) -> Vec<PathBuf> {
    if is_folder || classify(from) != FileKind::Note {
        return Vec::new();
    }
    let beside = from.parent().unwrap_or(Path::new(""));
    let mut found = Vec::new();
    if let Some(name) = assets_folder_name(from) {
        found.push(beside.join(name));
    }
    if placement == AssetsPlacement::NextToNote {
        if let Some(folder) = note_assets_folder(AssetsPlacement::NextToNote, from, &[]) {
            if !found.contains(&folder) {
                found.push(folder);
            }
        }
    }
    found
}

/// Шаги верхнего уровня: сам элемент и существующие папки снимков.
///
/// `new_path` — новое место элемента; папка снимков занимает прежнее
/// положение относительно заметки.
pub fn plan_steps(
    from: &Path,
    new_path: &Path,
    existing_snapshot_folders: &[PathBuf],
) -> Vec<(PathBuf, PathBuf)> {
    let mut steps = vec![(from.to_path_buf(), new_path.to_path_buf())];
    let old_beside = from.parent().unwrap_or(Path::new(""));
    let new_beside = new_path.parent().unwrap_or(Path::new(""));
    for folder in existing_snapshot_folders {
        if let Ok(tail) = folder.strip_prefix(old_beside) {
            steps.push((folder.clone(), new_beside.join(tail)));
        }
    }
    steps
}

/// Имя занятого места в целевой папке, как его показывают пользователю.
pub fn conflict_name(new_beside: &Path, dest: &Path) -> String {
    dest.strip_prefix(new_beside)
        .unwrap_or(dest)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Какие источники потеряют рабочую ссылку (FR-016, FR-017).
///
/// `sources` — (путь до переноса, текст). Ссылка разрывается, только если до
/// переноса её цель существовала, а после — нет; после — значит с учётом
/// переезда и источника (относительные ссылки разрешаются от его папки), и
/// цели. Поэтому ссылки между заметками одной переносимой папки не рвутся, а
/// уже битая ссылка в список не попадает. Результат отсортирован, без повторов.
pub fn broken_by_move(
    sources: &[(PathBuf, String)],
    existing: &HashSet<PathBuf>,
    mapping: &[PathChange],
) -> Vec<PathBuf> {
    let map: HashMap<&Path, &Path> = mapping
        .iter()
        .map(|change| (change.from.as_path(), change.to.as_path()))
        .collect();
    let moved = |path: &Path| -> PathBuf { map.get(path).copied().unwrap_or(path).to_path_buf() };
    let existing_after: HashSet<PathBuf> = existing.iter().map(|path| moved(path)).collect();

    let mut broken = BTreeSet::new();
    for (source, text) in sources {
        let source_after = moved(source);
        for raw in extract_links(text) {
            let Some(before) = resolve_link_target(source, &raw) else {
                continue;
            };
            // Ссылка на себя битой стать не может (так же `graph::links_in`).
            if before == *source || !existing.contains(&before) {
                continue;
            }
            let survives = resolve_link_target(&source_after, &raw)
                .is_some_and(|after| existing_after.contains(&after));
            if !survives {
                broken.insert(source.clone());
                break;
            }
        }
    }
    broken.into_iter().collect()
}

/// Папки, которые могут принять `from`, в порядке показа (FR-012, FR-022).
///
/// Корень — первым, далее по пути. Не входят: папка, где элемент уже лежит;
/// сама `from` и её потомки; служебная папка. `query` — подстрока без учёта
/// регистра по имени и по пути; пустой — все. Папки выводятся из путей
/// заметок и досок, как в дереве.
pub fn move_targets(
    notes: &[Note],
    sections: &[SectionMark],
    root_name: &str,
    from: &Path,
    query: &str,
) -> Vec<MoveTarget> {
    let Some(from) = normalize(from).filter(|path| !path.as_os_str().is_empty()) else {
        return Vec::new();
    };
    let current_parent = from.parent().unwrap_or(Path::new("")).to_path_buf();

    let mut folders: BTreeSet<PathBuf> = BTreeSet::new();
    folders.insert(PathBuf::new());
    for note in notes {
        let mut dir = note.path.parent();
        while let Some(current) = dir {
            if current.as_os_str().is_empty() {
                break;
            }
            folders.insert(current.to_path_buf());
            dir = current.parent();
        }
    }

    let query = query.trim();
    let query_lower = query.to_lowercase();
    let mut targets = Vec::new();
    for folder in folders {
        if folder == current_parent
            || folder.starts_with(&from)
            || service_dir::is_inside_service_dir(&folder)
        {
            continue;
        }
        let is_root = folder.as_os_str().is_empty();
        let name = if is_root {
            root_name.to_string()
        } else {
            folder
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        let path_text = folder.to_string_lossy().replace('\\', "/");
        let name_match = if query.is_empty() {
            None
        } else {
            find_ignoring_case(&name, &query_lower)
        };
        if !query.is_empty()
            && name_match.is_none()
            && !(!is_root && path_text.to_lowercase().contains(&query_lower))
        {
            continue;
        }
        let parent = folder
            .parent()
            .map(|parent| parent.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        targets.push(MoveTarget {
            is_section: sections.iter().any(|section| section.path == folder),
            path: folder,
            name,
            parent: if is_root { String::new() } else { parent },
            name_match,
        });
    }
    targets
}

/// Первое вхождение `needle_lower` в `haystack` без учёта регистра. Диапазон — в
/// кодовых единицах UTF-16 по **исходной** строке: нижний регистр иных букв
/// длиннее исходного, и смещения по перекодированной строке сдвинули бы
/// подсветку.
fn find_ignoring_case(haystack: &str, needle_lower: &str) -> Option<TextSpan> {
    let needle: Vec<char> = needle_lower.chars().collect();
    if needle.is_empty() {
        return None;
    }
    let mut lowered: Vec<char> = Vec::new();
    let mut origin: Vec<(usize, usize)> = Vec::new();
    let mut position = 0;
    for ch in haystack.chars() {
        let width = ch.len_utf16();
        for lower in ch.to_lowercase() {
            lowered.push(lower);
            origin.push((position, position + width));
        }
        position += width;
    }
    if lowered.len() < needle.len() {
        return None;
    }
    (0..=lowered.len() - needle.len())
        .find(|&start| lowered[start..start + needle.len()] == needle[..])
        .map(|start| TextSpan {
            start: origin[start].0,
            end: origin[start + needle.len() - 1].1,
        })
}

/// События собственного переноса: `NoteRenamed` на каждую заметку и доску.
///
/// Без `BulkChange*` намеренно (FR-008): массовая пачка сообщила бы
/// интерфейсу о внешнем изменении, а это не оно.
pub fn rename_events(moved: &[PathChange]) -> Vec<VaultEvent> {
    moved
        .iter()
        .map(|change| VaultEvent::NoteRenamed {
            from_path: change.from.clone(),
            to_path: change.to.clone(),
        })
        .collect()
}

/// Обратный запрос: элемент на новом месте → прежняя папка (research.md §7).
pub fn undo_request(from: &Path, new_path: &Path) -> MoveRequest {
    MoveRequest {
        from: new_path.to_path_buf(),
        to_folder: from.parent().unwrap_or(Path::new("")).to_path_buf(),
    }
}

// ---------------------------------------------------------------------------
// Помощники поверх `VaultBackend` — одни на оба таргета
// ---------------------------------------------------------------------------

/// Политика расположения снимков vault; нечитаемые настройки — умолчание.
///
/// Перенос не должен отказывать из-за испорченного `vault.json`: по умолчанию
/// «рядом с заметкой» с вероятностью, что папка снимков лежит именно там.
pub async fn placement_of<B: VaultBackend>(backend: &B) -> AssetsPlacement {
    read_vault_settings(backend)
        .await
        .map(|settings| settings.placement())
        .unwrap_or_default()
}

/// Реализация `VaultBackend::move_targets` (общая).
pub async fn move_targets_of<B: VaultBackend>(
    backend: &B,
    from: &Path,
    query: &str,
) -> Vec<MoveTarget> {
    let notes = backend.list().await;
    let sections = read_vault_settings(backend)
        .await
        .map(|settings| settings.sections)
        .unwrap_or_default();
    let root_name = backend
        .root_path()
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    move_targets(&notes, &sections, &root_name, from, query)
}

/// Реализация `VaultBackend::move_impact` после проверок (общая).
///
/// Читается текст только кандидатов: переезжающих заметок и источников
/// обратных ссылок на них (research.md §4). Доски в кандидаты не входят —
/// `[[…]]` в их JSON бывает массивом координат, а не ссылкой.
pub async fn impact_of<B: VaultBackend>(
    backend: &B,
    notes: &[Note],
    moved: &[PathChange],
) -> MoveImpact {
    let existing: HashSet<PathBuf> = notes.iter().map(|note| note.path.clone()).collect();
    let mut candidates: BTreeSet<PathBuf> = BTreeSet::new();
    for change in moved {
        if classify(&change.from) == FileKind::Note {
            candidates.insert(change.from.clone());
        }
        for source in backend.backlinks(&change.from).await {
            if source.kind == FileKind::Note {
                candidates.insert(source.path);
            }
        }
    }
    let mut sources = Vec::with_capacity(candidates.len());
    for path in candidates {
        if let Ok(bytes) = backend.read_bytes_any_size(&path).await {
            sources.push((path, String::from_utf8_lossy(&bytes).into_owned()));
        }
    }
    let broken = broken_by_move(&sources, &existing, moved);
    let by_path: HashMap<&Path, &Note> = notes
        .iter()
        .map(|note| (note.path.as_path(), note))
        .collect();
    MoveImpact {
        broken: broken
            .iter()
            .filter_map(|path| by_path.get(path.as_path()).map(|note| (*note).clone()))
            .collect(),
    }
}

/// Пометки разделов едут за папкой (FR-018), отметки «намеренно без ссылок»
/// (028) — за заметкой и папкой. Отказ записи настроек перенос не отменяет —
/// файлы уже переехали, как и в `rename`.
pub async fn move_section_marks<B: VaultBackend>(
    backend: &mut B,
    is_folder: bool,
    from: &Path,
    new_path: &Path,
) {
    if is_folder {
        let _ = move_sections(backend, from, new_path).await;
    }
    let _ = super::vault_settings::move_intentional_orphans(backend, from, new_path).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stamp::FileStamp;

    fn note(path: &str) -> Note {
        Note::new(PathBuf::from(path), FileStamp::from_content(0, b""))
    }

    fn p(path: &str) -> PathBuf {
        PathBuf::from(path)
    }

    #[test]
    fn same_place_is_a_noop_refusal() {
        assert_eq!(
            check_move(Path::new("A/x.typ"), Path::new("A")),
            Err(MoveRefusal::SamePlace)
        );
        assert_eq!(
            check_move(Path::new("x.typ"), Path::new("")),
            Err(MoveRefusal::SamePlace)
        );
    }

    #[test]
    fn a_folder_cannot_enter_itself_or_a_descendant() {
        for target in ["A", "A/C", "A/C/D"] {
            assert_eq!(
                check_move(Path::new("A"), Path::new(target)),
                Err(MoveRefusal::IntoItself),
                "{target}"
            );
        }
        // Сравнение по компонентам: «Работа-архив» не потомок «Работа».
        assert_eq!(
            check_move(Path::new("Работа"), Path::new("Работа-архив")),
            Ok(p("Работа-архив/Работа"))
        );
    }

    #[test]
    fn the_service_folder_is_neither_moved_nor_a_target() {
        assert_eq!(
            check_move(Path::new(".typsidian"), Path::new("A")),
            Err(MoveRefusal::ServiceFolder)
        );
        assert_eq!(
            check_move(Path::new("x.typ"), Path::new(".typsidian/tmp")),
            Err(MoveRefusal::ServiceFolder)
        );
    }

    #[test]
    fn the_new_place_keeps_the_name() {
        assert_eq!(
            check_move(Path::new("A/x.typ"), Path::new("B/C")),
            Ok(p("B/C/x.typ"))
        );
        assert_eq!(
            check_move(Path::new("A/x.typ"), Path::new("")),
            Ok(p("x.typ"))
        );
        // Непригодные пути не доходят до диска.
        assert_eq!(
            check_move(Path::new("../x.typ"), Path::new("B")),
            Err(MoveRefusal::NotFound)
        );
        assert_eq!(
            check_move(Path::new("x.typ"), Path::new("/abs")),
            Err(MoveRefusal::TargetNotFound)
        );
    }

    #[test]
    fn a_file_moves_alone_and_a_folder_takes_everything_under_it() {
        let notes = [
            note("A/x.typ"),
            note("A/C/y.typ"),
            note("A/C/D/z.excalidraw"),
            note("A-b/w.typ"),
        ];
        let (moved, folders) = path_changes(&notes, Path::new("A/x.typ"), Path::new("B/x.typ"));
        assert_eq!(
            moved,
            vec![PathChange {
                from: p("A/x.typ"),
                to: p("B/x.typ")
            }]
        );
        assert!(folders.is_empty());

        let (moved, folders) = path_changes(&notes, Path::new("A"), Path::new("B/A"));
        let pairs: Vec<(String, String)> = moved
            .iter()
            .map(|c| {
                (
                    c.from.to_string_lossy().into_owned(),
                    c.to.to_string_lossy().into_owned(),
                )
            })
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("A/C/D/z.excalidraw".into(), "B/A/C/D/z.excalidraw".into()),
                ("A/C/y.typ".into(), "B/A/C/y.typ".into()),
                ("A/x.typ".into(), "B/A/x.typ".into()),
            ]
        );
        let folder_pairs: Vec<&PathBuf> = folders.iter().map(|c| &c.to).collect();
        assert_eq!(folder_pairs, vec![&p("B/A"), &p("B/A/C"), &p("B/A/C/D")]);
    }

    #[test]
    fn only_a_note_takes_its_own_snapshot_folders_along() {
        let next = AssetsPlacement::NextToNote;
        assert_eq!(
            snapshot_folders_of(Path::new("A/план.typ"), false, next),
            vec![p("A/план.assets"), p("A/assets/план")]
        );
        // Общая папка корня или раздела не едет: вариантов нет, кроме прежнего.
        assert_eq!(
            snapshot_folders_of(Path::new("A/план.typ"), false, AssetsPlacement::VaultRoot),
            vec![p("A/план.assets")]
        );
        assert!(snapshot_folders_of(Path::new("A/доска.excalidraw"), false, next).is_empty());
        assert!(snapshot_folders_of(Path::new("A"), true, next).is_empty());
    }

    #[test]
    fn snapshot_folders_keep_their_place_relative_to_the_note() {
        let steps = plan_steps(
            Path::new("A/план.typ"),
            Path::new("B/C/план.typ"),
            &[p("A/план.assets"), p("A/assets/план")],
        );
        assert_eq!(
            steps,
            vec![
                (p("A/план.typ"), p("B/C/план.typ")),
                (p("A/план.assets"), p("B/C/план.assets")),
                (p("A/assets/план"), p("B/C/assets/план")),
            ]
        );
    }

    #[test]
    fn a_relative_link_to_a_moved_note_breaks_and_so_does_a_rooted_one() {
        let existing: HashSet<PathBuf> = [p("a.typ"), p("b.typ"), p("c.typ")].into();
        let mapping = [PathChange {
            from: p("b.typ"),
            to: p("B/b.typ"),
        }];
        let sources = vec![
            (p("a.typ"), "см. [[b]]".to_string()),
            (p("c.typ"), "см. [[/b]]".to_string()),
        ];
        assert_eq!(
            broken_by_move(&sources, &existing, &mapping),
            vec![p("a.typ"), p("c.typ")]
        );
    }

    #[test]
    fn the_moved_note_own_relative_links_break_too() {
        let existing: HashSet<PathBuf> = [p("a.typ"), p("b.typ")].into();
        let mapping = [PathChange {
            from: p("a.typ"),
            to: p("B/a.typ"),
        }];
        let sources = vec![(p("a.typ"), "[[b]]".to_string())];
        assert_eq!(
            broken_by_move(&sources, &existing, &mapping),
            vec![p("a.typ")]
        );
        // Корневая ссылка переезда не замечает.
        let rooted = vec![(p("a.typ"), "[[/b]]".to_string())];
        assert!(broken_by_move(&rooted, &existing, &mapping).is_empty());
    }

    #[test]
    fn links_inside_a_moved_folder_survive_and_already_broken_ones_stay_out() {
        let existing: HashSet<PathBuf> = [p("A/x.typ"), p("A/y.typ")].into();
        let mapping = [
            PathChange {
                from: p("A/x.typ"),
                to: p("B/A/x.typ"),
            },
            PathChange {
                from: p("A/y.typ"),
                to: p("B/A/y.typ"),
            },
        ];
        let sources = vec![
            (p("A/x.typ"), "[[y]] и [[нет такой]]".to_string()),
            (p("A/y.typ"), "[[x]]".to_string()),
        ];
        assert!(broken_by_move(&sources, &existing, &mapping).is_empty());
    }

    #[test]
    fn targets_exclude_the_current_parent_the_item_and_its_descendants() {
        let notes = [
            note("A/x.typ"),
            note("A/C/y.typ"),
            note("B/z.typ"),
            note("B/D/w.typ"),
        ];
        let paths = |from: &str| -> Vec<String> {
            move_targets(&notes, &[], "vault", Path::new(from), "")
                .into_iter()
                .map(|t| t.path.to_string_lossy().into_owned())
                .collect()
        };
        assert_eq!(paths("A/x.typ"), vec!["", "A/C", "B", "B/D"]);
        assert_eq!(paths("A"), vec!["B", "B/D"]);
        assert_eq!(paths("B/z.typ"), vec!["", "A", "A/C", "B/D"]);
    }

    #[test]
    fn targets_flag_sections_and_name_the_root_after_the_vault() {
        let notes = [note("A/x.typ"), note("B/z.typ")];
        let sections = [SectionMark::new(Path::new("B")).unwrap()];
        let targets = move_targets(&notes, &sections, "vault", Path::new("A/x.typ"), "");
        assert_eq!(targets[0].name, "vault");
        assert_eq!(targets[0].parent, "");
        assert!(!targets[0].is_section);
        assert_eq!(targets[1].name, "B");
        assert!(targets[1].is_section);
    }

    #[test]
    fn the_filter_matches_name_with_a_span_and_path_without() {
        let notes = [note("Работа/Проекты/x.typ"), note("Личное/y.typ")];
        let found = move_targets(&notes, &[], "vault", Path::new("z.typ"), "ПРОЕ");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, p("Работа/Проекты"));
        assert_eq!(found[0].name_match, Some(TextSpan { start: 0, end: 4 }));
        assert_eq!(found[0].parent, "Работа");

        // Совпало в пути, не в имени.
        let by_path = move_targets(&notes, &[], "vault", Path::new("z.typ"), "работа/пр");
        assert_eq!(by_path.len(), 1);
        assert_eq!(by_path[0].name_match, None);

        // Корень находится по имени vault.
        let root = move_targets(&notes, &[], "Мой Vault", Path::new("A/z.typ"), "vault");
        assert_eq!(root.len(), 1);
        assert_eq!(root[0].path, p(""));
        assert_eq!(root[0].name_match, Some(TextSpan { start: 4, end: 9 }));

        assert!(move_targets(&notes, &[], "vault", Path::new("z.typ"), "нет").is_empty());
    }
}
