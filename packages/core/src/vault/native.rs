use crate::vault::backend::{
    valid_note_name, CreateError, DeleteError, OpenError, ReadError, RenameError, ResolveError,
    VaultBackend, WriteError,
};
use crate::vault::cache::{
    record_derived, record_links, CacheMove, Derived, IndexCache, SqliteCache,
};
use crate::vault::conflict::{line_diff, ConflictRegistry, ConflictStatus, DiffLine, EditConflict};
use crate::vault::events::{links_invalidated, wrap_bulk_if_needed, VaultEvent};
use crate::vault::index::{FileStamp, Index, Note};
use crate::vault::relocate::{
    check_move, conflict_name, normalize_request, placement_of, plan_steps, snapshot_folders_of,
    MoveControl, MoveError, MoveImpact, MoveOutcome, MovePlan, MoveRefusal, MoveRequest,
    MoveTarget,
};
use crate::vault::{AccessState, NOTE_SIZE_LIMIT_BYTES, SUPPORTED_NOTE_COUNT};
use notify::event::{ModifyKind, RenameMode};
use notify::{RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, DebouncedEvent, FileIdMap};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex as AsyncMutex;

const HIDDEN_OR_VCS: &[&str] = &[".git", ".svn", ".hg"];

fn is_excluded(entry_name: &str) -> bool {
    entry_name.starts_with('.') || HIDDEN_OR_VCS.contains(&entry_name)
}

/// Попадает ли файл в дерево. Правило одно на оба backend — `super::classify`
/// (research.md §3 фичи 002): заметки и доски да, снимки и посторонние файлы
/// нет.
fn is_listed(path: &Path) -> bool {
    super::classify(path).is_listed()
}

/// Удаление в корзину ОС — там, где корзина есть.
///
/// `false` означает «в корзину не получилось», и вызывающий уходит на
/// безвозвратное удаление. Это не мобильная ветка, а обычный исход: корзина
/// недоступна и на рабочем столе — на другом томе, на сетевом диске, без прав.
///
/// На Android корзины нет вовсе: ни спецификации freedesktop, ни системного
/// аналога, видимого файловому приложению, — и крейт `trash` под этот таргет не
/// собирается даже теоретически (его платформенные модули исключают
/// `target_os = "android"` явно). Поэтому здесь не заглушка «пока не сделали», а
/// зафиксированное поведение платформы: на Android удаление сразу
/// безвозвратное.
#[cfg(not(target_os = "android"))]
fn move_to_trash(path: &Path) -> bool {
    trash::delete(path).is_ok()
}

#[cfg(target_os = "android")]
fn move_to_trash(_path: &Path) -> bool {
    false
}

/// Перенос в свою корзину (фича 037). Не вышло — `false`, и файл удаляется, как
/// при отсутствии системной корзины.
fn move_to_own_trash(dir: &Path, path: &Path, full: &Path) -> bool {
    let flat = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("__");
    let target = dir.join(format!("{}-{}", crate::properties::now_ms(), flat));
    std::fs::create_dir_all(dir).is_ok() && std::fs::rename(full, target).is_ok()
}

/// Записывает файл так, что на месте цели в любой момент лежит либо прежнее
/// содержимое целиком, либо новое целиком (009 FR-030, research.md §4).
///
/// Запись идёт во временный файл, затем `sync_all` и `rename` на место. Сбой
/// или убитый процесс между этими шагами оставляет прежний файл нетронутым, а
/// временный — в `.typsidian/tmp/`, откуда его убирает следующее открытие
/// vault.
///
/// Если `rename` из служебной папки невозможен из-за другой файловой системы
/// (внутрь vault смонтировано что-то своё), временный файл кладётся рядом с
/// целью под скрытым именем — `rename` в пределах одной папки атомарен всегда.
pub(crate) fn write_atomically(root: &Path, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if !super::is_inside(path) {
        return Err(std::io::ErrorKind::PermissionDenied.into());
    }
    let target = root.join(path);
    let tmp_dir = root.join(super::service_dir::tmp_dir());
    std::fs::create_dir_all(&tmp_dir)?;
    let tmp = tmp_dir.join(unique_tmp_name());
    write_synced(&tmp, bytes)?;
    match std::fs::rename(&tmp, &target) {
        Ok(()) => Ok(()),
        Err(error) if is_cross_device(&error) => {
            let _ = std::fs::remove_file(&tmp);
            let name = target
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let beside = target.with_file_name(format!(".{name}.{}", unique_tmp_name()));
            write_synced(&beside, bytes)?;
            std::fs::rename(&beside, &target).inspect_err(|_| {
                let _ = std::fs::remove_file(&beside);
            })
        }
        Err(error) => {
            let _ = std::fs::remove_file(&tmp);
            Err(error)
        }
    }
}

fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let result = (|| {
        let mut file = std::fs::File::create(path)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(path);
    }
    result
}

/// Имя временного файла, уникальное в пределах машины: процесс и счётчик.
/// Два окна на одном vault не должны писать в один и тот же временный файл.
fn unique_tmp_name() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}.typsidian-tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// `EXDEV`: переименование через границу файловых систем.
fn is_cross_device(error: &std::io::Error) -> bool {
    #[cfg(unix)]
    {
        error.raw_os_error() == Some(18)
    }
    #[cfg(windows)]
    {
        // ERROR_NOT_SAME_DEVICE
        error.raw_os_error() == Some(17)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = error;
        false
    }
}

/// Создаёт недостающие папки до `dir`, запоминая, какие создал именно она, —
/// чтобы откат переноса убрал только их (фича 023): путь папки снимков
/// `assets/<заметка>` в целевой папке может потребовать нового `assets`.
fn create_dirs_tracking(dir: &Path, created: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut missing = Vec::new();
    let mut current = dir;
    while !current.exists() {
        missing.push(current.to_path_buf());
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    for folder in missing.into_iter().rev() {
        std::fs::create_dir(&folder)?;
        created.push(folder);
    }
    Ok(())
}

fn stamp_for(path: &Path) -> std::io::Result<FileStamp> {
    let content = std::fs::read(path)?;
    let metadata = std::fs::metadata(path)?;
    let mtime_millis = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    Ok(FileStamp::from_content(mtime_millis, &content))
}

/// Читает файл ровно один раз и отдаёт всё, что из этого чтения нужно: снимок
/// для индекса и исходящие ссылки для таблицы `links` (T085). Раздельные обходы
/// «сначала метаданные, потом ссылки» стоили бы второго полного чтения vault на
/// каждом открытии (SC-002).
fn note_and_links_from_path(root: &Path, path: &Path) -> Option<(Note, Derived)> {
    let content = std::fs::read(path).ok()?;
    let metadata = std::fs::metadata(path).ok()?;
    let mtime_millis = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let rel = path.strip_prefix(root).ok()?.to_path_buf();
    // Ссылки и свойства (фича 020) — из того же чтения файла.
    let links = Derived::of(&rel, &String::from_utf8_lossy(&content));
    let stamp = FileStamp::from_content(mtime_millis, &content);
    Some((Note::new(rel, stamp), links))
}

/// Обновляет индекс свежими метаданными файла, не сбрасывая
/// `has_unsaved_edits`, если заметка уже была в индексе. Внешний конвейер
/// (`watch()`/сверка) не имеет права затирать состояние открытой в
/// редакторе сессии — backend обязан не терять и не откатывать локальное
/// состояние сам по себе (FR-035), даже когда обновляет метаданные по
/// внешнему изменению; фактическое поднятие `ConflictDetected` вместо
/// тихого `NoteUpdated` при наличии несохранённых правок — предмет US4
/// (T071+), эта функция лишь не допускает регресса до неё.
fn upsert_preserving_unsaved_edits(index: &mut Index, mut fresh: Note) {
    if let Some(existing) = index.get(&fresh.path) {
        fresh.has_unsaved_edits = existing.has_unsaved_edits;
    }
    index.insert(fresh);
}

/// Классифицированное сырое изменение на диске, полученное из потока
/// `notify`/`notify-debouncer-full` — абсолютные пути, ещё не сверенные с
/// индексом (это делает `spawn_change_processor`). Отдельный от
/// `VaultEvent` тип нужен, потому что один `RawChange` может породить ноль
/// событий (например, `Created`/`Updated` файла, уже исчезнувшего к моменту
/// обработки — временный файл atomic-write, T046) или потребовать сверки с
/// текущим индексом, чтобы решить Created-vs-Updated.
#[derive(Debug, Clone)]
enum RawChange {
    Created(PathBuf),
    Updated(PathBuf),
    Deleted(PathBuf),
    Renamed(PathBuf, PathBuf),
    /// Изменился файл-кандидат в источники (фича 017): какой — решает сверка
    /// отпечатков в библиотеке, поэтому без пути.
    SourceFiles,
}

/// Классифицирует один `DebouncedEvent` в ноль или более [`RawChange`],
/// отправляемых в `tx`. Ключевое отличие от наивного матчинга по
/// `EventKind` — явная обработка `ModifyKind::Name(RenameMode::*)`
/// (research.md §2, FR-016):
///
/// - `RenameMode::Both` — дебаунсер (или сам ОС-бэкенд `notify`, см.
///   `notify::inotify`) уже скоррелировал пару `From`/`To` в одно событие с
///   двумя путями `[from, to]` (`push_rename_event` в
///   `notify-debouncer-full`, `paths` в порядке `(from, to)` по контракту
///   `RenameMode::Both`). Если обе стороны — заметки (`.typ`), это честный
///   `NoteRenamed`. Если ровно одна сторона — заметка, а другая нет
///   (например, `atomic.typ.tmp` -> `atomic.typ`), это не переименование
///   заметки, а появление/исчезновение заметки под тем именем, которое нас
///   интересует — трактуется как `Created`/`Deleted` соответственно.
/// - `RenameMode::From`/`RenameMode::To` без пары — ОС не смогла
///   скоррелировать (перемещение за пределы watched-дерева, обрезанный
///   от/to у забюджеченного backend) — деградируем до `Deleted`/`Created`
///   вместо переименования, не теряя сам факт изменения.
fn dispatch_debounced_event(ev: &DebouncedEvent, tx: &UnboundedSender<RawChange>) {
    // Файлы источников (017) в дерево не попадают, но их изменение обязано
    // дойти до библиотеки: создание, правка, удаление, переименование.
    if !matches!(ev.event.kind, notify::EventKind::Access(_))
        && ev
            .event
            .paths
            .iter()
            .any(|path| crate::sources::discover::is_source_candidate(path))
    {
        let _ = tx.send(RawChange::SourceFiles);
    }
    match &ev.event.kind {
        notify::EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
            if let [from, to] = ev.event.paths.as_slice() {
                match (is_listed(from), is_listed(to)) {
                    (true, true) => {
                        let _ = tx.send(RawChange::Renamed(from.clone(), to.clone()));
                    }
                    (true, false) => {
                        let _ = tx.send(RawChange::Deleted(from.clone()));
                    }
                    (false, true) => {
                        let _ = tx.send(RawChange::Created(to.clone()));
                    }
                    (false, false) => {}
                }
            }
        }
        notify::EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
            if let Some(path) = ev.event.paths.first() {
                if is_listed(path) {
                    let _ = tx.send(RawChange::Deleted(path.clone()));
                }
            }
        }
        notify::EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
            if let Some(path) = ev.event.paths.first() {
                if is_listed(path) {
                    let _ = tx.send(RawChange::Created(path.clone()));
                }
            }
        }
        notify::EventKind::Create(_) => {
            for path in &ev.event.paths {
                if is_listed(path) {
                    let _ = tx.send(RawChange::Created(path.clone()));
                }
            }
        }
        notify::EventKind::Modify(_) => {
            for path in &ev.event.paths {
                if is_listed(path) {
                    let _ = tx.send(RawChange::Updated(path.clone()));
                }
            }
        }
        notify::EventKind::Remove(_) => {
            for path in &ev.event.paths {
                if is_listed(path) {
                    let _ = tx.send(RawChange::Deleted(path.clone()));
                }
            }
        }
        _ => {}
    }
}

/// Период опроса доступности vault (T056, FR-020, FR-036) — не привязан ни
/// к одному SC-порогу напрямую (в spec.md нет отдельного бюджета для
/// обнаружения потери доступа), выбран достаточно коротким для отзывчивого
/// UI-уведомления и достаточно длинным, чтобы не создавать заметной
/// фоновой нагрузки (сама проверка — дешёвые `metadata`/`read_dir`, не
/// полный обход).
const ACCESS_CHECK_INTERVAL: Duration = Duration::from_millis(500);

/// Определяет `AccessState` по фактическому состоянию `root` на диске:
/// путь недоступен для чтения (удалён, отключён диск, отозвано разрешение
/// на уровне ОС) -> `Unavailable` (FR-020); путь читается, но не
/// принимает записи (биты доступа запрещают запись) -> `ReadOnly`
/// (FR-036); иначе -> `ReadWrite`. Не имеет побочных эффектов — не
/// создаёт/не удаляет файлы-пробники (в отличие от гипотетической
/// probe-write стратегии), чтобы не полагаться на то, что диск не заполнен
/// под конец теста Edge Case «диск заполнен».
fn probe_access_state(root: &Path) -> AccessState {
    let Ok(metadata) = std::fs::metadata(root) else {
        return AccessState::Unavailable;
    };
    if !metadata.is_dir() {
        return AccessState::Unavailable;
    }
    if std::fs::read_dir(root).is_err() {
        return AccessState::Unavailable;
    }
    if metadata.permissions().readonly() {
        AccessState::ReadOnly
    } else {
        AccessState::ReadWrite
    }
}

/// Конфликтные копии Syncthing (фича 011, FR-024). Обходит и служебное место,
/// но не каталог истории и не прочие скрытые каталоги: там копий, о которых
/// стоит говорить пользователю, не бывает.
fn walk_conflict_copies(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                if !is_excluded(&name) || name == super::service_dir::SERVICE_DIR_NAME {
                    stack.push(entry.path());
                }
            } else if file_type.is_file() && crate::sync::conflict_copy::is_conflict_copy(&name) {
                if let Ok(relative) = entry.path().strip_prefix(root) {
                    found.push(relative.to_path_buf());
                }
            }
        }
    }
    found.sort();
    found
}

/// Рекурсивный обход vault (FR-002/FR-003), исключающий скрытые/служебные
/// каталоги (FR-004); символические ссылки не раскрываются (Assumptions).
///
/// `limit` — предел числа заметок (FR-038). Обход прерывается, как только
/// найденных заметок стало **больше** предела, и возвращает [`Err`]: считать
/// весь оставшийся vault только ради того, чтобы отказать, бессмысленно, а на
/// достаточно большой папке — ещё и долго. `limit: usize::MAX` отключает
/// проверку (сверка уже открытого vault, где предел применён при открытии).
fn walk(root: &Path, limit: usize) -> Result<Vec<(Note, Derived)>, TooManyNotes> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_excluded(&name) {
                continue;
            }
            let file_type = match entry.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                stack.push(path);
            } else if file_type.is_file() && is_listed(&path) {
                if let Some(entry) = note_and_links_from_path(root, &path) {
                    found.push(entry);
                    if found.len() > limit {
                        return Err(TooManyNotes);
                    }
                }
            }
        }
    }
    Ok(found)
}

/// Обход прерван по пределу FR-038.
struct TooManyNotes;

/// Считает заметки, не читая их, и останавливается сразу за `limit`.
///
/// Нужен для быстрого пути SC-003: там полного обхода нет, список берётся из
/// кэша, — а кэш производен и неавторитетен (принцип VI конституции), поэтому
/// решать по нему «vault в пределах лимита» нельзя. Vault мог вырасти за
/// предел, пока приложение было закрыто, и без этой проверки открылся бы в
/// обход FR-038.
///
/// Стоит существенно дешевле `walk`: только `read_dir`/`file_type`, без
/// чтения содержимого и вычисления снимков, — поэтому бюджет SC-003 (≤1 с) он
/// не съедает.
fn exceeds_note_limit(root: &Path, limit: usize) -> bool {
    let mut count = 0usize;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_excluded(&name) {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                stack.push(entry.path());
            } else if file_type.is_file() && is_listed(&entry.path()) {
                count += 1;
                if count > limit {
                    return true;
                }
            }
        }
    }
    false
}

/// Все файлы папки с размерами, кроме скрытых записей (фича 021): их число
/// возвращается отдельно. Символические ссылки не раскрываются. Порядок — по
/// пути, чтобы план импорта не зависел от порядка `read_dir`.
fn walk_source_files(root: &Path) -> crate::vault::SourceListing {
    let mut listing = crate::vault::SourceListing::default();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_excluded(&name) {
                listing.excluded += 1;
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            if !file_type.is_file() {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            listing.files.push(crate::vault::SourceFile {
                path: relative.to_path_buf(),
                size: metadata.len(),
            });
        }
    }
    listing
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    listing
}

/// Шрифты vault по метаданным, без чтения содержимого (FR-027).
///
/// Правила обхода те же, что у [`walk`]: скрытые каталоги и символические
/// Кандидаты в файлы источников (фича 017): обход шрифтов с другим отбором.
fn walk_source_candidates(root: &Path) -> Vec<crate::sources::SourceCandidate> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_excluded(&name) {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            if !file_type.is_file() || !crate::sources::discover::is_source_candidate(relative) {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let modified_ms = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |since| since.as_millis() as i64);
            found.push(crate::sources::SourceCandidate {
                path: relative.to_string_lossy().replace('\\', "/"),
                size: metadata.len(),
                modified_ms,
            });
        }
    }
    found.sort_by(|left, right| left.path.cmp(&right.path));
    found
}

/// ссылки пропускаются. Порядок — по пути, чтобы набор шрифтов, а с ним и выбор
/// Изображения vault (фича 016): тот же обход, что у шрифтов, — скрытые папки
/// и символические ссылки пропускаются, читаются только метаданные.
fn walk_images(root: &Path) -> Vec<crate::vault::ImageFile> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_excluded(&name) {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            push_image(root, &path, &entry, &mut found);
        }
    }
    found.sort_by(|left, right| left.path.cmp(&right.path));
    found
}

/// Изображения одной папки без вложенных (фича 016). Папка может быть скрытой
/// (`.attachments`): её назвал пользователь, и скрытость тут не правило обхода.
fn images_in_folder(root: &Path, folder: &Path) -> Vec<crate::vault::ImageFile> {
    let mut found = Vec::new();
    if !super::is_inside(folder) {
        return found;
    }
    let Ok(entries) = std::fs::read_dir(root.join(folder)) else {
        return found;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_file() {
            push_image(root, &entry.path(), &entry, &mut found);
        }
    }
    found.sort_by(|left, right| left.path.cmp(&right.path));
    found
}

fn push_image(
    root: &Path,
    path: &Path,
    entry: &std::fs::DirEntry,
    found: &mut Vec<crate::vault::ImageFile>,
) {
    let Ok(relative) = path.strip_prefix(root) else {
        return;
    };
    if !crate::vault::is_image_file(relative) {
        return;
    }
    let Ok(metadata) = entry.metadata() else {
        return;
    };
    if !metadata.is_file() {
        return;
    }
    found.push(crate::vault::ImageFile {
        path: relative.to_path_buf(),
        size: metadata.len(),
    });
}

/// начертания при совпадении имён, не зависел от порядка `read_dir`.
fn walk_fonts(root: &Path) -> Vec<crate::vault::FontFile> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_excluded(&name) {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            if !file_type.is_file() || !crate::vault::is_font_file(relative) {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let modified_ms = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |since| since.as_millis() as i64);
            found.push(crate::vault::FontFile {
                path: relative.to_path_buf(),
                size: metadata.len(),
                modified_ms,
            });
        }
    }
    found.sort_by(|left, right| left.path.cmp(&right.path));
    found
}

/// Нативная реализация `VaultBackend`: прямой `std::fs` + `notify` внутри
/// процесса приложения — без обращения к Tauri fs-плагину по IPC (research.md §1).
/// `index`/`cache`/`access_state` — за `Arc<Mutex<_>>`, потому что фоновая сверка
/// при быстром повторном открытии (T026, FR-018), `watch()`-пайплайн (T053/T058)
/// и периодическая проверка доступности (T056) обновляют их конкурентно с уже
/// выданными вызывающему методами и друг с другом.
pub struct NativeBackend {
    root_path: PathBuf,
    access_state: Arc<StdMutex<AccessState>>,
    index: Arc<AsyncMutex<Index>>,
    cache: Arc<AsyncMutex<Option<SqliteCache>>>,
    /// Несохранённые буферы и активные конфликты (US4). За `std::sync::Mutex`,
    /// а не `AsyncMutex`: все обращения к нему точечные и не пересекают
    /// `.await`, а `pending_conflict`/`has_unsaved_edits` вызываются из
    /// синхронного кода UI.
    conflicts: Arc<StdMutex<ConflictRegistry>>,
    events: Arc<StdMutex<VecDeque<VaultEvent>>>,
    /// Сигнал «в очереди событий что-то появилось» (001 T114).
    ///
    /// Нужен оболочке рабочего стола: обнаружение здесь push-овое (`notify` +
    /// дебаунсер), и пересылать события в окно по таймеру означало бы добавить
    /// поллинг там, где его нет. `notify_one`, а не `notify_waiters`: если в
    /// момент публикации никто не ждёт, разрешение сохраняется, и следующий
    /// `notified().await` вернётся сразу — иначе события залежались бы в
    /// очереди до следующего изменения файла.
    events_ready: Arc<tokio::sync::Notify>,
    /// Существовало ли `.typsidian/` до открытия (фича 006, FR-024).
    ///
    /// Снимок момента открытия, а не текущее состояние диска: открытие само
    /// создаёт эту папку, поэтому спросить позже — значит всегда получить
    /// `true` и никогда не сообщить пользователю об утрате настроек.
    service_dir_existed: bool,
    /// Своя корзина вместо системной (фича 037): на сервере удалённые заметки
    /// аккаунта кладутся в папку данных сервера, а не в корзину его пользователя ОС,
    /// где смешались бы с чужими. `None` — системная корзина, как на рабочем столе.
    trash_dir: Option<PathBuf>,
    _watcher: Option<notify_debouncer_full::Debouncer<notify::RecommendedWatcher, FileIdMap>>,
    _access_task: Option<tokio::task::JoinHandle<()>>,
}

impl NativeBackend {
    /// Открывает папку **только для чтения**, ничего в ней не создавая (фича
    /// 021, FR-002, SC-005).
    ///
    /// В отличие от [`VaultBackend::open`] не заводит `.typsidian/`, файл кэша
    /// и слежение за изменениями, не убирает прежний кэш и временные файлы и не
    /// строит индекс: так открывается папка-источник импорта, которая vault
    /// не является. Состояние доступа — `ReadOnly` всегда.
    pub fn open_read_only(root_path: &Path) -> Result<Self, OpenError> {
        if !root_path.exists() {
            return Err(OpenError::NotFound);
        }
        if !root_path.is_dir() {
            return Err(OpenError::NotADirectory);
        }
        if std::fs::read_dir(root_path).is_err() {
            return Err(OpenError::AccessDenied);
        }
        Ok(Self {
            root_path: root_path.to_path_buf(),
            access_state: Arc::new(StdMutex::new(AccessState::ReadOnly)),
            index: Arc::new(AsyncMutex::new(Index::new())),
            cache: Arc::new(AsyncMutex::new(None)),
            conflicts: Arc::new(StdMutex::new(ConflictRegistry::new())),
            events: Arc::new(StdMutex::new(VecDeque::new())),
            events_ready: Arc::new(tokio::sync::Notify::new()),
            service_dir_existed: true,
            trash_dir: None,
            _watcher: None,
            _access_task: None,
        })
    }

    /// Кэш индекса внутри служебного места (фича 006, FR-020).
    fn cache_db_path(root: &Path) -> PathBuf {
        root.join(super::service_dir::cache_path())
    }

    /// Готовит служебное место к открытию vault (фича 006).
    ///
    /// Делает две вещи, обе — при открытии и обе только если папка доступна на
    /// запись (FR-025):
    ///
    /// 1. Создаёт `.typsidian/`. Одинаково с web, где файла кэша нет вовсе
    ///    (research.md §12): расхождение в том, что пользователь видит в своей
    ///    папке, — не тот случай, где принцип II соблюдён «по форме API».
    /// 2. Убирает файл кэша прежней версии из корня (FR-021). Содержимое не
    ///    переносится: кэш производен и полностью восстановим, а код миграции
    ///    схемы понадобился бы ровно один раз и остался бы навсегда
    ///    (research.md §11).
    ///
    /// Отказ на любом шаге открытие **не** блокирует: vault остаётся рабочим,
    /// а невозможность завести служебное место проявится там, где её надо
    /// назвать пользователю, — при попытке записать настройки.
    /// Возвращает, **существовало ли** служебное место до вызова: этот факт
    /// нужен FR-024, а спросить его позже нельзя — здесь оно и создаётся.
    fn prepare_service_dir(root: &Path) -> bool {
        let service_dir = root.join(super::service_dir::service_dir());
        let existed = service_dir.is_dir();

        let legacy = root.join(super::service_dir::legacy_cache_path());
        if legacy.exists() {
            let _ = std::fs::remove_file(&legacy);
        }
        // Временные файлы записи, оставшиеся от прерванного процесса: ни один
        // из них не был переименован на место, значит ни один не нужен
        // (009 SC-006).
        let _ = std::fs::remove_dir_all(root.join(super::service_dir::tmp_dir()));
        let _ = std::fs::create_dir_all(&service_dir);
        existed
    }

    /// Единственный гейт записи для всех четырёх операций, названных в FR-036
    /// (сохранение, создание, переименование, удаление). Общий предикат, а не
    /// повторённое условие: разъехавшиеся копии этой проверки — ровно тот
    /// дефект, из-за которого `create`/`rename`/`delete` пропускали её вовсе.
    fn write_denied(&self) -> bool {
        *self.access_state.lock().unwrap() != AccessState::ReadWrite
    }

    /// Запускает нативный FS-watcher (T053): `notify` + дебаунсер
    /// (research.md §2) классифицирует сырые ОС-события в [`RawChange`]
    /// (включая корреляцию rename — см. `dispatch_debounced_event`) и
    /// передаёт их через `tokio::mpsc` в фоновую задачу
    /// (`spawn_change_processor`), которая уже обновляет `index`/`cache` и
    /// публикует итоговые `VaultEvent` (T058).
    fn start_watch(&mut self) {
        let (raw_tx, raw_rx) = tokio::sync::mpsc::unbounded_channel::<RawChange>();
        let mut debouncer = match new_debouncer(
            Duration::from_millis(400),
            None,
            move |result: DebounceEventResult| {
                if let Ok(debounced_events) = result {
                    for ev in &debounced_events {
                        dispatch_debounced_event(ev, &raw_tx);
                    }
                }
                // `Err(errors)` — сбои самого watcher-бэкенда (например,
                // переполнение очереди ОС-событий). Обнаружение
                // недоступности vault (T056) идёт отдельным периодическим
                // опросом (`spawn_access_watch`), а не через этот канал:
                // ошибки watcher'а не всегда означают потерю доступа
                // (могут быть транзиентными), а периодический опрос даёт
                // единообразную (в том числе с web) политику определения
                // access_state.
            },
        ) {
            Ok(d) => d,
            Err(_) => return,
        };
        // Сидируем `FileIdMap` корнем vault: без явного `add_root` кэш
        // file-id не знает о уже существующих на момент `open()` файлах, и
        // rename одного из них в первые же секунды после открытия не
        // сможет матчиться по file id на бэкендах ОС, не дающих rename
        // cookie (см. notify-debouncer-full README/lib.rs пример).
        debouncer
            .cache()
            .add_root(&self.root_path, RecursiveMode::Recursive);
        if debouncer
            .watcher()
            .watch(&self.root_path, RecursiveMode::Recursive)
            .is_ok()
        {
            self.spawn_change_processor(raw_rx);
            self._watcher = Some(debouncer);
        }
    }

    /// Фоновая задача (T058), обрабатывающая поток [`RawChange`] из
    /// `start_watch`: обновляет `index`/`cache` и публикует итоговые
    /// `VaultEvent` в общую очередь `events` — тот же канал, что и
    /// `spawn_reconciliation`, чтобы UI не различал источник события.
    fn spawn_change_processor(
        &mut self,
        mut raw_rx: UnboundedReceiver<RawChange>,
    ) -> tokio::task::JoinHandle<()> {
        let root = self.root_path.clone();
        let index = self.index.clone();
        let cache = self.cache.clone();
        let conflicts = self.conflicts.clone();
        let events = self.events.clone();
        let events_ready = self.events_ready.clone();
        tokio::spawn(async move {
            while let Some(first) = raw_rx.recv().await {
                // Забираем всё, что уже скопилось в канале, одной пачкой —
                // это и есть точка, где массовое внешнее изменение (T048,
                // T055) становится одной пачкой `VaultEvent`, а не потоком
                // одиночных пар Started/Settled на каждое отдельное
                // изменение.
                let mut batch = vec![first];
                while let Ok(next) = raw_rx.try_recv() {
                    batch.push(next);
                }

                // Стамп-вычисления (чтение содержимого + метаданных с
                // диска) — блокирующий I/O, выносится в пул блокирующих
                // потоков (тот же приём, что и в `spawn_reconciliation`/
                // `open()` для `walk()`), иначе один из worker-потоков
                // tokio удерживался бы на время обработки всей пачки
                // (FR-019, SC-005: backend должен оставаться отзывчивым).
                let root_for_blocking = root.clone();
                let to_stamp: Vec<PathBuf> = batch
                    .iter()
                    .filter_map(|c| match c {
                        RawChange::Created(p) | RawChange::Updated(p) => Some(p.clone()),
                        RawChange::Renamed(_, to) => Some(to.clone()),
                        RawChange::Deleted(_) | RawChange::SourceFiles => None,
                    })
                    .collect();
                let stamped: HashMap<PathBuf, Option<(Note, Derived)>> =
                    tokio::task::spawn_blocking(move || {
                        to_stamp
                            .into_iter()
                            .map(|abs| {
                                let entry = note_and_links_from_path(&root_for_blocking, &abs);
                                (abs, entry)
                            })
                            .collect()
                    })
                    .await
                    .unwrap_or_default();

                let mut idx = index.lock().await;
                let mut cache_guard = cache.lock().await;
                let mut pending = Vec::with_capacity(batch.len());
                // Пути, появление/исчезновение/переезд которых меняет статус
                // входящих на них ссылок (FR-030, T087).
                let mut link_targets_touched: Vec<PathBuf> = Vec::new();
                let mut sources_changed = false;
                for change in batch {
                    match change {
                        RawChange::Created(abs) | RawChange::Updated(abs) => {
                            let Some(rel) = abs.strip_prefix(&root).ok().map(Path::to_path_buf)
                            else {
                                continue;
                            };
                            // Файл уже исчез к моменту обработки (например,
                            // временный файл atomic-write, T046) — не
                            // ошибка, просто нет события для этого пути.
                            let Some((note, links)) = stamped.get(&abs).cloned().flatten() else {
                                continue;
                            };
                            // Отпечаток снят до замка индекса: удалённый за это
                            // время файл не должен вернуться в индекс (001
                            // FR-030) — та же проверка, что в сверке.
                            if !abs.exists() {
                                continue;
                            }
                            let known_stamp = idx.get(&rel).map(|n| n.stamp.clone());
                            let is_new = known_stamp.is_none();
                            // US4/T074: конфликт вместо тихого обновления, если
                            // у заметки есть несохранённые правки и содержимое
                            // на диске действительно разошлось (FR-021).
                            // Блокировка не пересекает `.await` — иначе
                            // `std::sync::MutexGuard` не был бы Send.
                            let conflicted = match &known_stamp {
                                Some(known) => conflicts.lock().unwrap().observe_disk_change(
                                    &rel,
                                    known,
                                    &note.stamp,
                                ),
                                None => false,
                            };
                            upsert_preserving_unsaved_edits(&mut idx, note.clone());
                            if let Some(c) = cache_guard.as_mut() {
                                c.upsert_note(&note).await;
                                record_derived(c, &rel, &links).await;
                            }
                            if is_new {
                                link_targets_touched.push(rel.clone());
                            }
                            pending.push(if conflicted {
                                VaultEvent::ConflictDetected {
                                    note_path: rel,
                                    disk_snapshot: note.stamp.clone(),
                                }
                            } else if is_new {
                                VaultEvent::NoteCreated { path: rel }
                            } else {
                                VaultEvent::NoteUpdated { path: rel }
                            });
                        }
                        RawChange::Deleted(abs) => {
                            let Some(rel) = abs.strip_prefix(&root).ok().map(Path::to_path_buf)
                            else {
                                continue;
                            };
                            if idx.get(&rel).is_some() {
                                // FR-025: удаление снаружи заметки с
                                // несохранёнными правками предупреждает, а не
                                // молча уносит буфер вместе с файлом.
                                let had_edits =
                                    conflicts.lock().unwrap().observe_disk_deletion(&rel);
                                idx.remove(&rel);
                                if let Some(c) = cache_guard.as_mut() {
                                    c.remove_note(&rel).await;
                                }
                                link_targets_touched.push(rel.clone());
                                pending.push(if had_edits {
                                    VaultEvent::NoteDeletedWithUnsavedEdits { note_path: rel }
                                } else {
                                    VaultEvent::NoteDeleted { path: rel }
                                });
                            }
                        }
                        RawChange::SourceFiles => sources_changed = true,
                        RawChange::Renamed(from_abs, to_abs) => {
                            let (Some(from_rel), Some(to_rel)) = (
                                from_abs.strip_prefix(&root).ok().map(Path::to_path_buf),
                                to_abs.strip_prefix(&root).ok().map(Path::to_path_buf),
                            ) else {
                                continue;
                            };
                            // `Index::rename` сохраняет `has_unsaved_edits`
                            // заметки (FR-015) — здесь дополнительно
                            // обновляется только `stamp`, если удалось
                            // прочитать актуальное содержимое по новому
                            // пути (контент мог измениться в рамках того
                            // же atomic move, T046).
                            idx.rename(&from_rel, &to_rel);
                            if let Some((fresh, links)) = stamped.get(&to_abs).cloned().flatten() {
                                if let Some(entry) = idx.get_mut(&to_rel) {
                                    entry.stamp = fresh.stamp.clone();
                                }
                                if let Some(c) = cache_guard.as_mut() {
                                    c.rename_note(&from_rel, &to_rel).await;
                                    c.upsert_note(&fresh).await;
                                    record_derived(c, &to_rel, &links).await;
                                }
                            } else if let Some(c) = cache_guard.as_mut() {
                                c.rename_note(&from_rel, &to_rel).await;
                            }
                            // Переезд гасит входящие ссылки на старый путь и
                            // может починить ссылки на новый (FR-030).
                            link_targets_touched.push(from_rel.clone());
                            link_targets_touched.push(to_rel.clone());
                            pending.push(VaultEvent::NoteRenamed {
                                from_path: from_rel,
                                to_path: to_rel,
                            });
                        }
                    }
                }
                let mut affected_by_links: Vec<PathBuf> = Vec::new();
                if let Some(c) = cache_guard.as_ref() {
                    for target in &link_targets_touched {
                        affected_by_links.extend(c.backlinks(target).await);
                    }
                }
                drop(idx);
                drop(cache_guard);
                if sources_changed {
                    pending.push(VaultEvent::SourceFilesChanged);
                }
                if !pending.is_empty() {
                    let mut batch = wrap_bulk_if_needed(pending);
                    batch.extend(links_invalidated(affected_by_links));
                    events.lock().unwrap().extend(batch);
                    events_ready.notify_one();
                }
            }
        })
    }

    /// Периодическая проверка доступности vault (T056, FR-020, FR-036):
    /// единственный источник переходов `access_state` на native. Дешёвая
    /// (`metadata`/`read_dir`, без полного обхода), поэтому вынесена в
    /// `spawn_blocking` только на время самой проверки, не постоянно.
    /// Останавливается сама, как только один раз обнаружила `Unavailable` —
    /// по data-model.md восстановление из `Unavailable` возможно только
    /// через явное переоткрытие (FR-018), дальнейший опрос этим же
    /// экземпляром `NativeBackend` бессмыслен.
    fn spawn_access_watch(&mut self) -> tokio::task::JoinHandle<()> {
        let root = self.root_path.clone();
        let access_state = self.access_state.clone();
        let events = self.events.clone();
        let events_ready = self.events_ready.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(ACCESS_CHECK_INTERVAL);
            // Первый тик срабатывает немедленно — пропускаем его, `open()`
            // уже установил начальное состояние синхронно.
            interval.tick().await;
            loop {
                interval.tick().await;
                let root_for_probe = root.clone();
                let observed =
                    tokio::task::spawn_blocking(move || probe_access_state(&root_for_probe))
                        .await
                        .unwrap_or(AccessState::Unavailable);
                let previous = *access_state.lock().unwrap();
                if observed != previous {
                    *access_state.lock().unwrap() = observed;
                    events
                        .lock()
                        .unwrap()
                        .push_back(VaultEvent::VaultAccessChanged {
                            access_state: observed,
                        });
                    events_ready.notify_one();
                }
                if observed == AccessState::Unavailable {
                    break;
                }
            }
        })
    }

    /// Проверки переноса и план (фича 023, contracts/core-move-api.md, шаг 1).
    ///
    /// Занятость имён смотрится **по диску**, а не по индексу (research.md §3):
    /// в цели может лежать файл, которого дерево не показывает. Сравнение имён
    /// отдано файловой системе — как в `create` и `rename` (001 §FR-012): на
    /// системе без учёта регистра `План` и `план` конфликтуют.
    async fn plan_move(&self, request: &MoveRequest) -> Result<MovePlan, MoveRefusal> {
        if self.write_denied() {
            return Err(MoveRefusal::AccessDenied);
        }
        let (from, to_folder) = normalize_request(request)?;
        let new_path = check_move(&from, &to_folder)?;

        let full_from = self.root_path.join(&from);
        if std::fs::symlink_metadata(&full_from).is_err() {
            return Err(MoveRefusal::NotFound);
        }
        let is_folder = full_from.is_dir();
        if !to_folder.as_os_str().is_empty() && !self.root_path.join(&to_folder).is_dir() {
            return Err(MoveRefusal::TargetNotFound);
        }

        let placement = placement_of(self).await;
        let existing_snapshots: Vec<PathBuf> = snapshot_folders_of(&from, is_folder, placement)
            .into_iter()
            .filter(|folder| self.root_path.join(folder).is_dir())
            .collect();
        let steps = plan_steps(&from, &new_path, &existing_snapshots);
        for (_, dest) in &steps {
            if std::fs::symlink_metadata(self.root_path.join(dest)).is_ok() {
                return Err(MoveRefusal::NameConflict {
                    name: conflict_name(&to_folder, dest),
                });
            }
        }

        let indexed = self.index.lock().await.all();
        Ok(MovePlan::new(&indexed, from, new_path, is_folder, steps))
    }

    /// Публикует `LinksInvalidated` для заметок, ссылающихся на любой из
    /// `targets` (FR-030, T087). Вызывается локальными create/rename/delete:
    /// их сами по себе видит и watch()-пайплайн, но к моменту его срабатывания
    /// заметки уже нет в индексе, поэтому обратные ссылки надо собрать здесь.
    async fn publish_links_invalidated(&self, targets: &[PathBuf]) {
        let mut affected = Vec::new();
        if let Some(cache) = self.cache.lock().await.as_ref() {
            for target in targets {
                affected.extend(cache.backlinks(target).await);
            }
        }
        if let Some(event) = links_invalidated(affected) {
            self.events.lock().unwrap().push_back(event);
            self.events_ready.notify_one();
        }
    }

    /// Сигнал «в очереди событий что-то появилось» (001 T114).
    ///
    /// Оболочка рабочего стола ждёт его и пересылает `poll_events()` в окно,
    /// вместо того чтобы опрашивать ядро по таймеру: обнаружение на native
    /// push-овое, и превращать его в поллинг на границе процесса значило бы
    /// добавить задержку, которой в самом ядре нет.
    /// Удалённое — в эту папку, а не в системную корзину (фича 037). Имя в ней:
    /// время, затем путь заметки в vault с `__` вместо разделителей.
    pub fn set_trash_dir(&mut self, dir: PathBuf) {
        self.trash_dir = Some(dir);
    }

    pub fn events_ready(&self) -> Arc<tokio::sync::Notify> {
        self.events_ready.clone()
    }

    /// Фоновая сверка кэша с реальным состоянием диска (FR-018): полный обход,
    /// диффинг против уже отданного (из кэша) индекса, публикация расхождений
    /// как обычных `VaultEvent` через тот же канал, что и `watch()` — UI не
    /// обязан знать, что событие пришло из сверки, а не из FS-watcher.
    fn spawn_reconciliation(&self) {
        let root = self.root_path.clone();
        let index = self.index.clone();
        let cache = self.cache.clone();
        let conflicts = self.conflicts.clone();
        let events = self.events.clone();
        let events_ready = self.events_ready.clone();
        tokio::spawn(async move {
            let root_for_walk = root.clone();
            let fresh = tokio::task::spawn_blocking(move || {
                let root = root_for_walk;
                // Если root уже недоступен на момент самой сверки (гонка
                // между `open()` и потерей доступа сразу после него,
                // либо просто задержка фонового планирования) — не
                // трактуем «ничего не нашли» как «всё удалено» (FR-020):
                // об истинной потере доступа сообщит отдельно
                // `spawn_access_watch`, а эта сверка просто пропускает
                // текущий проход, не трогая индекс.
                if std::fs::read_dir(&root).is_err() {
                    None
                } else {
                    // Предел FR-038 применён при открытии; здесь vault уже
                    // открыт, и сверка не может «закрыть» его задним числом —
                    // поэтому обход без ограничения.
                    walk(&root, usize::MAX).ok()
                }
            })
            .await
            .unwrap_or(None);
            let Some(fresh) = fresh else {
                return;
            };
            let mut idx = index.lock().await;
            let known_paths: std::collections::HashSet<_> =
                idx.all().into_iter().map(|n| n.path).collect();
            let fresh_paths: std::collections::HashSet<_> =
                fresh.iter().map(|(n, _)| n.path.clone()).collect();
            let mut link_targets_touched: Vec<PathBuf> = Vec::new();

            let mut cache_guard = cache.lock().await;
            // Собираем события в локальный Vec и берём std::sync::Mutex только
            // в самом конце, одним разом — иначе MutexGuard пришлось бы
            // удерживать через `.await` (cache.upsert_note), а std::sync::Mutex
            // не Send через точки await.
            let mut pending = Vec::new();
            for (note, links) in &fresh {
                // Обход шёл без замка индекса: файл, удалённый приложением
                // после обхода, вернулся бы в индекс устаревшей записью
                // (001 FR-030). `delete` убирает файл до замка, поэтому
                // проверка под замком закрывает гонку.
                if !root.join(&note.path).exists() {
                    continue;
                }
                let changed = match idx.get(&note.path) {
                    Some(existing) => existing.stamp != note.stamp,
                    None => true,
                };
                if changed {
                    let known_stamp = idx.get(&note.path).map(|n| n.stamp.clone());
                    let is_new = !known_paths.contains(&note.path);
                    // Тот же разбор, что и в `spawn_change_processor`: сверка —
                    // равноправный источник внешних изменений, и расхождение,
                    // накопившееся пока приложение было закрыто, не имеет права
                    // молча затереть открытый буфер (FR-018 + FR-021).
                    let conflicted = match &known_stamp {
                        Some(known) => conflicts.lock().unwrap().observe_disk_change(
                            &note.path,
                            known,
                            &note.stamp,
                        ),
                        None => false,
                    };
                    upsert_preserving_unsaved_edits(&mut idx, note.clone());
                    if let Some(c) = cache_guard.as_mut() {
                        c.upsert_note(note).await;
                        record_derived(c, &note.path, links).await;
                    }
                    if is_new {
                        link_targets_touched.push(note.path.clone());
                    }
                    pending.push(if conflicted {
                        VaultEvent::ConflictDetected {
                            note_path: note.path.clone(),
                            disk_snapshot: note.stamp.clone(),
                        }
                    } else if is_new {
                        VaultEvent::NoteCreated {
                            path: note.path.clone(),
                        }
                    } else {
                        VaultEvent::NoteUpdated {
                            path: note.path.clone(),
                        }
                    });
                }
            }
            for stale_path in known_paths.difference(&fresh_paths) {
                let had_edits = conflicts.lock().unwrap().observe_disk_deletion(stale_path);
                idx.remove(stale_path);
                if let Some(c) = cache_guard.as_mut() {
                    c.remove_note(stale_path).await;
                }
                link_targets_touched.push(stale_path.clone());
                pending.push(if had_edits {
                    VaultEvent::NoteDeletedWithUnsavedEdits {
                        note_path: stale_path.clone(),
                    }
                } else {
                    VaultEvent::NoteDeleted {
                        path: stale_path.clone(),
                    }
                });
            }
            let mut affected_by_links: Vec<PathBuf> = Vec::new();
            if let Some(c) = cache_guard.as_ref() {
                for target in &link_targets_touched {
                    affected_by_links.extend(c.backlinks(target).await);
                }
            }
            if !pending.is_empty() {
                let mut batch = wrap_bulk_if_needed(pending);
                batch.extend(links_invalidated(affected_by_links));
                events.lock().unwrap().extend(batch);
                events_ready.notify_one();
            }
        });
    }
}

impl Drop for NativeBackend {
    fn drop(&mut self) {
        // `_watcher` (Debouncer) останавливает свой внутренний поток и
        // закрывает `raw_tx` в собственном `Drop` — `spawn_change_processor`
        // естественно завершится, когда `raw_rx.recv()` вернёт `None`,
        // отдельная отмена не нужна. Периодическая проверка доступности не
        // имеет такого естественного сигнала (не привязана к закрытию
        // канала), поэтому останавливается явно.
        if let Some(handle) = self._access_task.take() {
            handle.abort();
        }
    }
}

impl VaultBackend for NativeBackend {
    async fn open(root_path: &Path) -> Result<Self, OpenError> {
        if !root_path.exists() {
            return Err(OpenError::NotFound);
        }
        if !root_path.is_dir() {
            return Err(OpenError::NotADirectory);
        }
        if std::fs::read_dir(root_path).is_err() {
            return Err(OpenError::AccessDenied);
        }

        // FR-007 («предложить последний открытый vault при запуске») с фичи 004
        // обслуживается настройками: `Settings::current_vault` вместе со
        // списком известных vault. Прежний вызов `last_vault::remember` снят
        // вместе с самим модулем — он был вторым источником истины об одном и
        // том же факте и существовал только под native (research.md §10 фичи
        // 004). Запоминание текущего vault теперь делает вызывающая сторона,
        // применяя `SettingsChange::current_vault` после успешного открытия.

        // Служебное место готовится до открытия кэша: файл кэша лежит внутри
        // него, а файл прежней версии обязан исчезнуть из корня (фича 006).
        let service_dir_existed = Self::prepare_service_dir(root_path);

        let mut cache = SqliteCache::open(&Self::cache_db_path(root_path))
            .await
            .ok();
        // Кэш со свойствами другого формата (или без них — до фичи 020) не
        // годится для быстрого пути: он выбрасывается, и холодный обход ниже
        // наполняет ссылки и свойства заново (research.md §5).
        let properties_ready = match &cache {
            Some(c) => c.properties_current().await,
            None => true,
        };
        let cached_notes = match &cache {
            Some(c) if properties_ready => c.load_notes().await,
            _ => Vec::new(),
        };

        // SC-003: если валидный кэш уже есть, отдаём его немедленно и сверяем
        // с диском в фоне (spawn_reconciliation ниже) — вместо полного
        // синхронного обхода на каждое открытие.
        let notes = if !cached_notes.is_empty() {
            // Быстрый путь SC-003 не должен становиться лазейкой в обход
            // FR-038: кэш производен и неавторитетен (принцип VI), а vault мог
            // вырасти за предел, пока приложение было закрыто. Считаем по
            // диску — дёшево, без чтения содержимого.
            let owned_root = root_path.to_path_buf();
            let over_limit = tokio::task::spawn_blocking(move || {
                exceeds_note_limit(&owned_root, SUPPORTED_NOTE_COUNT)
            })
            .await
            .unwrap_or(false);
            if over_limit {
                return Err(OpenError::TooManyNotes);
            }
            cached_notes
        } else {
            // FR-005: обход MUST NOT блокировать поток исполнения.
            let owned_root = root_path.to_path_buf();
            let Ok(found) =
                tokio::task::spawn_blocking(move || walk(&owned_root, SUPPORTED_NOTE_COUNT))
                    .await
                    .unwrap_or(Ok(Vec::new()))
            else {
                // FR-038: обход прерван на превышении предела. Ни индекс, ни
                // кэш не наполняются — частичное дерево не должно пережить
                // отказ и попасть в быстрый путь следующего открытия.
                return Err(OpenError::TooManyNotes);
            };
            // Холодный обход MUST наполнить кэш, иначе следующее открытие не
            // получит быстрый путь SC-003. Ссылки пишутся из того же обхода —
            // второго чтения vault ради таблицы `links` не происходит.
            if let Some(c) = cache.as_mut() {
                for (note, links) in &found {
                    c.upsert_note(note).await;
                    record_derived(c, &note.path, links).await;
                }
                c.mark_properties_current().await;
            }
            found.into_iter().map(|(note, _)| note).collect()
        };

        let mut index = Index::new();
        for note in notes {
            index.insert(note);
        }

        let mut backend = Self {
            root_path: root_path.to_path_buf(),
            access_state: Arc::new(StdMutex::new(AccessState::ReadWrite)),
            index: Arc::new(AsyncMutex::new(index)),
            cache: Arc::new(AsyncMutex::new(cache)),
            conflicts: Arc::new(StdMutex::new(ConflictRegistry::new())),
            events: Arc::new(StdMutex::new(VecDeque::new())),
            events_ready: Arc::new(tokio::sync::Notify::new()),
            service_dir_existed,
            trash_dir: None,
            _watcher: None,
            _access_task: None,
        };
        backend.start_watch();
        backend.spawn_reconciliation();
        backend._access_task = Some(backend.spawn_access_watch());
        Ok(backend)
    }

    fn root_path(&self) -> &Path {
        &self.root_path
    }

    fn access_state(&self) -> AccessState {
        *self.access_state.lock().unwrap()
    }

    fn service_dir_existed_at_open(&self) -> bool {
        self.service_dir_existed
    }

    async fn list(&self) -> Vec<Note> {
        self.index.lock().await.all()
    }

    async fn list_folder(&self, folder: &Path) -> Vec<Note> {
        self.index.lock().await.in_folder(folder)
    }

    async fn list_fonts(&self) -> Vec<crate::vault::FontFile> {
        walk_fonts(&self.root_path)
    }

    async fn list_source_files(&self) -> crate::vault::SourceListing {
        walk_source_files(&self.root_path)
    }

    async fn list_conflict_copies(&self) -> Vec<PathBuf> {
        walk_conflict_copies(&self.root_path)
    }

    async fn list_images(&self, folder: Option<&Path>) -> Vec<crate::vault::ImageFile> {
        match folder {
            None => walk_images(&self.root_path),
            Some(folder) => images_in_folder(&self.root_path, folder),
        }
    }

    async fn list_source_candidates(&self) -> Vec<crate::sources::SourceCandidate> {
        walk_source_candidates(&self.root_path)
    }

    async fn crosses_symlink(&self, path: &Path) -> bool {
        let mut current = self.root_path.clone();
        let Some(parent) = path.parent() else {
            return false;
        };
        for component in parent.components() {
            current.push(component);
            match std::fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => return true,
                Ok(_) => {}
                // Дальше пути ещё нет — ссылок на нём быть не может.
                Err(_) => return false,
            }
        }
        false
    }

    async fn is_symlink(&self, path: &Path) -> bool {
        if !super::is_inside(path) {
            return false;
        }
        std::fs::symlink_metadata(self.root_path.join(path))
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
    }

    async fn file_mark(&self, path: &Path) -> Option<(u64, i64)> {
        if !super::is_inside(path) {
            return None;
        }
        let metadata = std::fs::metadata(self.root_path.join(path)).ok()?;
        if !metadata.is_file() {
            return None;
        }
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |since| since.as_millis() as i64);
        Some((metadata.len(), modified))
    }

    /// Тот же приём, что у [`probe_access_state`], только про вложенную папку:
    /// дешёвые `metadata`/`read_dir` без файлов-пробников.
    async fn folder_access(&self, folder: &Path) -> crate::vault::FolderAccess {
        use crate::vault::{FolderAccess, FolderProblem};

        if !super::is_inside(folder) {
            return FolderAccess::Absent;
        }
        let full = self.root_path.join(folder);
        let metadata = match std::fs::metadata(&full) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return FolderAccess::Absent
            }
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                return FolderAccess::Unreadable {
                    reason: FolderProblem::AccessDenied,
                }
            }
            Err(_) => {
                return FolderAccess::Unreadable {
                    reason: FolderProblem::Io,
                }
            }
        };
        if !metadata.is_dir() {
            return FolderAccess::Unreadable {
                reason: FolderProblem::NotADirectory,
            };
        }
        if let Err(error) = std::fs::read_dir(&full) {
            return FolderAccess::Unreadable {
                reason: if error.kind() == std::io::ErrorKind::PermissionDenied {
                    FolderProblem::AccessDenied
                } else {
                    FolderProblem::Io
                },
            };
        }
        if metadata.permissions().readonly() {
            return FolderAccess::NotWritable {
                reason: FolderProblem::AccessDenied,
            };
        }
        FolderAccess::Present
    }

    async fn read(&self, path: &Path) -> Result<String, ReadError> {
        if !super::is_inside(path) {
            return Err(ReadError::NotFound);
        }
        let full = self.root_path.join(path);
        // Порог проверяется по метаданным, ДО чтения: весь смысл предела в
        // том, чтобы не втягивать в память файл, который мы всё равно
        // отказываемся открывать (spec Assumptions, Edge Cases). Заметка при
        // этом остаётся в индексе и видна в дереве — отказ касается только
        // открытия.
        if std::fs::metadata(&full).is_ok_and(|m| m.len() > NOTE_SIZE_LIMIT_BYTES) {
            return Err(ReadError::TooLarge);
        }
        // Отсутствие файла — отдельная причина, а не общий отказ ввода-вывода
        // (FR-008, FR-014 фичи 001): пользователю показывается «заметку удалили
        // или переименовали снаружи», и эта формулировка недостижима, если
        // схлопнуть всё в `Io`. `WebBackend::read` различает эти случаи с
        // самого начала — принцип II требует одинакового поведения.
        std::fs::read_to_string(&full).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => ReadError::NotFound,
            _ => ReadError::Io,
        })
    }

    async fn read_bytes(&self, path: &Path) -> Result<Vec<u8>, ReadError> {
        if !super::is_inside(path) {
            return Err(ReadError::NotFound);
        }
        let full = self.root_path.join(path);
        // Тот же порядок проверок, что и в `read`: предел по метаданным до
        // чтения, отсутствие файла — отдельной причиной.
        if std::fs::metadata(&full).is_ok_and(|m| m.len() > NOTE_SIZE_LIMIT_BYTES) {
            return Err(ReadError::TooLarge);
        }
        std::fs::read(&full).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => ReadError::NotFound,
            _ => ReadError::Io,
        })
    }

    async fn read_bytes_any_size(&self, path: &Path) -> Result<Vec<u8>, ReadError> {
        if !super::is_inside(path) {
            return Err(ReadError::NotFound);
        }
        std::fs::read(self.root_path.join(path)).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => ReadError::NotFound,
            _ => ReadError::Io,
        })
    }

    async fn write_bytes(&mut self, path: &Path, bytes: &[u8]) -> Result<(), WriteError> {
        if self.write_denied() || !super::is_inside(path) {
            return Err(WriteError::AccessDenied);
        }
        // Индекс и таблица ссылок не трогаются: байтами пишется то, что в
        // дереве не показывается (файлы шаблонов), а `classify` для них всё
        // равно вернул бы `Service`. Разбирать картинку как Typst-заметку было
        // бы прямой ошибкой.
        write_atomically(&self.root_path, path, bytes).map_err(|error| {
            if error.kind() == std::io::ErrorKind::PermissionDenied {
                WriteError::AccessDenied
            } else {
                WriteError::Io
            }
        })?;
        // Файл источников, записанный самим приложением (017, research.md §7).
        if crate::sources::discover::is_source_candidate(path) {
            self.events
                .lock()
                .unwrap()
                .push_back(VaultEvent::SourceFilesChanged);
            self.events_ready.notify_one();
        }
        Ok(())
    }

    async fn write(
        &mut self,
        path: &Path,
        content: &str,
        expected: Option<FileStamp>,
    ) -> Result<(), WriteError> {
        if self.write_denied() || !super::is_inside(path) {
            return Err(WriteError::AccessDenied);
        }
        let full = self.root_path.join(path);
        if let Some(expected) = expected {
            if let Ok(current) = stamp_for(&full) {
                if current != expected {
                    return Err(WriteError::Conflict);
                }
            }
        }
        std::fs::write(&full, content).map_err(|_| WriteError::Io)?;
        // Сохранение снимает несохранённое состояние вместе со всем, что от
        // него зависело: версия пользователя теперь на диске, и конфликт по
        // ней больше не о чем (FR-021 — конфликт существует, только пока
        // расходятся буфер и диск).
        self.conflicts.lock().unwrap().clear(path);
        // В дерево попадает только то, что в дереве показывается: снимок
        // вставки — обычный файл vault, но не запись дерева (FR-021 фичи 002).
        // Без этого гейта первая же вставка добавляла бы свой `.svg` в дерево, а
        // `record_links` разбирал бы SVG как Typst-заметку.
        if let (Ok(stamp), true) = (stamp_for(&full), crate::vault::classify(path).is_listed()) {
            let note = Note::new(path.to_path_buf(), stamp);
            self.index.lock().await.insert(note.clone());
            if let Some(cache) = self.cache.lock().await.as_mut() {
                cache.upsert_note(&note).await;
                // FR-031: исходящие ссылки заметки обновляются в том же шаге,
                // что и её содержимое, — иначе таблица `links` отстаёт от диска
                // до ближайшего внешнего события.
                record_links(cache, path, content).await;
            }
        }
        Ok(())
    }

    async fn ensure_folder(&mut self, path: &Path) -> Result<(), WriteError> {
        if self.write_denied() || !super::is_inside(path) {
            return Err(WriteError::AccessDenied);
        }
        std::fs::create_dir_all(self.root_path.join(path)).map_err(|_| WriteError::Io)
    }

    async fn create(&mut self, path: &Path) -> Result<(), CreateError> {
        // FR-036: отклоняется КАЖДАЯ попытка записи, а не только `write` —
        // создание названо в перечне наравне с сохранением, переименованием и
        // удалением. При `Unavailable` — тем более: до явного переоткрытия
        // приложение к vault не обращается (FR-020).
        if self.write_denied() {
            return Err(CreateError::AccessDenied);
        }
        // FR-013: имя проверяется первым, до любого обращения к диску —
        // в частности, до `full.exists()`, иначе пустой путь (совпадающий с
        // самим `root_path`) ошибочно читался бы как «конфликт имён» вместо
        // «недопустимое имя».
        let name = valid_note_name(path)
            .filter(|_| super::is_inside(path))
            .ok_or(CreateError::InvalidName)?
            .to_string();
        let full = self.root_path.join(path);
        if full.exists() {
            return Err(CreateError::NameConflict);
        }
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).map_err(|_| CreateError::Io)?;
        }
        std::fs::write(&full, "").map_err(|_| CreateError::Io)?;
        if let Ok(stamp) = stamp_for(&full) {
            let note = Note::with_name(path.to_path_buf(), name, stamp);
            self.index.lock().await.insert(note.clone());
            if let Some(cache) = self.cache.lock().await.as_mut() {
                cache.upsert_note(&note).await;
                record_links(cache, path, "").await;
            }
        }
        // Появление заметки чинит ранее битые ссылки на неё (FR-030).
        self.publish_links_invalidated(&[path.to_path_buf()]).await;
        Ok(())
    }

    async fn rename(&mut self, from: &Path, to: &Path) -> Result<(), RenameError> {
        // FR-036 и FR-020: переименование прямо названо среди действий,
        // запрещённых при недоступном vault, и среди попыток записи,
        // отклоняемых при отозванных правах.
        if self.write_denied() {
            return Err(RenameError::AccessDenied);
        }
        // FR-013: тот же порядок, что и в `create` — имя валидируется прежде
        // любой проверки существования на диске.
        if valid_note_name(to).is_none() || !super::is_inside(to) {
            return Err(RenameError::InvalidName);
        }
        if !super::is_inside(from) {
            return Err(RenameError::NotFound);
        }
        let full_from = self.root_path.join(from);
        let full_to = self.root_path.join(to);
        if !full_from.exists() {
            return Err(RenameError::NotFound);
        }
        if full_to.exists() {
            return Err(RenameError::NameConflict);
        }
        // Спрашивается ДО переименования: после него исходного пути уже нет, а
        // ответ нужен для переноса пометки раздела ниже (фича 006, FR-018).
        let renamed_a_folder = full_from.is_dir();
        std::fs::rename(&full_from, &full_to).map_err(|_| RenameError::Io)?;
        self.index.lock().await.rename(from, to);
        if let Some(cache) = self.cache.lock().await.as_mut() {
            cache.rename_note(from, to).await;
        }

        // FR-018: пометка раздела едет за папкой, переименованной **средствами
        // приложения**. Приложение об этом переименовании знает, и терять
        // пометку было бы просто дефектом; переименование снаружи её
        // по-прежнему теряет (FR-019), и розыска нет ни там, ни здесь.
        //
        // Отказ записи настроек переименование не отменяет: файлы уже
        // переехали, и откатывать успешную операцию из-за служебного файла
        // нельзя.
        if renamed_a_folder {
            let _ = super::vault_settings::move_sections(self, from, to).await;
        }
        // Фича 028: отметка «намеренно без ссылок» едет так же.
        let _ = super::vault_settings::move_intentional_orphans(self, from, to).await;
        // Фича 029: история снимков едет за заметкой.
        let _ = crate::snapshots::move_snapshots(self, from, to).await;
        // Фича 032: закладки едут за заметкой или папкой.
        let _ = crate::bookmarks::move_bookmarks(self, from, to).await;
        // FR-028/FR-030: ссылки на старое имя стали битыми, на новое — могли
        // починиться. Само подтверждение операции — забота вызывающей стороны
        // (contracts/vault-backend-trait.md), здесь только факт изменения.
        self.publish_links_invalidated(&[from.to_path_buf(), to.to_path_buf()])
            .await;
        Ok(())
    }

    async fn delete(&mut self, path: &Path) -> Result<(), DeleteError> {
        // FR-036 и FR-020: удаление — второе из двух действий, которые US3 AS6
        // называет разрушительными, и запрещено при недоступном vault явно.
        if self.write_denied() {
            return Err(DeleteError::AccessDenied);
        }
        if !super::is_inside(path) {
            return Err(DeleteError::NotFound);
        }
        let full = self.root_path.join(path);
        if !full.exists() {
            return Err(DeleteError::NotFound);
        }
        // Фича 029: снимок и отметка «удалена» — до того, как файла не станет.
        crate::snapshots::before_delete(self, path, crate::properties::now_ms()).await;
        let trashed = match &self.trash_dir {
            Some(dir) => move_to_own_trash(dir, path, &full),
            None => move_to_trash(&full),
        };
        if !trashed {
            std::fs::remove_file(&full).map_err(|_| DeleteError::Io)?;
        }
        self.index.lock().await.remove(path);
        if let Some(cache) = self.cache.lock().await.as_mut() {
            cache.remove_note(path).await;
        }
        self.publish_links_invalidated(&[path.to_path_buf()]).await;
        Ok(())
    }

    async fn remove_service_file(&mut self, path: &Path) -> Result<(), WriteError> {
        if self.write_denied()
            || !super::is_inside(path)
            || !super::service_dir::is_inside_service_dir(path)
        {
            return Err(WriteError::AccessDenied);
        }
        match std::fs::remove_file(self.root_path.join(path)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(WriteError::Io),
        }
    }

    async fn move_targets(&self, from: &Path, query: &str) -> Vec<MoveTarget> {
        super::relocate::move_targets_of(self, from, query).await
    }

    async fn move_impact(&self, request: &MoveRequest) -> Result<MoveImpact, MoveRefusal> {
        let plan = self.plan_move(request).await?;
        let notes = self.index.lock().await.all();
        Ok(super::relocate::impact_of(self, &notes, &plan.notes).await)
    }

    async fn move_entry(
        &mut self,
        request: &MoveRequest,
        control: &dyn MoveControl,
    ) -> Result<MoveOutcome, MoveError> {
        // Все отказы — до первого изменения диска (contracts/core-move-api.md).
        let plan = self.plan_move(request).await?;
        if control.cancelled() {
            return Ok(MoveOutcome::Cancelled);
        }

        // Каждый шаг плана — один `rename`: атомарно и мгновенно на одной
        // файловой системе при любом размере папки (research.md §5). Если
        // отказал не первый шаг, прежние возвращаются обратным `rename`, и
        // vault остаётся как был.
        let mut done: Vec<(PathBuf, PathBuf)> = Vec::new();
        let mut created_dirs: Vec<PathBuf> = Vec::new();
        let total = plan.steps.len() as u32;
        for (index, (source, dest)) in plan.steps.iter().enumerate() {
            let (abs_source, abs_dest) = (self.root_path.join(source), self.root_path.join(dest));
            let moved = match abs_dest.parent() {
                Some(parent) => create_dirs_tracking(parent, &mut created_dirs),
                None => Ok(()),
            }
            .and_then(|()| std::fs::rename(&abs_source, &abs_dest));
            if moved.is_err() {
                for (source, dest) in done.iter().rev() {
                    let _ = std::fs::rename(dest, source);
                }
                for dir in created_dirs.iter().rev() {
                    let _ = std::fs::remove_dir(dir);
                }
                return Err(MoveError::Io);
            }
            done.push((abs_source, abs_dest));
            control.progress(index as u32 + 1, total);
        }

        // Файлы переехали; дальше состояние приложения догоняет диск ДО того,
        // как наблюдатель `notify` доставит события: он увидит, что индекс уже
        // на новых путях, и не покажет перенос как внешнее изменение или как
        // удаление открытой заметки (FR-008). Замки — в порядке index -> cache.
        let root = self.root_path.clone();
        let renamed = plan.notes.clone();
        let cache_moves: Vec<CacheMove> = tokio::task::spawn_blocking(move || {
            renamed
                .into_iter()
                .map(|change| {
                    // Ссылки пересчитываются из текста под новым путём; у доски
                    // содержимое не Typst, и её ссылки не трогаются.
                    let text = (crate::vault::classify(&change.to) == crate::vault::FileKind::Note)
                        .then(|| std::fs::read(root.join(&change.to)).ok())
                        .flatten()
                        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
                    let links = text
                        .as_deref()
                        .map(|text| crate::graph::links_in(&change.to, text));
                    // Вставленные файлы (фича 025) разрешаются от папки заметки.
                    let embeds = text
                        .as_deref()
                        .map(|text| crate::graph::embeds_in(&change.to, text));
                    CacheMove {
                        from: change.from,
                        to: change.to,
                        links,
                        embeds,
                    }
                })
                .collect()
        })
        .await
        .unwrap_or_default();

        let mut affected = Vec::new();
        {
            let mut index = self.index.lock().await;
            index.rename_prefix(&plan.from, &plan.new_path);
            let mut cache_guard = self.cache.lock().await;
            if let Some(cache) = cache_guard.as_mut() {
                cache.relocate(&cache_moves).await;
                for change in &plan.notes {
                    affected.extend(cache.backlinks(&change.from).await);
                    affected.extend(cache.backlinks(&change.to).await);
                }
            }
            self.conflicts
                .lock()
                .unwrap()
                .rename_prefix(&plan.from, &plan.new_path);
        }
        // Настройки читаются и пишутся через `self`, поэтому замки уже отпущены.
        super::relocate::move_section_marks(self, plan.is_folder, &plan.from, &plan.new_path).await;

        // Без `wrap_bulk_if_needed`: пачка из двадцати `NoteRenamed` стала бы
        // `BulkChange*`, то есть внешним изменением (FR-008).
        let mut batch = super::relocate::rename_events(&plan.notes);
        batch.extend(links_invalidated(affected));
        if !batch.is_empty() {
            self.events.lock().unwrap().extend(batch);
            self.events_ready.notify_one();
        }
        Ok(plan.done())
    }

    async fn backlinks(&self, path: &Path) -> Vec<Note> {
        // Два замка берутся строго последовательно, а не вложенно. Фоновые
        // конвейеры (`spawn_change_processor`, `spawn_reconciliation`) держат
        // порядок index -> cache; удержание здесь cache во время взятия index
        // дало бы инверсию порядка и взаимную блокировку с ними, как только
        // обратные ссылки запрашиваются одновременно с обработкой внешнего
        // изменения. Промежуточный `sources` дешёв (пути, не содержимое).
        let sources = {
            let cache_guard = self.cache.lock().await;
            let Some(cache) = cache_guard.as_ref() else {
                return Vec::new();
            };
            cache.backlinks(path).await
        };
        let index = self.index.lock().await;
        sources
            .into_iter()
            .filter_map(|p| index.get(&p).cloned())
            .collect()
    }

    async fn broken_links(&self, path: &Path) -> Vec<PathBuf> {
        // Тот же последовательный порядок замков, что и в `backlinks`.
        let targets = {
            let cache_guard = self.cache.lock().await;
            let Some(cache) = cache_guard.as_ref() else {
                return Vec::new();
            };
            cache.links_from(path).await
        };
        let index = self.index.lock().await;
        targets
            .into_iter()
            .filter(|target| index.get(target).is_none())
            .collect()
    }

    async fn indexed_notes(&self) -> Vec<crate::properties::IndexedNote> {
        let notes = self.index.lock().await.all();
        let Some(properties) = ({
            let guard = self.cache.lock().await;
            match guard.as_ref() {
                Some(cache) => Some(cache.all_properties().await),
                None => None,
            }
        }) else {
            // Кэш не открылся: индекса свойств нет, читаем заметки.
            return crate::properties::index::scan_notes(self).await;
        };
        crate::properties::index::join_notes(notes, properties, |note| {
            Some(note.stamp.mtime_millis)
        })
    }

    async fn set_indexed_properties(
        &mut self,
        path: &Path,
        entries: Vec<crate::properties::PropertyEntry>,
    ) {
        if let Some(cache) = self.cache.lock().await.as_mut() {
            cache.set_properties(path, &entries).await;
        }
    }

    async fn graph_source(&self) -> crate::graph::view::GraphSource {
        use crate::graph::view::GraphSource;
        let notes = self.indexed_notes().await;
        let boards = self
            .index
            .lock()
            .await
            .all()
            .into_iter()
            .filter(|note| note.kind == crate::vault::FileKind::Board)
            .map(|note| crate::properties::IndexedNote::new(&note.path, &note.name, None))
            .collect();
        let guard = self.cache.lock().await;
        let Some(cache) = guard.as_ref() else {
            drop(guard);
            return crate::graph::view::scan_graph_source(self).await;
        };
        GraphSource {
            notes,
            boards,
            links: cache.all_links().await,
            embeds: cache.all_embeds().await,
        }
    }

    async fn properties_current(&self) -> bool {
        match self.cache.lock().await.as_ref() {
            Some(cache) => cache.properties_current().await,
            None => true,
        }
    }

    async fn mark_properties_current(&mut self) {
        if let Some(cache) = self.cache.lock().await.as_mut() {
            cache.mark_properties_current().await;
        }
    }

    fn poll_events(&mut self) -> Vec<VaultEvent> {
        let mut queue = self.events.lock().unwrap();
        queue.drain(..).collect()
    }

    async fn begin_edit(&mut self, path: &Path, buffer: String) {
        self.conflicts.lock().unwrap().set_buffer(path, buffer);
        if let Some(note) = self.index.lock().await.get_mut(path) {
            note.has_unsaved_edits = true;
        }
    }

    async fn discard_edit(&mut self, path: &Path) {
        self.conflicts.lock().unwrap().clear(path);
        if let Some(note) = self.index.lock().await.get_mut(path) {
            note.has_unsaved_edits = false;
        }
    }

    fn has_unsaved_edits(&self, path: &Path) -> bool {
        self.conflicts.lock().unwrap().has_unsaved_edits(path)
    }

    fn pending_conflict(&self, path: &Path) -> Option<EditConflict> {
        self.conflicts.lock().unwrap().conflict(path).cloned()
    }

    fn unresolved_edit_paths(&self) -> Vec<PathBuf> {
        self.conflicts.lock().unwrap().unresolved_paths()
    }

    async fn conflict_diff(&self, path: &Path) -> Vec<DiffLine> {
        let Some(local) = self
            .conflicts
            .lock()
            .unwrap()
            .buffer(path)
            .map(str::to_string)
        else {
            return Vec::new();
        };
        // Внешняя сторона читается с диска, а не берётся из `disk_snapshot`:
        // снимок хранит хэш, а не содержимое (research.md §5). Файла может уже
        // не быть (FR-025) — тогда внешняя сторона пуста, и весь буфер
        // показывается как локальные строки.
        let external = self.read(path).await.unwrap_or_default();
        line_diff(&local, &external)
    }

    async fn resolve_conflict_keep_local(&mut self, path: &Path) -> Result<(), ResolveError> {
        let buffer = self
            .conflicts
            .lock()
            .unwrap()
            .buffer(path)
            .map(str::to_string);
        let Some(buffer) = buffer else {
            return Err(ResolveError::NoPendingConflict);
        };
        // `expected = None` намеренно: пользователь уже увидел внешнюю версию
        // и явно выбрал перезаписать её (US4 AS2) — повторная проверка снимка
        // отклонила бы ровно то действие, ради которого диалог и показан.
        self.write(path, &buffer, None).await?;
        self.conflicts
            .lock()
            .unwrap()
            .resolve(path, ConflictStatus::ResolvedKeepLocal);
        Ok(())
    }

    async fn resolve_conflict_accept_external(
        &mut self,
        path: &Path,
    ) -> Result<Option<String>, ResolveError> {
        {
            let registry = self.conflicts.lock().unwrap();
            if !registry.has_unsaved_edits(path)
                && registry.conflict(path).is_none()
                && !registry.was_deleted_with_edits(path)
            {
                return Err(ResolveError::NoPendingConflict);
            }
        }
        // FR-025: отсутствие внешней версии — не ошибка чтения, а сам смысл
        // ответа: «принять внешнюю» для удалённой заметки означает принять
        // удаление, поэтому `ReadError` схлопывается в `None`.
        let external = self.read(path).await.ok();
        // Отбрасывание буфера происходит здесь и только здесь — то есть после
        // явного действия пользователя (FR-023).
        self.conflicts
            .lock()
            .unwrap()
            .resolve(path, ConflictStatus::ResolvedAcceptExternal);
        if let Some(note) = self.index.lock().await.get_mut(path) {
            note.has_unsaved_edits = false;
        }
        Ok(external)
    }
}

/// Аналог `SystemTime::now()`, вынесен отдельно ради единообразия с web-стороной,
/// где `SystemTime::now()` в некоторых WASM-рантаймах недоступен напрямую.
pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
