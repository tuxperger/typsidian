//! Свойства заметок и выборки — общая таблица для обоих таргетов (фича 020).
//!
//! Разбор, правка, отбор и блок выборки платформенно нейтральны и проверяются
//! на тексте и на `MemoryVault`; кэш свойств — через трейт `IndexCache`, на
//! который каждый раннер подставляет свою реализацию (SQLite или память).
//! Файлы `properties_native.rs` и `properties_wasm.rs` только вызывают эти
//! функции своим harness'ом (принцип II).

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use typsidian_core::elements::{EditOutcome, PlanEditError, TextEdit};
use typsidian_core::export;
use typsidian_core::preview::{
    compile_note, compile_note_resolved, CompileOutcome, FontSet, PackageCache,
};
use typsidian_core::properties::{
    self, edit, read, run_selection, select_blocks, select_import_edits, select_inputs,
    select_library, CompareOp, Condition, DateRange, IndexedNote, LibraryStatus, PropertiesProblem,
    PropertyEdit, PropertyEntry, PropertyValue, SelectView, Selection, SortKey, Ymd,
};
use typsidian_core::search::{parse_conditions, search, SearchQuery};
use typsidian_core::vault::cache::IndexCache;
use typsidian_core::vault::relocate::MoveControl;
use typsidian_core::vault::vault_settings::VaultOverrides;

use super::MemoryVault;

const MAIN: &str = "заметка.typ";
const DAY: i64 = 86_400_000;

// ── Помощники ───────────────────────────────────────────────────────────────

/// Применяет замены (UTF-16, относятся к исходному тексту).
fn apply(text: &str, edits: &[TextEdit]) -> String {
    let mut units: Vec<u16> = text.encode_utf16().collect();
    let mut sorted: Vec<&TextEdit> = edits.iter().collect();
    sorted.sort_by_key(|e| std::cmp::Reverse(e.from));
    for e in sorted {
        units.splice(e.from..e.to, e.insert.encode_utf16());
    }
    String::from_utf16(&units).unwrap()
}

fn edits_of(text: &str, change: &PropertyEdit) -> Vec<TextEdit> {
    match edit(text, change) {
        EditOutcome::Planned { edits } => edits,
        EditOutcome::Refused { error } => panic!("правка отклонена: {error:?}"),
    }
}

fn edited(text: &str, change: &PropertyEdit) -> String {
    apply(text, &edits_of(text, change))
}

fn set(key: &str, value: PropertyValue) -> PropertyEdit {
    PropertyEdit::Set {
        key: key.to_string(),
        value,
    }
}

fn remove(key: &str) -> PropertyEdit {
    PropertyEdit::Remove {
        key: key.to_string(),
    }
}

fn text(value: &str) -> PropertyValue {
    PropertyValue::text(value)
}

fn value_of(text: &str, key: &str) -> Option<PropertyValue> {
    read(text)
        .properties
        .into_iter()
        .find(|p| p.key == key)
        .map(|p| p.value)
}

fn keys_of(text: &str) -> Vec<String> {
    read(text).properties.into_iter().map(|p| p.key).collect()
}

fn note(path: &str, props: Vec<(&str, PropertyValue)>) -> IndexedNote {
    let name = path.rsplit('/').next().unwrap();
    let mut indexed = IndexedNote::new(Path::new(path), name, None);
    indexed.properties = props
        .into_iter()
        .map(|(key, value)| PropertyEntry {
            key: key.to_string(),
            value,
        })
        .collect();
    indexed
}

fn tags(values: &[&str]) -> PropertyValue {
    PropertyValue::list(values.iter().copied())
}

fn number(value: f64) -> PropertyValue {
    PropertyValue::Number { value }
}

fn selection(conditions: Vec<Condition>) -> Selection {
    Selection {
        conditions,
        ..Selection::default()
    }
}

fn paths(result: &properties::SelectionResult) -> Vec<&str> {
    result.rows.iter().map(|row| row.path.as_str()).collect()
}

const FULL: &str = "#metadata((
  tags: (\"проект\", \"срочно\"),
  статус: \"черновик\",
  часы: 12,
  доля: 0.5,
  срок: datetime(year: 2026, month: 10, day: 15),
  готово: false,
  \"двое слов\": \"x\",
  минус: -3,
)) <properties>
= Заголовок
";

// ── Разбор (T005) ───────────────────────────────────────────────────────────

/// FR-005: все виды значений читаются.
pub fn all_value_kinds_are_read() {
    let found = read(FULL);
    assert!(found.editable);
    assert_eq!(found.problem, None);
    assert!(found.record.is_some());
    assert_eq!(value_of(FULL, "tags"), Some(tags(&["проект", "срочно"])));
    assert_eq!(value_of(FULL, "статус"), Some(text("черновик")));
    assert_eq!(value_of(FULL, "часы"), Some(number(12.0)));
    assert_eq!(value_of(FULL, "доля"), Some(number(0.5)));
    assert_eq!(
        value_of(FULL, "срок"),
        Some(PropertyValue::date(2026, 10, 15))
    );
    assert_eq!(
        value_of(FULL, "готово"),
        Some(PropertyValue::Flag { value: false })
    );
    assert_eq!(value_of(FULL, "минус"), Some(number(-3.0)));
}

/// Кириллические ключи-идентификаторы и строковые ключи, порядок записи
/// сохраняется.
pub fn cyrillic_and_string_keys_keep_their_order() {
    assert_eq!(
        keys_of(FULL),
        [
            "tags",
            "статус",
            "часы",
            "доля",
            "срок",
            "готово",
            "двое слов",
            "минус"
        ]
    );
    assert_eq!(value_of(FULL, "двое слов"), Some(text("x")));
}

/// Не литерал читается как «не определено», правка выключена (FR-004).
pub fn a_non_literal_value_is_unknown_and_blocks_editing() {
    let source = "#let v = 2\n#metadata((a: 1, b: v + 1)) <properties>\n";
    let found = read(source);
    assert!(!found.editable);
    assert_eq!(
        found.problem,
        Some(PropertiesProblem::NotLiteral { key: "b".into() })
    );
    assert_eq!(value_of(source, "a"), Some(number(1.0)));
    assert_eq!(
        value_of(source, "b"),
        Some(PropertyValue::Unknown {
            source: "v + 1".into()
        })
    );
    // В индекс нераспознанное не попадает.
    assert_eq!(properties::entries_of(source).len(), 1);
}

pub fn a_repeated_key_is_ambiguous() {
    let found = read("#metadata((a: 1, a: 2)) <properties>");
    assert!(!found.editable);
    assert_eq!(
        found.problem,
        Some(PropertiesProblem::DuplicateKey { key: "a".into() })
    );
}

pub fn a_second_record_is_ambiguous() {
    let source = "#metadata((a: 1)) <properties>\n\n#metadata((b: 2)) <properties>\n";
    let found = read(source);
    assert!(!found.editable);
    assert_eq!(found.problem, Some(PropertiesProblem::SecondRecord));
    assert_eq!(keys_of(source), ["a"], "значения берутся из первой записи");
}

pub fn a_non_dictionary_argument_is_ambiguous() {
    let found = read("#let p = (a: 1)\n#metadata(p) <properties>\n");
    assert!(!found.editable);
    assert_eq!(found.problem, Some(PropertiesProblem::NotADictionary));
    assert!(found.properties.is_empty());
}

pub fn a_spread_is_ambiguous() {
    let found = read("#let p = (a: 1)\n#metadata((..p, b: 2)) <properties>\n");
    assert!(!found.editable);
    assert_eq!(found.problem, Some(PropertiesProblem::Spread));
}

/// Запись не первой — всё равно находится; чужие `metadata` и записи в
/// блоках — нет.
pub fn the_record_is_found_anywhere_on_the_top_level_only() {
    let late = "Привет\n\n#metadata((a: 1)) <properties>\n";
    assert_eq!(keys_of(late), ["a"]);

    for ignored in [
        "#metadata((a: 1))\n",
        "#metadata((a: 1)) <pdfpc>\n",
        "#metadata((a: 1))\n\n<properties>\n",
        "#[#metadata((a: 1)) <properties>]\n",
    ] {
        let found = read(ignored);
        assert!(found.record.is_none(), "{ignored:?}");
        assert!(found.editable);
        assert!(found.properties.is_empty());
    }
}

/// Места — в UTF-16: кириллица и символы вне BMP перед записью.
pub fn places_count_utf16_units() {
    let source = "Ж𝔸 #metadata((ключ: \"я\")) <properties>";
    let found = read(source);
    let record = found.record.unwrap();
    // Ж — 1 единица, 𝔸 — 2, пробел — 1.
    assert_eq!(record.from, 4);
    let property = &found.properties[0];
    let before: Vec<u16> = source.encode_utf16().collect();
    let shown =
        String::from_utf16(&before[property.place.from as usize..property.place.to as usize])
            .unwrap();
    assert_eq!(shown, "\"я\"");
    assert_eq!(property.place.line, 1);
}

/// Любой текст разбирается без паники.
pub fn broken_text_never_panics() {
    for source in [
        "",
        "#metadata((",
        "#metadata((a: ) <properties>",
        "#metadata((a: \"незакрытая)) <properties>",
        "#metadata(()) <properties>",
        "#metadata((:)) <properties>",
        "#metadata((tags: (1, 2),)) <properties>",
        "#metadata((d: datetime(year: 2026))) <properties>",
        "#metadata((d: datetime(year: 2026, month: 13, day: 1))) <properties>",
    ] {
        let found = read(source);
        let _ = edit(source, &set("x", text("y")));
        let _ = edit(source, &remove("a"));
        let _ = found;
    }
    // Пустой словарь — запись есть, свойств нет, править можно.
    let empty = read("#metadata((:)) <properties>");
    assert!(empty.editable);
    assert!(empty.properties.is_empty());
}

// ── Правка (T007) ───────────────────────────────────────────────────────────

/// Создать запись: в начало, с переводом строки; удалить последнее — вернуть
/// исходный текст побайтно.
pub fn creating_and_removing_the_last_property_round_trip() {
    let original = "= Заметка\nТекст.\n";
    let created = edited(original, &set("tags", tags(&["проект"])));
    assert_eq!(
        created,
        "#metadata((tags: (\"проект\",))) <properties>\n= Заметка\nТекст.\n"
    );
    assert!(read(&created).editable);
    let back = edited(&created, &remove("tags"));
    assert_eq!(back, original, "запись исчезла, остальной текст прежний");
}

/// Добавить, изменить, удалить; остальной текст побайтно тот же.
pub fn add_change_and_remove_touch_only_the_record() {
    let original = "#metadata((статус: \"черновик\", часы: 3)) <properties>\n= Заголовок\nТекст\n";

    let added = edited(original, &set("автор", text("Иван")));
    assert_eq!(
        added,
        "#metadata((статус: \"черновик\", часы: 3, автор: \"Иван\")) <properties>\n= Заголовок\nТекст\n"
    );
    assert_eq!(
        keys_of(&added),
        ["статус", "часы", "автор"],
        "новый — в конец"
    );

    let changed = edited(original, &set("статус", text("готово")));
    assert_eq!(
        changed,
        "#metadata((статус: \"готово\", часы: 3)) <properties>\n= Заголовок\nТекст\n"
    );

    let first_removed = edited(original, &remove("статус"));
    assert_eq!(
        first_removed,
        "#metadata((часы: 3)) <properties>\n= Заголовок\nТекст\n"
    );
    let last_removed = edited(original, &remove("часы"));
    assert_eq!(
        last_removed,
        "#metadata((статус: \"черновик\")) <properties>\n= Заголовок\nТекст\n"
    );

    // Тело заметки после записи неизменно побайтно.
    for result in [&added, &changed, &first_removed, &last_removed] {
        assert!(result.ends_with("<properties>\n= Заголовок\nТекст\n"));
    }
}

/// Многострочная запись остаётся многострочной; комментарии не теряются.
pub fn multiline_records_keep_their_shape() {
    let original = "#metadata((\n  // главное\n  a: 1,\n  b: 2,\n)) <properties>\n";

    let added = edited(original, &set("c", number(3.0)));
    assert_eq!(
        added,
        "#metadata((\n  // главное\n  a: 1,\n  b: 2,\n  c: 3,\n)) <properties>\n"
    );
    let removed = edited(original, &remove("b"));
    assert_eq!(
        removed,
        "#metadata((\n  // главное\n  a: 1,\n)) <properties>\n"
    );
    let middle = edited(original, &remove("a"));
    assert_eq!(
        middle,
        "#metadata((\n  // главное\n  b: 2,\n)) <properties>\n"
    );
    assert!(read(&added).editable && read(&removed).editable);
}

/// Все виды значений записываются так, что читаются обратно теми же.
pub fn written_values_read_back_equal() {
    let values = [
        ("t", text("с \"кавычками\" и \\")),
        ("n", number(2.0)),
        ("f", number(-1.5)),
        ("d", PropertyValue::date(2026, 2, 28)),
        ("b", PropertyValue::Flag { value: true }),
        ("l0", tags(&[])),
        ("l1", tags(&["один"])),
        ("l2", tags(&["раз", "два"])),
    ];
    let mut source = String::from("Текст\n");
    for (key, value) in &values {
        source = edited(&source, &set(key, value.clone()));
    }
    for (key, value) in &values {
        assert_eq!(value_of(&source, key).as_ref(), Some(value), "{key}");
    }
    assert!(source.ends_with("Текст\n"));
}

/// Ключ, который нельзя писать идентификатором, берётся в кавычки.
pub fn awkward_keys_are_quoted() {
    let source = edited("", &set("двое слов", number(1.0)));
    assert!(
        source.starts_with("#metadata((\"двое слов\": 1))"),
        "{source}"
    );
    let keyword = edited("", &set("true", number(1.0)));
    assert!(keyword.starts_with("#metadata((\"true\": 1))"), "{keyword}");
    assert_eq!(keys_of(&keyword), ["true"]);
}

pub fn rename_replaces_only_the_key() {
    let original = "#metadata((старое: 1, b: 2)) <properties>\n";
    let renamed = edited(
        original,
        &PropertyEdit::Rename {
            from: "старое".into(),
            to: "новое имя".into(),
        },
    );
    assert_eq!(
        renamed,
        "#metadata((\"новое имя\": 1, b: 2)) <properties>\n"
    );

    let clash = edit(
        original,
        &PropertyEdit::Rename {
            from: "старое".into(),
            to: "b".into(),
        },
    );
    assert!(matches!(
        clash,
        EditOutcome::Refused {
            error: PlanEditError::Invalid { .. }
        }
    ));
    let absent = edit(
        original,
        &PropertyEdit::Rename {
            from: "нет".into(),
            to: "x".into(),
        },
    );
    assert!(matches!(
        absent,
        EditOutcome::Refused {
            error: PlanEditError::Gone
        }
    ));
}

/// Нечего менять — пустой список замен (как в 008).
pub fn a_no_op_edit_plans_nothing() {
    let original = "#metadata((a: 1)) <properties>\n";
    assert!(edits_of(original, &set("a", number(1.0))).is_empty());
    assert!(edits_of(original, &remove("нет")).is_empty());
    assert!(edits_of("Текст", &remove("a")).is_empty());
}

/// FR-004: неоднозначная запись не переписывается.
pub fn an_ambiguous_record_is_never_rewritten() {
    for source in [
        "#metadata((a: 1, a: 2)) <properties>\n",
        "#let v = 1\n#metadata((a: v)) <properties>\n",
        "#metadata((a: 1)) <properties>\n#metadata((b: 1)) <properties>\n",
    ] {
        for change in [set("x", number(1.0)), remove("a")] {
            match edit(source, &change) {
                EditOutcome::Refused {
                    error: PlanEditError::Properties { .. },
                } => {}
                other => panic!("{source:?}: ожидался отказ, получено {other:?}"),
            }
        }
    }
}

/// Значения, которые записать нельзя, отклоняются названно.
pub fn unwritable_values_are_refused_with_the_key() {
    for value in [
        number(f64::NAN),
        number(f64::INFINITY),
        PropertyValue::date(2026, 2, 30),
        PropertyValue::Unknown { source: "x".into() },
    ] {
        match edit("", &set("k", value)) {
            EditOutcome::Refused {
                error: PlanEditError::Invalid { errors },
            } => assert_eq!(errors[0].name, "k"),
            other => panic!("ожидался отказ, получено {other:?}"),
        }
    }
    assert!(matches!(
        edit("", &set("  ", text("x"))),
        EditOutcome::Refused {
            error: PlanEditError::Invalid { .. }
        }
    ));
}

/// Кириллица и символы вне BMP перед записью: замены в UTF-16 попадают точно.
pub fn edits_use_utf16_offsets() {
    let original = "Ж𝔸 #metadata((a: \"я\")) <properties>";
    let changed = edited(original, &set("a", text("ю")));
    assert_eq!(changed, "Ж𝔸 #metadata((a: \"ю\")) <properties>");
}

/// SC-003: запись собирается без входов и не выводит ни одного глифа.
pub fn the_record_builds_and_prints_nothing() {
    let record = "#metadata((tags: (\"проект\",), статус: \"черновик\", срок: datetime(year: 2026, month: 10, day: 15))) <properties>\n";
    let alone = compile_note(MAIN, record, 1, FontSet::builtin()).unwrap();
    assert_eq!(
        alone.outcome,
        CompileOutcome::Succeeded,
        "{:?}",
        alone.diagnostics
    );
    assert!(
        alone.diagnostics.is_empty(),
        "запись не должна давать и предупреждений: {:?}",
        alone.diagnostics
    );
    for page in &alone.pages {
        assert!(page.text_runs.is_empty(), "на странице нет ни одного глифа");
    }

    // Заметка с записью выглядит так же, как без неё.
    let with = compile_note(
        MAIN,
        &format!("{record}= Заголовок\nТекст\n"),
        1,
        FontSet::builtin(),
    )
    .unwrap();
    let without = compile_note(MAIN, "= Заголовок\nТекст\n", 1, FontSet::builtin()).unwrap();
    assert_eq!(with.pages.len(), without.pages.len());
    assert_eq!(
        with.pages[0].svg, without.pages[0].svg,
        "запись не влияет на вёрстку"
    );
}

// ── Отбор (T012) ────────────────────────────────────────────────────────────

fn vault_notes() -> Vec<IndexedNote> {
    vec![
        note(
            "Проекты/а.typ",
            vec![
                ("tags", tags(&["проект", "срочно"])),
                ("статус", text("черновик")),
                ("часы", number(8.0)),
                ("срок", PropertyValue::date(2026, 11, 1)),
            ],
        ),
        note(
            "Проекты/б.typ",
            vec![
                ("tags", tags(&["Проект"])),
                ("статус", text("готово")),
                ("часы", number(20.0)),
                ("срок", PropertyValue::date(2026, 10, 1)),
            ],
        ),
        note(
            "Проекты/Архив/в.typ",
            vec![("tags", tags(&["архив"])), ("статус", text("готово"))],
        ),
        note(
            "г.typ",
            vec![("часы", text("много")), ("срок", text("скоро"))],
        ),
        note("д.typ", vec![]),
    ]
}

/// US2 AS1: по тегу; регистр и ведущий # не важны.
pub fn a_tag_selects_its_notes() {
    let notes = vault_notes();
    for tag in ["проект", "#Проект"] {
        let result = run_selection(
            &notes,
            &selection(vec![Condition::TagHas { tag: tag.into() }]),
            0,
        );
        assert_eq!(paths(&result), ["Проекты/а.typ", "Проекты/б.typ"], "{tag}");
        assert_eq!(result.total, 2);
        assert!(!result.truncated);
    }
}

/// US2 AS2: условия складываются через «и».
pub fn conditions_combine_with_and() {
    let notes = vault_notes();
    let result = run_selection(
        &notes,
        &selection(vec![
            Condition::TagHas {
                tag: "проект".into(),
            },
            Condition::Compare {
                key: "статус".into(),
                op: CompareOp::Eq,
                value: text("готово"),
            },
        ]),
        0,
    );
    assert_eq!(paths(&result), ["Проекты/б.typ"]);
}

/// Сравнения по видам: число, дата, текст, «задано», папка.
pub fn comparisons_work_per_kind() {
    let notes = vault_notes();
    let run = |condition| {
        paths(&run_selection(&notes, &selection(vec![condition]), 0))
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    let compare = |key: &str, op, value| Condition::Compare {
        key: key.into(),
        op,
        value,
    };

    assert_eq!(
        run(compare("часы", CompareOp::Gt, number(10.0))),
        ["Проекты/б.typ"]
    );
    assert_eq!(
        run(compare("часы", CompareOp::Ge, number(8.0))),
        ["Проекты/а.typ", "Проекты/б.typ"]
    );
    assert_eq!(
        run(compare(
            "срок",
            CompareOp::Lt,
            PropertyValue::date(2026, 10, 15)
        )),
        ["Проекты/б.typ"]
    );
    assert_eq!(
        run(compare("статус", CompareOp::Ne, text("готово"))),
        ["Проекты/а.typ"]
    );
    // Список и текст: «равно» — «содержит».
    assert_eq!(
        run(compare("tags", CompareOp::Eq, text("срочно"))),
        ["Проекты/а.typ"]
    );
    assert_eq!(
        run(Condition::Exists {
            key: "срок".into()
        }),
        ["Проекты/а.typ", "Проекты/б.typ", "г.typ"]
    );
    assert_eq!(
        run(Condition::Folder {
            folder: "Проекты".into()
        }),
        ["Проекты/Архив/в.typ", "Проекты/а.typ", "Проекты/б.typ"]
    );
    assert_eq!(
        run(Condition::Folder {
            folder: "Проекты/Архив/".into()
        }),
        ["Проекты/Архив/в.typ"]
    );
    assert_eq!(
        run(Condition::Folder {
            folder: String::new()
        })
        .len(),
        5
    );
}

/// Edge Cases: несравнимые значения в условии не проходят.
pub fn incomparable_values_do_not_pass() {
    let notes = vault_notes();
    // «часы» у г.typ — текст: ни > 0, ни != 5 для него не выполняются.
    let result = run_selection(
        &notes,
        &selection(vec![Condition::Compare {
            key: "часы".into(),
            op: CompareOp::Ne,
            value: number(5.0),
        }]),
        0,
    );
    assert_eq!(paths(&result), ["Проекты/а.typ", "Проекты/б.typ"]);
    // Дата против текста — тоже нет.
    let result = run_selection(
        &notes,
        &selection(vec![Condition::Compare {
            key: "срок".into(),
            op: CompareOp::Gt,
            value: PropertyValue::date(2000, 1, 1),
        }]),
        0,
    );
    assert_eq!(paths(&result), ["Проекты/а.typ", "Проекты/б.typ"]);
}

/// Заметка без свойств участвует в выборках по встроенным сведениям.
pub fn built_in_facts_work_without_properties() {
    let mut notes = vault_notes();
    notes[4].modified_ms = Some(20_000 * DAY);
    let now = 20_003 * DAY;

    let recent = run_selection(
        &notes,
        &selection(vec![Condition::Modified {
            range: DateRange::LastDays { days: 5 },
        }]),
        now,
    );
    assert_eq!(paths(&recent), ["д.typ"]);
    let old = run_selection(
        &notes,
        &selection(vec![Condition::Modified {
            range: DateRange::LastDays { days: 1 },
        }]),
        now,
    );
    assert!(old.rows.is_empty());

    let (year, month, day) = (2024, 10, 4); // 20_000 суток от эпохи
    let between = run_selection(
        &notes,
        &selection(vec![Condition::Modified {
            range: DateRange::Between {
                from: Ymd {
                    year,
                    month,
                    day: day - 1,
                },
                to: Ymd {
                    year,
                    month,
                    day: day + 1,
                },
            },
        }]),
        now,
    );
    // 20_000 суток от эпохи — 2024-10-04.
    assert_eq!(paths(&between), ["д.typ"]);

    // «Создана» в индексе нет нигде: условие не проходит, ячейка пуста.
    let created = run_selection(
        &notes,
        &selection(vec![Condition::Created {
            range: DateRange::LastDays { days: 100_000 },
        }]),
        now,
    );
    assert!(created.rows.is_empty());
    let cells = run_selection(
        &notes,
        &Selection {
            columns: vec![
                "название".into(),
                "папка".into(),
                "изменена".into(),
                "создана".into(),
            ],
            conditions: vec![Condition::Folder {
                folder: "Проекты/Архив".into(),
            }],
            ..Selection::default()
        },
        now,
    );
    assert_eq!(
        cells.rows[0].cells,
        vec![Some(text("в")), Some(text("Проекты/Архив")), None, None]
    );
}

/// Сортировка: пустые и несравнимые — в конце; направление; ничья по пути.
pub fn sorting_puts_empty_and_incomparable_last() {
    let notes = vault_notes();
    let by = |key: &str, descending| {
        let result = run_selection(
            &notes,
            &Selection {
                sort: Some(SortKey {
                    key: key.into(),
                    descending,
                }),
                ..Selection::default()
            },
            0,
        );
        paths(&result)
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    // «часы»: числа (8, 20), затем текст «много», затем без значения.
    assert_eq!(
        by("часы", false),
        [
            "Проекты/а.typ",
            "Проекты/б.typ",
            "г.typ",
            "Проекты/Архив/в.typ",
            "д.typ"
        ]
    );
    assert_eq!(
        by("часы", true),
        [
            "Проекты/б.typ",
            "Проекты/а.typ",
            "г.typ",
            "Проекты/Архив/в.typ",
            "д.typ"
        ]
    );
    // По названию — встроенное.
    assert_eq!(by("название", false)[0], "Проекты/а.typ");
}

/// `limit` и `truncated`.
pub fn limit_cuts_rows_and_reports_truncation() {
    let notes = vault_notes();
    let result = run_selection(
        &notes,
        &Selection {
            limit: 2,
            ..Selection::default()
        },
        0,
    );
    assert_eq!(result.rows.len(), 2);
    assert!(result.truncated);
    assert_eq!(result.total, 5);
    assert_eq!(result.columns, ["название"], "столбец по умолчанию");

    let many: Vec<IndexedNote> = (0..600)
        .map(|i| note(&format!("н{i:03}.typ"), vec![]))
        .collect();
    let capped = run_selection(
        &many,
        &Selection {
            limit: 10_000,
            ..Selection::default()
        },
        0,
    );
    assert_eq!(capped.rows.len(), 500, "потолок — 500 строк");
    assert!(capped.truncated);
}

/// Выборка по `MemoryVault`: индекс-умолчание читает свежий текст; правка
/// свойства меняет результат без повторного запроса (US2 AS3).
pub async fn the_vault_selection_follows_note_edits() {
    let vault = MemoryVault::new()
        .with_file(
            "а.typ",
            "#metadata((tags: (\"проект\",))) <properties>\nТекст",
        )
        .with_file("б.typ", "Без свойств")
        .with_file("рисунок.excalidraw", "{\"properties\": 1}");
    let by_tag = selection(vec![Condition::TagHas {
        tag: "проект".into(),
    }]);

    let before = properties::select_notes(&vault, &by_tag).await;
    assert_eq!(paths(&before), ["а.typ"]);

    let mut vault = vault;
    use typsidian_core::vault::backend::VaultBackend;
    vault
        .write(
            Path::new("б.typ"),
            "#metadata((tags: (\"проект\",))) <properties>\nТеперь с тегом",
            None,
        )
        .await
        .unwrap();
    let after = properties::select_notes(&vault, &by_tag).await;
    assert_eq!(paths(&after), ["а.typ", "б.typ"]);

    let keys = properties::property_keys(&vault).await;
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].key, "tags");
    assert_eq!(keys[0].count, 2);
}

/// Условия поиска: теги и поля отбирают заметки без чтения текста запросом.
pub async fn search_filters_by_properties() {
    let vault = MemoryVault::new()
        .with_file(
            "а.typ",
            "#metadata((tags: (\"проект\",), статус: \"черновик\")) <properties>\nуравнение",
        )
        .with_file(
            "б.typ",
            "#metadata((tags: (\"идея\",))) <properties>\nуравнение",
        )
        .with_file("в.typ", "уравнение");

    let only_properties = search(
        &vault,
        &SearchQuery {
            properties: vec![Condition::TagHas {
                tag: "проект".into(),
            }],
            ..SearchQuery::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        only_properties.total_notes, 1,
        "условие без текста — не пустой запрос"
    );
    assert_eq!(only_properties.groups[0].hits[0].path, "а.typ");

    let with_text = search(
        &vault,
        &SearchQuery {
            text: "уравнение".into(),
            properties: vec![Condition::Exists { key: "tags".into() }],
            ..SearchQuery::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(with_text.total_notes, 2, "текст и свойства — «и»");
}

/// Слова запроса превращаются в условия; чужое остаётся текстом.
pub fn typed_words_become_conditions() {
    let known = vec!["статус".to_string(), "срок".to_string(), "часы".to_string()];
    let parsed = parse_conditions(
        "тег:проект папка:\"Мои проекты\" статус:черновик срок:<2026-11-01 часы:>=5 изменена:7 время:10:30 просто текст",
        &known,
    );
    assert_eq!(parsed.text, "время:10:30 просто текст");
    assert_eq!(
        parsed.conditions,
        vec![
            Condition::TagHas {
                tag: "проект".into()
            },
            Condition::Folder {
                folder: "Мои проекты".into()
            },
            Condition::Compare {
                key: "статус".into(),
                op: CompareOp::Eq,
                value: text("черновик"),
            },
            Condition::Compare {
                key: "срок".into(),
                op: CompareOp::Lt,
                value: PropertyValue::date(2026, 11, 1),
            },
            Condition::Compare {
                key: "часы".into(),
                op: CompareOp::Ge,
                value: number(5.0),
            },
            Condition::Modified {
                range: DateRange::LastDays { days: 7 },
            },
        ]
    );
    let exists = parse_conditions("статус:*", &known);
    assert_eq!(
        exists.conditions,
        vec![Condition::Exists {
            key: "статус".into()
        }]
    );
}

// ── Кэш свойств (T006, T016) ────────────────────────────────────────────────

/// Свойства в кэше: запись, замена, переименование, удаление; версия формата.
pub async fn the_cache_stores_properties<C: IndexCache>(mut cache: C) {
    let a = Path::new("а.typ");
    let b = Path::new("б.typ");
    let entries = vec![
        PropertyEntry {
            key: "tags".into(),
            value: tags(&["проект"]),
        },
        PropertyEntry {
            key: "срок".into(),
            value: PropertyValue::date(2026, 10, 15),
        },
        PropertyEntry {
            key: "готово".into(),
            value: PropertyValue::Flag { value: true },
        },
    ];
    cache.set_properties(a, &entries).await;
    assert_eq!(cache.all_properties().await.get(a), Some(&entries));

    // Замена, а не добавление.
    let replaced = vec![PropertyEntry {
        key: "x".into(),
        value: number(1.5),
    }];
    cache.set_properties(a, &replaced).await;
    assert_eq!(cache.all_properties().await.get(a), Some(&replaced));

    // Переименование заметки уносит свойства с собой.
    cache.rename_note(a, b).await;
    let all = cache.all_properties().await;
    assert!(all.get(a).is_none());
    assert_eq!(all.get(b), Some(&replaced));

    // Пустой список — свойств нет.
    cache.set_properties(b, &[]).await;
    assert!(cache.all_properties().await.is_empty());

    cache.set_properties(a, &entries).await;
    cache.remove_note(a).await;
    assert!(cache.all_properties().await.is_empty());

    cache.mark_properties_current().await;
    assert!(cache.properties_current().await);
}

struct Stop {
    after: u32,
    seen: std::sync::atomic::AtomicU32,
}

impl MoveControl for Stop {
    fn progress(&self, done: u32, _total: u32) {
        self.seen.store(done, std::sync::atomic::Ordering::Relaxed);
    }
    fn cancelled(&self) -> bool {
        self.seen.load(std::sync::atomic::Ordering::Relaxed) >= self.after
    }
}

struct Never;

impl MoveControl for Never {
    fn progress(&self, _done: u32, _total: u32) {}
    fn cancelled(&self) -> bool {
        false
    }
}

/// T016: пересчёт идёт шагами, отмена между шагами; без отмены доходит до конца.
pub async fn reindex_runs_in_steps_and_can_be_cancelled() {
    use typsidian_core::vault::backend::VaultBackend;
    let mut vault = MemoryVault::new();
    for i in 0..120 {
        vault = vault.with_file(&format!("н{i:03}.typ"), "Текст");
    }

    let cancelled = properties::reindex_properties(
        &mut vault,
        &Stop {
            after: 50,
            seen: Default::default(),
        },
    )
    .await;
    assert!(cancelled.cancelled);
    assert_eq!(cancelled.done, 50, "остановка между шагами");
    assert_eq!(cancelled.total, 120);

    let full = properties::reindex_properties(&mut vault, &Never).await;
    assert!(!full.cancelled);
    assert_eq!((full.done, full.total), (120, 120));

    // Шаг за пределом списка — пустой и завершённый.
    let step = properties::reindex_step(&mut vault, 500, 50).await;
    assert!(step.finished);
    assert_eq!(step.done, 120);
    let _ = vault.list().await;
}

// ── Блок выборки (T017) ─────────────────────────────────────────────────────

const CALL: &str = "#import \"/.typsidian/lib/select.typ\": select\n\
#select(where: ((tag: \"проект\"), (key: \"статус\", op: \"=\", value: \"готово\")), sort: \"-срок\", columns: (\"название\", \"статус\", \"срок\"), limit: 20)\n";

/// Блоки находятся, ключ не зависит от порядка аргументов и пробелов.
pub fn blocks_are_found_with_an_order_independent_key() {
    let blocks = select_blocks(CALL);
    assert_eq!(blocks.len(), 1);
    let block = &blocks[0];
    assert!(block.editable);
    let selection = block.selection.as_ref().unwrap();
    assert_eq!(selection.limit, 20);
    assert_eq!(
        selection.sort,
        Some(SortKey {
            key: "срок".into(),
            descending: true
        })
    );
    assert_eq!(selection.conditions.len(), 2);
    assert_eq!(selection.columns, ["название", "статус", "срок"]);

    let shuffled = "#select(limit: 20, columns: (\"название\", \"статус\", \"срок\"),\n  sort: \"-срок\",\n  where: (\n    (tag: \"проект\"),\n    (op: \"=\", value: \"готово\", key: \"статус\"),\n  ))";
    let other = select_blocks(shuffled);
    assert_eq!(
        other[0].key, block.key,
        "порядок аргументов и ключей словаря не важен"
    );

    // Умолчания: явные и неявные дают один ключ.
    let implicit = select_blocks("#select()");
    let explicit = select_blocks(
        "#select(where: (), sort: none, columns: (\"название\",), limit: 50, view: \"table\")",
    );
    assert_eq!(implicit[0].key, explicit[0].key);

    // Место блока — с «#», в UTF-16.
    let source = "Ж𝔸 #select()";
    assert_eq!(select_blocks(source)[0].place.from, 4);
}

/// Не литерал — блок не редактируется формой и входа не получает.
pub fn a_computed_block_is_not_editable() {
    let blocks = select_blocks("#let t = \"проект\"\n#select(where: ((tag: t),))");
    assert_eq!(blocks.len(), 1);
    assert!(!blocks[0].editable);
    assert!(blocks[0].selection.is_none());
    assert!(blocks[0].key.is_empty());
    // Метод с таким именем — не блок.
    assert!(select_blocks("#x.select(1)").is_empty());
    assert!(select_blocks("Текст без блока").is_empty());
}

/// Текст блока из выборки читается обратно той же выборкой.
pub fn a_rendered_call_reads_back() {
    use properties::select::render_call;
    let wanted = Selection {
        conditions: vec![
            Condition::TagHas {
                tag: "проект".into(),
            },
            Condition::Compare {
                key: "срок".into(),
                op: CompareOp::Le,
                value: PropertyValue::date(2026, 12, 31),
            },
            Condition::Modified {
                range: DateRange::Between {
                    from: Ymd {
                        year: 2026,
                        month: 1,
                        day: 1,
                    },
                    to: Ymd {
                        year: 2026,
                        month: 2,
                        day: 1,
                    },
                },
            },
            Condition::Exists {
                key: "автор".into(),
            },
            Condition::Folder {
                folder: "Проекты".into(),
            },
        ],
        sort: Some(SortKey {
            key: "срок".into(),
            descending: false,
        }),
        columns: vec!["название".into(), "срок".into()],
        limit: 7,
        view: SelectView::List,
    };
    let call = render_call(&wanted).unwrap();
    let back = select_blocks(&call);
    assert_eq!(back[0].selection.as_ref(), Some(&wanted), "{call}");
    assert_eq!(render_call(&Selection::default()).unwrap(), "#select()");
}

/// Строка `#import` добавляется один раз, после записи свойств.
pub fn the_import_line_is_added_once() {
    let plain = select_import_edits("Текст");
    assert_eq!(plain.len(), 1);
    assert_eq!(
        apply("Текст", &plain),
        "#import \"/.typsidian/lib/select.typ\": select\nТекст"
    );
    let with_record = "#metadata((a: 1)) <properties>\nТекст";
    assert_eq!(
        apply(with_record, &select_import_edits(with_record)),
        "#metadata((a: 1)) <properties>\n#import \"/.typsidian/lib/select.typ\": select\nТекст"
    );
    assert!(select_import_edits(CALL).is_empty());
}

fn project_notes() -> Vec<IndexedNote> {
    vec![
        note(
            "Проекты/а.typ",
            vec![
                ("tags", tags(&["проект"])),
                ("статус", text("готово")),
                ("срок", PropertyValue::date(2026, 11, 1)),
            ],
        ),
        note(
            "Проекты/б.typ",
            vec![
                ("tags", tags(&["проект"])),
                ("статус", text("готово")),
                ("срок", PropertyValue::date(2026, 12, 31)),
            ],
        ),
        note("в.typ", vec![("tags", tags(&["идея"]))]),
    ]
}

/// Входы: по ключу блока — строки с тремя столбцами, порядок и ссылки.
pub fn inputs_carry_rows_by_block_key() {
    let inputs = select_inputs(&project_notes(), CALL, 0).unwrap();
    let all: serde_json::Value = serde_json::from_str(&inputs).unwrap();
    let key = &select_blocks(CALL)[0].key;
    let found = &all[key];
    assert_eq!(found["total"], 2);
    assert_eq!(found["truncated"], false);
    // По убыванию срока: б (31.12) раньше а (01.11).
    assert_eq!(found["rows"][0]["cells"][0]["s"], "б");
    assert_eq!(found["rows"][0]["cells"][0]["l"], "[[/Проекты/б]]");
    assert_eq!(found["rows"][0]["cells"][2]["s"], "31.12.2026");
    assert_eq!(found["rows"][0]["cells"][1]["l"], serde_json::Value::Null);
    assert_eq!(found["rows"][1]["cells"][0]["s"], "а");

    assert!(select_inputs(&project_notes(), "Без блоков", 0).is_none());
}

fn texts_of(document: &typsidian_core::preview::CompiledDocument) -> String {
    document
        .pages
        .iter()
        .flat_map(|page| page.text_runs.iter())
        .map(|run| run.text.clone())
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn library_vault(extra: &str, text: &str) -> MemoryVault {
    let _ = extra;
    MemoryVault::new()
        .with_file(select_library().path.as_str(), select_library().text)
        .with_file(MAIN, text)
}

async fn build(vault: &MemoryVault, source: &str) -> typsidian_core::preview::CompiledDocument {
    compile_note_resolved(
        vault,
        &PackageCache::new(),
        MAIN,
        source,
        1,
        FontSet::builtin(),
    )
    .await
    .unwrap()
}

/// US3 AS1: сборка по vault даёт таблицу из подходящих заметок, с названиями
/// столбцов и значениями.
pub async fn the_block_builds_into_a_table() {
    let source = "#set text(lang: \"ru\")\n#import \"/.typsidian/lib/select.typ\": select\n#select(where: ((tag: \"проект\"),), sort: \"название\", columns: (\"название\", \"статус\", \"срок\"))\n";
    let vault = library_vault("", source)
        .with_file(
            "Проекты/альфа.typ",
            "#metadata((tags: (\"проект\",), статус: \"готово\", срок: datetime(year: 2026, month: 11, day: 1))) <properties>\n",
        )
        .with_file(
            "Проекты/бета.typ",
            "#metadata((tags: (\"проект\",), статус: \"в работе\")) <properties>\n",
        )
        .with_file("идея.typ", "#metadata((tags: (\"идея\",))) <properties>\n");

    let document = build(&vault, source).await;
    assert_eq!(
        document.outcome,
        CompileOutcome::Succeeded,
        "{:?}",
        document.diagnostics
    );
    assert!(
        document.diagnostics.is_empty(),
        "без предупреждений: {:?}",
        document.diagnostics
    );
    let shown = texts_of(&document);
    for word in [
        "Название",
        "Статус",
        "Срок",
        "альфа",
        "бета",
        "готово",
        "в работе",
        "01.11.2026",
        "—",
    ] {
        assert!(shown.contains(word), "в таблице нет «{word}»: {shown}");
    }
    assert!(!shown.contains("идея"), "лишняя заметка: {shown}");
    assert!(!shown.contains("Typsidian"), "это не замещающий текст");
}

/// US3 AS2: изменился vault — изменилась таблица; отпечаток входов меняется.
pub async fn the_table_follows_the_vault() {
    let source =
        "#import \"/.typsidian/lib/select.typ\": select\n#select(where: ((tag: \"проект\"),))\n";
    let mut vault = library_vault("", source)
        .with_file("а.typ", "#metadata((tags: (\"проект\",))) <properties>\n");
    let before = build(&vault, source).await;
    assert!(texts_of(&before).contains('а'));
    assert!(!texts_of(&before).contains("новая"));
    let digest_before = properties::inputs_digest(&vault, source).await.unwrap();

    use typsidian_core::vault::backend::VaultBackend;
    vault
        .write(
            Path::new("новая.typ"),
            "#metadata((tags: (\"проект\",))) <properties>\n",
            None,
        )
        .await
        .unwrap();
    let after = build(&vault, source).await;
    assert!(texts_of(&after).contains("новая"), "{}", texts_of(&after));
    assert_ne!(
        properties::inputs_digest(&vault, source).await.unwrap(),
        digest_before
    );
}

/// US3 AS3: пустой результат — пояснение с условиями, а не пустота.
pub async fn an_empty_result_explains_itself() {
    let source = "#import \"/.typsidian/lib/select.typ\": select\n#select(where: ((tag: \"нет-такого\"),))\n";
    let vault = library_vault("", source).with_file("а.typ", "Текст");
    let document = build(&vault, source).await;
    assert_eq!(
        document.outcome,
        CompileOutcome::Succeeded,
        "{:?}",
        document.diagnostics
    );
    let shown = texts_of(&document);
    assert!(shown.contains("No notes match the selection"), "{shown}");
    assert!(shown.contains("нет-такого"), "условия названы: {shown}");

    let russian = format!("#set text(lang: \"ru\")\n{source}");
    let shown = texts_of(&build(&library_vault("", &russian), &russian).await);
    assert!(shown.contains("Нет заметок по выборке"), "{shown}");
}

/// Edge Cases, Clarifications: без приложения (без входов) — замещающий текст
/// с условиями; сборка проходит без ошибок.
pub fn without_inputs_the_block_prints_a_placeholder() {
    let library = select_library();
    let source = "#set text(lang: \"ru\")\n#import \"/.typsidian/lib/select.typ\": select\n#select(where: ((tag: \"проект\"), (key: \"статус\", op: \"=\", value: \"черновик\"), (modified: 30)))\n";
    // Сборка без vault-цикла: библиотеку подкладываем в снимок через sync-путь
    // нельзя, поэтому текст библиотеки вставляется в заметку целиком.
    let inline = format!(
        "{}\n{}",
        library.text,
        source.replace("#import \"/.typsidian/lib/select.typ\": select\n", "")
    );
    let document = compile_note(MAIN, &inline, 1, FontSet::builtin()).unwrap();
    assert_eq!(
        document.outcome,
        CompileOutcome::Succeeded,
        "{:?}",
        document.diagnostics
    );
    let shown = texts_of(&document);
    assert!(
        shown.contains("Выборка показывается в Typsidian"),
        "{shown}"
    );
    assert!(
        shown.contains("проект") && shown.contains("статус") && shown.contains("30"),
        "{shown}"
    );
}

/// Список вместо таблицы и отметка «показаны первые».
pub async fn the_list_view_and_the_truncation_note() {
    let source =
        "#import \"/.typsidian/lib/select.typ\": select\n#select(limit: 1, view: \"list\")\n";
    let vault = library_vault("", source)
        .with_file("а.typ", "Текст")
        .with_file("б.typ", "Текст");
    let document = build(&vault, source).await;
    assert_eq!(
        document.outcome,
        CompileOutcome::Succeeded,
        "{:?}",
        document.diagnostics
    );
    let shown = texts_of(&document);
    assert!(shown.contains("Showing the first 1 of "), "{shown}");
}

/// Блок с вычисляемыми аргументами входа не получает: на его месте
/// замещающий текст, а сборка проходит.
pub async fn a_computed_block_falls_back_to_the_placeholder() {
    let source = "#import \"/.typsidian/lib/select.typ\": select\n#let t = \"проект\"\n#select(where: ((tag: t),))\n";
    let vault = library_vault("", source)
        .with_file("а.typ", "#metadata((tags: (\"проект\",))) <properties>\n");
    let document = build(&vault, source).await;
    assert_eq!(
        document.outcome,
        CompileOutcome::Succeeded,
        "{:?}",
        document.diagnostics
    );
    assert!(texts_of(&document).contains("The selection is shown in Typsidian"));
}

/// US3 AS4: экспорт идёт через тот же цикл сборки, поэтому PDF получает входы
/// выборки: меняется vault — меняется PDF; тот же vault — тот же PDF.
pub async fn export_builds_with_the_same_inputs() {
    let source =
        "#import \"/.typsidian/lib/select.typ\": select\n#select(where: ((tag: \"проект\"),))\n";
    let note = |tag: &str| format!("#metadata((tags: (\"{tag}\",))) <properties>\n");
    let overrides = VaultOverrides {
        auto_download_templates: Some(false),
        ..VaultOverrides::default()
    };
    let export_pdf = |mut vault: MemoryVault| {
        let overrides = overrides.clone();
        async move {
            let report = export::export_note_pdf(
                &mut vault,
                &PackageCache::new(),
                MAIN,
                Some(source),
                FontSet::builtin(),
                &overrides,
            )
            .await;
            assert!(
                matches!(report.outcome, export::ExportOutcome::Written { .. }),
                "{:?} {:?}",
                report.outcome,
                report.diagnostics
            );
            vault.bytes_of(&report.pdf_path).expect("PDF записан")
        }
    };

    let one = library_vault("", source).with_file("а.typ", note("проект"));
    let one_again = library_vault("", source).with_file("а.typ", note("проект"));
    let two = library_vault("", source)
        .with_file("а.typ", note("проект"))
        .with_file("б.typ", note("проект"));
    let none = library_vault("", source).with_file("а.typ", note("идея"));

    let (pdf_one, pdf_again, pdf_two, pdf_none) = (
        export_pdf(one).await,
        export_pdf(one_again).await,
        export_pdf(two).await,
        export_pdf(none).await,
    );
    assert_eq!(pdf_one, pdf_again, "тот же vault — тот же PDF");
    assert_ne!(pdf_one, pdf_two, "ещё одна заметка — другая таблица");
    assert_ne!(pdf_one, pdf_none, "пустая выборка — другое содержимое");
}

// ── Библиотека (T018) ───────────────────────────────────────────────────────

/// §10: библиотека пишется, если её нет; не трогается, если актуальна; правка
/// руками не затирается.
pub async fn the_library_is_written_once_and_never_overwrites_edits() {
    use typsidian_core::vault::backend::VaultBackend;
    let mut vault = MemoryVault::new();
    let path = select_library().path;

    assert_eq!(
        properties::ensure_select_library(&mut vault).await.unwrap(),
        LibraryStatus::Written
    );
    assert_eq!(
        vault.read(Path::new(&path)).await.unwrap(),
        select_library().text
    );
    assert_eq!(
        properties::ensure_select_library(&mut vault).await.unwrap(),
        LibraryStatus::UpToDate
    );

    let edited = format!("{}\n// моя правка\n", select_library().text);
    vault
        .write_bytes(Path::new(&path), edited.as_bytes())
        .await
        .unwrap();
    assert_eq!(
        properties::ensure_select_library(&mut vault).await.unwrap(),
        LibraryStatus::Customized
    );
    assert_eq!(
        vault.read(Path::new(&path)).await.unwrap(),
        edited,
        "правка цела"
    );

    let first_line = select_library().text.lines().next().unwrap().to_string();
    assert_eq!(first_line, "// typsidian-select v1");
}

// ── Значения не попадают в сообщения (T023, принцип IX) ─────────────────────

/// Тексты отказов называют ключ, но не значение; `Debug` записи свойств в
/// журнал не пишется — проверяется то, что ядро само формирует как сообщение.
pub fn error_messages_never_carry_property_values() {
    const SECRET: &str = "СЕКРЕТНОЕ-ЗНАЧЕНИЕ-1234";
    let sources = [
        format!("#metadata((a: \"{SECRET}\", a: 2)) <properties>"),
        format!("#metadata((a: \"{SECRET}\")) <properties>\n#metadata((b: 1)) <properties>"),
        format!("#let v = \"{SECRET}\"\n#metadata((пароль: v)) <properties>"),
    ];
    for source in &sources {
        let found = read(source);
        // Проблема записи: ключ допустим, значение — нет.
        let problem = format!("{:?}", found.problem);
        assert!(!problem.contains(SECRET), "{problem}");
        for change in [
            set("x", text(SECRET)),
            remove("a"),
            set("a", PropertyValue::date(2026, 2, 30)),
        ] {
            if let EditOutcome::Refused { error } = edit(source, &change) {
                let shown = format!("{error:?}");
                assert!(
                    !shown.contains(SECRET),
                    "отказ правки показывает значение: {shown}"
                );
            }
        }
    }
    // Отказ из-за недопустимого значения — тоже без него.
    if let EditOutcome::Refused { error } = edit("", &set("k", number(f64::NAN))) {
        assert!(!format!("{error:?}").contains("NaN"));
    }
    // Сообщения сборки при блоке с чужим значением не содержат его.
}

/// Хранимый путь: у переданных в `PathBuf` имён нет сюрпризов.
pub fn indexed_note_paths_use_forward_slashes() {
    let indexed = IndexedNote::new(&PathBuf::from("а").join("б.typ"), "б.typ", Some(5));
    assert_eq!(indexed.path, "а/б.typ");
    assert_eq!(indexed.modified_ms, Some(5));
    assert_eq!(
        IndexedNote::new(Path::new("x.typ"), "x.typ", Some(0)).modified_ms,
        None
    );
}

// ── Форма блока в параметрах элемента 008 (T020) ────────────────────────────

fn field(name: &str, plain: &str) -> typsidian_core::elements::FieldValue {
    typsidian_core::elements::FieldValue {
        name: name.to_string(),
        source: None,
        plain: Some(plain.to_string()),
    }
}

/// Схема элемента `select`: пять полей, три из них — новых видов.
pub fn the_select_element_has_a_form_schema() {
    use typsidian_core::elements::{element_schema, ParamKind};
    let spec = element_schema("call:select").expect("схема блока выборки");
    let names: Vec<&str> = spec.params.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["where", "sort", "columns", "limit", "view"]);
    assert_eq!(spec.param("where").unwrap().kind, ParamKind::Conditions);
    assert_eq!(spec.param("sort").unwrap().kind, ParamKind::Order);
    assert_eq!(spec.param("columns").unwrap().kind, ParamKind::Columns);
    assert_eq!(spec.param("limit").unwrap().kind, ParamKind::Int);
    assert!(matches!(
        spec.param("view").unwrap().kind,
        ParamKind::Choice { .. }
    ));
    assert_eq!(spec.form_on_insert, Some(true));
    // Виды полей пересекают границу как размеченные объекты.
    let json = serde_json::to_value(&spec.param("where").unwrap().kind).unwrap();
    assert_eq!(json, serde_json::json!({ "type": "conditions" }));
}

/// Форма пишет блок и читает его обратно; правка меняет только изменённое.
pub fn the_form_writes_reads_and_edits_a_block() {
    use typsidian_core::elements::{
        element_at, plan_element_edit, render_element, ElementAtResult,
    };

    let conditions = serde_json::to_string(&vec![
        Condition::TagHas {
            tag: "проект".into(),
        },
        Condition::Compare {
            key: "статус".into(),
            op: CompareOp::Ne,
            value: text("готово"),
        },
    ])
    .unwrap();
    let fields = vec![
        field("where", &conditions),
        field("sort", r#"{"key":"срок","descending":true}"#),
        field("columns", r#"["название","статус"]"#),
        field("limit", "10"),
        typsidian_core::elements::FieldValue {
            name: "view".into(),
            source: Some("\"list\"".into()),
            plain: None,
        },
    ];
    let rendered = render_element("call:select", &fields, None, &[]).expect("текст блока");
    assert!(rendered.text.starts_with("#select("), "{}", rendered.text);

    // Блок читается ядром выборок теми же условиями.
    let blocks = select_blocks(&rendered.text);
    let selection = blocks[0].selection.clone().expect("литералы");
    assert_eq!(selection.conditions.len(), 2);
    assert_eq!(selection.limit, 10);
    assert_eq!(selection.view, SelectView::List);
    assert_eq!(selection.sort.as_ref().unwrap().key, "срок");

    // Форма читает блок: значения полей — JSON, а не исходный текст.
    let document = format!("Ж {}\n", rendered.text);
    let caret = "Ж #sel".encode_utf16().count();
    let ElementAtResult::Found { element } = element_at(&document, caret) else {
        panic!("блок под кареткой не найден");
    };
    assert_eq!(element.id, "call:select");
    let where_value = element.values.iter().find(|v| v.name == "where").unwrap();
    assert!(!where_value.expression);
    assert_eq!(where_value.plain, conditions);
    let sort_value = element.values.iter().find(|v| v.name == "sort").unwrap();
    assert_eq!(sort_value.plain, r#"{"key":"срок","descending":true}"#);

    // Правка предела меняет ровно значение предела.
    let edits = plan_element_edit(&document, &element, &[field("limit", "25")], None).unwrap();
    assert_eq!(edits.len(), 1);
    let changed = apply(&document, &edits);
    assert_eq!(changed, document.replacen("limit: 10", "limit: 25", 1));

    // Выбор умолчаний: пустые условия, порядок «нет», столбец «название» не пишутся.
    let bare = render_element(
        "call:select",
        &[
            field("where", "[]"),
            field("sort", "null"),
            field("columns", r#"["название"]"#),
        ],
        None,
        &[],
    )
    .unwrap();
    assert_eq!(bare.text, "#select()");

    // Недопустимое значение поля — названная ошибка, а не паника.
    let broken = render_element("call:select", &[field("where", "не json")], None, &[]);
    assert!(broken.is_err());
}

/// Блок с выражением в аргументах форма читает как «как есть».
pub fn a_computed_argument_stays_an_expression_in_the_form() {
    use typsidian_core::elements::{element_at, ElementAtResult};
    let source = "#let t = \"проект\"\n#select(where: ((tag: t),), limit: 5)\n";
    let caret = source.find("#select").unwrap() + 3;
    let units = source[..caret].encode_utf16().count();
    let ElementAtResult::Found { element } = element_at(source, units) else {
        panic!("блок не найден");
    };
    let where_value = element.values.iter().find(|v| v.name == "where").unwrap();
    assert!(
        where_value.expression,
        "условия вычислены — поле «как есть»"
    );
    let limit = element.values.iter().find(|v| v.name == "limit").unwrap();
    assert!(!limit.expression);
}
