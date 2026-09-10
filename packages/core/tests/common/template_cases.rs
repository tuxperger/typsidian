//! Шаблоны и сниппеты — общая таблица проверок для обоих таргетов (фича 003).
//!
//! Один набор ожиданий на `cargo test` и `wasm-bindgen-test`: правила
//! библиотеки платформенно нейтральны, и расхождение результата между таргетами
//! является дефектом, а не особенностью платформы (принцип II). Две копии
//! таблицы разошлись бы при первой же правке контракта, оставив один таргет
//! проверенным по устаревшему правилу.

#![allow(dead_code)]

use typsidian_core::templates::{
    apply_insertion, check_name, folder_state, library, preview_of, read_template,
    save_as_template, ExcerptOnlySource, ExcerptReason, InsertionPoint, NameCheck, NameProblem,
    PreviewCache, PreviewSource, PreviewState, PreviewUnavailable, ReadTemplateError,
    SaveTemplateError, TemplateFolderState, TextSpan, EXCERPT_LINES,
};
use typsidian_core::vault::backend::{ReadError, VaultBackend};
use typsidian_core::vault::FolderProblem;

use super::MemoryVault;

const RAZDEL: &str = "---\nstatus: draft\n---\n= {{заголовок}}\n#lorem(30)\n";

/// Vault с двумя заготовками и одной обычной заметкой.
fn vault_with_templates() -> MemoryVault {
    MemoryVault::new()
        .with_file("templates/Раздел диссертации.typ", RAZDEL)
        .with_file(
            "templates/формулы/Выключная формула.typ",
            "$ {{выражение}} $ <eq:{{метка}}>\n",
        )
        .with_file("Диссертация/Результаты.typ", "= Результаты\n")
}

// ── Phase 2: состав библиотеки и состояние папки (T007, T008) ───────────

/// FR-009, FR-016: отсутствующая папка — штатное состояние, а не ошибка.
pub async fn a_missing_folder_is_not_an_error() {
    let vault = MemoryVault::new().with_file("Заметки/первая.typ", "= Первая\n");
    assert_eq!(folder_state(&vault).await, TemplateFolderState::Absent);

    let view = library(&vault).await;
    assert!(view.templates.is_empty(), "пустая библиотека — это норма");
    assert_eq!(view.folder_state, TemplateFolderState::Absent);
    assert!(
        view.folder_state.accepts_writes(),
        "«папки ещё нет» чинится первым же сохранением (FR-002)"
    );
}

/// FR-004: нечитаемая папка — отдельное состояние с названной причиной.
pub async fn an_unreadable_folder_does_not_collapse_into_absent() {
    let vault = vault_with_templates()
        .with_folder_problem("templates", FolderProblem::AccessDenied);

    let state = folder_state(&vault).await;
    assert_eq!(
        state,
        TemplateFolderState::Unreadable {
            reason: FolderProblem::AccessDenied
        },
        "«папки нет» и «папка не читается» — разные состояния"
    );
    assert!(!state.accepts_writes());
}

/// US2 AS6: чтение есть, записи нет — галерея работает, сохранение отклоняется.
pub async fn a_read_only_vault_makes_the_folder_not_writable() {
    let vault = vault_with_templates().read_only();

    let state = folder_state(&vault).await;
    assert_eq!(
        state,
        TemplateFolderState::NotWritable {
            reason: FolderProblem::VaultReadOnly
        }
    );
    assert!(!state.accepts_writes());
    assert_eq!(
        library(&vault).await.templates.len(),
        2,
        "состав библиотеки от права записи не зависит"
    );
}

/// FR-007: заметка попадает в библиотеку независимо от того, как она там
/// оказалась; вложенная папка даёт группу.
pub async fn any_note_inside_the_folder_is_a_template() {
    let view = library(&vault_with_templates()).await;
    assert_eq!(view.folder_state, TemplateFolderState::Present);

    let names: Vec<&str> = view
        .templates
        .iter()
        .map(|template| template.name.as_str())
        .collect();
    assert_eq!(names, vec!["Раздел диссертации.typ", "Выключная формула.typ"]);

    let groups: Vec<Option<&str>> = view
        .templates
        .iter()
        .map(|template| template.group.as_deref())
        .collect();
    assert_eq!(groups, vec![None, Some("формулы")]);
}

/// Доска внутри `templates/` заготовкой не является: её содержимое для ядра —
/// непрозрачные байты (фича 002), вставить их в текст нельзя.
pub async fn a_board_in_the_folder_is_not_a_template() {
    let vault = vault_with_templates().with_file("templates/схема.excalidraw", "{}");
    let view = library(&vault).await;
    assert_eq!(view.templates.len(), 2);
    assert!(!view
        .templates
        .iter()
        .any(|template| template.name.ends_with(".excalidraw")));
}

/// Папка заготовок — та, что в корне vault, а не любая с таким именем.
pub async fn a_nested_templates_folder_is_someone_elses() {
    let vault = MemoryVault::new()
        .with_file("templates/раздел.typ", RAZDEL)
        .with_file("курс/templates/лекция.typ", "= Лекция\n");

    let view = library(&vault).await;
    assert_eq!(view.templates.len(), 1);
    assert_eq!(view.templates[0].name, "раздел.typ");
}

// ── US1: превью и применение заготовки (T010-T015) ──────────────────────

/// FR-017: нечитаемый файл — `Unreadable`, и он не перезаписан.
pub async fn an_unreadable_template_is_reported_and_left_alone() {
    // Не-UTF-8: файл существует и читается с диска, но текстом не является.
    let broken: Vec<u8> = vec![0xff, 0xfe, 0x00, 0x41];
    let vault = MemoryVault::new().with_file("templates/битая.typ", broken.clone());
    let before = vault.stamp_of("templates/битая.typ");

    assert_eq!(
        read_template(&vault, std::path::Path::new("templates/битая.typ")).await,
        Err(ReadTemplateError::Unreadable)
    );
    assert_eq!(
        vault.stamp_of("templates/битая.typ"),
        before,
        "из нечитаемого состояния нет пути к записи"
    );
}

/// Отказ диска и нечитаемое содержимое — разные причины.
pub async fn a_disk_failure_is_not_the_same_as_bad_content() {
    let vault = MemoryVault::new()
        .with_file("templates/раздел.typ", RAZDEL)
        .with_failure("templates/раздел.typ", ReadError::Io);

    assert_eq!(
        read_template(&vault, std::path::Path::new("templates/раздел.typ")).await,
        Err(ReadTemplateError::Io)
    );
    assert_eq!(
        read_template(&vault, std::path::Path::new("templates/нет.typ")).await,
        Err(ReadTemplateError::NotFound)
    );
}

/// data-model.md: фрагмент режется по границе строки и не рвёт слово.
pub async fn the_excerpt_stops_at_a_line_boundary() {
    let long = (1..=40)
        .map(|number| format!("строка {number}"))
        .collect::<Vec<_>>()
        .join("\n");
    let taken = typsidian_core::templates::excerpt(&long);

    assert_eq!(taken.lines().count(), EXCERPT_LINES);
    assert!(taken.ends_with("строка 12"));
    assert!(
        !taken.contains("строка 13"),
        "предел по строкам обязан соблюдаться"
    );

    let one_line = "слово ".repeat(400);
    let cut = typsidian_core::templates::excerpt(&one_line);
    assert!(cut.chars().count() <= typsidian_core::templates::EXCERPT_CHARS);
    assert!(
        cut.ends_with("слово") || cut.ends_with("слово "),
        "обрубка посреди слова быть не должно, получено: …{}",
        &cut[cut.len().saturating_sub(20)..]
    );
}

/// **Ключевой тест деградации** (T012, FR-012, contracts/preview-source.md):
/// `ExcerptOnlySource` на любом содержимом даёт `CompilerUnavailable`, а не
/// `DoesNotCompile`. Схлопывание причин сказало бы пользователю, что сломаны
/// все его заготовки, тогда как сломан конвейер превью.
pub async fn the_excerpt_source_never_claims_the_content_is_broken() {
    let source = ExcerptOnlySource;
    for content in [RAZDEL, "", "#table(columns: 3,", "совершенно валидный текст"] {
        match source.preview(content) {
            PreviewState::SourceExcerpt { reason, .. } => assert_eq!(
                reason,
                ExcerptReason::CompilerUnavailable,
                "источник без компилятора не вправе утверждать, что содержимое не собирается"
            ),
            other => panic!("ожидался SourceExcerpt, получено {other:?}"),
        }
    }
    assert_eq!(
        source.availability(),
        typsidian_core::templates::PreviewAvailability::CompilerUnavailable
    );
}

/// Источник, считающий обращения к себе: по счётчику видно попадание в кэш.
struct CountingSource {
    calls: std::cell::Cell<usize>,
}

impl CountingSource {
    fn new() -> Self {
        Self {
            calls: std::cell::Cell::new(0),
        }
    }
}

impl PreviewSource for CountingSource {
    fn availability(&self) -> typsidian_core::templates::PreviewAvailability {
        typsidian_core::templates::PreviewAvailability::CompilerUnavailable
    }

    fn preview(&self, content: &str) -> PreviewState {
        self.calls.set(self.calls.get() + 1);
        ExcerptOnlySource.preview(content)
    }
}

/// SC-010, research.md §4: ключ кэша — хэш содержимого. Попадание и промах
/// дают одинаковый результат; правка содержимого пересчитывает превью,
/// переименование файла — нет.
pub async fn the_cache_is_keyed_by_content_not_by_path() {
    let source = CountingSource::new();
    let mut cache = PreviewCache::new();

    let vault = MemoryVault::new().with_file("templates/раздел.typ", RAZDEL);
    let template = library(&vault).await.templates.remove(0);

    let first = preview_of(&vault, &source, &mut cache, &template).await;
    let second = preview_of(&vault, &source, &mut cache, &template).await;
    assert_eq!(first, second, "попадание и промах дают одинаковый результат");
    assert_eq!(source.calls.get(), 1, "второй раз порт не вызывается");

    // Переименование: путь другой, содержимое то же — пересчёта нет.
    let renamed = MemoryVault::new().with_file("templates/другое имя.typ", RAZDEL);
    let moved = library(&renamed).await.templates.remove(0);
    let after_rename = preview_of(&renamed, &source, &mut cache, &moved).await;
    assert_eq!(after_rename, first);
    assert_eq!(
        source.calls.get(),
        1,
        "переименование файла превью не инвалидирует"
    );

    // Правка содержимого: хэш другой — превью пересчитывается.
    let edited = MemoryVault::new().with_file("templates/раздел.typ", "= другое\n");
    let changed = library(&edited).await.templates.remove(0);
    preview_of(&edited, &source, &mut cache, &changed).await;
    assert_eq!(source.calls.get(), 2, "изменение содержимого вызывает пересчёт");
}

/// FR-017, запрет 2 из data-model.md: нечитаемый файл даёт `Unavailable`, а не
/// `SourceExcerpt` с пустым фрагментом.
pub async fn an_unreadable_template_previews_as_unavailable() {
    let vault = MemoryVault::new()
        .with_file("templates/битая.typ", vec![0xff, 0xfe])
        .with_file("templates/целая.typ", RAZDEL);
    let templates = library(&vault).await.templates;
    let broken = templates
        .iter()
        .find(|template| template.name == "битая.typ")
        .expect("битая заготовка в библиотеке");

    let mut cache = PreviewCache::new();
    let state = preview_of(&vault, &ExcerptOnlySource, &mut cache, broken).await;
    assert_eq!(
        state,
        PreviewState::Unavailable {
            reason: PreviewUnavailable::Unreadable
        },
        "нечитаемый файл и некомпилируемое содержимое требуют разных действий"
    );

    // Галерея при этом не прерывается: остальные заготовки на месте.
    assert_eq!(templates.len(), 2);
}

/// FR-023, SC-008, инвариант 1: применение заготовки не изменяет её файл.
pub async fn applying_a_template_leaves_its_file_untouched() {
    let mut vault = MemoryVault::new().with_file("templates/раздел.typ", RAZDEL);
    let before = vault.stamp_of("templates/раздел.typ");

    // Создание заметки из заготовки — чтение плюс существующие примитивы 001.
    let content = read_template(&vault, std::path::Path::new("templates/раздел.typ"))
        .await
        .expect("заготовка читается");
    vault
        .write(std::path::Path::new("Глава 4.typ"), &content, None)
        .await
        .expect("новая заметка пишется");

    assert_eq!(
        vault.stamp_of("templates/раздел.typ"),
        before,
        "файл заготовки не изменён"
    );
    assert_eq!(
        vault.stamp_of("Глава 4.typ").map(|stamp| stamp.content_hash),
        before.map(|stamp| stamp.content_hash),
        "новая заметка получила ровно содержимое заготовки (FR-021)"
    );
}

// ── US2: сохранение как заготовки (T026-T030) ──────────────────────────

/// FR-029, инвариант 6: недопустимое имя отклоняется, на диске ничего не
/// меняется.
pub async fn an_invalid_name_never_touches_the_disk() {
    let mut vault = MemoryVault::new().with_file("Диссертация/Результаты.typ", "= Результаты\n");
    let before = library(&vault).await.templates.len();

    assert_eq!(
        check_name(&vault, "папка/заготовка").await,
        NameCheck::Invalid {
            reason: NameProblem::IllegalCharacters
        }
    );
    assert_eq!(
        check_name(&vault, "  ").await,
        NameCheck::Invalid {
            reason: NameProblem::Empty
        }
    );

    assert_eq!(
        save_as_template(&mut vault, "папка/заготовка", "= что-то\n", false).await,
        Err(SaveTemplateError::InvalidName {
            reason: NameProblem::IllegalCharacters
        })
    );
    assert_eq!(
        library(&vault).await.templates.len(),
        before,
        "отказ по имени не создаёт файлов"
    );
}

/// FR-028: занятое имя без подтверждения не трогает существующий файл.
pub async fn a_taken_name_without_confirmation_keeps_the_existing_file() {
    let mut vault = MemoryVault::new().with_file("templates/Раздел.typ", RAZDEL);
    let before = vault.stamp_of("templates/Раздел.typ");

    assert_eq!(check_name(&vault, "Раздел").await, NameCheck::NameTaken);
    assert_eq!(
        save_as_template(&mut vault, "Раздел", "= совсем другое\n", false).await,
        Err(SaveTemplateError::NameTaken)
    );
    assert_eq!(
        vault.stamp_of("templates/Раздел.typ"),
        before,
        "существующий файл не тронут"
    );

    // С подтверждением — перезаписывается.
    let saved = save_as_template(&mut vault, "Раздел", "= совсем другое\n", true)
        .await
        .expect("подтверждённая перезапись проходит");
    assert_eq!(saved.path, std::path::PathBuf::from("templates/Раздел.typ"));
    assert_ne!(vault.stamp_of("templates/Раздел.typ"), before);
}

/// FR-002: папка заготовок создаётся первым сохранением, без отдельных
/// действий пользователя.
pub async fn the_first_save_creates_the_folder() {
    let mut vault = MemoryVault::new().with_file("Диссертация/Результаты.typ", "= Результаты\n");
    assert_eq!(folder_state(&vault).await, TemplateFolderState::Absent);

    let saved = save_as_template(&mut vault, "Раздел диссертации", RAZDEL, false)
        .await
        .expect("сохранение в отсутствующую папку проходит");

    assert_eq!(
        saved.path,
        std::path::PathBuf::from("templates/Раздел диссертации.typ")
    );
    assert_eq!(folder_state(&vault).await, TemplateFolderState::Present);
    assert_eq!(library(&vault).await.templates.len(), 1);
}

/// FR-030: папка недоступна на запись — отказ с причиной, исходная заметка
/// нетронута.
pub async fn a_read_only_vault_refuses_the_save_with_a_reason() {
    let mut vault = MemoryVault::new()
        .with_file("templates/Раздел.typ", RAZDEL)
        .with_file("Диссертация/Результаты.typ", "= Результаты\n")
        .read_only();
    let source = vault.stamp_of("Диссертация/Результаты.typ");

    assert_eq!(
        save_as_template(&mut vault, "Новая", "= что-то\n", false).await,
        Err(SaveTemplateError::FolderNotWritable {
            reason: FolderProblem::VaultReadOnly
        })
    );
    assert_eq!(
        vault.stamp_of("Диссертация/Результаты.typ"),
        source,
        "исходная заметка не тронута"
    );
    assert!(
        vault.stamp_of("templates/Новая.typ").is_none(),
        "файла заготовки не появилось"
    );
}

/// FR-027, инвариант 3: содержимое заготовки зафиксировано на момент
/// сохранения — правка исходной заметки её не меняет.
pub async fn the_saved_content_is_frozen_at_save_time() {
    let mut vault = MemoryVault::new().with_file("Диссертация/Результаты.typ", "= Было\n");

    save_as_template(&mut vault, "Снимок", "= Было\n", false)
        .await
        .expect("сохранение проходит");
    let saved = vault.stamp_of("templates/Снимок.typ");

    vault
        .write(
            std::path::Path::new("Диссертация/Результаты.typ"),
            "= Стало\n",
            None,
        )
        .await
        .expect("правка исходной заметки проходит");

    assert_eq!(
        vault.stamp_of("templates/Снимок.typ"),
        saved,
        "связи между заметкой и заготовкой не существует"
    );
    assert_eq!(
        read_template(&vault, std::path::Path::new("templates/Снимок.typ")).await,
        Ok("= Было\n".to_owned())
    );
}

// ── US3: вставка в позицию курсора (T035, T036) ────────────────────────

/// FR-032, FR-035: содержимое вставляется дословно и заменяет выделение.
pub async fn insertion_is_verbatim_and_replaces_the_selection() {
    let text = "Начало и конец";
    let snippet = "$ {{выражение}} $";

    let at_cursor = InsertionPoint {
        offset: 7,
        selection: None,
    };
    assert_eq!(
        apply_insertion(text, &at_cursor, snippet),
        "Начало $ {{выражение}} $и конец",
        "подстановок нет: параметризация вне скоупа (инвариант 4)"
    );

    let over_selection = InsertionPoint {
        offset: 0,
        selection: Some(TextSpan { start: 0, end: 6 }),
    };
    assert_eq!(
        apply_insertion(text, &over_selection, snippet),
        "$ {{выражение}} $ и конец"
    );
    assert_eq!(
        typsidian_core::templates::caret_after_insertion(&over_selection, snippet),
        snippet.encode_utf16().count(),
        "курсор встаёт сразу за вставленным — одинаково от вставки к вставке (FR-034)"
    );

    // Смещения — кодовые единицы UTF-16, как их считает редактор. Символ вне
    // BMP занимает две: считай ядро в `char`, курсор уезжал бы на каждом таком.
    let astral = InsertionPoint {
        offset: 2,
        selection: None,
    };
    assert_eq!(apply_insertion("𝔸б", &astral, "!"), "𝔸!б");
    assert_eq!(
        typsidian_core::templates::caret_after_insertion(
            &InsertionPoint {
                offset: 0,
                selection: None
            },
            "𝔸"
        ),
        2
    );
}

/// SC-008, инвариант 1: вставка не изменяет файл заготовки.
pub async fn inserting_does_not_touch_the_template_file() {
    let vault = MemoryVault::new()
        .with_file("templates/формула.typ", "$ x = y $")
        .with_file("Заметка.typ", "текст");
    let before = vault.stamp_of("templates/формула.typ");

    let content = read_template(&vault, std::path::Path::new("templates/формула.typ"))
        .await
        .expect("заготовка читается");
    let _ = apply_insertion(
        "текст",
        &InsertionPoint {
            offset: 5,
            selection: None,
        },
        &content,
    );

    assert_eq!(vault.stamp_of("templates/формула.typ"), before);
}

// ── Phase 7: наследство от vault-manager (T048-T050) ────────────────────

/// FR-005: папка заготовок присутствует в дереве как обычная папка, а
/// заготовки — как обычные заметки. Отдельной ветки для них нет.
pub async fn templates_appear_in_the_tree_as_ordinary_notes() {
    let vault = vault_with_templates();
    let tree = VaultBackend::list(&vault).await;

    let in_templates: Vec<&typsidian_core::vault::index::Note> = tree
        .iter()
        .filter(|note| note.path.starts_with("templates"))
        .collect();
    assert_eq!(in_templates.len(), 2);
    assert!(
        in_templates
            .iter()
            .all(|note| note.kind == typsidian_core::vault::FileKind::Note),
        "заготовка — обычная заметка, а не отдельный вид файла"
    );

    // Непосредственные дети `templates/` доступны тем же вызовом, что и любая
    // другая папка дерева.
    let direct = VaultBackend::list_folder(&vault, std::path::Path::new("templates")).await;
    assert_eq!(direct.len(), 1);
    assert_eq!(direct[0].name, "Раздел диссертации.typ");
}

/// FR-006 (**MUST NOT**): отдельного пути редактирования заготовки не
/// существует. Проверяется отсутствие второго пути, а не наличие первого:
/// заготовка открывается, правится и сохраняется теми же вызовами
/// `VaultBackend`, что и любая заметка.
pub async fn a_template_is_edited_through_the_ordinary_note_path() {
    let mut vault = MemoryVault::new().with_file("templates/Раздел.typ", RAZDEL);
    let path = std::path::Path::new("templates/Раздел.typ");

    let text = VaultBackend::read(&vault, path)
        .await
        .expect("заготовка читается обычным `read`");
    assert_eq!(text, RAZDEL);

    VaultBackend::begin_edit(&mut vault, path, format!("{text}// правка\n")).await;
    VaultBackend::write(&mut vault, path, &format!("{text}// правка\n"), None)
        .await
        .expect("заготовка пишется обычным `write`");

    assert_eq!(
        VaultBackend::read(&vault, path).await.unwrap(),
        format!("{RAZDEL}// правка\n")
    );
}

/// FR-008: заготовка участвует в графе ссылок наравне с прочими заметками.
pub async fn templates_take_part_in_the_link_graph() {
    let vault = MemoryVault::new()
        // Ссылки от корня vault: цель без ведущей косой черты разрешается
        // относительно папки самой заметки (`graph::resolve_link_target`), и
        // из `templates/` это была бы другая заметка.
        .with_file(
            "templates/Раздел.typ",
            "= Раздел\nСм. [[/Диссертация/Результаты]]\n",
        )
        .with_file(
            "Диссертация/Результаты.typ",
            "= Результаты\nПо шаблону [[/templates/Раздел]]\n",
        );

    let to_note = VaultBackend::backlinks(&vault, std::path::Path::new("Диссертация/Результаты.typ"))
        .await;
    assert_eq!(
        to_note
            .iter()
            .map(|note| note.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Раздел.typ"],
        "ссылка из заготовки на заметку видна графу"
    );

    let to_template =
        VaultBackend::backlinks(&vault, std::path::Path::new("templates/Раздел.typ")).await;
    assert_eq!(
        to_template
            .iter()
            .map(|note| note.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Результаты.typ"],
        "ссылка на заготовку видна графу тоже"
    );
}
