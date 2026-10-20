//! Дополнение меток и ссылок (фича 018): общие случаи для
//! `completion_native.rs` и `completion_wasm.rs`. Вся логика лежит над
//! `VaultBackend` и разбором Typst, поэтому проверяется на бэкенде в памяти
//! одинаково на обоих таргетах (принцип II).

#![allow(dead_code)]

use typsidian_core::completion::{
    completions, context_at, definition_at, labels, linked_index, reference_marks, CompletionList,
    CompletionRequest, CompletionTarget, DefinitionOutcome, DefinitionRequest, GroupKind,
    ItemDetail, LinkedIndex, MarkKind, MarksRequest, NoCompletionReason, NoDefinition,
    ReferenceMark, SourceKey,
};
use typsidian_core::preview::evidence::BuildEvidence;
use typsidian_core::preview::{compile_note, CompileOutcome, FontSet, PackageCache};
use typsidian_core::structure::LabelKind;

use super::MemoryVault;

const NOTE: &str = "глава/заметка.typ";

/// Каретка — на месте знака `|` в тексте; возвращает текст без него и
/// смещение в UTF-16.
fn caret(marked: &str) -> (String, u32) {
    let at = marked.find('|').expect("в тексте нет каретки");
    let text = format!("{}{}", &marked[..at], &marked[at + 1..]);
    (text, marked[..at].encode_utf16().count() as u32)
}

fn request(text: &str, offset: u32) -> CompletionRequest {
    CompletionRequest {
        note_path: NOTE.to_owned(),
        text: text.to_owned(),
        offset,
        manual: false,
        linked: LinkedIndex {
            note_path: NOTE.to_owned(),
            ..LinkedIndex::default()
        },
        sources: Vec::new(),
    }
}

fn marks_request(text: &str) -> MarksRequest {
    MarksRequest {
        note_path: NOTE.to_owned(),
        text: text.to_owned(),
        linked: LinkedIndex {
            note_path: NOTE.to_owned(),
            ..LinkedIndex::default()
        },
        evidence: None,
        sources: Vec::new(),
        claimed: Vec::new(),
        held: false,
        previously_missing: Vec::new(),
    }
}

fn labels_of(list: &CompletionList) -> Vec<String> {
    list.groups
        .iter()
        .find(|group| group.kind == GroupKind::Labels)
        .map(|group| group.items.iter().map(|item| item.label.clone()).collect())
        .unwrap_or_default()
}

fn missing_names(marks: &[ReferenceMark]) -> Vec<String> {
    marks
        .iter()
        .filter_map(|mark| match &mark.kind {
            MarkKind::MissingLabel { name } => Some(name.clone()),
            _ => None,
        })
        .collect()
}

fn built(text: &str) -> BuildEvidence {
    let document = compile_note(NOTE, text, 1, FontSet::builtin()).expect("сборка");
    assert_eq!(
        document.outcome,
        CompileOutcome::Succeeded,
        "документ обязан собираться: {:?}",
        document.diagnostics
    );
    document
        .evidence
        .expect("у удачной сборки есть свидетельство")
}

// ── Контекст каретки (T007, FR-004) ─────────────────────────────────────────

pub fn context_offers_in_markup_and_content_blocks() {
    for marked in [
        "См. @рис|",
        "См. @|",
        "= Заголовок @вв|",
        "*жирное @a|*",
        "#figure([подпись @a|])",
        "#if true [ @a| ]",
        "Ссылка [[Пла|",
        "Ссылка [[|",
        "#box[ [[Пла| ]",
    ] {
        let (text, offset) = caret(marked);
        let found = context_at(&text, offset, false);
        assert!(found.is_ok(), "{marked:?}: {found:?}");
    }
    let (text, offset) = caret("См. @рис-сх|");
    let found = context_at(&text, offset, false).unwrap();
    assert_eq!(found.query, "рис-сх");
    assert_eq!(found.to, offset);
    assert_eq!(found.from, offset - "рис-сх".encode_utf16().count() as u32);
}

pub fn context_refuses_code_strings_comments_raw_math_and_addresses() {
    for (marked, reason) in [
        ("#let x = \"@a|\"", NoCompletionReason::InString),
        ("// заметка @a|", NoCompletionReason::InComment),
        ("/* блок @a| */", NoCompletionReason::InComment),
        ("`код @a|`", NoCompletionReason::InRaw),
        ("```\nблок @a|\n```", NoCompletionReason::InRaw),
        ("$ x @a| $", NoCompletionReason::InMath),
        ("пишите на user@exa|", NoCompletionReason::InAddress),
        ("https://example.com/@us|", NoCompletionReason::InAddress),
        ("#f(@a|)", NoCompletionReason::InCode),
        ("#let x = \"[[a|\"", NoCompletionReason::InString),
        ("просто текст|", NoCompletionReason::NoTrigger),
        ("[[закрыто]] и дальше|", NoCompletionReason::NoTrigger),
    ] {
        let (text, offset) = caret(marked);
        assert_eq!(
            context_at(&text, offset, false).map(|found| found.query),
            Err(reason),
            "{marked:?}"
        );
    }
}

pub fn context_counts_utf16_after_cyrillic_and_emoji() {
    let (text, offset) = caret("Привет 👋🏽 мир @ме|");
    let found = context_at(&text, offset, false).unwrap();
    assert_eq!(found.query, "ме");
    assert_eq!(found.from, offset - 2);
    let (text, offset) = caret("👋 [[Пл|]] дальше");
    let found = context_at(&text, offset, false).unwrap();
    assert_eq!(found.query, "Пл");
}

pub fn manual_call_without_trigger_says_why() {
    let (text, offset) = caret("Обычный абзац|");
    assert_eq!(
        context_at(&text, offset, true),
        Err(NoCompletionReason::NoTrigger)
    );
    // Открывающий знак — только в текущей строке.
    let (text, offset) = caret("@метка\nследующая строка|");
    assert_eq!(
        context_at(&text, offset, true),
        Err(NoCompletionReason::NoTrigger)
    );
}

// ── Связанные файлы (T012) ──────────────────────────────────────────────────

pub async fn linked_index_reads_typ_dependencies() {
    let vault = MemoryVault::new()
        .with_file(NOTE, "#include \"часть.typ\"\n= Заметка <своя>\n")
        .with_file(
            "глава/часть.typ",
            "= Часть <часть>\n#figure(rect(), caption: [Схема]) <рис-схема>\n",
        )
        .with_file("глава/картинка.png", vec![1u8, 2, 3])
        .with_file("глава/битый.typ", vec![0xffu8, 0xfe, 0x00]);
    let dependencies: Vec<String> = [
        "глава/часть.typ",
        "глава/картинка.png",
        NOTE,
        "глава/нет.typ",
        "глава/битый.typ",
    ]
    .iter()
    .map(|path| path.to_string())
    .collect();
    let index = linked_index(&vault, &PackageCache::new(), NOTE, &dependencies).await;
    assert_eq!(index.note_path, NOTE);
    let paths: Vec<&str> = index.files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(
        paths,
        vec!["глава/часть.typ"],
        "только .typ и не сама заметка"
    );
    assert_eq!(index.unreadable, vec!["глава/битый.typ", "глава/нет.typ"]);
    let labels = &index.files[0].labels;
    assert_eq!(labels.len(), 2);
    assert_eq!(labels[1].name, "рис-схема");
    assert_eq!(labels[1].kind, LabelKind::Figure);
    assert_eq!(labels[1].text, "Схема");
    assert_eq!(labels[1].line, 2);
    let source = "= Часть <часть>\n#figure(rect(), caption: [Схема]) <рис-схема>\n";
    assert_eq!(
        &source[labels[1].byte_start as usize..labels[1].byte_end as usize],
        "<рис-схема>"
    );
}

pub async fn a_failed_read_gives_an_unreadable_path() {
    let vault = MemoryVault::new()
        .with_file("глава/часть.typ", "<x>")
        .with_failure(
            "глава/часть.typ",
            typsidian_core::vault::backend::ReadError::Io,
        );
    let index = linked_index(
        &vault,
        &PackageCache::new(),
        NOTE,
        &["глава/часть.typ".to_owned()],
    )
    .await;
    assert!(index.files.is_empty());
    assert_eq!(index.unreadable, vec!["глава/часть.typ"]);
}

// ── Кандидаты-метки (T021) ──────────────────────────────────────────────────

pub fn label_candidates_carry_kind_text_and_file() {
    let (text, offset) = caret(
        "= Введение <введение>\n\
         #figure(rect(), caption: [Схема установки]) <рис-схема>\n\
         #figure(table(columns: 1)[a], caption: [Замеры]) <таб-замеры>\n\
         $ E = m c^2 $ <eq-энергия>\n\
         См. @|\n",
    );
    let linked = LinkedIndex {
        note_path: NOTE.to_owned(),
        files: vec![typsidian_core::completion::LinkedFile {
            path: "глава/часть.typ".to_owned(),
            labels: vec![typsidian_core::completion::LinkedLabel {
                name: "часть".to_owned(),
                kind: LabelKind::Heading,
                text: "Часть".to_owned(),
                line: 1,
                column: 9,
                byte_start: 0,
                byte_end: 0,
                top_level: true,
            }],
        }],
        unreadable: Vec::new(),
    };
    let groups = labels::candidates(&text, offset, "", &linked, &[]);
    assert_eq!(groups.len(), 1);
    let items = &groups[0].items;
    // Пустой запрос: ближайшие к каретке первыми, связанные файлы — после.
    let names: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(
        names,
        vec!["eq-энергия", "таб-замеры", "рис-схема", "введение", "часть"]
    );
    match &items[1].detail {
        ItemDetail::Label {
            label_kind,
            text,
            file,
            duplicate,
        } => {
            assert_eq!(*label_kind, LabelKind::Table);
            assert_eq!(text, "Замеры");
            assert_eq!(*file, None);
            assert!(!duplicate);
        }
        other => panic!("{other:?}"),
    }
    match &items[4].detail {
        ItemDetail::Label { file, .. } => assert_eq!(file.as_deref(), Some("глава/часть.typ")),
        other => panic!("{other:?}"),
    }
    assert_eq!(items[2].insert, "рис-схема");
    assert_eq!(items[2].preview, "@рис-схема");
}

pub fn label_candidates_filter_by_any_part_and_rank_by_tier() {
    let text = "<схема-рис> <рис-схема> <таблица> <приРИСовка> <Рис:два>\n";
    let linked = LinkedIndex::default();
    let groups = labels::candidates(text, 0, "рис", &linked, &[]);
    let names: Vec<&str> = groups[0]
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    // Начало имени → начало части после разделителя → прочее вхождение;
    // регистр не важен.
    assert_eq!(
        names,
        vec!["рис-схема", "Рис:два", "схема-рис", "приРИСовка"]
    );
    let pri = groups[0]
        .items
        .iter()
        .find(|item| item.label == "приРИСовка")
        .unwrap();
    assert_eq!(pri.matched, vec![(3, 6)]);
    assert_eq!(groups[0].total, 4);
}

pub fn label_candidates_cap_rows_and_fold_duplicates() {
    let mut text = String::new();
    for index in 0..500 {
        text.push_str(&format!("<метка-{index}>\n"));
    }
    text.push_str("<метка-7>\n");
    let groups = labels::candidates(&text, 0, "", &LinkedIndex::default(), &[]);
    assert_eq!(groups[0].items.len(), 50);
    assert_eq!(groups[0].total, 500, "повтор — одна строка");
    let sources = [SourceKey {
        key: "knuth1984".to_owned(),
        summary: "Knuth D. E. · 1984".to_owned(),
    }];
    let groups = labels::candidates(&text, 0, "метка-7", &LinkedIndex::default(), &sources);
    let seven = groups[0]
        .items
        .iter()
        .find(|item| item.label == "метка-7")
        .unwrap();
    assert!(matches!(
        seven.detail,
        ItemDetail::Label {
            duplicate: true,
            ..
        }
    ));
    // Ключ источника — своей группой после меток, только когда подходит.
    assert!(groups.iter().all(|group| group.kind != GroupKind::Sources));
    let groups = labels::candidates("<knu>", 0, "knu", &LinkedIndex::default(), &sources);
    let kinds: Vec<GroupKind> = groups.iter().map(|group| group.kind).collect();
    assert_eq!(kinds, vec![GroupKind::Labels, GroupKind::Sources]);
    assert!(matches!(
        &groups[1].items[0].detail,
        ItemDetail::Source { summary } if summary == "Knuth D. E. · 1984"
    ));
}

// ── Кандидаты-заметки (T022) ────────────────────────────────────────────────

pub async fn note_candidates_prefer_the_folder_and_insert_what_resolves() {
    let vault = MemoryVault::new()
        .with_file(NOTE, "")
        .with_file("глава/План.typ", "")
        .with_file("План.typ", "")
        .with_file("архив/старое/План работ.typ", "")
        .with_file("глава/доска.excalidraw", "{}")
        .with_file("глава/Прочее.typ", "");
    let (text, offset) = caret("См. [[пла|");
    let list = completions(&vault, &request(&text, offset)).await;
    assert_eq!(list.target, CompletionTarget::Note);
    assert_eq!(list.groups.len(), 1);
    let items = &list.groups[0].items;
    let rows: Vec<(&str, &str)> = items
        .iter()
        .map(|item| (item.label.as_str(), item.insert.as_str()))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("План", "План]]"),
            ("План", "/План]]"),
            ("План работ", "/архив/старое/План работ]]"),
        ]
    );
    assert!(matches!(
        &items[0].detail,
        ItemDetail::Note {
            same_folder: true,
            ..
        }
    ));
    match &items[2].detail {
        ItemDetail::Note {
            folder,
            same_folder,
        } => {
            assert_eq!(folder, "архив/старое");
            assert!(!same_folder);
        }
        other => panic!("{other:?}"),
    }
    // Запись разрешается графом обратно в выбранную заметку.
    for (item, path) in
        items
            .iter()
            .zip(["глава/План.typ", "План.typ", "архив/старое/План работ.typ"])
    {
        let inner = item.insert.trim_end_matches("]]");
        assert_eq!(
            typsidian_core::graph::resolve_link_target(std::path::Path::new(NOTE), inner),
            Some(std::path::PathBuf::from(path))
        );
        assert_eq!(item.preview, format!("[[{inner}]]"));
    }
    // Доска и сама заметка не предлагаются; `]]` не дублируется.
    let (text, offset) = caret("[[|]]");
    let list = completions(&vault, &request(&text, offset)).await;
    let names: Vec<&str> = list.groups[0]
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert!(
        !names.contains(&"доска") && !names.contains(&"заметка"),
        "{names:?}"
    );
    assert!(list.groups[0]
        .items
        .iter()
        .all(|item| !item.insert.ends_with("]]")));
}

// ── Сквозной запрос (T023) ──────────────────────────────────────────────────

pub async fn completions_explain_nothing_and_use_the_buffer() {
    let vault = MemoryVault::new().with_file(NOTE, "<с-диска>");
    let (text, offset) = caret("#let x = \"@a|\"");
    let list = completions(&vault, &request(&text, offset)).await;
    assert_eq!(
        list.target,
        CompletionTarget::None {
            reason: NoCompletionReason::InString
        }
    );
    assert!(list.groups.is_empty());
    // Неподходящий набор — пустые группы, а не все метки.
    let (text, offset) = caret("<буфер> @xyz|");
    let list = completions(&vault, &request(&text, offset)).await;
    assert_eq!(list.target, CompletionTarget::Label);
    assert!(list.groups.is_empty());
    // Текст — из запроса (FR-008): метки с диска не видны.
    let (text, offset) = caret("<буфер> @|");
    let list = completions(&vault, &request(&text, offset)).await;
    assert_eq!(labels_of(&list), vec!["буфер"]);
}

// ── Переход к определению (T042) ────────────────────────────────────────────

fn definition(text: &str, offset: u32, linked: LinkedIndex) -> DefinitionRequest {
    DefinitionRequest {
        note_path: NOTE.to_owned(),
        text: text.to_owned(),
        offset,
        linked,
        evidence: None,
        sources: vec!["knuth1984".to_owned()],
    }
}

pub async fn definition_reaches_labels_files_notes_and_sources() {
    let vault = MemoryVault::new()
        .with_file(NOTE, "")
        .with_file("глава/План.typ", "")
        .with_file("глава/часть.typ", "= Часть\n\n= Метка там <там>\n");
    let linked = linked_index(
        &vault,
        &PackageCache::new(),
        NOTE,
        &["глава/часть.typ".to_owned()],
    )
    .await;

    let body = "= Вв <здесь>\nСм. @здесь и @там, @knuth1984, [[План]].\n";
    let at = |needle: &str, shift: usize| -> u32 {
        (body[..body.find(needle).unwrap()].encode_utf16().count() + shift) as u32
    };

    // Метка в заметке; каретка сразу после ссылки тоже на ней.
    let end_of_here = at("@здесь", "@здесь".chars().count());
    match definition_at(&vault, &definition(body, end_of_here, linked.clone())).await {
        DefinitionOutcome::Place { location } => {
            assert_eq!(location.path, NOTE);
            assert!(location.is_open_note);
            assert_eq!((location.line, location.column), (1, 6));
            assert_eq!(
                &body[location.byte_start as usize..location.byte_end as usize],
                "<здесь>"
            );
        }
        other => panic!("{other:?}"),
    }
    // Метка в связанном файле.
    match definition_at(&vault, &definition(body, at("@там", 2), linked.clone())).await {
        DefinitionOutcome::Place { location } => {
            assert_eq!(location.path, "глава/часть.typ");
            assert!(!location.is_open_note);
            assert_eq!(location.line, 3);
        }
        other => panic!("{other:?}"),
    }
    // Ключ источника.
    assert_eq!(
        definition_at(&vault, &definition(body, at("@knuth", 3), linked.clone())).await,
        DefinitionOutcome::Source {
            key: "knuth1984".to_owned()
        }
    );
    // Заметка `[[…]]` — её начало.
    match definition_at(&vault, &definition(body, at("[[План", 3), linked.clone())).await {
        DefinitionOutcome::Place { location } => {
            assert_eq!(location.path, "глава/План.typ");
            assert_eq!((location.line, location.column), (1, 1));
        }
        other => panic!("{other:?}"),
    }
}

pub async fn definition_names_why_there_is_nowhere_to_go() {
    let vault = MemoryVault::new().with_file(NOTE, "");
    let linked = LinkedIndex::default();
    let body = "Текст @нет и @изкода, [[Нет]] и [[../../наружу]].\n<первая> <первая>\n";
    let at = |needle: &str| (body[..body.find(needle).unwrap()].encode_utf16().count() + 2) as u32;
    let cases = [
        (0, NoDefinition::NoReferenceAtCaret),
        (
            at("@нет"),
            NoDefinition::LabelMissing {
                name: "нет".to_owned(),
            },
        ),
        (
            at("[[Нет"),
            NoDefinition::NoteMissing {
                target: "Нет".to_owned(),
            },
        ),
        (
            at("[[../"),
            NoDefinition::OutsideVault {
                target: "../../наружу".to_owned(),
            },
        ),
    ];
    for (offset, reason) in cases {
        assert_eq!(
            definition_at(&vault, &definition(body, offset, linked.clone())).await,
            DefinitionOutcome::Nothing { reason }
        );
    }
    // Метка есть только в собранном документе.
    let mut request = definition(body, at("@изкода"), linked.clone());
    request.evidence = Some(built("#figure(rect(), caption: [x]) #label(\"изкода\")\n"));
    assert_eq!(
        definition_at(&vault, &request).await,
        DefinitionOutcome::Nothing {
            reason: NoDefinition::LabelFromBuild {
                name: "изкода".to_owned()
            }
        }
    );
    // Повторная метка — первое объявление.
    let twice = "<первая> <первая> @первая";
    match definition_at(
        &vault,
        &definition(twice, twice.encode_utf16().count() as u32, linked),
    )
    .await
    {
        DefinitionOutcome::Place { location } => assert_eq!(location.byte_start, 0),
        other => panic!("{other:?}"),
    }
}

// ── Отметки о метках (T052) ─────────────────────────────────────────────────

pub async fn missing_labels_are_marked_unless_known_somewhere() {
    let vault = MemoryVault::new().with_file(NOTE, "");
    let text = "<есть> @есть @нет @там @knuth1984 @занято 👋 @эмодзи\n";
    let mut request = marks_request(text);
    request
        .linked
        .files
        .push(typsidian_core::completion::LinkedFile {
            path: "глава/часть.typ".to_owned(),
            labels: vec![typsidian_core::completion::LinkedLabel {
                name: "там".to_owned(),
                kind: LabelKind::Other,
                text: String::new(),
                line: 1,
                column: 1,
                byte_start: 0,
                byte_end: 0,
                top_level: true,
            }],
        });
    request.sources = vec!["knuth1984".to_owned()];
    let busy = text[..text.find("@занято").unwrap()].encode_utf16().count() as u32;
    request.claimed = vec![(busy, busy + 7)];
    let marks = reference_marks(&vault, &request).await;
    assert_eq!(missing_names(&marks), vec!["нет", "эмодзи"]);
    // Места — в UTF-16, на `@имя`.
    let emoji = marks
        .iter()
        .find(|mark| matches!(&mark.kind, MarkKind::MissingLabel { name } if name == "эмодзи"))
        .unwrap();
    let start = text[..text.find("@эмодзи").unwrap()].encode_utf16().count() as u32;
    assert_eq!((emoji.from, emoji.to), (start, start + 7));
    assert_eq!(emoji.line, 1);

    // Непрочитанный связанный файл — «метки нет» не ставится вовсе, а отметки
    // о заметках остаются.
    request.linked.unreadable = vec!["глава/битый.typ".to_owned()];
    request.text = format!("{text}[[Нет такой]]\n");
    let marks = reference_marks(&vault, &request).await;
    assert!(missing_names(&marks).is_empty());
    assert!(marks
        .iter()
        .any(|mark| matches!(mark.kind, MarkKind::MissingNote { .. })));
}

// ── Нет ложных отметок (T053, SC-002) ───────────────────────────────────────

pub async fn a_document_that_builds_gets_no_missing_labels() {
    let vault = MemoryVault::new().with_file(NOTE, "");
    for text in [
        // Метки из цикла.
        "#for i in range(3) [ #figure(rect(), caption: [Рис #i]) #label(\"рис-\" + str(i)) ]\nСм. @рис-0 и @рис-2.\n",
        // Метка из функции.
        "#let глава(имя) = [#figure(rect(), caption: [Глава])#label(имя)]\n#глава(\"введение\")\nСм. @введение.\n",
        // `#label(…)` напрямую.
        "#set heading(numbering: \"1.\")\n#heading[Раздел]#label(\"раздел\")\nСм. @раздел.\n",
        // Правило `show ref` обрабатывает ссылку в никуда.
        "#show ref: it => [ссылка]\nСм. @куда-угодно.\n",
    ] {
        let evidence = built(text);
        let mut request = marks_request(text);
        request.evidence = Some(evidence.clone());
        let marks = reference_marks(&vault, &request).await;
        assert!(missing_names(&marks).is_empty(), "{text:?}: {marks:?}");
        // Без свидетельства те же ссылки отмечены: текст их не видит.
        let without = reference_marks(&vault, &marks_request(text)).await;
        assert!(!missing_names(&without).is_empty(), "{text:?}");

        // Правка без новой сборки: подтверждённое остаётся без отметки, новое
        // неизвестное — отмечено.
        let mut edited = marks_request(&format!("{text}И ещё @совсем-новая.\n"));
        edited.evidence = Some(evidence);
        let marks = reference_marks(&vault, &edited).await;
        assert_eq!(missing_names(&marks), vec!["совсем-новая"], "{text:?}");
    }
}

// ── Удержание и повторы (T054) ──────────────────────────────────────────────

pub async fn held_marks_wait_for_the_build_and_duplicates_need_proof() {
    let vault = MemoryVault::new().with_file(NOTE, "");
    let mut request = marks_request("@старая @новая");
    request.held = true;
    request.previously_missing = vec!["старая".to_owned()];
    let marks = reference_marks(&vault, &request).await;
    assert_eq!(missing_names(&marks), vec!["старая"]);
    request.held = false;
    let marks = reference_marks(&vault, &request).await;
    assert_eq!(missing_names(&marks), vec!["старая", "новая"]);

    let duplicates = |marks: &[ReferenceMark]| -> Vec<(String, Vec<u32>)> {
        marks
            .iter()
            .filter_map(|mark| match &mark.kind {
                MarkKind::DuplicateLabel { name, others } => Some((
                    name.clone(),
                    others.iter().map(|other| other.line).collect(),
                )),
                _ => None,
            })
            .collect()
    };
    // Две метки верхнего уровня — отмечены обе, с местом друг друга.
    let text = "= А <одна>\n\n= Б <одна>\n";
    let marks = reference_marks(&vault, &marks_request(text)).await;
    assert_eq!(
        duplicates(&marks),
        vec![("одна".to_owned(), vec![3]), ("одна".to_owned(), vec![1])]
    );
    // Ветки `if`/`else` — не повтор без сборки, повтор при count ≥ 2.
    let text = "#if true [= А <ветка>] else [= Б <ветка>]\n";
    assert!(duplicates(&reference_marks(&vault, &marks_request(text)).await).is_empty());
    let mut request = marks_request(text);
    request.evidence = Some(BuildEvidence {
        labels: vec![typsidian_core::preview::evidence::BuiltLabel {
            name: "ветка".to_owned(),
            count: 2,
        }],
        references: Vec::new(),
    });
    assert_eq!(
        duplicates(&reference_marks(&vault, &request).await).len(),
        2
    );
    // Заметка и связанный файл — только при count ≥ 2.
    let mut request = marks_request("<общая>\n");
    request
        .linked
        .files
        .push(typsidian_core::completion::LinkedFile {
            path: "глава/часть.typ".to_owned(),
            labels: vec![typsidian_core::completion::LinkedLabel {
                name: "общая".to_owned(),
                kind: LabelKind::Other,
                text: String::new(),
                line: 4,
                column: 1,
                byte_start: 0,
                byte_end: 0,
                top_level: true,
            }],
        });
    assert!(duplicates(&reference_marks(&vault, &request).await).is_empty());
    request.evidence = Some(BuildEvidence {
        labels: vec![typsidian_core::preview::evidence::BuiltLabel {
            name: "общая".to_owned(),
            count: 2,
        }],
        references: Vec::new(),
    });
    let marks = reference_marks(&vault, &request).await;
    match &marks[0].kind {
        MarkKind::DuplicateLabel { others, .. } => {
            assert_eq!(others.len(), 1);
            assert_eq!(others[0].path.as_deref(), Some("глава/часть.typ"));
            assert_eq!(others[0].line, 4);
        }
        other => panic!("{other:?}"),
    }
}

// ── Отметки о заметках (T055) ───────────────────────────────────────────────

pub async fn note_links_are_marked_when_they_lead_nowhere() {
    let vault = MemoryVault::new()
        .with_file(NOTE, "")
        .with_file("глава/План.typ", "");
    let text = "[[План]] [[Нет]] [[../../наружу]] [[заметка]] @нет\n";
    let marks = reference_marks(&vault, &marks_request(text)).await;
    let kinds: Vec<&MarkKind> = marks.iter().map(|mark| &mark.kind).collect();
    assert_eq!(
        kinds,
        vec![
            &MarkKind::MissingNote {
                target: "Нет".to_owned(),
                create_path: "глава/Нет.typ".to_owned(),
            },
            &MarkKind::LinkOutsideVault {
                target: "../../наружу".to_owned(),
            },
            &MarkKind::MissingLabel {
                name: "нет".to_owned(),
            },
        ]
    );
    // По порядку текста и без пересечений.
    assert!(marks.windows(2).all(|pair| pair[0].to <= pair[1].from));
    let missing = &marks[0];
    let start = text.find("[[Нет]]").unwrap();
    assert_eq!(missing.from, text[..start].encode_utf16().count() as u32);
    assert_eq!(
        missing.to - missing.from,
        "[[Нет]]".encode_utf16().count() as u32
    );
}

// ── Свидетельство сборки (T011) ─────────────────────────────────────────────

pub fn build_evidence_names_labels_and_references() {
    let evidence = built(
        "#for i in range(2) [ #figure(rect(), caption: [Р]) #label(\"рис-\" + str(i)) ]\n\
         #set heading(numbering: \"1.\")\n\
         = Раздел <раздел>\n\
         #figure(rect(), caption: [А]) <дубль>\n\
         #figure(rect(), caption: [Б]) <дубль2>\n\
         См. @рис-1 и @раздел и @рис-1.\n",
    );
    assert_eq!(evidence.count("рис-0"), 1);
    assert_eq!(evidence.count("рис-1"), 1);
    assert_eq!(evidence.count("раздел"), 1);
    assert_eq!(evidence.count("нет"), 0);
    assert_eq!(evidence.references, vec!["раздел", "рис-1"]);
    // Повтор метки при удачной сборке — через `#label` в цикле без ссылок.
    let twice = built("#for i in range(2) [ #figure(rect(), caption: [Р]) #label(\"один\") ]\n");
    assert_eq!(twice.count("один"), 2);
    // У неудачной сборки свидетельства нет.
    let failed = compile_note(NOTE, "#неизвестная()", 1, FontSet::builtin()).unwrap();
    assert_eq!(failed.outcome, CompileOutcome::Failed);
    assert!(failed.evidence.is_none());
}

// ── Настройка и сочетания (T024, FR-009) ────────────────────────────────────

pub fn completion_setting_defaults_to_on_and_lives_on_the_device() {
    use typsidian_core::settings::apply::{route, ChangeTarget, SettingsChange};
    use typsidian_core::settings::Settings;

    assert!(Settings::default().editor_completion_auto);
    // Поля нет в файле — читается как «включено».
    let read: Settings = serde_json::from_str("{\"editor_font_size\": 15}").unwrap();
    assert!(read.editor_completion_auto);

    let change = SettingsChange {
        editor_completion_auto: Some(false),
        ..SettingsChange::default()
    };
    let next = change.apply_to(&Settings::default());
    assert!(!next.editor_completion_auto);
    let again: Settings = serde_json::from_str(&serde_json::to_string(&next).unwrap()).unwrap();
    assert!(
        !again.editor_completion_auto,
        "значение переживает запись и чтение"
    );

    // Привычка набора — устройству, даже при открытом vault.
    let routed = route(&change, true);
    assert_eq!(routed.len(), 1);
    assert_eq!(routed[0].target, ChangeTarget::Device);
}

pub fn completion_commands_have_free_defaults() {
    use typsidian_core::settings::keybindings::{CommandId, COMMANDS};
    for (command, text) in [
        (CommandId::GotoDefinition, "Mod+Alt+F"),
        (CommandId::TriggerCompletion, "Mod+Alt+A"),
    ] {
        let shortcut = command.default_shortcut();
        assert_eq!(shortcut.to_text(), text);
        assert_eq!(shortcut.problem(), None);
        assert!(shortcut.code().is_some());
        let same = COMMANDS
            .iter()
            .filter(|other| other.default_shortcut() == shortcut)
            .count();
        assert_eq!(same, 1, "{text} ни с кем не совпадает");
    }
}
