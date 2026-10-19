pub mod backend;
pub mod cache;
pub mod conflict;
pub mod events;
pub mod index;
pub mod relocate;
pub mod sections;
pub mod service_dir;
pub mod vault_settings;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;
#[cfg(target_arch = "wasm32")]
pub mod web;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Расширение текстовой заметки.
pub const NOTE_EXTENSION: &str = "typ";
/// Расширение доски для рисования (фича 002, research.md §2).
pub const BOARD_EXTENSION: &str = "excalidraw";
/// Суффикс папки со снимками вставленных досок по **прежнему** правилу
/// (фича 002, research.md §4): `<имя заметки>.assets` рядом с заметкой.
///
/// Фича 006 перестала создавать такие папки, но распознавать их обязана
/// всегда: у пользователя они уже есть, и ни одно значение политики
/// расположения не отключает эту ветку (006 FR-009, FR-010).
pub const SNAPSHOT_FOLDER_SUFFIX: &str = ".assets";

/// Имя общей папки ассетов по правилу фичи 006: `<корень>/assets/<заметка>/`.
///
/// Корень выбирает пользователь (`boards::placement`), а раскладка внутри него
/// одна на все три варианта — различается только корень (006 FR-004).
pub const ASSETS_FOLDER_NAME: &str = "assets";

/// Чем является файл vault (фича 002, data-model.md «FileKind»).
///
/// Один классификатор на оба backend. До появления досок признак «это заметка»
/// жил в двух местах и по-разному — `is_note(&Path)` в `native.rs` и
/// `is_note(&str)` в `web.rs`. Пока правило было одно, расхождение не
/// проявлялось; с тремя правилами два независимых места разошлись бы
/// гарантированно, а принцип II требует одинакового поведения на таргетах
/// (research.md §3 фичи 002).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum FileKind {
    /// Текстовая заметка Typst.
    Note,
    /// Доска для рисования.
    Board,
    /// Снимок доски, принадлежащий вставке в заметке.
    Snapshot,
    /// Служебное состояние приложения: всё внутри `.typsidian/` (фича 006,
    /// FR-020).
    ///
    /// Отдельный вид, а не `Other`: `Other` означает «файл пользователя,
    /// который приложение не трогает», а здесь наоборот — файл приложения.
    /// Различие нужно `write_snapshot`, отказывающему писать не по пути
    /// снимка, и любой будущей проверке «не считать это заметкой».
    Service,
    /// Всё остальное: приложение такие файлы не трогает.
    Other,
}

impl FileKind {
    /// Показывается ли файл в дереве. Заметки и доски — да (FR-004 фичи 002),
    /// снимки, служебные и посторонние файлы — нет (FR-021 фичи 002,
    /// FR-022 фичи 006).
    pub fn is_listed(self) -> bool {
        matches!(self, FileKind::Note | FileKind::Board)
    }
}

/// Классифицирует файл **по пути**, не читая содержимого.
///
/// Чтение ради классификации сломало бы бюджет обхода SC-002: на пределе в
/// 10 000 заметок это 10 000 лишних открытий файла.
///
/// Снимок опознаётся по расположению, а не по расширению: снимком считается
/// любой файл внутри папки `*.assets`. Обратное правило — «SVG значит снимок» —
/// скрыло бы из дерева картинки, которые пользователь положил в vault сам.
///
/// Если заметка-владелец папки `*.assets` удалена, её файлы остаются
/// `Snapshot`: в дереве они скрыты в обоих прочтениях, и различать эти случаи
/// незачем.
///
/// # Два правила снимка (фича 006, research.md §5)
///
/// ```text
/// Service  ⟸ путь начинается с «.typsidian/»
/// Snapshot ⟸ внутри папки «*.assets»                                (прежнее)
///          ∨ внутри «…/assets/<любая>/» И имя вида «board-<hex>.svg» (новое)
/// ```
///
/// Второе условие проверяет **имя файла**, а не только папку, и это
/// существенно. Прежнее правило защищал суффикс `.assets`, которого человек
/// своей папке не даст; у папки с именем `assets` такой защиты нет, и правило
/// «всё внутри — снимок» спрятало бы картинки, которые пользователь положил
/// туда сам.
///
/// Проверка `Service` идёт **первой**: файл `.typsidian/assets/…`, если он
/// когда-нибудь появится, остаётся служебным, а не становится снимком.
pub fn classify(path: &Path) -> FileKind {
    if service_dir::is_inside_service_dir(path) {
        return FileKind::Service;
    }
    // Конфликтная копия Syncthing — не заметка, а спор о заметке (фича 011,
    // FR-024): в дерево, поиск и граф она не попадает ни на одном таргете,
    // потому что правило здесь одно для обоих обходов и для вотчера.
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(crate::sync::conflict_copy::is_conflict_copy)
    {
        return FileKind::Other;
    }
    if in_legacy_snapshot_folder(path) || is_snapshot_in_assets_folder(path) {
        return FileKind::Snapshot;
    }

    match path.extension().and_then(|extension| extension.to_str()) {
        Some(NOTE_EXTENSION) => FileKind::Note,
        Some(BOARD_EXTENSION) => FileKind::Board,
        _ => FileKind::Other,
    }
}

/// Расширения файлов шрифтов, которые сборка берёт из vault (фича 005, FR-027).
///
/// Ровно то, что умеет разобрать `typst::text::Font`: TrueType/OpenType и их
/// коллекции. WOFF сюда не входит — Typst его не читает, и найденный, но
/// неразбираемый файл был бы обещанием, которое сборка не выполнит.
pub const FONT_EXTENSIONS: &[&str] = &["ttf", "otf", "ttc", "otc"];

/// Файл шрифта пользователя в vault (FR-027).
///
/// Служебное место сюда не попадает: шрифт внутри `.typsidian/` — это файл
/// приложения, а не пользователя. Регистр расширения не важен — `Times.TTF`
/// такой же шрифт, как `times.ttf`.
pub fn is_font_file(path: &Path) -> bool {
    classify(path) == FileKind::Other
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                FONT_EXTENSIONS
                    .iter()
                    .any(|known| extension.eq_ignore_ascii_case(known))
            })
}

/// Расширения изображений, которые вставка вложений (фича 016) сопоставляет и
/// предпросмотр читает без предела размера заметки. Вид входящего файла
/// определяется по содержимому (`attachments::kind::sniff`); расширение здесь —
/// только отбор кандидатов на диске.
pub const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg"];

/// Изображение в vault — для сопоставления вложений по содержимому (фича 016,
/// research.md §7). Только метаданные: байты читаются, лишь когда размер
/// совпал.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ImageFile {
    /// Путь относительно корня vault.
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    pub path: PathBuf,
    pub size: u64,
}

/// Файл изображения в vault (фича 016): по расширению без учёта регистра, вне
/// служебного места. Снимки досок сюда входят — это тоже картинки vault.
pub fn is_image_file(path: &Path) -> bool {
    classify(path) != FileKind::Service
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                IMAGE_EXTENSIONS
                    .iter()
                    .any(|known| extension.eq_ignore_ascii_case(known))
            })
}

/// Шрифт, найденный в vault, с дешёвым отпечатком `{size, modified}`.
///
/// Отпечаток — не хэш содержимого, и это намеренно: шрифты весят мегабайты, а
/// список спрашивается на каждую сборку. Отпечаток говорит только «файл мог
/// измениться — перечитай», и для этого размера и времени изменения хватает.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFile {
    /// Путь относительно корня vault.
    pub path: PathBuf,
    pub size: u64,
    /// Время изменения в миллисекундах; `0`, если платформа его не дала.
    pub modified_ms: i64,
}

/// Прежнее правило: любой файл внутри папки `*.assets` (фича 002).
fn in_legacy_snapshot_folder(path: &Path) -> bool {
    path.parent().is_some_and(|parent| {
        parent.components().any(|component| {
            component
                .as_os_str()
                .to_str()
                .is_some_and(|name| name.ends_with(SNAPSHOT_FOLDER_SUFFIX))
        })
    })
}

/// Новое правило: `<корень>/assets/<заметка>/board-<hex>.svg`.
///
/// Родитель файла обязан быть **прямым** ребёнком папки `assets`: глубже
/// приложение снимков не кладёт, а считать снимком что попало из чужого дерева
/// нельзя.
fn is_snapshot_in_assets_folder(path: &Path) -> bool {
    let is_snapshot_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(crate::boards::snapshot::is_snapshot_file_name);
    if !is_snapshot_name {
        return false;
    }
    path.parent()
        .and_then(|note_folder| note_folder.parent())
        .and_then(|assets_folder| assets_folder.file_name())
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == ASSETS_FOLDER_NAME)
}

/// Почему папка внутри vault недоступна.
///
/// Отдельные варианты, а не текст: причина показывается пользователю и от неё
/// зависит, что делать — «прав нет» чинит он сам, «по пути лежит файл» чинится
/// переименованием, `Io` не чинится вовсе.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum FolderProblem {
    /// Права на уровне ОС запрещают операцию.
    AccessDenied,
    /// По этому пути лежит не папка.
    NotADirectory,
    /// Vault целиком открыт без права записи (FR-036) — папка тут ни при чём.
    ///
    /// Backend этот вариант **не** возвращает: он знает только про сам путь.
    /// Его выставляет тот, кто складывает состояние папки с состоянием vault —
    /// сегодня это [`crate::templates::library::folder_state`].
    VaultReadOnly,
    /// Отказ ввода-вывода.
    Io,
}

/// Состояние папки внутри vault: существует ли, читается ли, пишется ли.
///
/// Отдельный вопрос к backend, а не вывод из [`VaultBackend::list`]: обход не
/// различает «папки нет» и «папка есть, но не читается» — в обоих случаях он
/// просто ничего оттуда не приносит. Схлопывание этих двух случаев превратило
/// бы штатное состояние нового vault (папки заготовок ещё нет — 003 FR-009) в
/// неотличимую от него поломку.
///
/// Порядок вариантов — лестница: `Absent` → `Unreadable` → `NotWritable` →
/// `Present`. Каждый следующий предполагает выполненными условия предыдущих.
///
/// [`VaultBackend::list`]: backend::VaultBackend::list
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "state")]
pub enum FolderAccess {
    /// Папка есть, читается и принимает запись.
    Present,
    /// Папки по этому пути нет. **Не ошибка**: её ещё не создали.
    Absent,
    /// Папка есть, но прочитать её нельзя.
    Unreadable { reason: FolderProblem },
    /// Папка читается, но записи не принимает.
    NotWritable { reason: FolderProblem },
}

/// data-model.md «Vault». Переходы: ReadWrite -> ReadOnly (права на запись
/// отозваны, чтение работает, FR-036), ReadWrite|ReadOnly -> Unavailable (папка/
/// диск/разрешение пропали, FR-020), Unavailable -> ReadWrite|ReadOnly только
/// после явного переоткрытия (FR-018).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "camelCase")]
pub enum AccessState {
    ReadWrite,
    ReadOnly,
    Unavailable,
}

/// Жёсткий верхний предел размера vault (FR-038, SC-002): бюджеты «дерево
/// пригодно для навигации ≤3 с» и «полный обход ≤10 с» измерены и
/// гарантируются до 10 000 заметок на эталонном оборудовании (SSD,
/// современный потребительский класс) — см. `benches/vault_scale.rs`, где эти
/// пороги проверяются как условие прохождения.
///
/// Vault, содержащий **больше** этого числа заметок, не открывается вовсе:
/// обход прерывается, как только счётчик найденных заметок превысил предел,
/// частичное дерево не показывается как рабочий vault, а вызывающая сторона
/// получает [`backend::OpenError::TooManyNotes`] и обязана назвать причину
/// пользователю. Приложение при этом остаётся работоспособным — это отказ
/// открыть конкретную папку, а не сбой.
///
/// Режима «best effort выше предела» не существует (кларификация 2026-08-09,
/// FR-038): гарантировать за пределом нечего, а показать наполовину обойдённое
/// дерево как рабочее — значит соврать пользователю о том, что он видит.
pub const SUPPORTED_NOTE_COUNT: usize = 10_000;

/// Предел размера **отдельной** заметки (spec Assumptions, Edge Cases):
/// заявленное поведение гарантируется до 10 МБ включительно.
///
/// Порог существует потому, что и обнаружение внешних изменений, и разрешение
/// конфликтов читают заметку целиком в память (`FileStamp::from_content`,
/// `conflict::diff`) — потокового пути чтения в ядре нет. 10 МБ с запасом
/// перекрывает любой реалистичный Typst-документ, поэтому вместо усложнения
/// чтения вводится честный отказ.
///
/// Заметка выше порога **остаётся видимой в дереве** — она существует, и
/// прятать её значило бы соврать о содержимом папки. Отклоняется только
/// открытие: [`backend::ReadError::TooLarge`] с названной причиной.
pub const NOTE_SIZE_LIMIT_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct VaultInfo {
    pub root_path: PathBuf,
    pub access_state: AccessState,
    /// Число заметок в индексе. Всегда `<= SUPPORTED_NOTE_COUNT`: vault
    /// большего размера не открывается (FR-038), поэтому это справочная
    /// величина, а не признак деградированного режима — режима, о котором UI
    /// должен был бы предупреждать, больше не существует.
    pub note_count: usize,
}

// Модуль `last_vault` упразднён фичей 004 (research.md §10).
//
// Он хранил «последний открытый vault» (FR-007) в собственном файле
// `~/.config/typsidian/last_vault.txt` и был объявлен только под
// `cfg(not(target_arch = "wasm32"))` — то есть уже нарушал принцип II, не имея
// веб-реализации. Его роль целиком поглощена списком известных vault в
// настройках: `Settings::known_vaults` + `Settings::current_vault`.
//
// Держать оба означало бы два расходящихся источника истины об одном и том же
// факте. Содержимое прежнего файла импортируется один раз при первом запуске
// после обновления — см. `settings::native::NativeStore::open_in_with_legacy`,
// — после чего файл удаляется.

/// Файл папки-источника импорта (фича 021).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    /// Путь относительно корня, с разделителями платформы.
    pub path: PathBuf,
    pub size: u64,
}

/// Все файлы папки, которая vault не является, и число служебных записей,
/// которые обход пропустил (фича 021, research.md §9).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceListing {
    /// В порядке пути.
    pub files: Vec<SourceFile>,
    /// Скрытые записи (`.obsidian`, `.trash`, `.git`, `.DS_Store`): внутрь
    /// не заходим и не считаем.
    pub excluded: u32,
}

#[cfg(test)]
mod image_file_tests {
    use super::*;

    #[test]
    fn image_extensions_are_recognised_regardless_of_case() {
        for path in ["a.png", "b/c.JPG", "d.jpeg", "e.Gif", "f.webp", "g.SVG"] {
            assert!(is_image_file(Path::new(path)), "{path}");
        }
    }

    #[test]
    fn other_files_and_the_service_folder_are_not_images() {
        for path in ["a.typ", "b.pdf", "c.heic", "noext", ".typsidian/x.png"] {
            assert!(!is_image_file(Path::new(path)), "{path}");
        }
    }

    #[test]
    fn board_snapshots_are_images_too() {
        assert!(is_image_file(Path::new("Доска.assets/снимок.svg")));
    }
}
