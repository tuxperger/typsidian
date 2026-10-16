//! Быстрый захват — общая таблица для обоих таргетов (фича 022).
//!
//! Правила чистые и работают поверх `VaultBackend`, поэтому одни и те же
//! проверки гоняются на native и в браузере (принцип II): расхождение между
//! таргетами было бы дефектом. Файлы `capture_native.rs` и `capture_wasm.rs`
//! только вызывают эти функции своим harness'ом.

#![allow(dead_code)]

use typsidian_core::capture::{
    append, capture_settings, capture_target_path, capture_write, daily_date_of, daily_neighbours,
    daily_path, entry, open_daily, Capture, CaptureContent, CaptureFailure, CaptureMoment,
    CaptureOutcome, CaptureOverrides, CaptureSettings, CaptureTarget, DailyDate, DailyError,
    DailyName, DailyTemplateUse, EntryStyle,
};
use typsidian_core::messages::AppMessage;
use typsidian_core::vault::vault_settings::{
    parse, read_vault_settings, render, write_vault_settings, VaultSettingsError,
};
use typsidian_core::vault::NOTE_SIZE_LIMIT_BYTES;

use super::MemoryVault;

const SETTINGS_FILE: &str = ".typsidian/vault.json";

fn moment() -> CaptureMoment {
    CaptureMoment {
        year: 2026,
        month: 10,
        day: 16,
        hour: 9,
        minute: 5,
    }
}

fn date(year: u16, month: u8, day: u8) -> DailyDate {
    DailyDate { year, month, day }
}

fn capture(content: CaptureContent) -> Capture {
    Capture {
        id: "c1".to_owned(),
        content,
        note: None,
        at: moment(),
        target: CaptureTarget::Inbox,
    }
}

fn text(text: &str) -> Capture {
    capture(CaptureContent::Text {
        text: text.to_owned(),
    })
}

fn to(mut capture: Capture, target: CaptureTarget) -> Capture {
    capture.target = target;
    capture
}

fn defaults() -> CaptureSettings {
    CaptureSettings::default()
}

fn with(configure: impl FnOnce(&mut CaptureSettings)) -> CaptureSettings {
    let mut settings = defaults();
    configure(&mut settings);
    settings
}

fn content_of(vault: &MemoryVault, path: &str) -> String {
    String::from_utf8(
        vault
            .bytes_of(path)
            .unwrap_or_else(|| panic!("нет файла {path}")),
    )
    .unwrap()
}

/// Настройки vault одной строкой JSON.
fn vault_with_settings(json: &str) -> MemoryVault {
    MemoryVault::new().with_file(SETTINGS_FILE, json)
}

/// Заголовок PNG и немного «пикселей» — для определения вида по байтам.
fn png(extra: u8) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend_from_slice(&[0, 0, 0, 13, extra]);
    bytes
}

fn image_capture() -> Capture {
    capture(CaptureContent::Image {
        file: "pending/c1.bin".to_owned(),
        mime: "image/png".to_owned(),
        name: "снимок.png".to_owned(),
    })
}

// ── Настройки ───────────────────────────────────────────────────────────────

/// Ничего не задано — действуют умолчания спеки (US3, AS2).
pub fn defaults_apply_when_nothing_is_set() {
    let settings = capture_settings(None);
    assert_eq!(settings.inbox, "входящие.typ");
    assert_eq!(settings.daily_folder, "дневник");
    assert_eq!(settings.daily_template, None);
    assert_eq!(settings.daily_name, DailyName::Iso);
    assert_eq!(settings.entry_style, EntryStyle::List);
    assert!(settings.entry_time);
    assert_eq!(
        settings,
        capture_settings(Some(&CaptureOverrides::default()))
    );
    assert_eq!(parse("").unwrap().capture, None);
}

/// Неизвестные и негодные значения — «не задано», файл остаётся читаемым.
pub fn unknown_values_fall_back_to_defaults() {
    let overrides = parse(
        r#"{
          "settings": {"theme": "dark"},
          "capture": {
            "inbox": "../снаружи.typ",
            "daily_folder": 7,
            "daily_name": "DD/MM",
            "entry_style": "table",
            "entry_time": "yes"
          }
        }"#,
    )
    .expect("негодное значение не делает файл нечитаемым");
    assert_eq!(overrides.capture, None);
    assert_eq!(capture_settings(overrides.capture.as_ref()), defaults());
    assert!(overrides.theme.is_some(), "остальные настройки целы");

    // Годное значение рядом с негодным выживает.
    let partial = parse(r#"{"capture": {"entry_style": "paragraph", "daily_name": "?"}}"#).unwrap();
    let settings = capture_settings(partial.capture.as_ref());
    assert_eq!(settings.entry_style, EntryStyle::Paragraph);
    assert_eq!(settings.daily_name, DailyName::Iso);
}

/// Все шесть полей переживают «записал → прочитал», образцы названий — в файле
/// словами (FR-010).
pub fn settings_survive_the_round_trip() {
    let overrides = CaptureOverrides {
        inbox: Some("Заметки/входящие.typ".into()),
        daily_folder: Some("дневник".into()),
        daily_template: Some("templates/день.typ".into()),
        daily_name: Some(DailyName::IsoNested),
        entry_style: Some(EntryStyle::Paragraph),
        entry_time: Some(false),
    };
    let file = render(
        "",
        &typsidian_core::vault::vault_settings::VaultOverrides {
            capture: Some(overrides.clone()),
            ..Default::default()
        },
    );
    assert!(
        file.contains("\"daily_name\": \"YYYY/MM/YYYY-MM-DD\""),
        "{file}"
    );
    assert!(file.contains("\"entry_style\": \"paragraph\""), "{file}");
    assert_eq!(parse(&file).unwrap().capture, Some(overrides));
}

/// Запись настроек захвата не трогает прочие ключи и ключи самой таблицы,
/// которых приложение не знает; пустые настройки убирают таблицу.
pub fn writing_capture_keeps_other_keys() {
    let authored = r#"{
  "settings": {"theme": "dark"},
  "capture": {"future_option": 1, "entry_time": false},
  "my_future_option": {"nested": true}
}"#;
    let mut overrides = parse(authored).unwrap();
    overrides.capture.as_mut().unwrap().entry_style = Some(EntryStyle::Paragraph);
    let rewritten = render(authored, &overrides);
    assert!(rewritten.contains("my_future_option"), "{rewritten}");
    assert!(rewritten.contains("future_option"), "{rewritten}");
    assert!(rewritten.contains("\"theme\": \"dark\""), "{rewritten}");
    assert!(
        rewritten.contains("\"entry_style\": \"paragraph\""),
        "{rewritten}"
    );

    overrides.capture = None;
    let cleared = render(&rewritten, &overrides);
    assert!(!cleared.contains("entry_style"), "{cleared}");
    assert!(!cleared.contains("entry_time"), "{cleared}");
    assert!(cleared.contains("my_future_option"));
}

/// Запись через backend приводит пути к нормальной форме, пустое поле
/// сбрасывает значение, негодный путь отклоняется до записи.
pub async fn writing_settings_normalizes_and_refuses_bad_paths() {
    let mut vault = MemoryVault::new();
    let written = typsidian_core::vault::vault_settings::VaultOverrides {
        capture: Some(CaptureOverrides {
            inbox: Some(" Мысли//входящие ".into()),
            daily_folder: Some("Дневник/".into()),
            daily_template: Some("   ".into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    write_vault_settings(&mut vault, &written).await.unwrap();
    let read = read_vault_settings(&vault).await.unwrap().capture.unwrap();
    assert_eq!(read.inbox.as_deref(), Some("Мысли/входящие.typ"));
    assert_eq!(read.daily_folder.as_deref(), Some("Дневник"));
    assert_eq!(read.daily_template, None);

    let before = vault.bytes_of(SETTINGS_FILE);
    let bad = typsidian_core::vault::vault_settings::VaultOverrides {
        capture: Some(CaptureOverrides {
            daily_folder: Some("../снаружи".into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let error = write_vault_settings(&mut vault, &bad).await.unwrap_err();
    assert!(
        matches!(error, VaultSettingsError::InvalidCapture { .. }),
        "{error:?}"
    );
    assert_eq!(vault.bytes_of(SETTINGS_FILE), before, "файл не тронут");
}

// ── Ежедневная заметка ──────────────────────────────────────────────────────

/// Путь по трём видам названия (FR-011).
pub fn daily_path_follows_the_three_name_forms() {
    let day = date(2026, 10, 6);
    assert_eq!(
        daily_path(&defaults(), &day).unwrap(),
        "дневник/2026-10-06.typ"
    );
    let nested = with(|s| s.daily_name = DailyName::IsoNested);
    assert_eq!(
        daily_path(&nested, &day).unwrap(),
        "дневник/2026/10/2026-10-06.typ"
    );
    let dotted = with(|s| {
        s.daily_name = DailyName::Dotted;
        s.daily_folder = "Журнал/личное".into();
    });
    assert_eq!(
        daily_path(&dotted, &day).unwrap(),
        "Журнал/личное/06.10.2026.typ"
    );
}

/// Несуществующий день пути не имеет; високосный год учтён.
pub fn impossible_days_have_no_path() {
    for (year, month, day) in [
        (2026, 2, 29),
        (2026, 4, 31),
        (2026, 13, 1),
        (2026, 0, 1),
        (2026, 1, 0),
        (0, 1, 1),
    ] {
        assert_eq!(
            daily_path(&defaults(), &date(year, month, day)),
            None,
            "{year}-{month}-{day}"
        );
    }
    assert!(daily_path(&defaults(), &date(2028, 2, 29)).is_some());
    assert!(daily_path(&defaults(), &date(2000, 2, 29)).is_some());
    assert_eq!(daily_path(&defaults(), &date(1900, 2, 29)), None);
}

/// Дата по пути — обратная к `daily_path` и отвергает всё прочее.
pub fn the_date_of_a_path_is_the_inverse_of_the_path() {
    let day = date(2026, 10, 6);
    for name in [DailyName::Iso, DailyName::IsoNested, DailyName::Dotted] {
        let settings = with(|s| s.daily_name = name);
        let path = daily_path(&settings, &day).unwrap();
        assert_eq!(daily_date_of(&settings, &path), Some(day), "{path}");
    }
    let nested = with(|s| s.daily_name = DailyName::IsoNested);
    // Не та папка, не та вложенность, не тот вид названия, нет такого дня.
    assert_eq!(daily_date_of(&defaults(), "другая/2026-10-06.typ"), None);
    assert_eq!(daily_date_of(&defaults(), "дневник/2026-10-6.typ"), None);
    assert_eq!(daily_date_of(&defaults(), "дневник/06.10.2026.typ"), None);
    assert_eq!(daily_date_of(&defaults(), "дневник/2026-02-30.typ"), None);
    assert_eq!(daily_date_of(&nested, "дневник/2026-10-06.typ"), None);
    assert_eq!(
        daily_date_of(&nested, "дневник/2025/10/2026-10-06.typ"),
        None
    );
}

/// Нет заметки — создана в папке настроек и названа датой; без заготовки это
/// заголовок с названием (FR-007, US2 AS1).
pub async fn today_creates_the_note_without_a_template() {
    let mut vault = MemoryVault::new();
    let opened = open_daily(&mut vault, &defaults(), &date(2026, 10, 16))
        .await
        .unwrap();
    assert_eq!(opened.path, "дневник/2026-10-16.typ");
    assert!(opened.created);
    assert_eq!(opened.template, DailyTemplateUse::None);
    assert_eq!(content_of(&vault, &opened.path), "= 2026-10-16\n");
}

/// Заготовка 003 ложится в заметку дословно.
pub async fn today_creates_the_note_from_the_template() {
    let mut vault = MemoryVault::new().with_file("templates/день.typ", "= План\n\n- [ ] главное\n");
    let settings = with(|s| s.daily_template = Some("templates/день.typ".into()));
    let opened = open_daily(&mut vault, &settings, &date(2026, 10, 16))
        .await
        .unwrap();
    assert!(opened.created);
    assert_eq!(opened.template, DailyTemplateUse::Applied);
    assert_eq!(
        content_of(&vault, &opened.path),
        "= План\n\n- [ ] главное\n"
    );
    // Сама заготовка не тронута (003, инвариант 1).
    assert_eq!(
        content_of(&vault, "templates/день.typ"),
        "= План\n\n- [ ] главное\n"
    );
}

/// Заготовка указана, но её нет: «Сегодня» не отказывает, это видно в ответе.
pub async fn a_missing_template_does_not_block_today() {
    let mut vault = MemoryVault::new();
    let settings = with(|s| s.daily_template = Some("templates/нет.typ".into()));
    let opened = open_daily(&mut vault, &settings, &date(2026, 10, 16))
        .await
        .unwrap();
    assert!(opened.created);
    assert_eq!(opened.template, DailyTemplateUse::Missing);
    assert_eq!(content_of(&vault, &opened.path), "= 2026-10-16\n");
}

/// Сколько бы раз ни нажали — одна заметка, написанное в ней цело (SC-004,
/// US2 AS2); новые сутки — новая заметка (AS3).
pub async fn today_twice_is_one_note_and_a_new_day_is_a_new_one() {
    let mut vault = MemoryVault::new();
    let first = open_daily(&mut vault, &defaults(), &date(2026, 10, 16))
        .await
        .unwrap();
    assert!(first.created);
    vault.replace_externally(&first.path, "= 2026-10-16\nмоё\n");
    for _ in 0..3 {
        let again = open_daily(&mut vault, &defaults(), &date(2026, 10, 16))
            .await
            .unwrap();
        assert_eq!(again.path, first.path);
        assert!(!again.created);
    }
    assert_eq!(content_of(&vault, &first.path), "= 2026-10-16\nмоё\n");
    assert_eq!(vault.paths_under("дневник").len(), 1);

    let next = open_daily(&mut vault, &defaults(), &date(2026, 10, 17))
        .await
        .unwrap();
    assert_eq!(next.path, "дневник/2026-10-17.typ");
    assert!(next.created);
    assert_eq!(vault.paths_under("дневник").len(), 2);
}

/// Vault только для чтения: существующая заметка открывается, новая не
/// создаётся и причина названа; недоступный vault — отдельная причина.
pub async fn today_in_a_read_only_vault() {
    let mut vault = MemoryVault::new()
        .with_file("дневник/2026-10-16.typ", "есть")
        .read_only();
    let existing = open_daily(&mut vault, &defaults(), &date(2026, 10, 16))
        .await
        .unwrap();
    assert!(!existing.created);
    assert_eq!(
        open_daily(&mut vault, &defaults(), &date(2026, 10, 17)).await,
        Err(DailyError::ReadOnly)
    );
    assert_eq!(vault.paths_under("дневник").len(), 1);

    let mut gone = MemoryVault::new().unavailable();
    assert_eq!(
        open_daily(&mut gone, &defaults(), &date(2026, 10, 17)).await,
        Err(DailyError::NoVault)
    );
    assert_eq!(
        open_daily(&mut MemoryVault::new(), &defaults(), &date(2026, 2, 30)).await,
        Err(DailyError::InvalidDate)
    );
}

/// ‹ ›: ближайшая существующая в обе стороны через пропуски дней; посторонние
/// заметки и другой вид названия не мешают (FR-008).
pub async fn neighbours_skip_the_gaps() {
    let vault = MemoryVault::new()
        .with_file("дневник/2026-10-01.typ", "")
        .with_file("дневник/2026-10-05.typ", "")
        .with_file("дневник/2026-10-20.typ", "")
        .with_file("дневник/2025-12-31.typ", "")
        .with_file("дневник/заметка.typ", "")
        .with_file("дневник/06.10.2026.typ", "")
        .with_file("другая/2026-10-09.typ", "");
    let middle = daily_neighbours(&vault, &defaults(), &date(2026, 10, 10)).await;
    assert_eq!(middle.previous.as_deref(), Some("дневник/2026-10-05.typ"));
    assert_eq!(middle.next.as_deref(), Some("дневник/2026-10-20.typ"));

    let on_a_note = daily_neighbours(&vault, &defaults(), &date(2026, 10, 5)).await;
    assert_eq!(
        on_a_note.previous.as_deref(),
        Some("дневник/2026-10-01.typ")
    );
    assert_eq!(on_a_note.next.as_deref(), Some("дневник/2026-10-20.typ"));

    let first = daily_neighbours(&vault, &defaults(), &date(2025, 12, 31)).await;
    assert_eq!(first.previous, None);
    let last = daily_neighbours(&vault, &defaults(), &date(2026, 10, 20)).await;
    assert_eq!(last.next, None);
}

/// Соседи находятся и во вложенных папках по годам и месяцам.
pub async fn neighbours_work_with_nested_folders() {
    let nested = with(|s| s.daily_name = DailyName::IsoNested);
    let vault = MemoryVault::new()
        .with_file("дневник/2026/09/2026-09-30.typ", "")
        .with_file("дневник/2026/10/2026-10-03.typ", "")
        .with_file("дневник/2026/10/2026-10-02.typ", "")
        .with_file("дневник/2025/10/2026-10-04.typ", "");
    let around = daily_neighbours(&vault, &nested, &date(2026, 10, 2)).await;
    assert_eq!(
        around.previous.as_deref(),
        Some("дневник/2026/09/2026-09-30.typ")
    );
    assert_eq!(
        around.next.as_deref(),
        Some("дневник/2026/10/2026-10-03.typ")
    );
}

// ── Запись захвата ──────────────────────────────────────────────────────────

/// Список с временем, абзац, без времени (FR-010, US3).
pub fn entry_styles_and_time() {
    let hello = text("привет");
    assert_eq!(entry(&defaults(), &hello, None), "- 09:05 привет");
    let paragraph = with(|s| s.entry_style = EntryStyle::Paragraph);
    assert_eq!(entry(&paragraph, &hello, None), "09:05 привет");
    let no_time = with(|s| s.entry_time = false);
    assert_eq!(entry(&no_time, &hello, None), "- привет");
    let bare = with(|s| {
        s.entry_time = false;
        s.entry_style = EntryStyle::Paragraph;
    });
    assert_eq!(entry(&bare, &hello, None), "привет");
}

/// Ссылка с заголовком и без, картинка, приписка через « — ».
pub fn entry_kinds_and_the_note() {
    let titled = capture(CaptureContent::Link {
        url: "https://example.org/a_b?x=1".to_owned(),
        title: Some("Статья [1]".to_owned()),
    });
    assert_eq!(
        entry(&defaults(), &titled, None),
        "- 09:05 #link(\"https://example.org/a_b?x=1\")[Статья \\[1\\]]"
    );
    for title in [
        None,
        Some("https://example.org".to_owned()),
        Some("  ".to_owned()),
    ] {
        let plain = capture(CaptureContent::Link {
            url: "https://example.org".to_owned(),
            title,
        });
        assert_eq!(
            entry(&defaults(), &plain, None),
            "- 09:05 #link(\"https://example.org\")"
        );
    }

    assert_eq!(
        entry(&defaults(), &image_capture(), Some("assets/а/б.png")),
        "- 09:05 #image(\"assets/а/б.png\")"
    );
    assert_eq!(
        entry(&defaults(), &image_capture(), None),
        "- 09:05 #image(\"снимок.png\")"
    );

    let mut noted = text("идея");
    noted.note = Some("  проверить  ".to_owned());
    assert_eq!(entry(&defaults(), &noted, None), "- 09:05 идея — проверить");
    let mut only_note = text("");
    only_note.note = Some("мысль".to_owned());
    assert_eq!(entry(&defaults(), &only_note, None), "- 09:05 мысль");
    let mut with_link = titled.clone();
    with_link.note = Some("прочесть # вечером".to_owned());
    assert!(entry(&defaults(), &with_link, None).ends_with(" — прочесть \\# вечером"));
}

/// Захваченный текст остаётся текстом: ничто не становится разметкой.
pub fn entry_escapes_markup() {
    let hostile = text("#import \"x\" *жирный* _курсив_ $x$ `код` @ссылка <метка> [a]");
    assert_eq!(
        entry(&defaults(), &hostile, None),
        "- 09:05 \\#import \"x\" \\*жирный\\* \\_курсив\\_ \\$x\\$ \\`код\\` \\@ссылка \\<метка> \\[a\\]"
    );
    // В абзаце без времени начало строки тоже экранируется.
    let bare = with(|s| {
        s.entry_style = EntryStyle::Paragraph;
        s.entry_time = false;
    });
    assert_eq!(
        entry(&bare, &text("= не заголовок"), None),
        "\\= не заголовок"
    );
    assert_eq!(entry(&bare, &text("- не список"), None), "\\- не список");
    assert_eq!(
        entry(&bare, &text("1. не нумерация"), None),
        "1\\. не нумерация"
    );
    // После времени строка уже не начало — лишних знаков нет.
    assert_eq!(
        entry(&defaults(), &text("= почти"), None),
        "- 09:05 = почти"
    );
}

/// Многострочный текст: в списке остаётся одним пунктом (отступ), в абзаце —
/// строками; края и «\r\n» не мешают; строки с маркерами не оживают.
pub fn entry_keeps_multiline_text_together() {
    let body = text("\n первая\r\n- вторая\n\nтретья  \n");
    assert_eq!(
        entry(&defaults(), &body, None),
        "- 09:05 первая\n  \\- вторая\n\n  третья"
    );
    let paragraph = with(|s| s.entry_style = EntryStyle::Paragraph);
    assert_eq!(
        entry(&paragraph, &body, None),
        "09:05 первая\n\\- вторая\n\nтретья"
    );
}

/// Дописывание: прежний текст побайтно цел, перевод строки только там, где
/// нужен, абзац отделён пустой строкой (FR-003).
pub fn append_never_changes_the_previous_text() {
    let cases = [
        ("", EntryStyle::List, "- а", "- а\n"),
        ("# Входящие\n", EntryStyle::List, "- а", "# Входящие\n- а\n"),
        ("# Входящие", EntryStyle::List, "- а", "# Входящие\n- а\n"),
        ("- 1\n\n", EntryStyle::List, "- а", "- 1\n\n- а\n"),
        ("", EntryStyle::Paragraph, "а", "а\n"),
        ("текст", EntryStyle::Paragraph, "а", "текст\n\nа\n"),
        ("текст\n", EntryStyle::Paragraph, "а", "текст\n\nа\n"),
        ("текст\n\n", EntryStyle::Paragraph, "а", "текст\n\nа\n"),
    ];
    for (before, style, entry, expected) in cases {
        let appended = append(before, style, entry);
        assert_eq!(appended.new_text, expected, "{before:?}");
        assert!(appended.new_text.as_bytes().starts_with(before.as_bytes()));
    }
}

/// Вставка описана в UTF-16, как позиции редактора, и даёт тот же текст.
pub fn append_edit_is_in_utf16_units() {
    let before = "😀 привет\n";
    let appended = append(before, EntryStyle::List, "- а");
    let units = before.encode_utf16().count();
    assert_eq!((appended.edit.from, appended.edit.to), (units, units));
    assert_eq!(appended.edit.insert, "- а\n");

    let mut spliced: Vec<u16> = before.encode_utf16().collect();
    spliced.extend(appended.edit.insert.encode_utf16());
    assert_eq!(String::from_utf16(&spliced).unwrap(), appended.new_text);
}

/// Назначение: входящие, ежедневная, выбранная; не-заметка и выход из vault
/// назначением не бывают.
pub fn target_paths() {
    let settings = defaults();
    assert_eq!(
        capture_target_path(&settings, &text("а")).unwrap(),
        "входящие.typ"
    );
    assert_eq!(
        capture_target_path(&settings, &to(text("а"), CaptureTarget::Daily)).unwrap(),
        "дневник/2026-10-16.typ"
    );
    let chosen = |path: &str| {
        to(
            text("а"),
            CaptureTarget::Note {
                path: path.to_owned(),
            },
        )
    };
    assert_eq!(
        capture_target_path(&settings, &chosen("Проекты//Альфа.typ")).unwrap(),
        "Проекты/Альфа.typ"
    );
    for bad in [
        "../чужая.typ",
        "/абсолют.typ",
        "доска.excalidraw",
        "папка",
        "",
        ".typsidian/vault.json",
    ] {
        assert_eq!(capture_target_path(&settings, &chosen(bad)), None, "{bad}");
    }
}

/// Без заметки входящих она создаётся и в неё дописано; повторный захват
/// встаёт ниже, порядок поступления сохранён (US1 AS2, Edge Cases).
pub async fn a_capture_creates_the_inbox_and_keeps_the_order() {
    let mut vault = MemoryVault::new();
    for (i, word) in ["первый", "второй", "третий"].into_iter().enumerate() {
        let mut next = text(word);
        next.at.minute = i as u8;
        let outcome = capture_write(&mut vault, &next, None, None).await;
        assert_eq!(
            outcome,
            CaptureOutcome::Written {
                path: "входящие.typ".to_owned()
            }
        );
    }
    assert_eq!(
        content_of(&vault, "входящие.typ"),
        "- 09:00 первый\n- 09:01 второй\n- 09:02 третий\n"
    );
}

/// Прежний текст заметки назначения цел (FR-003), в том числе без перевода
/// строки в конце.
pub async fn a_capture_keeps_the_previous_text() {
    let before = "# Входящие\nстрока без перевода";
    let mut vault = MemoryVault::new().with_file("входящие.typ", before);
    let outcome = capture_write(&mut vault, &text("новое"), None, None).await;
    assert!(
        matches!(outcome, CaptureOutcome::Written { .. }),
        "{outcome:?}"
    );
    let after = content_of(&vault, "входящие.typ");
    assert!(after.starts_with(before));
    assert_eq!(after, format!("{before}\n- 09:05 новое\n"));
}

/// Выбранная из недавних; пропавшая — отказ, ничего не создано.
pub async fn a_capture_to_a_chosen_note() {
    let mut vault = MemoryVault::new().with_file("Проекты/Альфа.typ", "= Альфа\n");
    let chosen = |path: &str| {
        to(
            text("а"),
            CaptureTarget::Note {
                path: path.to_owned(),
            },
        )
    };
    assert_eq!(
        capture_write(&mut vault, &chosen("Проекты/Альфа.typ"), None, None).await,
        CaptureOutcome::Written {
            path: "Проекты/Альфа.typ".to_owned()
        }
    );
    assert_eq!(
        content_of(&vault, "Проекты/Альфа.typ"),
        "= Альфа\n- 09:05 а\n"
    );

    let failed = capture_write(&mut vault, &chosen("Проекты/Пропала.typ"), None, None).await;
    assert_eq!(
        failed,
        CaptureOutcome::Failed {
            reason: CaptureFailure::TargetMissing
        }
    );
    let invalid = capture_write(&mut vault, &chosen("../чужая.typ"), None, None).await;
    assert_eq!(
        invalid,
        CaptureOutcome::Failed {
            reason: CaptureFailure::InvalidTarget
        }
    );
    assert_eq!(vault.paths_under("").len(), 1, "лишних файлов нет");
}

/// Ежедневная создаётся по заготовке и принимает захват (US1, назначение
/// «сегодня»).
pub async fn a_capture_to_the_daily_note_creates_it_from_the_template() {
    let mut vault = vault_with_settings(
        r#"{"capture": {"daily_template": "templates/день.typ", "daily_name": "DD.MM.YYYY"}}"#,
    )
    .with_file("templates/день.typ", "= День\n");
    let outcome = capture_write(
        &mut vault,
        &to(text("мысль"), CaptureTarget::Daily),
        None,
        None,
    )
    .await;
    assert_eq!(
        outcome,
        CaptureOutcome::Written {
            path: "дневник/16.10.2026.typ".to_owned()
        }
    );
    assert_eq!(
        content_of(&vault, "дневник/16.10.2026.typ"),
        "= День\n- 09:05 мысль\n"
    );
}

/// Настройки vault действуют: своя заметка входящих, абзац, без времени.
pub async fn vault_settings_drive_the_entry() {
    let mut vault = vault_with_settings(
        r#"{"capture": {"inbox": "Заметки/inbox", "entry_style": "paragraph", "entry_time": false}}"#,
    )
    .with_file("Заметки/inbox.typ", "= Inbox\n");
    let outcome = capture_write(&mut vault, &text("раз"), None, None).await;
    assert_eq!(
        outcome,
        CaptureOutcome::Written {
            path: "Заметки/inbox.typ".to_owned()
        }
    );
    assert_eq!(content_of(&vault, "Заметки/inbox.typ"), "= Inbox\n\nраз\n");
}

/// Битый файл настроек захват не останавливает.
pub async fn a_broken_settings_file_does_not_stop_a_capture() {
    let mut vault = vault_with_settings("{это не json");
    let outcome = capture_write(&mut vault, &text("а"), None, None).await;
    assert_eq!(
        outcome,
        CaptureOutcome::Written {
            path: "входящие.typ".to_owned()
        }
    );
    assert_eq!(
        content_of(&vault, SETTINGS_FILE),
        "{это не json",
        "файл настроек не тронут"
    );
}

/// Заметка открыта в редакторе: файл не пишется, возвращается правка буфера, и
/// несохранённый текст в ней цел (FR-004, US1 AS4).
pub async fn an_open_note_gets_a_buffer_edit_and_no_file_write() {
    let mut vault = MemoryVault::new().with_file("входящие.typ", "на диске\n");
    let buffer = "на диске\nнесохранённая правка";
    let outcome = capture_write(&mut vault, &text("а"), Some(buffer), None).await;
    let CaptureOutcome::EditBuffer { path, edit } = outcome else {
        panic!("ждали правку буфера: {outcome:?}");
    };
    assert_eq!(path, "входящие.typ");
    assert_eq!(edit.from, buffer.encode_utf16().count());
    assert_eq!(edit.to, edit.from);
    assert_eq!(edit.insert, "\n- 09:05 а\n");
    assert_eq!(vault.write_count("входящие.typ"), 0);
    assert_eq!(content_of(&vault, "входящие.typ"), "на диске\n");
}

/// Несохранённые правки вне редактора: дописывать некуда, захват не теряется
/// (отказ с причиной), файл цел.
pub async fn unsaved_edits_elsewhere_refuse_the_capture() {
    let mut vault = MemoryVault::new()
        .with_file("входящие.typ", "а\n")
        .with_unsaved_edits("входящие.typ");
    let outcome = capture_write(&mut vault, &text("б"), None, None).await;
    assert_eq!(
        outcome,
        CaptureOutcome::Failed {
            reason: CaptureFailure::UnsavedElsewhere
        }
    );
    assert_eq!(vault.write_count("входящие.typ"), 0);
}

/// Заметку изменили на другом устройстве между чтением и записью: запись
/// повторяется на свежем тексте, чужая правка не затёрта (Edge Cases).
pub async fn a_conflict_retries_on_the_fresh_text() {
    let mut vault = MemoryVault::new()
        .checking_expected()
        .with_file("входящие.typ", "было\n")
        .with_conflict_once("входящие.typ", "было\nс телефона\n");
    let outcome = capture_write(&mut vault, &text("новое"), None, None).await;
    assert_eq!(
        outcome,
        CaptureOutcome::Written {
            path: "входящие.typ".to_owned()
        }
    );
    assert_eq!(
        content_of(&vault, "входящие.typ"),
        "было\nс телефона\n- 09:05 новое\n"
    );
}

/// Конфликт не проходит три попытки подряд — отказ с причиной, файл как есть.
pub async fn a_persistent_conflict_is_refused_after_three_attempts() {
    let mut vault = MemoryVault::new()
        .checking_expected()
        .with_file("входящие.typ", "было\n")
        .with_lagging_index("входящие.typ");
    vault.replace_externally("входящие.typ", "стало\n");
    let outcome = capture_write(&mut vault, &text("новое"), None, None).await;
    assert_eq!(
        outcome,
        CaptureOutcome::Failed {
            reason: CaptureFailure::Conflict
        }
    );
    assert_eq!(content_of(&vault, "входящие.typ"), "стало\n");
}

/// Предел размера заметки: сверх него — отказ с причиной, файл цел (Edge Cases).
pub async fn a_capture_over_the_size_limit_is_refused() {
    let almost = "я".repeat((NOTE_SIZE_LIMIT_BYTES as usize - 20) / 2);
    let mut vault = MemoryVault::new().with_file("входящие.typ", almost.clone());
    let outcome = capture_write(
        &mut vault,
        &text("ещё достаточно длинный текст захвата"),
        None,
        None,
    )
    .await;
    assert_eq!(
        outcome,
        CaptureOutcome::Failed {
            reason: CaptureFailure::TooLarge
        }
    );
    assert_eq!(content_of(&vault, "входящие.typ"), almost);

    let huge = text(&"б".repeat(NOTE_SIZE_LIMIT_BYTES as usize));
    let mut empty = MemoryVault::new().with_file("а.typ", "");
    let big = capture_write(
        &mut empty,
        &to(
            huge.clone(),
            CaptureTarget::Note {
                path: "а.typ".to_owned(),
            },
        ),
        None,
        None,
    )
    .await;
    assert_eq!(
        big,
        CaptureOutcome::Failed {
            reason: CaptureFailure::TooLarge
        }
    );
    // Для открытой заметки предел тот же.
    let in_editor = capture_write(&mut empty, &huge, Some(""), None).await;
    assert_eq!(
        in_editor,
        CaptureOutcome::Failed {
            reason: CaptureFailure::TooLarge
        }
    );
}

/// Только чтение и недоступный vault: причина названа, ничего не создано
/// (FR-005, Edge Cases).
pub async fn read_only_and_unavailable_vaults_refuse_with_a_reason() {
    let mut read_only = MemoryVault::new()
        .with_file("входящие.typ", "а\n")
        .read_only();
    assert_eq!(
        capture_write(&mut read_only, &text("б"), None, None).await,
        CaptureOutcome::Failed {
            reason: CaptureFailure::ReadOnly
        }
    );
    assert_eq!(content_of(&read_only, "входящие.typ"), "а\n");

    let mut gone = MemoryVault::new().unavailable();
    assert_eq!(
        capture_write(&mut gone, &text("б"), None, None).await,
        CaptureOutcome::Failed {
            reason: CaptureFailure::NoVault
        }
    );
    assert_eq!(gone.paths_under("").len(), 0);
}

/// Пустому захвату писать нечего.
pub async fn an_empty_capture_is_refused() {
    let mut vault = MemoryVault::new();
    for empty in [
        text("  \n "),
        capture(CaptureContent::Link {
            url: " ".to_owned(),
            title: None,
        }),
    ] {
        assert_eq!(
            capture_write(&mut vault, &empty, None, None).await,
            CaptureOutcome::Failed {
                reason: CaptureFailure::Empty
            }
        );
    }
    assert_eq!(vault.paths_under("").len(), 0);
    // Одна приписка захватом является.
    let mut noted = text("");
    noted.note = Some("мысль".to_owned());
    assert!(matches!(
        capture_write(&mut vault, &noted, None, None).await,
        CaptureOutcome::Written { .. }
    ));
}

/// Картинка ложится вложением рядом с заметкой (006 по умолчанию), вставка —
/// с относительным путём (FR-006, US1 AS3).
pub async fn an_image_becomes_an_attachment_next_to_the_note() {
    let bytes = png(1);
    let mut vault = MemoryVault::new().with_file("Проекты/Альфа.typ", "= Альфа\n");
    let target = to(
        image_capture(),
        CaptureTarget::Note {
            path: "Проекты/Альфа.typ".to_owned(),
        },
    );
    let outcome = capture_write(&mut vault, &target, None, Some(&bytes)).await;
    assert_eq!(
        outcome,
        CaptureOutcome::Written {
            path: "Проекты/Альфа.typ".to_owned()
        }
    );

    let file = "Проекты/assets/Альфа/Альфа-2026-10-16-0905.png";
    assert_eq!(vault.bytes_of(file), Some(bytes));
    assert_eq!(
        content_of(&vault, "Проекты/Альфа.typ"),
        "= Альфа\n- 09:05 #image(\"assets/Альфа/Альфа-2026-10-16-0905.png\")\n"
    );
}

/// Политика vault «в корне» (006) действует и для захвата: путь вставки идёт
/// через `../`.
pub async fn the_attachment_follows_the_vault_placement_policy() {
    let mut vault = vault_with_settings(r#"{"assets": {"placement": "vault-root"}}"#)
        .with_file("Проекты/Альфа.typ", "");
    let target = to(
        image_capture(),
        CaptureTarget::Note {
            path: "Проекты/Альфа.typ".to_owned(),
        },
    );
    capture_write(&mut vault, &target, None, Some(&png(1))).await;
    assert!(vault
        .bytes_of("assets/Альфа/Альфа-2026-10-16-0905.png")
        .is_some());
    assert!(content_of(&vault, "Проекты/Альфа.typ")
        .contains("#image(\"../assets/Альфа/Альфа-2026-10-16-0905.png\")"));
}

/// В открытую заметку картинка тоже сохраняется файлом, а в буфер уходит
/// вставка.
pub async fn an_image_into_an_open_note_returns_the_markup_edit() {
    let mut vault = MemoryVault::new().with_file("входящие.typ", "");
    let outcome = capture_write(&mut vault, &image_capture(), Some("текст"), Some(&png(1))).await;
    let CaptureOutcome::EditBuffer { edit, .. } = outcome else {
        panic!("{outcome:?}");
    };
    assert_eq!(
        edit.insert,
        "\n- 09:05 #image(\"assets/входящие/входящие-2026-10-16-0905.png\")\n"
    );
    assert!(vault
        .bytes_of("assets/входящие/входящие-2026-10-16-0905.png")
        .is_some());
    assert_eq!(vault.write_count("входящие.typ"), 0);
}

/// Повтор того же захвата не плодит копии; другая картинка в ту же минуту
/// получает следующее имя.
pub async fn attachments_are_not_duplicated_and_names_do_not_collide() {
    let mut vault = MemoryVault::new();
    let same = png(1);
    capture_write(&mut vault, &image_capture(), None, Some(&same)).await;
    capture_write(&mut vault, &image_capture(), None, Some(&same)).await;
    assert_eq!(vault.paths_under("assets").len(), 1);

    capture_write(&mut vault, &image_capture(), None, Some(&png(2))).await;
    assert_eq!(
        vault.paths_under("assets"),
        [
            "assets/входящие/входящие-2026-10-16-0905-2.png",
            "assets/входящие/входящие-2026-10-16-0905.png",
        ]
    );
}

/// Не картинка (HEIC, пусто) — отказ, в vault ничего.
pub async fn an_unsupported_image_is_refused() {
    let mut vault = MemoryVault::new();
    let heic = b"\0\0\0\x18ftypheic".to_vec();
    for bytes in [Some(heic.as_slice()), Some(&[][..]), None] {
        assert_eq!(
            capture_write(&mut vault, &image_capture(), None, bytes).await,
            CaptureOutcome::Failed {
                reason: CaptureFailure::BadImage
            }
        );
    }
    assert_eq!(vault.paths_under("").len(), 0);
}

// ── Граница с оболочками ────────────────────────────────────────────────────

/// Запись очереди разбирается из JSON так, как её пишет оболочка, а исходы
/// уходят размеченными объектами (контракт для плагина и UI).
pub fn queue_json_and_outcomes_cross_the_boundary() {
    let queued: Capture = serde_json::from_str(
        r#"{"id":"u1","content":{"kind":"Link","url":"https://a.b","title":"T"},
            "at":{"year":2026,"month":10,"day":16,"hour":9,"minute":5},
            "target":{"kind":"Note","path":"а.typ"}}"#,
    )
    .unwrap();
    assert_eq!(queued.note, None);
    assert_eq!(
        queued.target,
        CaptureTarget::Note {
            path: "а.typ".to_owned()
        }
    );
    let image: Capture = serde_json::from_str(
        r#"{"id":"u2","content":{"kind":"Image","file":"u2.bin","mime":"image/png","name":"a.png"},
            "note":"п","at":{"year":2026,"month":1,"day":2,"hour":3,"minute":4},
            "target":{"kind":"Inbox"}}"#,
    )
    .unwrap();
    assert_eq!(image.note.as_deref(), Some("п"));

    let failed = serde_json::to_value(CaptureOutcome::Failed {
        reason: CaptureFailure::TooLarge,
    })
    .unwrap();
    assert_eq!(
        failed,
        serde_json::json!({"kind": "Failed", "reason": "tooLarge"})
    );
    let written = serde_json::to_value(CaptureOutcome::Written {
        path: "а.typ".to_owned(),
    })
    .unwrap();
    assert_eq!(
        written,
        serde_json::json!({"kind": "Written", "path": "а.typ"})
    );
    let message = serde_json::to_value(AppMessage::CaptureRefused {
        reason: CaptureFailure::ReadOnly,
    })
    .unwrap();
    assert_eq!(
        message,
        serde_json::json!({"code": "CaptureRefused", "reason": "readOnly"})
    );
    assert_eq!(
        AppMessage::CaptureRefused {
            reason: CaptureFailure::ReadOnly
        }
        .log_text(),
        "the capture was not saved: the vault is read-only"
    );
}
