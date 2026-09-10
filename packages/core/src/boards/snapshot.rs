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
use super::placement::{self, AssetsPlacement};
use crate::vault::backend::{VaultBackend, WriteError};
use crate::vault::sections::SectionMark;
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
    ///
    /// С фичи 006 это правило работает ещё и как **признак снимка** в
    /// классификаторе: у общей папки `assets/` нет защитного суффикса, который
    /// был у `*.assets`, и отличить снимок от картинки пользователя можно
    /// только по имени файла — см. [`is_snapshot_file_name`].
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

/// Похоже ли имя файла на снимок, созданный приложением (фича 006, FR-012).
///
/// Тонкая обёртка над [`EmbedId::from_file_name`] — намеренно, чтобы правило
/// «что считать снимком» осталось в одном месте. Классификатор
/// (`vault::classify`) спрашивает именно это: без проверки имени правило
/// «файл внутри `assets/` — снимок» скрыло бы картинки, которые пользователь
/// положил в папку с таким именем сам.
pub fn is_snapshot_file_name(file_name: &str) -> bool {
    EmbedId::from_file_name(file_name).is_some()
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

/// Имя папки снимков для заметки по **прежнему** правилу:
/// `<имя заметки без расширения>.assets`.
///
/// `None` — путь не является путём заметки.
///
/// Фича 006 таких папок больше не создаёт — расположение выбирает пользователь,
/// а раскладка внутри корня единая (`boards::placement`). Функция осталась
/// потому, что уже созданные папки обязаны продолжать работать (006 FR-009), и
/// правило их именования должно жить в одном месте.
pub fn assets_folder_name(note_path: &Path) -> Option<String> {
    if classify(note_path) != FileKind::Note {
        return None;
    }
    let stem = note_path.file_stem()?.to_str()?;
    Some(format!("{stem}{SNAPSHOT_FOLDER_SUFFIX}"))
}

/// Путь снимка относительно корня vault по выбранной политике.
///
/// Тонкая обёртка над [`placement::snapshot_path`] — расчёт живёт там
/// (contracts/asset-placement.md), здесь только точка входа для тех, кто уже
/// работает со снимками.
pub fn snapshot_path(
    placement: AssetsPlacement,
    note_path: &Path,
    sections: &[SectionMark],
    embed_id: &EmbedId,
) -> Option<PathBuf> {
    placement::snapshot_path(placement, note_path, sections, embed_id)
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
    placement: AssetsPlacement,
    sections: &[SectionMark],
) -> Result<EmbedPlan, EmbedError> {
    if classify(note_path) != FileKind::Note {
        return Err(EmbedError::NotANote);
    }
    if classify(board_path) != FileKind::Board {
        return Err(EmbedError::NotABoard);
    }
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

    // Расположение выбирает пользователь; расчёт живёт в `placement`
    // (contracts/asset-placement.md). `NotANote` здесь недостижим — вид пути
    // уже проверен выше, — но обрабатывается, чтобы отказ не превратился в
    // панику при будущей правке классификатора.
    let snapshot_path = placement::snapshot_path(placement, note_path, sections, &embed_id)
        .ok_or(EmbedError::NotANote)?;
    let markup_path = placement::markup_path(placement, note_path, sections, &embed_id)
        .ok_or(EmbedError::NotANote)?;

    Ok(EmbedPlan {
        snapshot_path,
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

    /// Планирование по умолчанию — политика «рядом с заметкой», без разделов.
    fn plan(note: &str, text: &str, board: &str) -> Result<EmbedPlan, EmbedError> {
        plan_embed(
            Path::new(note),
            text,
            Path::new(board),
            AssetsPlacement::NextToNote,
            &[],
        )
    }

    #[test]
    fn snapshot_lives_next_to_its_note() {
        // 002 research.md §4: снимок едет вместе с заметкой при её переносе.
        // Раскладка внутри папки заметки — по правилу 006 (FR-004): одна общая
        // `assets/` с подпапкой на заметку вместо отдельной папки на каждую.
        let plan = plan("Проект/Идея.typ", "", "Схема.excalidraw").unwrap();
        assert!(plan
            .snapshot_path
            .starts_with(Path::new("Проект/assets/Идея")));
        assert!(plan.markup_path.starts_with("assets/Идея/"));
    }

    #[test]
    fn two_embeds_of_the_same_board_in_one_note_get_different_files() {
        // FR-020: у каждой вставки свой файл, даже если доска одна.
        let first = plan("Идея.typ", "", "Схема.excalidraw").unwrap();
        let text = embed_markup(&first);
        let second = plan("Идея.typ", &text, "Схема.excalidraw").unwrap();
        assert_ne!(first.embed_id, second.embed_id);
        assert_ne!(first.snapshot_path, second.snapshot_path);
    }

    #[test]
    fn markup_is_plain_typst() {
        // FR-022: никаких маркеров приложения в тексте заметки.
        let plan = plan("Идея.typ", "", "Схема.excalidraw").unwrap();
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
            plan("Схема.excalidraw", "", "Другая.excalidraw"),
            Err(EmbedError::NotANote)
        );
        assert_eq!(
            plan("Идея.typ", "", "Картинка.png"),
            Err(EmbedError::NotABoard)
        );
    }
}
