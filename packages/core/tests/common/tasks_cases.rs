//! Задачи из заметок (фича 033) — общая таблица для обоих таргетов.
//!
//! Файлы `tasks_native.rs` / `tasks_wasm.rs` только вызывают эти функции
//! своим harness'ом (принцип II). «Сегодня» передаётся явно.

#![allow(dead_code)]

use typsidian_core::properties::Condition;
use typsidian_core::tasks::{
    task_toggle, task_toggle_at, tasks_in, tasks_view, DueFilter, DueState, TaskFilter,
    TaskGrouping, TaskState, TaskToggleOutcome, TaskToggleRequest, TasksView,
};

use super::MemoryVault;

const TODAY: &str = "2026-10-23";

const INTRO: &str = "= Введение\n\n- [ ] Сверить цель 📅 2026-10-20\n- [x] Собрать литературу\n- [ ] Дописать актуальность срок: 2026-10-23\n  - [ ] Найти три обзора\n- обычный пункт\n\n```\n- [ ] не задача в коде\n```\n// - [ ] не задача в комментарии\n";
const METHOD: &str = "#metadata((tags: (\"глава\",))) <properties>\n= Методика\n\n+ [ ] Пересчитать погрешность due: 2026-10-27\n+ [X] Откалибровать термопары\n";
const DIARY: &str = "= 2026-10-23\n\n- [ ] Отправить главу\n- [ ] Купить бумагу 📅 2026-12-01\n";

fn vault() -> MemoryVault {
    MemoryVault::new()
        .with_file("Диссертация/Введение.typ", INTRO)
        .with_file("Диссертация/Методика.typ", METHOD)
        .with_file("дневник/2026-10-23.typ", DIARY)
}

fn filter() -> TaskFilter {
    TaskFilter::default()
}

fn texts(view: &TasksView) -> Vec<String> {
    view.groups
        .iter()
        .flat_map(|group| group.rows.iter().map(|row| row.task.text.clone()))
        .collect()
}

// ── US1 ─────────────────────────────────────────────────────────────────────

/// Разбор: отметки, `+`, три вида срока, вложенность; код и комментарий не
/// считаются (FR-001, Edge Cases).
pub fn tasks_are_parsed_from_markup_only() {
    let tasks = tasks_in("Введение.typ", INTRO);
    let texts: Vec<&str> = tasks.iter().map(|task| task.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "Сверить цель",
            "Собрать литературу",
            "Дописать актуальность",
            "Найти три обзора"
        ]
    );
    assert_eq!(tasks[0].due.as_deref(), Some("2026-10-20"));
    assert!(tasks[1].done);
    assert_eq!(tasks[2].due.as_deref(), Some("2026-10-23"));
    assert_eq!(tasks[3].parent, Some(tasks[2].line));
    assert_eq!(tasks[3].line, 6);

    let method = tasks_in("Методика.typ", METHOD);
    assert_eq!(method.len(), 2);
    assert_eq!(method[0].due.as_deref(), Some("2026-10-27"));
    assert!(method[1].done);
}

/// Окно собирает задачи всех заметок, по заметкам; счётчики (FR-001).
pub async fn view_collects_all_notes() {
    let mut vault = vault();
    let view = tasks_view(&mut vault, &filter(), TODAY).await;
    assert_eq!(view.open, 6);
    assert_eq!(view.overdue, 1);
    assert_eq!(view.shown, 6);
    let titles: Vec<&str> = view
        .groups
        .iter()
        .map(|group| group.title.as_str())
        .collect();
    assert_eq!(titles, ["Введение", "Методика", "2026-10-23"]);
    assert!(
        vault.bytes_of(".typsidian/tasks.json").is_some(),
        "индекс записан"
    );
}

/// Индекс: неизменённые заметки не перечитываются, изменённая — да (FR-005,
/// SC-001).
pub async fn index_rereads_only_changed_notes() {
    let mut vault = vault();
    tasks_view(&mut vault, &filter(), TODAY).await;
    let reads = vault.total_reads();
    tasks_view(&mut vault, &filter(), TODAY).await;
    // Второй показ читает только сам индекс.
    assert!(
        vault.total_reads() - reads <= 2,
        "{}",
        vault.total_reads() - reads
    );

    vault.replace_externally(
        "дневник/2026-10-23.typ",
        "= 2026-10-23\n\n- [ ] Новая задача\n",
    );
    let view = tasks_view(&mut vault, &filter(), TODAY).await;
    assert!(texts(&view).contains(&"Новая задача".to_owned()));
    assert!(!texts(&view).contains(&"Отправить главу".to_owned()));
}

fn request(path: &str, line: u32, raw: &str) -> TaskToggleRequest {
    TaskToggleRequest {
        path: path.into(),
        line,
        raw: raw.into(),
    }
}

/// Отметка закрытой заметки меняет ровно одну строку; открытой — правка
/// редактора; строка сдвинулась — найдена по тексту; исчезла — `Stale`
/// (FR-002, SC-002, Edge Cases).
pub async fn toggling_changes_exactly_one_line() {
    let mut vault = vault().checking_expected();
    let path = "Диссертация/Введение.typ";
    let outcome = task_toggle(
        &mut vault,
        &request(path, 3, "- [ ] Сверить цель 📅 2026-10-20"),
        None,
    )
    .await;
    assert_eq!(outcome, TaskToggleOutcome::Written);
    let after = String::from_utf8(vault.bytes_of(path).unwrap()).unwrap();
    let changed: Vec<(&str, &str)> = INTRO
        .lines()
        .zip(after.lines())
        .filter(|(old, new)| old != new)
        .collect();
    assert_eq!(
        changed,
        [(
            "- [ ] Сверить цель 📅 2026-10-20",
            "- [x] Сверить цель 📅 2026-10-20"
        )]
    );
    assert_eq!(INTRO.len(), after.len());

    // Открытая заметка: строку сдвинули правкой выше — найдена по тексту.
    let editor = format!("Новая строка\n{after}");
    match task_toggle(
        &mut vault,
        &request(path, 4, "- [x] Собрать литературу"),
        Some(&editor),
    )
    .await
    {
        TaskToggleOutcome::EditorEdit { edit } => {
            assert_eq!(edit.insert, " ");
            assert_eq!(edit.to - edit.from, 1);
        }
        other => panic!("ждали EditorEdit: {other:?}"),
    }

    // Строки больше нет — ничего не меняется.
    assert_eq!(
        task_toggle(&mut vault, &request(path, 3, "- [ ] удалённая"), None).await,
        TaskToggleOutcome::Stale
    );
    let mut read_only = vault.read_only();
    assert_eq!(
        task_toggle(
            &mut read_only,
            &request(path, 5, "- [ ] Дописать актуальность срок: 2026-10-23"),
            None
        )
        .await,
        TaskToggleOutcome::ReadOnly
    );
}

// ── US2 ─────────────────────────────────────────────────────────────────────

/// Фильтры: состояние, срок, папка, условия свойств; группы по сроку (FR-003).
pub async fn filters_and_due_groups() {
    let mut vault = vault();
    let done = tasks_view(
        &mut vault,
        &TaskFilter {
            state: TaskState::Done,
            ..filter()
        },
        TODAY,
    )
    .await;
    assert_eq!(
        texts(&done),
        ["Собрать литературу", "Откалибровать термопары"]
    );

    let due = |due: DueFilter| TaskFilter { due, ..filter() };
    assert_eq!(
        texts(&tasks_view(&mut vault, &due(DueFilter::Overdue), TODAY).await),
        ["Сверить цель"]
    );
    assert_eq!(
        texts(&tasks_view(&mut vault, &due(DueFilter::Today), TODAY).await),
        ["Дописать актуальность"]
    );
    assert_eq!(
        texts(&tasks_view(&mut vault, &due(DueFilter::Week), TODAY).await),
        ["Дописать актуальность", "Пересчитать погрешность"]
    );
    assert_eq!(
        texts(&tasks_view(&mut vault, &due(DueFilter::NoDue), TODAY).await),
        ["Найти три обзора", "Отправить главу"]
    );

    let folder = TaskFilter {
        folder: Some("дневник".into()),
        ..filter()
    };
    assert_eq!(
        texts(&tasks_view(&mut vault, &folder, TODAY).await),
        ["Отправить главу", "Купить бумагу"]
    );

    let tagged = TaskFilter {
        conditions: vec![Condition::TagHas {
            tag: "глава".into(),
        }],
        ..filter()
    };
    assert_eq!(
        texts(&tasks_view(&mut vault, &tagged, TODAY).await),
        ["Пересчитать погрешность"]
    );

    let by_due = tasks_view(
        &mut vault,
        &TaskFilter {
            group: TaskGrouping::ByDue,
            ..filter()
        },
        TODAY,
    )
    .await;
    let keys: Vec<&str> = by_due
        .groups
        .iter()
        .map(|group| group.key.as_str())
        .collect();
    assert_eq!(keys, ["Overdue", "Today", "Week", "Later", "NoDue"]);
    assert_eq!(by_due.groups[0].rows[0].due_state, DueState::Overdue);
}

// ── US3 ─────────────────────────────────────────────────────────────────────

/// «Переключить задачу»: задача ↔ отметка; пункт списка → задача; прочее —
/// ничего.
pub fn toggle_at_caret() {
    let text = "- [ ] один\n- пункт\nтекст\n  + [x] два\n";
    let at = |line: usize| -> u32 {
        text.split_inclusive('\n')
            .take(line)
            .map(|l| l.encode_utf16().count() as u32)
            .sum::<u32>()
            + 1
    };
    let edit = task_toggle_at(text, at(0)).unwrap();
    assert_eq!((edit.insert.as_str(), edit.to - edit.from), ("x", 1));
    let edit = task_toggle_at(text, at(1)).unwrap();
    assert_eq!((edit.insert.as_str(), edit.from, edit.to), ("[ ] ", 13, 13));
    assert!(task_toggle_at(text, at(2)).is_none());
    let edit = task_toggle_at(text, at(3)).unwrap();
    assert_eq!(edit.insert, " ");
}
