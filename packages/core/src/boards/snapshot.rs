//! Снимок доски: где лежит файл и что в нём записано (фича 002, T041,
//! research.md §4, §5).
//!
//! Снимок — единственный файл vault, который целиком порождён приложением, и
//! единственное место, куда приложение пишет свои метаданные: заметка обязана
//! остаться обычным Typst (FR-022), а файл доски — обычным Excalidraw
//! (research.md §2). Отсюда всё остальное: связь «снимок → доска» и хэш
//! состояния доски живут внутри SVG, в его `<desc>`.
//!
//! Ядро не рисует и не экспортирует: SVG приходит от движка уже готовым
//! (research.md §9). Здесь — путь, имя, метаданные и запись.

use super::digest::BoardDigest;
use super::embed;
use crate::vault::backend::{VaultBackend, WriteError};
use crate::vault::{classify, FileKind, SNAPSHOT_FOLDER_SUFFIX};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Расширение файла снимка (research.md §4: SVG, а не растр).
pub const SNAPSHOT_EXTENSION: &str = "svg";

/// Префикс имени файла снимка. Существует, чтобы файл был узнаваем человеком в
/// файловом менеджере: имя вида `board-1f4c9a2b.svg` читается как «снимок
/// доски», а не как случайный мусор.
const EMBED_ID_PREFIX: &str = "board-";

/// Сколько шестнадцатеричных знаков хэша попадает в идентификатор. Восемь — это
/// 4 млрд значений на одну заметку при том, что коллизия внутри заметки всё
/// равно разрешается перебором (см. [`plan_embed`]), а имя остаётся коротким:
/// оно попадает в текст заметки, который читает человек.
const EMBED_ID_HEX_LEN: usize = 8;

/// Атрибут с адресом доски-источника.
const BOARD_ATTRIBUTE: &str = "data-typsidian-board";
/// Атрибут с хэшем состояния доски, отражённого снимком.
const BOARD_HASH_ATTRIBUTE: &str = "data-typsidian-board-hash";

/// Идентификатор вставки (data-model.md «Embed»).
///
/// Закрепляется за вставкой в момент её создания и при обновлении не меняется:
/// обновление переписывает тот же файл, а не создаёт новый (FR-028, FR-029).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct EmbedId {
    value: String,
}

impl EmbedId {
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Имя файла снимка для этого идентификатора.
    pub fn file_name(&self) -> String {
        format!("{}.{SNAPSHOT_EXTENSION}", self.value)
    }

    /// Восстанавливает идентификатор из имени файла снимка. `None` — файл в
    /// папке снимков создан не приложением: такой файл вставкой не считается.
    pub fn from_file_name(file_name: &str) -> Option<Self> {
        let stem = file_name.strip_suffix(&format!(".{SNAPSHOT_EXTENSION}"))?;
        if !stem.starts_with(EMBED_ID_PREFIX) || stem.len() <= EMBED_ID_PREFIX.len() {
            return None;
        }
        Some(Self {
            value: stem.to_string(),
        })
    }
}

/// Метаданные снимка (data-model.md «Snapshot»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct SnapshotMetadata {
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    pub board_path: PathBuf,
    pub board_digest: BoardDigest,
}

/// Почему метаданные снимка не прочитаны.
///
/// `NoMetadata` — не ошибка вызова, а факт о файле: снимок, заменённый
/// сторонним инструментом, метаданных не несёт, и вставка по нему получает
/// `EmbedState::Undeterminable`, а не «актуально» (SC-008).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum SnapshotError {
    #[error("файла снимка нет")]
    NotFound,
    #[error("в снимке нет метаданных доски")]
    NoMetadata,
    #[error("снимок не читается")]
    Unreadable,
}

/// Почему снимок не записан.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum SnapshotWriteError {
    /// Путь не ведёт в папку снимков — писать туда приложение не станет:
    /// иначе оно положило бы свой файл с метаданными в чужое место vault.
    #[error("это не путь снимка")]
    NotASnapshotPath,
    /// Экспорт движка не похож на SVG: корневого элемента нет, вкладывать
    /// метаданные некуда.
    #[error("экспорт не похож на SVG")]
    NotAnSvg,
    #[error("vault недоступен на запись")]
    VaultNotWritable,
    #[error("не удалось записать снимок")]
    Io,
}

/// Почему вставка не спланирована.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum EmbedError {
    #[error("вставлять доску можно только в текстовую заметку")]
    NotANote,
    #[error("вставить можно только доску")]
    NotABoard,
}

/// Куда и под каким именем UI пишет снимок новой вставки.
///
/// Файл здесь не создаётся: его пишет UI после того, как движок вернул SVG
/// (contracts/board-store-trait.md).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct EmbedPlan {
    pub embed_id: EmbedId,
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    pub board_path: PathBuf,
    /// Путь снимка относительно корня vault — им пользуется запись файла.
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    pub snapshot_path: PathBuf,
    /// Путь снимка относительно папки заметки — он попадает в текст заметки.
    pub markup_path: String,
}

/// Имя папки снимков для заметки: `<имя заметки без расширения>.assets`.
///
/// `None` — путь не является путём заметки. Рядом с заметкой, а не в общей
/// папке: при переносе заметки снимки едут вместе с ней (research.md §4).
pub fn assets_folder_name(note_path: &Path) -> Option<String> {
    if classify(note_path) != FileKind::Note {
        return None;
    }
    let stem = note_path.file_stem()?.to_str()?;
    Some(format!("{stem}{SNAPSHOT_FOLDER_SUFFIX}"))
}

/// Путь снимка относительно корня vault.
pub fn snapshot_path(note_path: &Path, embed_id: &EmbedId) -> Option<PathBuf> {
    let folder = assets_folder_name(note_path)?;
    let parent = note_path.parent().unwrap_or(Path::new(""));
    Some(parent.join(folder).join(embed_id.file_name()))
}

/// Планирует вставку доски в заметку (FR-018, FR-020, FR-023).
///
/// Идентификатор выводится из пути заметки и пути доски, а не из случайного
/// источника: одинаковый вход даёт одинаковый результат на обоих таргетах, и
/// тест не зависит от генератора случайных чисел. Вторая вставка той же доски в
/// ту же заметку видит первую в тексте и берёт следующий идентификатор — так
/// FR-020 («отдельный файл на каждую вставку») выполняется без хранения
/// состояния между вызовами.
pub fn plan_embed(
    note_path: &Path,
    note_text: &str,
    board_path: &Path,
) -> Result<EmbedPlan, EmbedError> {
    if classify(note_path) != FileKind::Note {
        return Err(EmbedError::NotANote);
    }
    if classify(board_path) != FileKind::Board {
        return Err(EmbedError::NotABoard);
    }
    let folder = assets_folder_name(note_path).ok_or(EmbedError::NotANote)?;
    let taken: Vec<EmbedId> = embed::embeds_of(note_path, note_text)
        .into_iter()
        .map(|found| found.id)
        .collect();

    let mut attempt = 0u32;
    let embed_id = loop {
        let candidate = embed_id_for(note_path, board_path, attempt);
        if !taken.contains(&candidate) {
            break candidate;
        }
        attempt += 1;
    };

    let markup_path = format!("{folder}/{}", embed_id.file_name());
    let parent = note_path.parent().unwrap_or(Path::new(""));
    Ok(EmbedPlan {
        snapshot_path: parent.join(&folder).join(embed_id.file_name()),
        board_path: board_path.to_path_buf(),
        embed_id,
        markup_path,
    })
}

/// Текст, который UI вставляет в заметку (FR-022, research.md §11).
///
/// Обычный вызов `#image` без единого маркера приложения: любой маркер был бы
/// метаданными Typsidian в пользовательском файле и первым кандидатом на потерю
/// при правке заметки сторонним редактором.
pub fn embed_markup(plan: &EmbedPlan) -> String {
    format!("#image(\"{}\")", escape_typst_string(&plan.markup_path))
}

/// Метаданные, которые обязаны попасть в снимок (data-model.md «Snapshot»).
pub fn snapshot_metadata(board_path: &Path, digest: &BoardDigest) -> SnapshotMetadata {
    SnapshotMetadata {
        board_path: board_path.to_path_buf(),
        board_digest: digest.clone(),
    }
}

/// Вкладывает метаданные в SVG.
///
/// Ранее вложенные метаданные Typsidian заменяются, а не дублируются:
/// обновление вставки переписывает тот же файл (FR-028), и второй `<desc>` с
/// другим хэшем сделал бы состояние вставки неопределимым.
pub fn embed_metadata_in_svg(
    svg: &str,
    metadata: &SnapshotMetadata,
) -> Result<String, SnapshotWriteError> {
    let without_ours = remove_typsidian_desc(svg);
    let insert_at = root_tag_end(&without_ours).ok_or(SnapshotWriteError::NotAnSvg)?;
    let desc = format!(
        "<desc {BOARD_ATTRIBUTE}=\"{}\" {BOARD_HASH_ATTRIBUTE}=\"{}\"></desc>",
        escape_attribute(&metadata.board_path.to_string_lossy()),
        escape_attribute(metadata.board_digest.as_str()),
    );
    let mut result = String::with_capacity(without_ours.len() + desc.len());
    result.push_str(&without_ours[..insert_at]);
    result.push_str(&desc);
    result.push_str(&without_ours[insert_at..]);
    Ok(result)
}

/// Читает метаданные из содержимого снимка.
pub fn read_metadata_from_svg(svg: &str) -> Result<SnapshotMetadata, SnapshotError> {
    let board = attribute_value(svg, BOARD_ATTRIBUTE).ok_or(SnapshotError::NoMetadata)?;
    let hash = attribute_value(svg, BOARD_HASH_ATTRIBUTE).ok_or(SnapshotError::NoMetadata)?;
    // Испорченный хэш — это «сравнить не удалось», а не «расходится»:
    // сравнение с мусором дало бы `Stale` и предложило пользователю обновить
    // снимок, который, возможно, в порядке.
    let board_digest = BoardDigest::from_hex(&hash).ok_or(SnapshotError::NoMetadata)?;
    Ok(SnapshotMetadata {
        board_path: PathBuf::from(board),
        board_digest,
    })
}

/// Читает метаданные снимка из файла vault.
pub async fn read_snapshot_metadata<B: VaultBackend>(
    backend: &B,
    snapshot_path: &Path,
) -> Result<SnapshotMetadata, SnapshotError> {
    use crate::vault::backend::ReadError;
    let svg = backend
        .read(snapshot_path)
        .await
        .map_err(|error| match error {
            ReadError::NotFound => SnapshotError::NotFound,
            ReadError::TooLarge | ReadError::Io => SnapshotError::Unreadable,
        })?;
    read_metadata_from_svg(&svg)
}

/// Записывает снимок вставки (FR-019, FR-028).
///
/// Метаданные вкладываются здесь, а не вызывающей стороной: файл снимка,
/// написанный приложением, обязан нести связь с доской и хэш её состояния —
/// иначе вставка по нему станет `Undeterminable` сразу после создания.
///
/// Ровно один файл за вызов (инвариант 3 data-model.md): путь приходит
/// снаружи, и других записей эта функция не делает.
pub async fn write_snapshot<B: VaultBackend>(
    backend: &mut B,
    snapshot_path: &Path,
    svg: &str,
    metadata: &SnapshotMetadata,
) -> Result<(), SnapshotWriteError> {
    if classify(snapshot_path) != FileKind::Snapshot {
        return Err(SnapshotWriteError::NotASnapshotPath);
    }
    let with_metadata = embed_metadata_in_svg(svg, metadata)?;

    // Папку снимков создаёт первая же вставка (research.md §4). Отдельным
    // вызовом, а не внутри `write`: запись в несуществующую папку обязана
    // остаться отказом (001 FR-014).
    if let Some(folder) = snapshot_path.parent() {
        backend.ensure_folder(folder).await.map_err(to_write_error)?;
    }
    backend
        .write(snapshot_path, &with_metadata, None)
        .await
        .map_err(to_write_error)
}

fn to_write_error(error: WriteError) -> SnapshotWriteError {
    match error {
        WriteError::AccessDenied => SnapshotWriteError::VaultNotWritable,
        WriteError::Conflict | WriteError::Io => SnapshotWriteError::Io,
    }
}

/// Идентификатор для попытки номер `attempt`.
fn embed_id_for(note_path: &Path, board_path: &Path, attempt: u32) -> EmbedId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(note_path.to_string_lossy().as_bytes());
    hasher.update(b"\0");
    hasher.update(board_path.to_string_lossy().as_bytes());
    hasher.update(b"\0");
    hasher.update(attempt.to_string().as_bytes());
    let hex = hasher.finalize().to_hex();
    EmbedId {
        value: format!("{EMBED_ID_PREFIX}{}", &hex.as_str()[..EMBED_ID_HEX_LEN]),
    }
}

/// Конец открывающего тега `<svg …>` — место, куда вкладывается `<desc>`.
/// Кавычки учитываются: `>` внутри значения атрибута тег не закрывает.
fn root_tag_end(svg: &str) -> Option<usize> {
    let start = svg.find("<svg")?;
    let mut in_quotes: Option<char> = None;
    for (offset, symbol) in svg[start..].char_indices() {
        match (in_quotes, symbol) {
            (Some(quote), current) if current == quote => in_quotes = None,
            (Some(_), _) => {}
            (None, '"') | (None, '\'') => in_quotes = Some(symbol),
            (None, '>') => return Some(start + offset + symbol.len_utf8()),
            (None, _) => {}
        }
    }
    None
}

/// Убирает ранее вложенный нами `<desc>`. Чужие `<desc>` — например,
/// подпись, добавленную сторонним редактором, — не трогает.
fn remove_typsidian_desc(svg: &str) -> String {
    let mut result = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(open) = rest.find("<desc") {
        let Some(close) = rest[open..].find("</desc>").map(|at| open + at + "</desc>".len()) else {
            break;
        };
        if rest[open..close].contains(BOARD_ATTRIBUTE) {
            result.push_str(&rest[..open]);
        } else {
            result.push_str(&rest[..close]);
        }
        rest = &rest[close..];
    }
    result.push_str(rest);
    result
}

/// Значение атрибута по имени. Поиск идёт по `имя="`, поэтому
/// `data-typsidian-board` не совпадает с `data-typsidian-board-hash`.
fn attribute_value(svg: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = svg.find(&needle)? + needle.len();
    let end = start + svg[start..].find('"')?;
    Some(unescape_attribute(&svg[start..end]))
}

fn escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn unescape_attribute(value: &str) -> String {
    // `&amp;` разворачивается последним: иначе `&amp;quot;` превратился бы в
    // кавычку вместо литерала `&quot;`.
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Экранирование внутри строкового литерала Typst.
fn escape_typst_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boards::digest::digest;

    const BOARD: &str = r#"{"type":"excalidraw","elements":[{"id":"a"}]}"#;

    fn metadata_of(board_path: &str) -> SnapshotMetadata {
        snapshot_metadata(Path::new(board_path), &digest(BOARD.as_bytes()).unwrap())
    }

    #[test]
    fn snapshot_lives_next_to_its_note() {
        // research.md §4: снимок едет вместе с заметкой при её переносе.
        let plan = plan_embed(
            Path::new("Проект/Идея.typ"),
            "",
            Path::new("Схема.excalidraw"),
        )
        .unwrap();
        assert!(plan
            .snapshot_path
            .starts_with(Path::new("Проект/Идея.assets")));
        assert!(plan.markup_path.starts_with("Идея.assets/"));
    }

    #[test]
    fn two_embeds_of_the_same_board_in_one_note_get_different_files() {
        // FR-020: у каждой вставки свой файл, даже если доска одна.
        let note = Path::new("Идея.typ");
        let board = Path::new("Схема.excalidraw");
        let first = plan_embed(note, "", board).unwrap();
        let text = embed_markup(&first);
        let second = plan_embed(note, &text, board).unwrap();
        assert_ne!(first.embed_id, second.embed_id);
        assert_ne!(first.snapshot_path, second.snapshot_path);
    }

    #[test]
    fn markup_is_plain_typst() {
        // FR-022: никаких маркеров приложения в тексте заметки.
        let plan = plan_embed(Path::new("Идея.typ"), "", Path::new("Схема.excalidraw")).unwrap();
        let markup = embed_markup(&plan);
        assert!(markup.starts_with("#image(\""));
        assert!(markup.ends_with("\")"));
        assert!(!markup.to_lowercase().contains("typsidian"));
    }

    #[test]
    fn metadata_survives_the_round_trip() {
        let metadata = metadata_of("Проект/Схема.excalidraw");
        let svg = embed_metadata_in_svg("<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>", &metadata)
            .unwrap();
        assert_eq!(read_metadata_from_svg(&svg).unwrap(), metadata);
    }

    #[test]
    fn updating_metadata_does_not_leave_the_previous_hash_behind() {
        let first = metadata_of("Схема.excalidraw");
        let second = snapshot_metadata(
            Path::new("Схема.excalidraw"),
            &digest(br#"{"elements":[{"id":"b"}]}"#).unwrap(),
        );
        let once = embed_metadata_in_svg("<svg></svg>", &first).unwrap();
        let twice = embed_metadata_in_svg(&once, &second).unwrap();
        assert_eq!(read_metadata_from_svg(&twice).unwrap(), second);
        assert!(!twice.contains(first.board_digest.as_str()));
    }

    #[test]
    fn a_foreign_description_is_left_alone() {
        let metadata = metadata_of("Схема.excalidraw");
        let svg = embed_metadata_in_svg("<svg><desc>Подпись автора</desc></svg>", &metadata).unwrap();
        assert!(svg.contains("Подпись автора"));
    }

    #[test]
    fn a_snapshot_without_metadata_is_not_readable_as_up_to_date() {
        // SC-008: снимок, заменённый сторонним инструментом, метаданных не несёт.
        assert_eq!(
            read_metadata_from_svg("<svg></svg>"),
            Err(SnapshotError::NoMetadata)
        );
    }

    #[test]
    fn a_broken_hash_is_not_taken_for_a_digest() {
        let svg = format!(
            "<svg><desc {BOARD_ATTRIBUTE}=\"Схема.excalidraw\" {BOARD_HASH_ATTRIBUTE}=\"не хэш\"></desc></svg>"
        );
        assert_eq!(
            read_metadata_from_svg(&svg),
            Err(SnapshotError::NoMetadata)
        );
    }

    #[test]
    fn a_quote_in_a_board_name_does_not_break_the_attribute() {
        let metadata = metadata_of("Схема \"черновик\".excalidraw");
        let svg = embed_metadata_in_svg("<svg></svg>", &metadata).unwrap();
        assert_eq!(read_metadata_from_svg(&svg).unwrap(), metadata);
    }

    #[test]
    fn a_greater_than_inside_an_attribute_does_not_end_the_root_tag() {
        let metadata = metadata_of("Схема.excalidraw");
        let svg = embed_metadata_in_svg("<svg data-x=\"a>b\"><g/></svg>", &metadata).unwrap();
        assert!(svg.starts_with("<svg data-x=\"a>b\"><desc "));
    }

    #[test]
    fn only_notes_have_snapshot_folders() {
        assert_eq!(assets_folder_name(Path::new("Схема.excalidraw")), None);
        assert_eq!(
            assets_folder_name(Path::new("Идея.typ")).as_deref(),
            Some("Идея.assets")
        );
    }

    #[test]
    fn a_board_cannot_be_embedded_into_a_board() {
        assert_eq!(
            plan_embed(
                Path::new("Схема.excalidraw"),
                "",
                Path::new("Другая.excalidraw")
            ),
            Err(EmbedError::NotANote)
        );
        assert_eq!(
            plan_embed(Path::new("Идея.typ"), "", Path::new("Картинка.png")),
            Err(EmbedError::NotABoard)
        );
    }
}
