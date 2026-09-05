pub mod backend;
pub mod cache;
pub mod conflict;
pub mod events;
pub mod index;

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
/// Суффикс папки со снимками вставленных досок (фича 002, research.md §4).
pub const SNAPSHOT_FOLDER_SUFFIX: &str = ".assets";

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
    /// Всё остальное: приложение такие файлы не трогает.
    Other,
}

impl FileKind {
    /// Показывается ли файл в дереве. Заметки и доски — да (FR-004 фичи 002),
    /// снимки и посторонние файлы — нет (FR-021 фичи 002).
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
pub fn classify(path: &Path) -> FileKind {
    let inside_snapshot_folder = path.parent().is_some_and(|parent| {
        parent.components().any(|component| {
            component
                .as_os_str()
                .to_str()
                .is_some_and(|name| name.ends_with(SNAPSHOT_FOLDER_SUFFIX))
        })
    });
    if inside_snapshot_folder {
        return FileKind::Snapshot;
    }

    match path.extension().and_then(|extension| extension.to_str()) {
        Some(NOTE_EXTENSION) => FileKind::Note,
        Some(BOARD_EXTENSION) => FileKind::Board,
        _ => FileKind::Other,
    }
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
