//! Вставка доски в текст заметки (фича 002, T042, T055, research.md §11).
//!
//! Вставка **в файлах не хранится**: она вычисляется разбором текста заметки.
//! Опознаётся по адресу — вызов `#image`, чей путь ведёт в папку снимков этой
//! заметки, — а не по маркеру приложения: маркер был бы метаданными Typsidian в
//! пользовательском файле (FR-022) и первым, что потеряет сторонний редактор.
//!
//! Следствие, которое стоит держать в голове: удалил вызов из текста — вставки
//! больше нет, и уборки состояния не требуется (FR-024).

use super::digest::DigestError;
use super::snapshot::{read_snapshot_metadata, EmbedId, SnapshotError};
use super::store::read_board;
use super::{EmbedState, ReadBoardError};
use crate::vault::backend::VaultBackend;
use crate::vault::{classify, FileKind, ASSETS_FOLDER_NAME, SNAPSHOT_FOLDER_SUFFIX};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

/// Вызов, по которому опознаётся вставка.
const IMAGE_CALL: &str = "#image";

/// Положение вызова `#image(…)` в тексте заметки.
///
/// Смещения — **в байтах UTF-8**, как их считает Rust. Сторона, которой нужны
/// позиции в терминах редактора (JavaScript считает в кодовых единицах UTF-16),
/// обязана пересчитать их у себя: молчаливая подмена единиц измерения на границе
/// давала бы сдвиг ровно на заметках с кириллицей, то есть на всех.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

/// Вставка, найденная в тексте, — без состояния актуальности.
///
/// Отдельный тип от [`Embed`] намеренно: разбор текста файлов не читает, а
/// состояние без чтения файлов не определить. Смешать их значило бы завести
/// шестое состояние «ещё не выяснено», которого спека не знает.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct EmbedRef {
    pub id: EmbedId,
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    pub note_path: PathBuf,
    pub range: TextRange,
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    pub snapshot_path: PathBuf,
}

/// Вставка вместе с наблюдаемым состоянием (data-model.md «Embed»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Embed {
    pub id: EmbedId,
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    pub note_path: PathBuf,
    pub range: TextRange,
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
    pub snapshot_path: PathBuf,
    pub state: EmbedState,
    /// Доска-источник, если её удалось узнать из метаданных снимка. `None` —
    /// снимка нет или метаданных в нём нет: перейти к доске (FR-025) тогда
    /// некуда, и UI обязан это учитывать, а не подставлять догадку.
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string | undefined"))]
    pub board_path: Option<PathBuf>,
}

/// Все вставки досок в тексте заметки (FR-026).
///
/// Чистая функция: ни чтения файлов, ни записи, ни кэша — вызывающая сторона
/// передаёт текст, который у неё и так открыт. Поэтому разбор проверяется
/// таблицей входов и выходов на обоих таргетах, без файловой системы.
///
/// # Разбор не знает о текущей политике расположения — и не должен
///
/// Смена политики не двигает уже созданные файлы (006 FR-007), поэтому у одной
/// заметки вставки могут одновременно лежать по прежнему правилу, по новому
/// рядом с заметкой и по новому в корне. Если бы распознавание сверяло адрес с
/// **расчётным** путём для текущей политики, заметка теряла бы собственные
/// вставки сразу после переключения — прямо против FR-007 и FR-010.
///
/// Поэтому правило смотрит на сам адрес и на имя заметки, а не на настройку:
/// вставкой считается вызов `#image`, ведущий в папку снимков **этой** заметки
/// по любому из двух правил раскладки.
pub fn embeds_of(note_path: &Path, note_text: &str) -> Vec<EmbedRef> {
    if classify(note_path) != FileKind::Note {
        return Vec::new();
    }
    let Some(stem) = note_path.file_stem().and_then(|stem| stem.to_str()) else {
        return Vec::new();
    };
    let note_folder = note_path.parent().unwrap_or(Path::new(""));

    image_calls(note_text)
        .into_iter()
        .filter_map(|call| {
            let resolved = resolve_from(note_folder, &call.path)?;
            let id = embed_id_at(&resolved, stem)?;
            Some(EmbedRef {
                snapshot_path: resolved,
                id,
                note_path: note_path.to_path_buf(),
                range: call.range,
            })
        })
        .collect()
}

/// Наблюдаемое состояние одной вставки (data-model.md «EmbedState»).
///
/// `known_board` — доска, о которой вызывающая сторона знает из прошлого чтения
/// снимка. Она нужна ровно в одном случае: файла снимка больше нет, и адрес
/// доски вместе с ним пропал — связь «снимок → доска» живёт только в самом
/// снимке (research.md §11). Без этой подсказки отличить «снимок удалили»
/// от «доску удалили» неоткуда.
pub async fn embed_state<B: VaultBackend>(
    backend: &B,
    snapshot_path: &Path,
    known_board: Option<&Path>,
) -> EmbedState {
    match read_snapshot_metadata(backend, snapshot_path).await {
        Ok(metadata) => match read_board(backend, &metadata.board_path).await {
            Ok(content) => match content.digest() {
                Ok(current) => {
                    EmbedState::compare(Some(&current), Some(&metadata.board_digest))
                }
                Err(DigestError::Unreadable) => EmbedState::Undeterminable {
                    reason: "файл доски не разбирается".to_string(),
                },
            },
            // FR-031: доска удалена или перемещена. Заметка при этом остаётся
            // читаемой и редактируемой — состояние вставки на неё не влияет.
            Err(ReadBoardError::NotFound) => EmbedState::BoardMissing,
            Err(ReadBoardError::Unreadable) => EmbedState::Undeterminable {
                reason: "файл доски не разбирается".to_string(),
            },
            Err(ReadBoardError::Io) => EmbedState::Undeterminable {
                reason: "доску не удалось прочитать с диска".to_string(),
            },
        },
        // FR-032: снимка нет. Если про доску известно и она на месте — снимок
        // восстановим; если её тоже нет, отсутствует именно источник, и это
        // более сильный факт.
        Err(SnapshotError::NotFound) => match known_board {
            Some(board) => match read_board(backend, board).await {
                Ok(_) => EmbedState::SnapshotMissing,
                Err(ReadBoardError::NotFound) => EmbedState::BoardMissing,
                Err(_) => EmbedState::SnapshotMissing,
            },
            None => EmbedState::SnapshotMissing,
        },
        Err(SnapshotError::NoMetadata) => EmbedState::Undeterminable {
            reason: "в снимке нет метаданных доски".to_string(),
        },
        Err(SnapshotError::Unreadable) => EmbedState::Undeterminable {
            reason: "снимок не удалось прочитать".to_string(),
        },
    }
}

/// Вставки заметки вместе с состоянием каждой.
///
/// Ничего не пишет: ни один переход состояния сам по себе не приводит к записи
/// снимка (инвариант 2 data-model.md, FR-030).
pub async fn embeds_with_state<B: VaultBackend>(
    backend: &B,
    note_path: &Path,
    note_text: &str,
) -> Vec<Embed> {
    let mut embeds = Vec::new();
    for found in embeds_of(note_path, note_text) {
        let board_path = read_snapshot_metadata(backend, &found.snapshot_path)
            .await
            .ok()
            .map(|metadata| metadata.board_path);
        let state = embed_state(backend, &found.snapshot_path, board_path.as_deref()).await;
        embeds.push(Embed {
            id: found.id,
            note_path: found.note_path,
            range: found.range,
            snapshot_path: found.snapshot_path,
            state,
            board_path,
        });
    }
    embeds
}

/// Вызов `#image` в тексте: путь из первого аргумента и границы всего вызова.
struct ImageCall {
    path: String,
    range: TextRange,
}

/// Находит вызовы `#image(…)`. Разбирается ровно столько Typst, сколько нужно
/// для опознания вставки: имя функции, скобки и первый строковый аргумент.
/// Полного парсера Typst здесь нет и не должно быть — за разбор документа
/// отвечает сам Typst (принцип VII).
fn image_calls(text: &str) -> Vec<ImageCall> {
    let mut calls = Vec::new();
    let mut search_from = 0usize;

    while let Some(found) = text[search_from..].find(IMAGE_CALL) {
        let start = search_from + found;
        let after_name = start + IMAGE_CALL.len();
        search_from = after_name;

        // `#imagemap(...)` — другая функция: за именем обязана идти скобка.
        let rest = &text[after_name..];
        if !rest.starts_with('(') {
            continue;
        }
        let Some(call_end) = call_end(rest) else {
            continue;
        };
        let Some(path) = first_string_argument(&rest[1..call_end]) else {
            continue;
        };
        let end = after_name + call_end + 1;
        calls.push(ImageCall {
            path,
            range: TextRange { start, end },
        });
        search_from = end;
    }

    calls
}

/// Смещение закрывающей скобки вызова, начинающегося с `(`. Учитываются
/// вложенные скобки и строки: `#image("a)b.svg", width: f(1))`.
fn call_end(from_paren: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, symbol) in from_paren.char_indices() {
        if in_string {
            match symbol {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match symbol {
            '"' => in_string = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(offset);
                }
            }
            _ => {}
        }
    }
    None
}

/// Первый строковый литерал в аргументах вызова.
fn first_string_argument(arguments: &str) -> Option<String> {
    let start = arguments.find('"')? + 1;
    let mut value = String::new();
    let mut escaped = false;
    for symbol in arguments[start..].chars() {
        match symbol {
            _ if escaped => {
                value.push(symbol);
                escaped = false;
            }
            '\\' => escaped = true,
            '"' => return Some(value),
            _ => value.push(symbol),
        }
    }
    None
}

/// Приводит адрес из вызова `#image` к пути относительно корня vault.
///
/// `None` — адрес абсолютный либо выходит за корень vault: такой файл нашей
/// вставкой быть не может, а молча «подтянуть» его до корня значило бы
/// принять за вставку что-то из чужого дерева.
fn resolve_from(note_folder: &Path, image_path: &str) -> Option<PathBuf> {
    if image_path.starts_with('/') {
        return None;
    }
    let mut resolved: Vec<std::ffi::OsString> = note_folder
        .components()
        .map(|component| component.as_os_str().to_os_string())
        .collect();

    for component in Path::new(image_path).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop()?;
            }
            Component::Normal(name) => resolved.push(name.to_os_string()),
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(resolved.iter().collect())
}

/// Идентификатор вставки по пути снимка относительно корня vault.
///
/// `None` — путь ведёт не в папку снимков заметки со стволом `note_stem`:
/// обычная картинка пользователя вставкой доски не является (FR-026,
/// 002 research.md §11), как и снимок **другой** заметки.
///
/// Принимаются оба правила раскладки (006 FR-009):
///
/// ```text
/// …/<ствол>.assets/board-<hex>.svg     — прежнее
/// …/assets/<ствол>/board-<hex>.svg     — новое
/// ```
fn embed_id_at(snapshot_path: &Path, note_stem: &str) -> Option<EmbedId> {
    let file_name = snapshot_path.file_name()?.to_str()?;
    let id = EmbedId::from_file_name(file_name)?;
    let parent = snapshot_path.parent()?;
    let parent_name = parent.file_name()?.to_str()?;

    let legacy = parent_name == format!("{note_stem}{SNAPSHOT_FOLDER_SUFFIX}");
    let current = parent_name == note_stem
        && parent
            .parent()
            .and_then(|assets| assets.file_name())
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == ASSETS_FOLDER_NAME);

    (legacy || current).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "Идея.typ";

    fn ids(note_path: &str, text: &str) -> Vec<String> {
        embeds_of(Path::new(note_path), text)
            .into_iter()
            .map(|found| found.id.as_str().to_string())
            .collect()
    }

    #[test]
    fn an_image_pointing_into_the_snapshot_folder_is_an_embed() {
        let text = "Текст\n#image(\"Идея.assets/board-1f4c9a2b.svg\")\nещё текст";
        let found = embeds_of(Path::new(NOTE), text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id.as_str(), "board-1f4c9a2b");
        assert_eq!(
            found[0].snapshot_path,
            PathBuf::from("Идея.assets/board-1f4c9a2b.svg")
        );
        assert_eq!(&text[found[0].range.start..found[0].range.end], "#image(\"Идея.assets/board-1f4c9a2b.svg\")");
    }

    #[test]
    fn an_ordinary_image_is_not_an_embed() {
        // research.md §11: вставка опознаётся по адресу. Картинка, которую
        // пользователь положил в vault сам, вставкой доски не является.
        assert!(ids(NOTE, "#image(\"Фото.png\")").is_empty());
        assert!(ids(NOTE, "#image(\"Другая.assets/board-1f4c9a2b.svg\")").is_empty());
        assert!(ids(NOTE, "#image(\"Идея.assets/подпапка/board-1f4c9a2b.svg\")").is_empty());
        assert!(ids(NOTE, "#image(\"Идея.assets/своя-картинка.svg\")").is_empty());
    }

    #[test]
    fn two_embeds_are_found_with_their_own_ranges() {
        // FR-023: несколько вставок в одной заметке.
        let text = "#image(\"Идея.assets/board-aaaaaaaa.svg\")\n\n#image(\"Идея.assets/board-bbbbbbbb.svg\")";
        let found = embeds_of(Path::new(NOTE), text);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].id.as_str(), "board-aaaaaaaa");
        assert_eq!(found[1].id.as_str(), "board-bbbbbbbb");
        assert!(found[0].range.end <= found[1].range.start);
        for one in &found {
            assert_eq!(&text[one.range.start..one.range.end][..7], "#image(");
        }
    }

    #[test]
    fn named_arguments_and_relative_prefix_are_understood() {
        let text = "#image(\"./Идея.assets/board-1f4c9a2b.svg\", width: 80%)";
        let found = embeds_of(Path::new(NOTE), text);
        assert_eq!(found.len(), 1);
        assert_eq!(&text[found[0].range.start..found[0].range.end], text);
    }

    #[test]
    fn a_similarly_named_function_is_not_an_image_call() {
        assert!(ids(NOTE, "#imagemap(\"Идея.assets/board-1f4c9a2b.svg\")").is_empty());
    }

    #[test]
    fn a_board_has_no_embeds_of_its_own() {
        assert!(ids(
            "Схема.excalidraw",
            "#image(\"Схема.assets/board-1f4c9a2b.svg\")"
        )
        .is_empty());
    }

    #[test]
    fn removing_the_call_removes_the_embed() {
        // FR-024: никакого состояния, требующего уборки, вставка не оставляет.
        assert!(ids(NOTE, "текст без вставок").is_empty());
    }
}
