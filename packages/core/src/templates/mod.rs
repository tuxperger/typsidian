//! Шаблоны и сниппеты (фича 003).
//!
//! Публичный API фичи со стороны ядра — то, что
//! `contracts/template-library-trait.md` описывает как трейт
//! `TemplateLibrary`. Здесь это **свободные функции над
//! [`VaultBackend`]**, а не трейт, и вот почему: все операции библиотеки
//! читают или пишут vault, то есть асинхронны, а контракт писался в синхронных
//! сигнатурах, будто состав библиотеки лежит готовым в памяти. Он не лежит —
//! `VaultBackend::list()` асинхронен на обоих таргетах. Форма, выбранная здесь,
//! совпадает с той, которой уже пользуются доски (`crate::boards`) и поиск
//! (`crate::search`): одна реализация, обобщённая по бэкенду, а платформенные
//! различия остаются за трейтом фичи 001. Имена методов, их семантика и все
//! правила контракта сохранены дословно.
//!
//! # Границы
//!
//! Здесь: состав библиотеки, состояние папки, состояние превью, извлечение
//! фрагмента исходного текста, валидация имени и коллизия, применение
//! содержимого к тексту. Не здесь: галерея, виртуализация, момент построения
//! превью, фокус и курсор после вставки — это UI (research.md §9).
//!
//! Граница проведена по одному вопросу: проверяется ли это тестом ядра без
//! браузера.
//!
//! # Отдельного типа «сниппет» не существует
//!
//! Библиотека одна, деления на виды нет, способ применения не зависит от
//! содержимого (FR-018, FR-019, кларификация 2026-08-08). Любая заготовка
//! применяется двумя способами: развернуть в новую заметку или вставить в
//! текущую позицию курсора.

pub mod events;
pub mod library;
pub mod naming;
pub mod preview;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::stamp::FileStamp;
use crate::vault::backend::{ReadError, VaultBackend, WriteError};
use crate::vault::index::Note;

pub use events::{library_event, TemplateEvent};
pub use library::{
    folder_state, group_of, is_inside_templates_folder, library, templates_folder,
    TemplateFolderState, TemplateLibraryView, TEMPLATES_FOLDER,
};
pub use naming::{check_name, template_file_name, template_path, NameCheck, NameProblem};
pub use preview::{
    excerpt, ExcerptOnlySource, ExcerptReason, PreviewAvailability, PreviewCache, PreviewSource,
    PreviewState, PreviewUnavailable, EXCERPT_CHARS, EXCERPT_LINES,
};

/// Заготовка — обычная заметка, лежащая в папке шаблонов
/// (data-model.md «Template»).
///
/// Содержимого не хранит: оно читается по требованию — при построении превью,
/// при создании заметки и при вставке. Держать 200 файлов в памяти ради
/// галереи, показывающей десяток карточек, незачем (research.md §4, §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Template {
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    pub path: PathBuf,
    /// Имя файла — то, что видно в карточке и по чему идёт поиск (FR-015).
    pub name: String,
    /// Вложенная папка внутри `templates/`, если есть, — группировка галереи.
    pub group: Option<String>,
    pub stamp: FileStamp,
    /// `BLAKE3` содержимого — ключ кэша превью (research.md §4).
    ///
    /// Берётся из отпечатка файла, который индекс уже посчитал: второй проход
    /// по содержимому ради того же числа был бы лишним чтением на каждую
    /// карточку.
    ///
    /// `Option` — потому что файл может оказаться нечитаемым: тогда хэша нет, а
    /// карточка помечается недоступной (FR-017). Сегодня оба бэкенда без
    /// отпечатка в индекс файл не кладут, поэтому здесь всегда `Some`; вариант
    /// существует, чтобы бэкенд, который научится перечислять то, что не может
    /// прочитать, не получил взамен **неверный** хэш.
    #[cfg_attr(target_arch = "wasm32", tsify(type = "number[] | undefined"))]
    pub content_hash: Option<[u8; 32]>,
}

impl Template {
    /// Запись библиотеки из записи дерева.
    ///
    /// `None` — заметка лежит вне папки заготовок. Проверка здесь, а не у
    /// вызывающего, ровно затем, чтобы `Template` нельзя было собрать из
    /// произвольной заметки vault.
    pub fn from_note(note: &Note) -> Option<Self> {
        if !library::is_inside_templates_folder(&note.path) {
            return None;
        }
        Some(Self {
            group: library::group_of(&note.path),
            path: note.path.clone(),
            name: note.name.clone(),
            content_hash: Some(note.stamp.content_hash),
            stamp: note.stamp.clone(),
        })
    }
}

/// Точка вставки в открытой заметке (data-model.md «InsertionPoint»).
///
/// **Единственное место фичи, зависящее от режима редактора**, и потому
/// единственное, что упирается в гейт принципа V (research.md §5). Для
/// `PlainInput` смещения берутся из редактора в браузере; что они означают,
/// когда буфер принадлежит Neovim, — вопрос будущей сессии `/speckit-clarify`
/// по мосту (contracts/insertion-target.md).
///
/// # Единица смещения — кодовая единица UTF-16
///
/// Не байт и не скалярное значение Unicode. Смещение в строке — величина,
/// зависящая от кодировки (та же оговорка, что в `crate::search`): Rust считает
/// байты или `char`, а JS режет строки по UTF-16, и позиции CodeMirror — это
/// именно кодовые единицы UTF-16.
///
/// Здесь, в отличие от поиска, обойтись без смещений нельзя: правку в редактор
/// иначе не передать. Поэтому выбрана **единица второй стороны**: ядро считает
/// так же, как считает редактор, и переводить на границе ничего не нужно.
/// Считай ядро в `char`, курсор уезжал бы на каждом символе вне BMP —
/// математическом шрифте Typst (`𝔸`), эмодзи в заметке.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct InsertionPoint {
    /// Позиция курсора, в кодовых единицах UTF-16.
    pub offset: usize,
    /// Выделенный фрагмент, если он есть: вставка его заменяет (FR-035).
    pub selection: Option<TextSpan>,
}

/// Выделенный диапазон текста, в кодовых единицах UTF-16.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TextSpan {
    pub start: usize,
    pub end: usize,
}

/// Почему заготовка не прочитана (contracts/template-library-trait.md).
///
/// Из состояния `Unreadable` **нет пути к записи** (FR-017): нечитаемый файл не
/// перезаписывается — тот же принцип, что у повреждённого `settings.json` в 004
/// и повреждённой доски в 002.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum ReadTemplateError {
    #[error("заготовка не найдена")]
    NotFound,
    #[error("файл заготовки не читается как текст")]
    Unreadable,
    #[error("не удалось прочитать файл с диска")]
    Io,
}

impl ReadTemplateError {
    /// Причина недоступности превью — та же, что причина отказа чтения.
    ///
    /// Отображение существует, чтобы карточка называла ровно то, что произошло,
    /// а не общее «превью недоступно».
    pub fn as_preview_reason(self) -> PreviewUnavailable {
        match self {
            Self::NotFound => PreviewUnavailable::NotFound,
            Self::Unreadable => PreviewUnavailable::Unreadable,
            Self::Io => PreviewUnavailable::Io,
        }
    }
}

/// Почему сохранение как заготовки отклонено (FR-028, FR-029, FR-030).
///
/// Каждый вариант — названная причина, а не общий отказ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum SaveTemplateError {
    /// Заготовка с таким именем уже есть, а подтверждения перезаписи не было.
    /// Существующий файл при этом **не тронут**.
    #[error("заготовка с таким именем уже есть")]
    NameTaken,
    #[error("имя недопустимо для файловой системы")]
    InvalidName { reason: NameProblem },
    #[error("папку заготовок нельзя изменить")]
    FolderNotWritable { reason: crate::vault::FolderProblem },
    #[error("не удалось записать файл")]
    Io,
}

/// Читает содержимое заготовки для применения — создания заметки или вставки.
///
/// Читает **байтами**, а не текстом, и сам решает про кодировку. Разница видна
/// пользователю: файл, который не является UTF-8, — это `Unreadable`
/// («заготовка есть, но применить её нельзя»), а не `Io` («диск не ответил»),
/// и чинятся эти два случая по-разному.
///
/// Файл заготовки при этом не изменяется ни при каком исходе (инвариант 1).
pub async fn read_template<B: VaultBackend>(
    backend: &B,
    path: &Path,
) -> Result<String, ReadTemplateError> {
    let bytes = backend.read_bytes(path).await.map_err(|error| match error {
        ReadError::NotFound => ReadTemplateError::NotFound,
        // Файл существует, но применить его нельзя — то же следствие, что у
        // повреждённой кодировки.
        ReadError::TooLarge => ReadTemplateError::Unreadable,
        ReadError::Io => ReadTemplateError::Io,
    })?;
    String::from_utf8(bytes).map_err(|_| ReadTemplateError::Unreadable)
}

/// Превью одной заготовки: чтение, кэш и порт вместе (FR-011, FR-017).
///
/// Порядок существенный. Файл читается **до** обращения к кэшу и к порту,
/// потому что именно чтение отличает `Unavailable` от `SourceExcerpt`: заготовка
/// без хэша или без содержимого — недоступна, и подставлять ей пустой фрагмент
/// нельзя (запрет 2 из data-model.md).
pub async fn preview_of<B: VaultBackend, S: PreviewSource>(
    backend: &B,
    source: &S,
    cache: &mut PreviewCache,
    template: &Template,
) -> PreviewState {
    let Some(content_hash) = template.content_hash else {
        return PreviewState::Unavailable {
            reason: PreviewUnavailable::Unreadable,
        };
    };
    // Кэш спрашивается до чтения: попадание избавляет от чтения файла целиком,
    // а хэш содержимого уже известен из индекса (research.md §4).
    if let Some(known) = cache.get(&content_hash) {
        return known.clone();
    }

    match read_template(backend, &template.path).await {
        Ok(content) => cache.preview_with(source, content_hash, &content),
        Err(error) => PreviewState::Unavailable {
            reason: error.as_preview_reason(),
        },
    }
}

/// Сохраняет содержимое как заготовку (FR-002, FR-027, FR-028, FR-030).
///
/// Правило коллизии живёт **здесь**, а не в диалоге: без `confirm_overwrite`
/// занятое имя даёт [`SaveTemplateError::NameTaken`], даже если интерфейс
/// проверку пропустил. Это же снимает гонку «файл появился между проверкой и
/// записью» (research.md §8).
///
/// Папка заготовок создаётся, если её нет, — единственная новая операция фичи
/// над файловой системой, и отдельного действия от пользователя она не требует.
///
/// Содержимое фиксируется на момент вызова: заготовка получает **копию**, и
/// последующая правка исходной заметки её не меняет (инвариант 3). Отдельного
/// кода для этого нет и не нужно — связи между файлами не заводится вовсе.
pub async fn save_as_template<B: VaultBackend>(
    backend: &mut B,
    name: &str,
    content: &str,
    confirm_overwrite: bool,
) -> Result<Template, SaveTemplateError> {
    let path = template_path(name).map_err(|reason| SaveTemplateError::InvalidName { reason })?;

    // Порядок проверок: сначала то, что не трогает диск. Отказ по имени обязан
    // происходить до любых изменений (FR-029, инвариант 6).
    let state = folder_state(backend).await;
    match state {
        TemplateFolderState::Unreadable { reason } | TemplateFolderState::NotWritable { reason } => {
            return Err(SaveTemplateError::FolderNotWritable { reason })
        }
        TemplateFolderState::Present | TemplateFolderState::Absent => {}
    }

    if naming::exists(backend, &path).await && !confirm_overwrite {
        return Err(SaveTemplateError::NameTaken);
    }

    backend
        .ensure_folder(&templates_folder())
        .await
        .map_err(as_save_failure)?;
    // `expected: None` — перезапись по явному решению пользователя: снимок
    // сравнивать не с чем, заготовку он в редакторе не держал.
    backend
        .write(&path, content, None)
        .await
        .map_err(as_save_failure)?;

    // Отпечаток берётся из индекса — того же, из которого строится библиотека:
    // считать свой значило бы завести второй ответ на вопрос «какая версия
    // файла сейчас на диске». Индекс мог ещё не заметить запись — тогда
    // отпечаток считается по только что записанному содержимому, и это ровно
    // то, что на диске.
    let written = backend
        .list()
        .await
        .iter()
        .find(|note| note.path == path)
        .and_then(Template::from_note);

    Ok(written.unwrap_or_else(|| Template {
        group: group_of(&path),
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        content_hash: Some(*blake3::hash(content.as_bytes()).as_bytes()),
        stamp: FileStamp::from_content(0, content.as_bytes()),
        path,
    }))
}

/// Отказ записи как отказ сохранения заготовки.
fn as_save_failure(error: WriteError) -> SaveTemplateError {
    match error {
        WriteError::AccessDenied => SaveTemplateError::FolderNotWritable {
            reason: crate::vault::FolderProblem::AccessDenied,
        },
        // `Conflict` здесь недостижим: запись идёт без `expected`-снимка.
        WriteError::Conflict | WriteError::Io => SaveTemplateError::Io,
    }
}

/// Правка, которую нужно совершить над текстом заметки (FR-031…FR-035).
///
/// Именно **правка**, а не готовый новый текст, и это существенно. FR-033
/// требует, чтобы одна отмена возвращала текст целиком, а история предыдущих
/// правок при этом уцелела; реализация «прочитать значение, склеить строку,
/// записать обратно» это требование нарушает (research.md §6). Отдав диапазон и
/// текст, ядро оставляет редактору возможность выполнить ровно одну
/// транзакцию — то есть ровно одну единицу истории отмены.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct InsertionEdit {
    /// Диапазон, который заменяется. Пустой (`start == end`) — вставка без
    /// замены.
    pub replace: TextSpan,
    /// Что вписать. Содержимое заготовки **дословно**: подстановок нет, потому
    /// что параметризация вне скоупа (инвариант 4).
    pub text: String,
    /// Где окажется курсор после правки (FR-034) — сразу за вставленным, то же
    /// место, что после обычной вставки из буфера обмена. Одинаково от вставки
    /// к вставке.
    pub caret: usize,
}

/// Считает правку по точке вставки и содержимому заготовки.
///
/// Правило вставки живёт здесь, а не в реализациях порта `InsertionTarget`,
/// затем, чтобы три режима редактора (`PlainInput`, `SimplifiedEmulation`,
/// `FullBackend`) вставляли **одно и то же**, а не каждый по-своему.
///
/// `text_length` и смещения — в кодовых единицах UTF-16, см. [`InsertionPoint`].
/// Смещение за концом текста прижимается к концу, а не отбрасывает вставку:
/// курсор мог указывать в текст, который успел укоротиться.
pub fn plan_insertion(text_length: usize, point: &InsertionPoint, content: &str) -> InsertionEdit {
    let (start, end) = match point.selection {
        Some(span) => (
            span.start.min(span.end).min(text_length),
            span.start.max(span.end).min(text_length),
        ),
        None => {
            let at = point.offset.min(text_length);
            (at, at)
        }
    };

    InsertionEdit {
        replace: TextSpan { start, end },
        text: content.to_owned(),
        caret: start + utf16_len(content),
    }
}

/// Применяет содержимое заготовки к тексту заметки (FR-031, FR-032, FR-035).
///
/// Эталонная реализация того же правила, выраженная через [`plan_insertion`],
/// чтобы разойтись они не могли. Ею проверяется правило в тестах и пользуется
/// всякий, у кого текст есть целиком; редактор ею **не** пользуется — ему нужна
/// правка, а не новый текст (см. [`InsertionEdit`]).
///
/// Режет по тем же кодовым единицам UTF-16, в которых заданы смещения. Отсюда
/// `from_utf16_lossy`: разрезать суррогатную пару пополам могло бы только
/// смещение, которого редактор не выдаёт, и превращать этот невозможный случай
/// в панику незачем.
pub fn apply_insertion(text: &str, point: &InsertionPoint, content: &str) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let edit = plan_insertion(units.len(), point, content);
    let head = String::from_utf16_lossy(&units[..edit.replace.start]);
    let tail = String::from_utf16_lossy(&units[edit.replace.end..]);
    format!("{head}{}{tail}", edit.text)
}

/// Куда встаёт курсор после вставки (FR-034).
pub fn caret_after_insertion(point: &InsertionPoint, content: &str) -> usize {
    // `usize::MAX` как длина текста: здесь спрашивают про курсор, а не про
    // границы, и прижимать смещение не к чему.
    plan_insertion(usize::MAX, point, content).caret
}

/// Длина строки в кодовых единицах UTF-16 — той же мере, в которой считает
/// позиции редактор.
fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insertion_without_a_selection_keeps_the_rest_of_the_text() {
        let point = InsertionPoint {
            offset: 5,
            selection: None,
        };
        assert_eq!(apply_insertion("абвгдежз", &point, "—"), "абвгд—ежз");
        assert_eq!(caret_after_insertion(&point, "—"), 6);
    }

    #[test]
    fn a_selection_is_replaced() {
        let point = InsertionPoint {
            offset: 2,
            selection: Some(TextSpan { start: 2, end: 5 }),
        };
        assert_eq!(apply_insertion("абвгдежз", &point, "XY"), "абXYежз");
        assert_eq!(caret_after_insertion(&point, "XY"), 4);
    }

    #[test]
    fn the_content_is_inserted_verbatim() {
        // Инвариант 4: подстановок нет. `{{заголовок}}` — обычный текст.
        let point = InsertionPoint {
            offset: 0,
            selection: None,
        };
        let content = "= {{заголовок}}\n#lorem(30)";
        assert_eq!(apply_insertion("", &point, content), content);
    }

    #[test]
    fn an_offset_past_the_end_lands_at_the_end() {
        let point = InsertionPoint {
            offset: 99,
            selection: None,
        };
        assert_eq!(apply_insertion("абв", &point, "!"), "абв!");
    }

    #[test]
    fn offsets_are_utf16_code_units_like_the_editors_own() {
        // `𝔸` — вне BMP: одна кодовая точка, ДВЕ единицы UTF-16. Считай ядро в
        // `char`, курсор после такой вставки уехал бы на единицу назад, а
        // смещение, пришедшее от CodeMirror, указало бы не туда.
        let content = "𝔸";
        let at_start = InsertionPoint {
            offset: 0,
            selection: None,
        };
        assert_eq!(caret_after_insertion(&at_start, content), 2);

        // Курсор после `𝔸` в тексте — это смещение 2, а не 1.
        let after_astral = InsertionPoint {
            offset: 2,
            selection: None,
        };
        assert_eq!(apply_insertion("𝔸б", &after_astral, "!"), "𝔸!б");
    }
}
