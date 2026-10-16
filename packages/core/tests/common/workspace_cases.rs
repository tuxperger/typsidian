//! Вкладки и разделение редактора — проверки, общие для обоих таргетов
//! (фича 014, contracts/core-workspace-api.md).
//!
//! Переходы чистые: состояние и факты собираются в тесте, бэкенд нужен только
//! `facts_of`/`restore_with` (там — `MemoryVault`). Одна таблица на native и
//! wasm32 (принцип II).

#![allow(dead_code)]

use typsidian_core::vault::FileKind;
use typsidian_core::workspace::{
    apply, check_invariants, facts_of, restore, restore_with, snapshot, view, view_with, Direction,
    Facts, GroupSlot, NoteFact, Presentation, RefusalReason, Rejection, TabId, TextPlace,
    Transition, WorkspaceAction as A, WorkspaceState, COMPACT_MAX_WIDTH_PX, HISTORY_LIMIT,
    SPLIT_MIN_WIDTH_PX,
};

use super::MemoryVault;

const WIDE: u32 = 1200;

fn kind_of(path: &str) -> FileKind {
    if path.ends_with(".excalidraw") {
        FileKind::Board
    } else {
        FileKind::Note
    }
}

fn fact(path: &str) -> NoteFact {
    NoteFact {
        path: path.to_owned(),
        kind: kind_of(path),
        unsaved: false,
        conflict: false,
    }
}

fn facts(paths: &[&str], width_px: u32) -> Facts {
    Facts {
        notes: paths.iter().map(|p| fact(p)).collect(),
        width_px,
    }
}

fn with(mut facts: Facts, path: &str, unsaved: bool, conflict: bool) -> Facts {
    for note in &mut facts.notes {
        if note.path == path {
            note.unsaved = unsaved;
            note.conflict = conflict;
        }
    }
    facts
}

fn place(line: u32, column: u32) -> TextPlace {
    TextPlace { line, column }
}

const ALL: &[&str] = &[
    "a.typ",
    "b.typ",
    "c.typ",
    "d.typ",
    "x/n.typ",
    "y/n.typ",
    "z.excalidraw",
];

/// Применяет действие и проверяет инварианты результата.
fn step(state: WorkspaceState, action: A, facts: &Facts) -> Transition {
    let t = apply(state, &action, facts);
    check_invariants(&t.state).unwrap_or_else(|e| panic!("{e} после {action:?}"));
    t
}

fn open(state: WorkspaceState, path: &str) -> WorkspaceState {
    let t = step(
        state,
        A::Open {
            path: path.into(),
            target: None,
            leaving: None,
        },
        &facts(ALL, WIDE),
    );
    assert_eq!(t.rejected, None, "{path}");
    t.state
}

fn opened(paths: &[&str]) -> WorkspaceState {
    paths
        .iter()
        .fold(WorkspaceState::empty(), |s, p| open(s, p))
}

fn paths(state: &WorkspaceState, group: usize) -> Vec<String> {
    state.groups[group]
        .tabs
        .iter()
        .map(|t| t.path.clone())
        .collect()
}

fn active_path(state: &WorkspaceState, group: usize) -> Option<String> {
    let g = &state.groups[group];
    g.tabs
        .iter()
        .find(|t| Some(t.id) == g.active)
        .map(|t| t.path.clone())
}

fn tab_id(state: &WorkspaceState, group: usize, path: &str) -> TabId {
    state.groups[group]
        .tabs
        .iter()
        .find(|t| t.path == path)
        .unwrap()
        .id
}

fn split(state: WorkspaceState) -> WorkspaceState {
    let t = step(state, A::Split { leaving: None }, &facts(ALL, WIDE));
    assert_eq!(t.rejected, None);
    t.state
}

// ── Пустое состояние и вкладки ──────────────────────────────────────────────

pub fn the_empty_state_is_one_empty_group() {
    let s = WorkspaceState::empty();
    assert_eq!(s.groups.len(), 1);
    assert!(s.groups[0].tabs.is_empty());
    assert_eq!(s.groups[0].active, None);
    assert_eq!(check_invariants(&s), Ok(()));
    let v = view(&s, &facts(&[], WIDE));
    assert_eq!(v.active, None);
    assert_eq!(v.tab_count, 0);
}

pub fn open_adds_a_tab_and_reopening_activates_it() {
    let s = opened(&["a.typ", "b.typ"]);
    assert_eq!(paths(&s, 0), ["a.typ", "b.typ"]);
    assert_eq!(active_path(&s, 0).as_deref(), Some("b.typ"));
    let s = open(s, "a.typ");
    assert_eq!(paths(&s, 0), ["a.typ", "b.typ"], "вкладка не удвоилась");
    assert_eq!(active_path(&s, 0).as_deref(), Some("a.typ"));
}

pub fn open_of_a_missing_path_is_not_found() {
    let s = opened(&["a.typ"]);
    let t = step(
        s.clone(),
        A::Open {
            path: "nope.typ".into(),
            target: None,
            leaving: None,
        },
        &facts(ALL, WIDE),
    );
    assert_eq!(t.rejected, Some(Rejection::NotFound));
    assert_eq!(t.state, s);
}

pub fn activate_and_cycle_wrap_in_both_directions() {
    let s = opened(&["a.typ", "b.typ", "c.typ"]);
    let f = facts(ALL, WIDE);
    let a = tab_id(&s, 0, "a.typ");
    let s = step(
        s,
        A::Activate {
            tab: a,
            leaving: None,
        },
        &f,
    )
    .state;
    assert_eq!(active_path(&s, 0).as_deref(), Some("a.typ"));
    let s = step(
        s,
        A::Cycle {
            direction: Direction::Previous,
            leaving: None,
        },
        &f,
    )
    .state;
    assert_eq!(active_path(&s, 0).as_deref(), Some("c.typ"));
    let s = step(
        s,
        A::Cycle {
            direction: Direction::Next,
            leaving: None,
        },
        &f,
    )
    .state;
    assert_eq!(active_path(&s, 0).as_deref(), Some("a.typ"));
}

pub fn move_reorders_within_a_group() {
    let s = opened(&["a.typ", "b.typ", "c.typ"]);
    let a = tab_id(&s, 0, "a.typ");
    let t = step(
        s,
        A::Move {
            tab: a,
            group: GroupSlot::First,
            index: 2,
        },
        &facts(ALL, WIDE),
    );
    assert_eq!(paths(&t.state, 0), ["b.typ", "c.typ", "a.typ"]);
    assert_eq!(
        active_path(&t.state, 0).as_deref(),
        Some("c.typ"),
        "активная прежняя"
    );
}

pub fn an_unknown_tab_is_rejected_without_panic() {
    let s = opened(&["a.typ"]);
    let f = facts(ALL, WIDE);
    let ghost = TabId(999);
    for action in [
        A::Activate {
            tab: ghost,
            leaving: None,
        },
        A::Close {
            tab: ghost,
            board_unsaved: false,
        },
        A::CloseOthers { tab: ghost },
        A::Move {
            tab: ghost,
            group: GroupSlot::First,
            index: 0,
        },
        A::MoveToOtherGroup { tab: ghost },
    ] {
        let t = step(s.clone(), action, &f);
        assert_eq!(t.rejected, Some(Rejection::UnknownTab));
        assert_eq!(t.state, s);
    }
    // Действия в пустом состоянии.
    let empty = WorkspaceState::empty();
    for action in [
        A::Back { leaving: None },
        A::Forward { leaving: None },
        A::Unsplit,
    ] {
        assert!(step(empty.clone(), action, &f).rejected.is_some());
    }
    let _ = step(
        empty,
        A::Cycle {
            direction: Direction::Next,
            leaving: None,
        },
        &f,
    );
}

// ── Закрытие ────────────────────────────────────────────────────────────────

pub fn closing_the_active_tab_activates_the_most_recent_one() {
    let s = opened(&["a.typ", "b.typ", "c.typ"]);
    let f = facts(ALL, WIDE);
    let b = tab_id(&s, 0, "b.typ");
    let s = step(
        s,
        A::Activate {
            tab: b,
            leaving: None,
        },
        &f,
    )
    .state;
    let b_id = b;
    let t = step(
        s,
        A::Close {
            tab: b_id,
            board_unsaved: false,
        },
        &f,
    );
    assert_eq!(t.closed, vec![b_id]);
    assert_eq!(
        active_path(&t.state, 0).as_deref(),
        Some("c.typ"),
        "предыдущая по давности"
    );
    assert_eq!(t.focus.as_ref().map(|f| f.path.as_str()), Some("c.typ"));
}

pub fn unsaved_and_conflicting_tabs_refuse_to_close() {
    let s = opened(&["a.typ", "b.typ"]);
    let a = tab_id(&s, 0, "a.typ");
    let base = facts(ALL, WIDE);

    let f = with(base.clone(), "a.typ", true, false);
    let t = step(
        s.clone(),
        A::Close {
            tab: a,
            board_unsaved: false,
        },
        &f,
    );
    assert_eq!(t.refused.len(), 1);
    assert_eq!(t.refused[0].reason, RefusalReason::Unsaved);
    assert_eq!(t.state, s);
    assert!(t.closed.is_empty());

    let f = with(base, "a.typ", true, true);
    let t = step(
        s,
        A::Close {
            tab: a,
            board_unsaved: false,
        },
        &f,
    );
    assert_eq!(t.refused[0].reason, RefusalReason::Conflict);
    assert_eq!(
        active_path(&t.state, 0).as_deref(),
        Some("a.typ"),
        "вкладка конфликта активна"
    );
    assert_eq!(t.focus.as_ref().map(|f| f.tab), Some(a));
}

pub fn an_unsaved_board_refuses_to_close() {
    let s = opened(&["a.typ", "z.excalidraw"]);
    let z = tab_id(&s, 0, "z.excalidraw");
    let f = facts(ALL, WIDE);
    let t = step(
        s.clone(),
        A::Close {
            tab: z,
            board_unsaved: true,
        },
        &f,
    );
    assert_eq!(t.refused[0].reason, RefusalReason::Unsaved);
    assert_eq!(t.state, s);
    let t = step(
        s,
        A::Close {
            tab: z,
            board_unsaved: false,
        },
        &f,
    );
    assert!(t.refused.is_empty());
    assert_eq!(paths(&t.state, 0), ["a.typ"]);
}

pub fn close_others_closes_clean_tabs_and_lists_refusals() {
    let s = opened(&["a.typ", "b.typ", "c.typ", "d.typ"]);
    let keep = tab_id(&s, 0, "b.typ");
    let f = with(facts(ALL, WIDE), "d.typ", true, false);
    let t = step(s, A::CloseOthers { tab: keep }, &f);
    assert_eq!(paths(&t.state, 0), ["b.typ", "d.typ"]);
    assert_eq!(t.refused.len(), 1);
    assert_eq!(t.refused[0].path, "d.typ");
    assert_eq!(t.closed.len(), 2);
    assert_eq!(active_path(&t.state, 0).as_deref(), Some("b.typ"));
}

pub fn closing_the_last_tab_leaves_an_empty_group() {
    let s = opened(&["a.typ"]);
    let a = tab_id(&s, 0, "a.typ");
    let t = step(
        s,
        A::Close {
            tab: a,
            board_unsaved: false,
        },
        &facts(ALL, WIDE),
    );
    assert_eq!(t.state.groups.len(), 1);
    assert!(t.state.groups[0].tabs.is_empty());
    assert_eq!(t.state.groups[0].active, None);
    assert_eq!(t.focus, None);
}

// ── События vault и показ ───────────────────────────────────────────────────

pub fn renamed_keeps_the_tab_id_and_deleted_closes_the_tab() {
    let s = opened(&["a.typ", "b.typ"]);
    let a = tab_id(&s, 0, "a.typ");
    let f = facts(ALL, WIDE);
    let s = step(
        s,
        A::Renamed {
            from: "a.typ".into(),
            to: "c.typ".into(),
        },
        &f,
    )
    .state;
    assert_eq!(tab_id(&s, 0, "c.typ"), a);
    assert!(s.groups[0]
        .history
        .entries
        .iter()
        .all(|e| e.path != "a.typ"));

    let t = step(
        s.clone(),
        A::Deleted {
            path: "c.typ".into(),
        },
        &f,
    );
    assert_eq!(paths(&t.state, 0), ["b.typ"]);
    assert_eq!(t.closed, vec![a]);

    // Удаление с конфликтом вкладку оставляет.
    let f = with(f, "c.typ", true, true);
    let t = step(
        s.clone(),
        A::Deleted {
            path: "c.typ".into(),
        },
        &f,
    );
    assert_eq!(t.state, s);
}

pub fn view_titles_hints_and_flags() {
    let s = opened(&["a.typ", "x/n.typ", "y/n.typ", "z.excalidraw"]);
    let f = with(
        with(facts(ALL, WIDE), "a.typ", true, false),
        "y/n.typ",
        false,
        true,
    );
    let before = s.clone();
    let v = view(&s, &f);
    assert_eq!(s, before, "view состояние не меняет");
    let tabs = &v.groups[0].tabs;
    assert_eq!(tabs[0].title, "a");
    assert_eq!(tabs[0].hint, None);
    assert!(tabs[0].unsaved && !tabs[0].conflict);
    assert_eq!(tabs[1].title, "n");
    assert_eq!(tabs[1].hint.as_deref(), Some("x"));
    assert_eq!(tabs[2].hint.as_deref(), Some("y"));
    assert!(tabs[2].conflict);
    assert_eq!(tabs[3].kind, FileKind::Board);
    assert!(tabs[3].active);
    assert_eq!(v.tab_count, 4);
    let active = v.active.unwrap();
    assert_eq!(
        (active.path.as_str(), active.name.as_str()),
        ("z.excalidraw", "z.excalidraw")
    );
    assert_eq!(v.presentation, Presentation::Single);
}

// ── История ─────────────────────────────────────────────────────────────────

fn back(s: WorkspaceState, leaving: Option<TextPlace>) -> Transition {
    step(s, A::Back { leaving }, &facts(ALL, WIDE))
}

pub fn back_and_forward_return_to_the_place_of_leaving() {
    let f = facts(ALL, WIDE);
    let mut s = open(WorkspaceState::empty(), "a.typ");
    s = step(
        s,
        A::Open {
            path: "b.typ".into(),
            target: None,
            leaving: Some(place(3, 4)),
        },
        &f,
    )
    .state;
    s = step(
        s,
        A::Open {
            path: "c.typ".into(),
            target: None,
            leaving: Some(place(5, 6)),
        },
        &f,
    )
    .state;
    assert!(view(&s, &f).groups[0].can_back);
    assert!(!view(&s, &f).groups[0].can_forward);

    let t = back(s, Some(place(9, 9)));
    assert_eq!(
        t.focus.as_ref().map(|x| (x.path.as_str(), x.place)),
        Some(("b.typ", Some(place(5, 6))))
    );
    let t = back(t.state, None);
    assert_eq!(t.focus.as_ref().map(|x| x.path.as_str()), Some("a.typ"));
    let t = back(t.state, None);
    assert_eq!(t.rejected, Some(Rejection::NoHistory));

    let t = step(t.state, A::Forward { leaving: None }, &f);
    assert_eq!(t.focus.as_ref().map(|x| x.path.as_str()), Some("b.typ"));
    let t = step(t.state, A::Forward { leaving: None }, &f);
    // Место ухода с `c` запомнено при первом «Назад».
    assert_eq!(
        t.focus.as_ref().map(|x| (x.path.as_str(), x.place)),
        Some(("c.typ", Some(place(9, 9))))
    );
}

pub fn open_after_back_drops_the_forward_branch() {
    let s = opened(&["a.typ", "b.typ", "c.typ"]);
    let s = back(s, None).state;
    let s = open(s, "d.typ");
    let v = view(&s, &facts(ALL, WIDE));
    assert!(!v.groups[0].can_forward);
    assert!(v.groups[0].can_back);
}

pub fn history_reopens_closed_and_skips_deleted() {
    let f = facts(ALL, WIDE);
    let s = opened(&["a.typ", "b.typ", "c.typ"]);
    // Закрытая открывается заново.
    let b = tab_id(&s, 0, "b.typ");
    let s = step(
        s,
        A::Close {
            tab: b,
            board_unsaved: false,
        },
        &f,
    )
    .state;
    let s = back(s, None).state;
    assert!(paths(&s, 0).contains(&"b.typ".to_owned()));
    assert_eq!(active_path(&s, 0).as_deref(), Some("b.typ"));

    // Удалённая по фактам пропускается.
    let s = opened(&["a.typ", "b.typ", "c.typ"]);
    let missing = facts(&["a.typ", "c.typ"], WIDE);
    let t = step(s, A::Back { leaving: None }, &missing);
    assert_eq!(t.focus.as_ref().map(|x| x.path.as_str()), Some("a.typ"));

    // Удалённая по событию вычищается из истории.
    let s = opened(&["a.typ", "b.typ", "c.typ"]);
    let s = step(
        s,
        A::Deleted {
            path: "b.typ".into(),
        },
        &f,
    )
    .state;
    assert!(s.groups[0]
        .history
        .entries
        .iter()
        .all(|e| e.path != "b.typ"));
}

pub fn open_with_a_target_in_the_active_note_adds_an_entry() {
    let f = facts(ALL, WIDE);
    let s = opened(&["a.typ"]);
    let s = step(
        s,
        A::Open {
            path: "a.typ".into(),
            target: Some(place(10, 1)),
            leaving: Some(place(1, 1)),
        },
        &f,
    )
    .state;
    assert_eq!(s.groups[0].tabs.len(), 1);
    let t = back(s, None);
    assert_eq!(t.focus.as_ref().map(|x| x.place), Some(Some(place(1, 1))));
    // Активация уже активной вкладки записи не добавляет.
    let s = opened(&["a.typ"]);
    let a = tab_id(&s, 0, "a.typ");
    let s = step(
        s,
        A::Activate {
            tab: a,
            leaving: None,
        },
        &f,
    )
    .state;
    assert_eq!(s.groups[0].history.entries.len(), 1);
}

pub fn history_is_bounded() {
    let f = facts(ALL, WIDE);
    let mut s = opened(&["a.typ", "b.typ"]);
    for i in 0..(HISTORY_LIMIT + 20) {
        let path = if i % 2 == 0 { "a.typ" } else { "b.typ" };
        let id = tab_id(&s, 0, path);
        s = step(
            s,
            A::Activate {
                tab: id,
                leaving: None,
            },
            &f,
        )
        .state;
    }
    assert!(s.groups[0].history.entries.len() <= HISTORY_LIMIT);
}

pub fn back_from_an_empty_group_returns_the_last_closed() {
    let f = facts(ALL, WIDE);
    let s = opened(&["a.typ"]);
    let a = tab_id(&s, 0, "a.typ");
    let s = step(
        s,
        A::Close {
            tab: a,
            board_unsaved: false,
        },
        &f,
    )
    .state;
    assert!(view(&s, &f).groups[0].can_back);
    let t = back(s, None);
    assert_eq!(paths(&t.state, 0), ["a.typ"]);
}

// ── Группы ──────────────────────────────────────────────────────────────────

pub fn split_duplicates_the_note_and_moves_focus() {
    let s = split(opened(&["a.typ", "b.typ"]));
    assert_eq!(s.groups.len(), 2);
    assert_eq!(paths(&s, 1), ["b.typ"]);
    assert_eq!(paths(&s, 0), ["a.typ", "b.typ"]);
    assert_ne!(s.groups[0].tabs[1].id, s.groups[1].tabs[0].id);
    assert_eq!(s.active_group, GroupSlot::Second);
}

pub fn split_moves_a_board_and_refuses_when_nothing_to_split() {
    let f = facts(ALL, WIDE);
    let s = split(opened(&["a.typ", "z.excalidraw"]));
    assert_eq!(paths(&s, 0), ["a.typ"]);
    assert_eq!(paths(&s, 1), ["z.excalidraw"]);
    // Доска не открывается во второй группе повторно: фокус уходит к ней.
    let s2 = step(
        s.clone(),
        A::FocusGroup {
            group: GroupSlot::First,
            leaving: None,
        },
        &f,
    )
    .state;
    let s2 = open(s2, "z.excalidraw");
    assert_eq!(s2.groups[0].tabs.len(), 1);
    assert_eq!(s2.active_group, GroupSlot::Second);

    let only = opened(&["z.excalidraw"]);
    let t = step(only, A::Split { leaving: None }, &f);
    assert_eq!(t.rejected, Some(Rejection::NothingToSplit));
    assert!(!view(&opened(&["z.excalidraw"]), &f).can_split);

    let t = step(
        opened(&["a.typ"]),
        A::Split { leaving: None },
        &facts(ALL, SPLIT_MIN_WIDTH_PX - 1),
    );
    assert_eq!(t.rejected, Some(Rejection::TooNarrow));
}

pub fn moving_between_groups_keeps_the_tab_id_and_collapses_empty_groups() {
    let f = facts(ALL, WIDE);
    let s = split(opened(&["a.typ", "b.typ"]));
    let a = tab_id(&s, 0, "a.typ");
    let t = step(s, A::MoveToOtherGroup { tab: a }, &f);
    assert_eq!(paths(&t.state, 1), ["b.typ", "a.typ"]);
    assert_eq!(tab_id(&t.state, 1, "a.typ"), a);

    // Перенос последней вкладки группы убирает разделение.
    let b0 = tab_id(&t.state, 0, "b.typ");
    let t = step(
        t.state,
        A::Move {
            tab: b0,
            group: GroupSlot::Second,
            index: 0,
        },
        &f,
    );
    assert_eq!(t.state.groups.len(), 1);
    // На уже открытый путь — активация существующей.
    assert_eq!(paths(&t.state, 0).len(), 2);

    let t = step(
        opened(&["a.typ"]),
        A::MoveToOtherGroup { tab: TabId(1) },
        &f,
    );
    assert_eq!(t.rejected, Some(Rejection::SingleGroup));
}

pub fn closing_the_last_tab_of_a_group_removes_the_split() {
    let f = facts(ALL, WIDE);
    let s = split(opened(&["a.typ"]));
    let second = tab_id(&s, 1, "a.typ");
    let t = step(
        s.clone(),
        A::Close {
            tab: second,
            board_unsaved: false,
        },
        &f,
    );
    assert_eq!(t.state.groups.len(), 1);
    assert_eq!(t.state.active_group, GroupSlot::First);

    // Первая опустела при непустой второй: вторая становится первой.
    let s = split(opened(&["a.typ", "b.typ"]));
    let first_b = tab_id(&s, 0, "b.typ");
    let first_a = tab_id(&s, 0, "a.typ");
    let s = step(
        s,
        A::Close {
            tab: first_a,
            board_unsaved: false,
        },
        &f,
    )
    .state;
    let s = step(
        s,
        A::Close {
            tab: first_b,
            board_unsaved: false,
        },
        &f,
    )
    .state;
    assert_eq!(s.groups.len(), 1);
    assert_eq!(paths(&s, 0), ["b.typ"]);
}

pub fn unsplit_merges_without_duplicates() {
    let f = facts(ALL, WIDE);
    let s = split(opened(&["a.typ", "b.typ"]));
    let s = open(s, "c.typ");
    let t = step(s, A::Unsplit, &f);
    assert_eq!(t.state.groups.len(), 1);
    assert_eq!(paths(&t.state, 0), ["a.typ", "b.typ", "c.typ"]);
    assert_eq!(t.closed.len(), 1, "дубликат b закрыт");
    assert_eq!(active_path(&t.state, 0).as_deref(), Some("c.typ"));
    let t = step(t.state, A::Unsplit, &f);
    assert_eq!(t.rejected, Some(Rejection::SingleGroup));
}

pub fn focus_group_and_ratio() {
    let f = facts(ALL, WIDE);
    let s = split(opened(&["a.typ"]));
    let s = step(
        s,
        A::FocusGroup {
            group: GroupSlot::First,
            leaving: None,
        },
        &f,
    )
    .state;
    assert_eq!(view(&s, &f).active_group, GroupSlot::First);
    let s = step(s, A::SetRatio { permille: 50 }, &f).state;
    assert_eq!(s.ratio_permille, 200);
    let s = step(s, A::SetRatio { permille: 990 }, &f).state;
    assert_eq!(s.ratio_permille, 800);
    let t = step(
        opened(&["a.typ"]),
        A::FocusGroup {
            group: GroupSlot::Second,
            leaving: None,
        },
        &f,
    );
    assert_eq!(t.rejected, Some(Rejection::SingleGroup));
}

pub fn a_tab_open_in_the_other_group_closes_despite_unsaved() {
    let s = split(opened(&["a.typ"]));
    let second = tab_id(&s, 1, "a.typ");
    let f = with(facts(ALL, WIDE), "a.typ", true, false);
    let t = step(
        s,
        A::Close {
            tab: second,
            board_unsaved: false,
        },
        &f,
    );
    assert!(t.refused.is_empty());
    assert_eq!(t.state.groups.len(), 1);
}

pub fn every_group_has_its_own_history() {
    let f = facts(ALL, WIDE);
    let s = split(opened(&["a.typ", "b.typ"]));
    let s = open(s, "c.typ");
    assert_eq!(s.groups[1].history.entries.len(), 2);
    assert_eq!(s.groups[0].history.entries.len(), 2);
    let s = step(
        s,
        A::FocusGroup {
            group: GroupSlot::First,
            leaving: None,
        },
        &f,
    )
    .state;
    let t = step(s, A::Back { leaving: None }, &f);
    assert_eq!(
        t.focus.as_ref().map(|x| (x.group, x.path.as_str())),
        Some((GroupSlot::First, "a.typ"))
    );
    assert_eq!(
        active_path(&t.state, 1).as_deref(),
        Some("c.typ"),
        "вторая группа не тронута"
    );
}

// ── Подача ──────────────────────────────────────────────────────────────────

pub fn narrow_windows_merge_the_strips_without_touching_the_state() {
    let s = split(opened(&["a.typ", "b.typ"]));
    let s = open(s, "c.typ");
    let wide = view(&s, &facts(ALL, WIDE));
    assert_eq!(wide.presentation, Presentation::Split);
    assert_eq!(wide.groups.len(), 2);
    assert!(wide.can_unsplit && wide.can_move_to_other && !wide.can_split);

    let before = s.clone();
    let narrow = view(&s, &facts(ALL, SPLIT_MIN_WIDTH_PX - 1));
    assert_eq!(s, before);
    assert_eq!(narrow.presentation, Presentation::Single);
    assert_eq!(narrow.groups.len(), 1);
    let shown: Vec<_> = narrow.groups[0]
        .tabs
        .iter()
        .map(|t| t.path.as_str())
        .collect();
    assert_eq!(shown, ["a.typ", "b.typ", "c.typ"], "b показана один раз");
    assert!(!narrow.can_split);
    assert_eq!(narrow.tab_count, 3);

    let back_wide = view(&s, &facts(ALL, WIDE));
    assert_eq!(back_wide, wide);
}

pub fn compact_width_gives_the_list_presentation() {
    let s = opened(&["a.typ"]);
    assert_eq!(
        view(&s, &facts(ALL, COMPACT_MAX_WIDTH_PX)).presentation,
        Presentation::List
    );
    assert_eq!(
        view(&s, &facts(ALL, COMPACT_MAX_WIDTH_PX + 1)).presentation,
        Presentation::Single
    );
}

pub fn closing_in_the_merged_presentation_closes_both_tabs() {
    let s = split(opened(&["a.typ", "b.typ"]));
    let narrow = facts(ALL, 800);
    let b = tab_id(&s, 0, "b.typ");
    let t = step(
        s.clone(),
        A::Close {
            tab: b,
            board_unsaved: false,
        },
        &narrow,
    );
    assert_eq!(t.closed.len(), 2);
    assert_eq!(t.state.groups.len(), 1);
    assert_eq!(paths(&t.state, 0), ["a.typ"]);
    // В слитой подаче несохранённое отказывает, даже если путь есть в обеих.
    let f = with(narrow, "b.typ", true, false);
    let t = step(
        s,
        A::Close {
            tab: b,
            board_unsaved: false,
        },
        &f,
    );
    assert_eq!(t.refused.len(), 1);
}

// ── Сохранённый набор ───────────────────────────────────────────────────────

fn notes_of(paths: &[&str]) -> Vec<typsidian_core::vault::index::Note> {
    use typsidian_core::vault::index::Note;
    paths
        .iter()
        .map(|p| {
            Note::new(
                std::path::PathBuf::from(p),
                typsidian_core::stamp::FileStamp::from_content(0, b"x"),
            )
        })
        .collect()
}

pub fn snapshot_and_restore_round_trip() {
    let s = split(opened(&["a.typ", "b.typ", "z.excalidraw"]));
    let s = open(s, "c.typ");
    let saved = snapshot(&s);
    let r = restore(&saved, &notes_of(ALL));
    assert_eq!(check_invariants(&r), Ok(()));
    for g in 0..2 {
        assert_eq!(paths(&r, g), paths(&s, g));
        assert_eq!(active_path(&r, g), active_path(&s, g));
    }
    assert_eq!(r.active_group, s.active_group);
    assert_eq!(r.ratio_permille, s.ratio_permille);
    assert!(r.groups.iter().all(|g| g.history.entries.is_empty()));
    assert_eq!(snapshot(&r), saved);
}

pub fn restore_repairs_a_bad_set() {
    use typsidian_core::workspace::{SavedGroup, SavedWorkspace};
    let saved = SavedWorkspace {
        groups: vec![
            SavedGroup {
                tabs: vec!["gone.typ".into()],
                active: Some(0),
            },
            SavedGroup {
                tabs: vec!["a.typ".into(), "a.typ".into(), "b.typ".into()],
                active: Some(7),
            },
            SavedGroup {
                tabs: vec!["c.typ".into()],
                active: None,
            },
            SavedGroup {
                tabs: vec!["d.typ".into()],
                active: None,
            },
        ],
        active_group: GroupSlot::Second,
        ratio_permille: 5,
    };
    let r = restore(&saved, &notes_of(ALL));
    assert_eq!(check_invariants(&r), Ok(()));
    assert_eq!(r.groups.len(), 2, "пустая убрана, лишние отброшены");
    assert_eq!(paths(&r, 0), ["a.typ", "b.typ"]);
    assert_eq!(
        active_path(&r, 0).as_deref(),
        Some("a.typ"),
        "active вне диапазона"
    );
    assert_eq!(paths(&r, 1), ["c.typ"]);
    assert_eq!(r.ratio_permille, 200);
    assert_eq!(
        r.active_group,
        GroupSlot::First,
        "активной была вторая сохранённая — она стала первой"
    );

    let empty = restore(
        &SavedWorkspace {
            groups: vec![],
            active_group: GroupSlot::First,
            ratio_permille: 500,
        },
        &[],
    );
    assert_eq!(empty, WorkspaceState::empty());
}

// ── Факты из бэкенда ────────────────────────────────────────────────────────

pub async fn facts_of_reads_the_index() {
    let vault = MemoryVault::new()
        .with_file("a.typ", "a")
        .with_file("z.excalidraw", "{}")
        .with_unsaved_edits("a.typ");
    let s = opened(&["a.typ", "b.typ", "z.excalidraw"]);
    let f = facts_of(&vault, &s, None, 1000).await;
    assert_eq!(f.width_px, 1000);
    assert!(f.note("b.typ").is_none(), "отсутствующая не попадает");
    assert!(f.note("a.typ").unwrap().unsaved);
    // Несохранённые правки — ещё не конфликт: вкладка не должна мигать
    // «конфликтом» до автосохранения (020 T026).
    assert!(!f.note("a.typ").unwrap().conflict);
    assert_eq!(f.note("z.excalidraw").unwrap().kind, FileKind::Board);

    let open_b = A::Open {
        path: "b.typ".into(),
        target: None,
        leaving: None,
    };
    let vault = vault.with_file("b.typ", "b");
    let f = facts_of(&vault, &WorkspaceState::empty(), Some(&open_b), 1000).await;
    assert!(f.note("b.typ").is_some(), "путь действия назван");

    let v = view_with(&vault, &s, 1000).await;
    assert_eq!(v.groups[0].tabs.iter().filter(|t| t.unsaved).count(), 1);
    assert!(
        v.groups[0].tabs.iter().all(|t| !t.conflict),
        "ни одна вкладка не конфликтная"
    );

    // Файла нет, а правки остались (удаление с правками): это конфликт, и вкладка
    // не пропадает из фактов (research.md §7 фичи 014).
    let gone = MemoryVault::new().with_unsaved_edits("c.typ");
    let f = facts_of(&gone, &opened(&["c.typ"]), None, 1000).await;
    let fact = f.note("c.typ").expect("путь с правками без файла остаётся");
    assert!(fact.conflict && fact.unsaved);
}

pub async fn restore_with_skips_missing_notes() {
    let vault = MemoryVault::new().with_file("a.typ", "a");
    let s = opened(&["a.typ", "b.typ"]);
    let r = restore_with(&vault, &snapshot(&s)).await;
    assert_eq!(paths(&r, 0), ["a.typ"]);
}
