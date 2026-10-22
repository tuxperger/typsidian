//! Замена по vault (фича 027) — общая таблица для обоих таргетов.
//!
//! Файлы `search_native.rs` / `search_wasm.rs` только вызывают эти функции
//! своим harness'ом: правила замены платформенно нейтральны (принцип II).

#![allow(dead_code)]

use typsidian_core::links::OpenText;
use typsidian_core::search::replace::{
    digest, replace_in_note, replace_preview, replace_undo, PieceKind, ReplaceError,
    ReplaceNoteOutcome, ReplaceNoteRequest, ReplaceQuery, UndoOutcome, MAX_REPLACE_MATCHES,
};
use typsidian_core::search::SearchQuery;

use super::MemoryVault;

const INTRO: &str = "Научный руководитель — д.т.н. Иванов А. П.\n\
                     Методика предложена Ивановым в [[Методика]].\n\
                     #cite(<Иванов2024>)\n";
const RESULTS: &str = "Сравнение с данными Иванов (2024).\nРяд Иванов—Петрова сходится.\n";

fn vault() -> MemoryVault {
    MemoryVault::new()
        .with_file("Диссертация/Введение.typ", INTRO)
        .with_file("Диссертация/Результаты.typ", RESULTS)
        .with_file("Заметки/Встреча.typ", "- Петров: прислать замечания\n")
        .with_file("Заметки/схема.excalidraw", "{\"elements\":[\"Иванов\"]}")
        .checking_expected()
}

fn query(text: &str, replacement: &str) -> ReplaceQuery {
    ReplaceQuery {
        search: SearchQuery {
            text: text.to_owned(),
            case_sensitive: true,
            ..SearchQuery::default()
        },
        replacement: replacement.to_owned(),
    }
}

fn text_of(vault: &MemoryVault, path: &str) -> String {
    String::from_utf8(vault.bytes_of(path).expect("файл есть")).unwrap()
}

/// Предпросмотр перечисляет каждое совпадение каждой заметки — без предела
/// поиска в три строки на заметку — с кусками «было/станет» (FR-002).
pub async fn the_preview_lists_every_match_with_before_and_after() {
    let preview = replace_preview(&vault(), &query("Иванов", "Иванова"), &[])
        .await
        .expect("предпросмотр");

    assert_eq!(preview.total_notes, 2, "доска не участвует");
    assert_eq!(preview.total_matches, 5);
    let intro = &preview.notes[0];
    assert_eq!(intro.path, "Диссертация/Введение.typ");
    assert_eq!(intro.digest, digest(INTRO));
    assert_eq!(intro.matches.len(), 3);
    assert_eq!(
        intro.matches.iter().map(|m| m.line).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    let removed: Vec<&str> = intro.matches[0]
        .pieces
        .iter()
        .filter(|piece| piece.kind != PieceKind::Plain)
        .map(|piece| piece.text.as_str())
        .collect();
    assert_eq!(removed, vec!["Иванов", "Иванова"]);
}

/// Группы подставляются только в режиме выражения; в текстовом `$` буквален.
pub async fn groups_expand_only_for_patterns() {
    let mut pattern = query(r"(\w+)ов\b", "${1}ова");
    pattern.search.regex = true;
    let preview = replace_preview(&vault(), &pattern, &[]).await.unwrap();
    let inserted: Vec<String> = preview
        .notes
        .iter()
        .flat_map(|note| &note.matches)
        .flat_map(|m| &m.pieces)
        .filter(|piece| piece.kind == PieceKind::Inserted)
        .map(|piece| piece.text.clone())
        .collect();
    assert!(inserted.contains(&"Иванова".to_owned()), "{inserted:?}");

    let plain = replace_preview(&vault(), &query("Петров", "$1"), &[])
        .await
        .unwrap();
    assert!(plain.notes[0].matches[0]
        .pieces
        .iter()
        .any(|piece| piece.kind == PieceKind::Inserted && piece.text == "$1"));
}

/// Неверное выражение названо, предпросмотра нет (Edge Cases).
pub async fn a_broken_pattern_is_named() {
    let mut broken = query("Иванов(", "x");
    broken.search.regex = true;
    let error = replace_preview(&vault(), &broken, &[]).await.unwrap_err();
    assert!(matches!(error, ReplaceError::BadPattern { .. }));
}

/// Больше предела — замена недоступна, число названо (FR-008).
pub async fn too_many_matches_are_refused_with_the_count() {
    let text = "а ".repeat(MAX_REPLACE_MATCHES + 5);
    let vault = MemoryVault::new().with_file("Много.typ", text);
    let error = replace_preview(&vault, &query("а", "б"), &[])
        .await
        .unwrap_err();
    assert_eq!(
        error,
        ReplaceError::TooMany {
            count: (MAX_REPLACE_MATCHES + 5) as u32
        }
    );
}

/// Записываются только отмеченные совпадения; прочий текст байт в байт
/// прежний (FR-003, SC-002).
pub async fn only_selected_matches_are_written() {
    let mut vault = vault();
    let request = ReplaceNoteRequest {
        query: query("Иванов", "Иванова"),
        path: "Диссертация/Введение.typ".to_owned(),
        digest: digest(INTRO),
        selected: vec![0, 2],
    };
    let outcome = replace_in_note(&mut vault, &request, None).await;
    let ReplaceNoteOutcome::Written { undo } = outcome else {
        panic!("ожидалась запись: {outcome:?}");
    };
    assert_eq!(undo.before, INTRO);
    let after = text_of(&vault, "Диссертация/Введение.typ");
    assert_eq!(
        after,
        INTRO
            .replacen("д.т.н. Иванов", "д.т.н. Иванова", 1)
            .replace("<Иванов2024>", "<Иванова2024>")
    );
    assert_eq!(undo.after_digest, digest(&after));
}

/// Текст изменился после предпросмотра — ничего не записано (FR-007).
pub async fn a_note_changed_after_the_preview_is_skipped() {
    let mut vault = vault();
    vault.replace_externally("Диссертация/Введение.typ", "Новый текст про Иванов\n");
    let request = ReplaceNoteRequest {
        query: query("Иванов", "Иванова"),
        path: "Диссертация/Введение.typ".to_owned(),
        digest: digest(INTRO),
        selected: vec![0],
    };
    assert_eq!(
        replace_in_note(&mut vault, &request, None).await,
        ReplaceNoteOutcome::Stale
    );
    assert_eq!(vault.write_count("Диссертация/Введение.typ"), 0);
}

/// Открытая заметка: одна правка для редактора, на диск ничего (FR-004).
pub async fn an_open_note_gets_one_editor_edit() {
    let mut vault = vault();
    let editor = format!("{RESULTS}Несохранённое.\n");
    let preview = replace_preview(
        &vault,
        &query("Иванов", "Иванова"),
        &[OpenText {
            path: "Диссертация/Результаты.typ".to_owned(),
            text: editor.clone(),
        }],
    )
    .await
    .unwrap();
    let note = preview
        .notes
        .iter()
        .find(|note| note.path == "Диссертация/Результаты.typ")
        .unwrap();
    assert!(note.open_in_editor);

    let request = ReplaceNoteRequest {
        query: query("Иванов", "Иванова"),
        path: note.path.clone(),
        digest: note.digest.clone(),
        selected: vec![0, 1],
    };
    let outcome = replace_in_note(&mut vault, &request, Some(&editor)).await;
    let ReplaceNoteOutcome::EditorEdit { edit, undo } = outcome else {
        panic!("ожидалась правка редактора: {outcome:?}");
    };
    assert_eq!(vault.write_count("Диссертация/Результаты.typ"), 0);
    // Применённая правка даёт ровно замену обоих мест.
    let units: Vec<u16> = editor.encode_utf16().collect();
    let mut applied: Vec<u16> = units[..edit.from].to_vec();
    applied.extend(edit.insert.encode_utf16());
    applied.extend(&units[edit.to..]);
    let applied = String::from_utf16(&applied).unwrap();
    assert_eq!(
        applied,
        editor
            .replace("Иванов ", "Иванова ")
            .replace("Иванов—", "Иванова—")
    );
    assert_eq!(undo.after_digest, digest(&applied));
}

/// Несохранённые правки вне редактора — замена не пишет.
pub async fn unsaved_edits_elsewhere_block_the_write() {
    let mut vault = vault().with_unsaved_edits("Диссертация/Введение.typ");
    let request = ReplaceNoteRequest {
        query: query("Иванов", "Иванова"),
        path: "Диссертация/Введение.typ".to_owned(),
        digest: digest(INTRO),
        selected: vec![0],
    };
    assert_eq!(
        replace_in_note(&mut vault, &request, None).await,
        ReplaceNoteOutcome::UnsavedElsewhere
    );
}

/// Отмена возвращает текст, если заметку не меняли; иначе не трогает её
/// (FR-005, SC-003).
pub async fn undo_restores_untouched_notes_and_spares_changed_ones() {
    let mut vault = vault();
    let request = |path: &str, text: &str| ReplaceNoteRequest {
        query: query("Иванов", "Иванова"),
        path: path.to_owned(),
        digest: digest(text),
        selected: vec![0],
    };
    let ReplaceNoteOutcome::Written { undo: first } = replace_in_note(
        &mut vault,
        &request("Диссертация/Введение.typ", INTRO),
        None,
    )
    .await
    else {
        panic!("запись");
    };
    let ReplaceNoteOutcome::Written { undo: second } = replace_in_note(
        &mut vault,
        &request("Диссертация/Результаты.typ", RESULTS),
        None,
    )
    .await
    else {
        panic!("запись");
    };

    vault.replace_externally("Диссертация/Результаты.typ", "Поправлено руками\n");

    assert_eq!(
        replace_undo(&mut vault, &first, None).await,
        UndoOutcome::Restored
    );
    assert_eq!(text_of(&vault, "Диссертация/Введение.typ"), INTRO);
    assert_eq!(
        replace_undo(&mut vault, &second, None).await,
        UndoOutcome::ChangedSince
    );
    assert_eq!(
        text_of(&vault, "Диссертация/Результаты.typ"),
        "Поправлено руками\n"
    );
}

/// Отмена в открытой заметке — правка для редактора.
pub async fn undo_in_an_open_note_is_an_editor_edit() {
    let mut vault = vault();
    let after = INTRO.replacen("Иванов", "Иванова", 1);
    let record = typsidian_core::search::replace::UndoRecord {
        path: "Диссертация/Введение.typ".to_owned(),
        before: INTRO.to_owned(),
        after_digest: digest(&after),
    };
    let outcome = replace_undo(&mut vault, &record, Some(&after)).await;
    let UndoOutcome::EditorEdit { edit } = outcome else {
        panic!("ожидалась правка редактора: {outcome:?}");
    };
    assert_eq!(edit.insert, "");
    assert_eq!(edit.to - edit.from, 1, "убирается одна буква «а»");
}
