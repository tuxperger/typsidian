//! События досок на границе ядро → UI (фича 002,
//! contracts/frontend-board-events.md).
//!
//! Отдельного потока для досок нет: создание, изменение, переименование и
//! удаление файла доски приходят теми же `VaultEvent`, что и для заметок
//! (FR-005). Здесь — только то, чего в потоке vault нет: состояние сохранения,
//! конфликт и актуальность вставок.

use super::digest::BoardDigest;
use crate::vault::{classify, FileKind};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Видимое пользователю состояние сохранения открытой доски (FR-009).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "state")]
pub enum BoardSaveState {
    /// Буфер совпадает с файлом.
    Clean,
    /// Есть изменения, дебаунс ещё не истёк.
    Dirty,
    /// Запись идёт.
    Saving,
    /// Запись не удалась; содержимое полотна при этом не потеряно (FR-010).
    Failed { reason: String },
}

/// Что произошло с доской. Размеченное объединение: UI обязан обработать
/// каждый вариант исчерпывающе (принцип IV).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "type")]
pub enum BoardEvent {
    SaveStateChanged {
        #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
        path: PathBuf,
        state: BoardSaveState,
    },
    SaveFailed {
        #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
        path: PathBuf,
        reason: String,
    },
    ConflictDetected {
        #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
        path: PathBuf,
    },
    /// Состояние вставок открытой заметки изменилось.
    ///
    /// Приходит и тогда, когда изменилась **доска**, а не текст заметки:
    /// устаревание вызывается правкой источника. Без этого индикатор появлялся
    /// бы только при повторном открытии заметки, и SC-008 выполнялся бы с
    /// задержкой, которую пользователь читает как «не работает».
    EmbedsChanged {
        #[cfg_attr(target_arch = "wasm32", tsify(type = "string"))]
        note_path: PathBuf,
    },
}

/// Могло ли изменение файла `changed` затронуть вставки заметки `note_path`
/// (T056, FR-027).
///
/// Ключевое слово — «могло»: это не пересчёт состояния, а ответ на вопрос,
/// стоит ли его пересчитывать. Сам пересчёт делает
/// [`super::embed::embeds_with_state`], читая файлы.
///
/// Правило намеренно широкое по доскам: **любое** изменение любой доски
/// считается поводом пересчитать вставки открытой заметки. Узнать, на какую
/// именно доску смотрит вставка, можно только прочитав её снимок
/// (research.md §11), то есть ровно тем же чтением, которое мы пытались бы
/// сэкономить. А пропустить изменение нельзя: тогда индикатор устаревания
/// появлялся бы лишь при повторном открытии заметки, и пользователь читал бы это
/// как «не работает» (SC-008).
pub fn embeds_changed(note_path: &Path, note_text: &str, changed: &Path) -> Option<BoardEvent> {
    let has_embeds = !super::embed::embeds_of(note_path, note_text).is_empty();
    if !has_embeds {
        return None;
    }

    let affects = changed == note_path
        || classify(changed) == FileKind::Board
        || super::embed::embeds_of(note_path, note_text)
            .iter()
            .any(|embed| embed.snapshot_path == changed);

    affects.then(|| BoardEvent::EmbedsChanged {
        note_path: note_path.to_path_buf(),
    })
}

/// Наблюдаемое состояние вставки доски в заметку (data-model.md «EmbedState»).
///
/// Пять вариантов, а не флаг «устарела»: `Undeterminable` отделён от
/// `UpToDate`, потому что «не смогли сравнить» и «совпадает» — для пользователя
/// разные вещи, а молчаливое расхождение запрещено (SC-008). `SnapshotMissing`
/// отделён от `BoardMissing`, потому что у них разные действия восстановления
/// (FR-031 против FR-032).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "state")]
pub enum EmbedState {
    UpToDate,
    Stale,
    /// Доска-источник не найдена: заметка остаётся читаемой и редактируемой.
    BoardMissing,
    /// Файла снимка нет, доска на месте — снимок восстановим одним действием.
    SnapshotMissing,
    /// Снимок есть, но метаданных в нём нет или они не читаются.
    Undeterminable {
        reason: UndeterminableReason,
    },
}

/// Почему состояние вставки не определить (фича 010): вид, а не текст — слова
/// подбирает интерфейс на своём языке.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(rename_all = "camelCase")]
pub enum UndeterminableReason {
    /// Файл доски не разбирается.
    BoardUnparsable,
    /// Доску не удалось прочитать с диска.
    BoardUnreadable,
    /// В снимке нет метаданных доски.
    SnapshotWithoutMetadata,
    /// Снимок не удалось прочитать.
    SnapshotUnreadable,
}

impl EmbedState {
    /// Сравнивает состояние доски со снятым снимком.
    ///
    /// Отдельная функция, а не сравнение на стороне UI: правило «хэши равны —
    /// актуально» обязано жить в ядре (принцип I), иначе оно разойдётся между
    /// клиентами.
    pub fn compare(board: Option<&BoardDigest>, snapshot: Option<&BoardDigest>) -> Self {
        match (board, snapshot) {
            (None, _) => EmbedState::BoardMissing,
            (Some(_), None) => EmbedState::Undeterminable {
                reason: UndeterminableReason::SnapshotWithoutMetadata,
            },
            (Some(board), Some(snapshot)) if board == snapshot => EmbedState::UpToDate,
            (Some(_), Some(_)) => EmbedState::Stale,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boards::digest::digest;

    fn digest_of(json: &str) -> BoardDigest {
        digest(json.as_bytes()).unwrap()
    }

    #[test]
    fn equal_digests_mean_up_to_date() {
        let board = digest_of(r#"{"elements":[{"id":"a"}]}"#);
        let snapshot = board.clone();
        assert_eq!(
            EmbedState::compare(Some(&board), Some(&snapshot)),
            EmbedState::UpToDate
        );
    }

    #[test]
    fn different_digests_mean_stale() {
        let board = digest_of(r#"{"elements":[{"id":"a"}]}"#);
        let snapshot = digest_of(r#"{"elements":[{"id":"b"}]}"#);
        assert_eq!(
            EmbedState::compare(Some(&board), Some(&snapshot)),
            EmbedState::Stale
        );
    }

    #[test]
    fn a_snapshot_without_metadata_is_undeterminable_not_up_to_date() {
        // SC-008: молчаливое расхождение запрещено. Снимок, заменённый
        // сторонним инструментом, не должен читаться как «актуально».
        let board = digest_of(r#"{"elements":[]}"#);
        assert!(matches!(
            EmbedState::compare(Some(&board), None),
            EmbedState::Undeterminable { .. }
        ));
    }

    #[test]
    fn a_missing_board_is_reported_separately_from_a_missing_snapshot() {
        // FR-031 и FR-032 требуют разных действий восстановления.
        assert_eq!(EmbedState::compare(None, None), EmbedState::BoardMissing);
    }

    const NOTE_WITH_EMBED: &str = "#image(\"Идея.assets/board-aaaaaaaa.svg\")";

    #[test]
    fn a_changed_board_notifies_the_note_that_embeds_it() {
        // T056: устаревание вызывается правкой источника, а не текста заметки.
        assert_eq!(
            embeds_changed(
                Path::new("Идея.typ"),
                NOTE_WITH_EMBED,
                Path::new("Схема.excalidraw")
            ),
            Some(BoardEvent::EmbedsChanged {
                note_path: PathBuf::from("Идея.typ")
            })
        );
    }

    #[test]
    fn a_note_without_embeds_is_not_notified() {
        assert_eq!(
            embeds_changed(
                Path::new("Идея.typ"),
                "текст без вставок",
                Path::new("Схема.excalidraw")
            ),
            None
        );
    }

    #[test]
    fn an_unrelated_note_does_not_trigger_a_recount() {
        assert_eq!(
            embeds_changed(
                Path::new("Идея.typ"),
                NOTE_WITH_EMBED,
                Path::new("Другая.typ")
            ),
            None
        );
    }

    #[test]
    fn a_replaced_snapshot_file_triggers_a_recount() {
        // Снимок могли подменить сторонним инструментом — состояние вставки
        // после этого другое (SC-008).
        assert!(embeds_changed(
            Path::new("Идея.typ"),
            NOTE_WITH_EMBED,
            Path::new("Идея.assets/board-aaaaaaaa.svg")
        )
        .is_some());
    }
}
