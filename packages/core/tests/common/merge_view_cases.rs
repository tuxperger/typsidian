//! Пометки строк, участки с происхождением и сборка результата слияния
//! (фича 040). Общие случаи для `merge_view_native.rs` и `merge_view_wasm.rs`.

use typsidian_core::sync::merge::{
    merge3, merge_annotate, merge_view, Hunk, LineOrigin, MergeResult, Pick, Segment,
};

use LineOrigin::{Both, Manual, Ours, Same, Theirs};

/// Пример по мотивам макета (экран 1a): своя правит этапы, нагрузку и таблицу,
/// их — выборку, нагрузку, таблицу и подпись. Соседние правки разных сторон
/// по правилу 011 спорят, поэтому этапы и выборка разделены пустой строкой.
const BASE: &str = "= Глава 2. Методика\n\
#set par(justify: true)\n\
Эксперимент проводился в три этапа.\n\
\n\
Выборка: 40 образцов стали 09Г2С.\n\
\n\
Нагрузка менялась от 10 до 50 кН.\n\
\n\
Результаты сведены в @tab:results.\n\
#figure(\n\
  table(columns: 3, ..rows),\n\
  caption: [Результаты испытаний],\n\
) <tab:results>\n";

fn ours() -> String {
    BASE.replace("в три этапа", "в четыре этапа")
        .replace("от 10 до 50 кН", "от 10 до 60 кН")
        .replace("table(columns: 3, ..rows),\n", "results-table,\n")
}

fn theirs() -> String {
    BASE.replace("40 образцов", "42 образца")
        .replace("от 10 до 50 кН", "от 5 до 50 кН")
        .replace("columns: 3", "columns: 4")
        .replace("[Результаты испытаний]", "[Результаты испытаний стали]")
}

fn disputed(segments: &[Segment]) -> usize {
    segments
        .iter()
        .filter(|segment| matches!(segment, Segment::Disputed { .. }))
        .count()
}

pub fn disputed_segments_match_conflict_hunks() {
    let (ours, theirs) = (ours(), theirs());
    let view = merge_view(Some(BASE), &ours, &theirs).unwrap();
    let segments = view.segments.expect("общая версия есть");
    let MergeResult::Conflicted { hunks } = merge3(BASE, &ours, &theirs) else {
        panic!("ожидался конфликт");
    };
    let conflicts = hunks
        .iter()
        .filter(|hunk| matches!(hunk, Hunk::Conflict { .. }))
        .count();
    assert_eq!(disputed(&segments), conflicts);
    assert!(conflicts >= 2);
    // Своя сторона во всех спорных — тот же текст, что `text_preferring(true)`.
    let picks = vec![Pick::Ours; conflicts];
    assert_eq!(
        merge_annotate(&segments, &picks, None).text,
        merge3(BASE, &ours, &theirs).text_preferring(true)
    );
}

pub fn columns_mark_each_side_against_the_base() {
    let view = merge_view(Some(BASE), &ours(), &theirs()).unwrap();
    // Строки считаются с нуля.
    assert_eq!(view.ours[2], Ours, "в четыре этапа");
    assert_eq!(view.ours[4], Same, "выборку своя не трогала");
    assert_eq!(view.ours[6], Ours);
    assert_eq!(view.theirs[4], Theirs, "42 образца");
    assert_eq!(view.theirs[2], Same);
    let base = view.base.expect("общая версия есть");
    assert_eq!(base[2], Ours);
    assert_eq!(base[4], Theirs);
    assert_eq!(base[6], Both, "нагрузку правили обе");
    assert_eq!(base[0], Same);
    assert_eq!(view.ours.len(), ours().lines().count());
    assert_eq!(view.theirs.len(), theirs().lines().count());
}

pub fn agreed_segments_remember_their_origin() {
    let view = merge_view(Some(BASE), &ours(), &theirs()).unwrap();
    let segments = view.segments.unwrap();
    let origin_of = |needle: &str| {
        segments.iter().find_map(|segment| match segment {
            Segment::Agreed { text, origin } if text.contains(needle) => Some(*origin),
            _ => None,
        })
    };
    assert_eq!(origin_of("четыре этапа"), Some(Ours));
    assert_eq!(origin_of("42 образца"), Some(Theirs));
    assert_eq!(origin_of("= Глава 2"), Some(Same));
}

pub fn identical_edits_are_both() {
    let same = BASE.replace("в три этапа", "в пять этапов");
    let view = merge_view(Some(BASE), &same, &same).unwrap();
    let segments = view.segments.unwrap();
    assert!(segments.iter().any(|segment| matches!(
        segment,
        Segment::Agreed { text, origin: Both } if text.contains("пять")
    )));
    assert_eq!(disputed(&segments), 0);
}

pub fn without_base_sides_are_compared_to_each_other() {
    let view = merge_view(None, "а\nб\nв\n", "а\nX\nв\nг\n").unwrap();
    assert_eq!(view.ours, vec![Same, Ours, Same]);
    assert_eq!(view.theirs, vec![Same, Theirs, Same, Theirs]);
    assert!(view.base.is_none() && view.segments.is_none());
}

pub fn trailing_whitespace_counts_as_a_change() {
    let view = merge_view(Some("а\nб\n"), "а \nб\n", "а\nб\n").unwrap();
    assert_eq!(view.ours, vec![Ours, Same]);
}

pub fn picks_assemble_the_result_with_origins() {
    let segments = vec![
        Segment::Agreed {
            text: "общая\n".into(),
            origin: Same,
        },
        Segment::Disputed {
            base: "б\n".into(),
            ours: "своё\n".into(),
            theirs: "их\n".into(),
        },
        Segment::Agreed {
            text: "правка их\n".into(),
            origin: Theirs,
        },
    ];
    let theirs = merge_annotate(&segments, &[Pick::Theirs], None);
    assert_eq!(theirs.text, "общая\nих\nправка их\n");
    assert_eq!(theirs.lines, vec![Same, Theirs, Theirs]);
    let both = merge_annotate(&segments, &[Pick::Both], None);
    assert_eq!(both.text, "общая\nсвоё\nих\nправка их\n");
    assert_eq!(both.lines, vec![Same, Ours, Theirs, Theirs]);
    // Выбора нет — своё.
    assert_eq!(merge_annotate(&segments, &[], None).lines[1], Ours);
}

pub fn both_without_final_newline_keeps_lines_apart() {
    let segments = vec![Segment::Disputed {
        base: "б".into(),
        ours: "своё".into(),
        theirs: "их".into(),
    }];
    let both = merge_annotate(&segments, &[Pick::Both], None);
    assert_eq!(both.text, "своё\nих");
    assert_eq!(both.lines, vec![Ours, Theirs]);
}

pub fn manual_edit_marks_only_touched_lines() {
    let segments = vec![
        Segment::Agreed {
            text: "общая\n".into(),
            origin: Same,
        },
        Segment::Disputed {
            base: "б\n".into(),
            ours: "своё\n".into(),
            theirs: "их\n".into(),
        },
        Segment::Agreed {
            text: "хвост\n".into(),
            origin: Ours,
        },
    ];
    let edited = merge_annotate(
        &segments,
        &[Pick::Theirs],
        Some("общая\nих, поправлено\nновая\nхвост\n"),
    );
    assert_eq!(edited.text, "общая\nих, поправлено\nновая\nхвост\n");
    assert_eq!(edited.lines, vec![Same, Manual, Manual, Ours]);
    let removed = merge_annotate(&segments, &[Pick::Theirs], Some("общая\nхвост\n"));
    assert_eq!(removed.lines, vec![Same, Ours]);
    assert!(merge_annotate(&segments, &[], Some("")).lines.is_empty());
}

pub fn five_thousand_lines_stay_consistent() {
    let base: String = (0..5_000).map(|n| format!("строка {n}\n")).collect();
    let ours = base.replace("строка 10\n", "своя 10\n");
    let theirs = base
        .replace("строка 10\n", "их 10\n")
        .replace("строка 4000\n", "их 4000\n");
    let view = merge_view(Some(&base), &ours, &theirs).unwrap();
    assert_eq!(view.ours.len(), 5_000);
    let segments = view.segments.unwrap();
    assert_eq!(disputed(&segments), 1);
    let result = merge_annotate(&segments, &[Pick::Theirs], None);
    assert_eq!(result.lines.len(), 5_000);
    assert_eq!(result.lines[10], Theirs);
    assert_eq!(result.lines[4000], Theirs);
    assert_eq!(result.lines[0], Same);
}

pub fn versions_over_the_limit_get_no_line_diff() {
    use typsidian_core::sync::merge::LINE_LIMIT;
    let long: String = (0..=LINE_LIMIT).map(|n| format!("{n}\n")).collect();
    assert!(merge_view(None, &long, "а\n").is_none());
    assert!(merge_view(Some("а\n"), "а\n", &long).is_none());
    // Правка сверх предела не сравнивается: всё правленое — «вручную».
    let segments = vec![Segment::Agreed {
        text: "общая\n".into(),
        origin: Same,
    }];
    let edited = merge_annotate(&segments, &[], Some(&long));
    assert_eq!(edited.lines.len(), LINE_LIMIT + 1);
    assert!(edited.lines.iter().all(|line| *line == Manual));
}
