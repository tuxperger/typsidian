//! Таблица расположения файлов вставок — **один** источник ожиданий для
//! `placement_native.rs` и `placement_wasm.rs` (фича 006,
//! contracts/asset-placement.md → «Проверяемые сценарии»).
//!
//! Вынесена в общий модуль намеренно. Парные файлы 002 повторяют проверки
//! дословно, и там это оправдано: расхождение эталонного хэша между таргетами
//! обязано валить ровно один из двух тестов. Здесь ожидания — не эталон, а
//! таблица из контракта, и две её копии разошлись бы при первой же правке
//! контракта, оставив один таргет проверенным по устаревшему правилу.

#![allow(dead_code)] // каждый таргет использует свою часть модуля

use std::path::Path;
use typsidian_core::boards::placement::{markup_path, snapshot_path, AssetsPlacement};
use typsidian_core::boards::EmbedId;
use typsidian_core::vault::sections::SectionMark;

/// Идентификатор фиксирован: расположение от него не зависит, а настоящий хэш
/// только зашумил бы ожидаемые значения.
pub fn embed_id() -> EmbedId {
    EmbedId::from_file_name("board-47dc7994.svg").expect("правильное имя снимка")
}

pub fn marks(paths: &[&str]) -> Vec<SectionMark> {
    paths
        .iter()
        .map(|path| SectionMark::new(Path::new(path)).expect("пометка внутри vault"))
        .collect()
}

/// Строка таблицы: политика, заметка, разделы → ожидаемый путь снимка.
pub struct Case {
    pub placement: AssetsPlacement,
    pub note: &'static str,
    pub sections: &'static [&'static str],
    /// `None` — путь не является путём заметки.
    pub expected: Option<&'static str>,
    pub why: &'static str,
}

pub const SNAPSHOT_CASES: &[Case] = &[
    Case {
        placement: AssetsPlacement::NextToNote,
        note: "Проекты/Идея.typ",
        sections: &[],
        expected: Some("Проекты/assets/Идея/board-47dc7994.svg"),
        why: "рядом с заметкой: корень — папка заметки",
    },
    Case {
        placement: AssetsPlacement::NextToNote,
        note: "Идея.typ",
        sections: &[],
        expected: Some("assets/Идея/board-47dc7994.svg"),
        why: "заметка в корне vault",
    },
    Case {
        placement: AssetsPlacement::VaultRoot,
        note: "Проекты/Идея.typ",
        sections: &[],
        expected: Some("assets/Идея/board-47dc7994.svg"),
        why: "в корне: где лежит заметка, не важно",
    },
    Case {
        placement: AssetsPlacement::SectionRoot,
        note: "Работа/Проекты/Идея.typ",
        sections: &["Работа"],
        expected: Some("Работа/assets/Идея/board-47dc7994.svg"),
        why: "помеченный предок",
    },
    Case {
        placement: AssetsPlacement::SectionRoot,
        note: "Работа/Проекты/Идея.typ",
        sections: &["Работа", "Работа/Проекты"],
        expected: Some("Работа/Проекты/assets/Идея/board-47dc7994.svg"),
        why: "FR-016: ближайший помеченный предок перекрывает верхний",
    },
    Case {
        placement: AssetsPlacement::SectionRoot,
        note: "Личное/Мысли.typ",
        sections: &["Работа"],
        expected: Some("assets/Мысли/board-47dc7994.svg"),
        why: "FR-006: помеченных предков нет — корень vault, без ошибки",
    },
    Case {
        placement: AssetsPlacement::SectionRoot,
        note: "Работа-архив/Идея.typ",
        sections: &["Работа"],
        expected: Some("assets/Идея/board-47dc7994.svg"),
        why: "префикс сравнивается по компонентам, а не по строке",
    },
    Case {
        placement: AssetsPlacement::NextToNote,
        note: "Схема.excalidraw",
        sections: &[],
        expected: None,
        why: "доска заметкой не является",
    },
];

pub const MARKUP_CASES: &[Case] = &[
    Case {
        placement: AssetsPlacement::NextToNote,
        note: "Проекты/Идея.typ",
        sections: &[],
        expected: Some("assets/Идея/board-47dc7994.svg"),
        why: "адрес относителен папке заметки",
    },
    Case {
        placement: AssetsPlacement::VaultRoot,
        note: "Проекты/Идея.typ",
        sections: &[],
        expected: Some("../assets/Идея/board-47dc7994.svg"),
        why: "подъём на уровень",
    },
    Case {
        placement: AssetsPlacement::VaultRoot,
        note: "a/b/c/Идея.typ",
        sections: &[],
        expected: Some("../../../assets/Идея/board-47dc7994.svg"),
        why: "подъём на три уровня",
    },
];

/// Прогоняет таблицу путей снимка. Общая для обоих таргетов: расхождение
/// результата между ними является дефектом, а не особенностью платформы.
pub fn check_snapshot_cases() {
    for case in SNAPSHOT_CASES {
        let actual = snapshot_path(
            case.placement,
            Path::new(case.note),
            &marks(case.sections),
            &embed_id(),
        )
        .map(|path| path.to_string_lossy().replace('\\', "/"));
        assert_eq!(
            actual.as_deref(),
            case.expected,
            "{} ({:?}, {})",
            case.why,
            case.placement,
            case.note
        );
    }
}

/// Классификация путей после фичи 006 (research.md §5).
///
/// Ключевая проверка совместимости: **ни одно** значение политики не отключает
/// распознавание прежней раскладки `*.assets` (FR-009), а новое правило не
/// объявляет снимком картинку, которую пользователь сам положил в папку с
/// именем `assets` (FR-012).
pub fn check_classification() {
    use typsidian_core::vault::{classify, FileKind};

    let cases: &[(&str, FileKind, &str)] = &[
        (
            ".typsidian/vault.json",
            FileKind::Service,
            "служебное место",
        ),
        (
            ".typsidian/packages/preview/codly/1.3.0/lib.typ",
            FileKind::Service,
            "шаблон внутри служебного места — не заметка",
        ),
        (
            ".typsidian/assets/Идея/board-a1b2c3d4.svg",
            FileKind::Service,
            "проверка Service идёт первой: служебное важнее снимка",
        ),
        (
            "Идея.assets/board-a1b2c3d4.svg",
            FileKind::Snapshot,
            "FR-009: прежнее правило не сломано",
        ),
        (
            "Проекты/Идея.assets/board-a1b2c3d4.svg",
            FileKind::Snapshot,
            "прежнее правило во вложенной папке",
        ),
        (
            "assets/Идея/board-a1b2c3d4.svg",
            FileKind::Snapshot,
            "новое правило в корне",
        ),
        (
            "Работа/assets/Идея/board-a1b2c3d4.svg",
            FileKind::Snapshot,
            "новое правило в разделе",
        ),
        (
            "assets/Идея/моя-картинка.svg",
            FileKind::Other,
            "FR-012: картинка пользователя снимком не становится",
        ),
        (
            "assets/Идея/подпапка/board-a1b2c3d4.svg",
            FileKind::Other,
            "глубже приложение снимков не кладёт",
        ),
        ("Идея.typ", FileKind::Note, "обычная заметка"),
        ("Схема.excalidraw", FileKind::Board, "доска"),
    ];

    for (path, expected, why) in cases {
        assert_eq!(classify(Path::new(path)), *expected, "{why} ({path})");
    }

    // Ничего из служебного и снимков в дереве не показывается.
    assert!(!FileKind::Service.is_listed());
    assert!(!FileKind::Snapshot.is_listed());
    assert!(FileKind::Note.is_listed());
    assert!(FileKind::Board.is_listed());
}

/// Распознавание вставок не зависит от текущей политики (FR-007, FR-009,
/// FR-010).
///
/// Самая важная проверка совместимости во всей фиче. Смена политики не двигает
/// уже созданные файлы, поэтому у одной заметки вставки могут одновременно
/// лежать по трём разным адресам. Если бы разбор сверял адрес с **расчётным**
/// путём для текущей политики, заметка теряла бы собственные вставки сразу
/// после переключения.
pub fn check_embed_recognition() {
    use typsidian_core::boards::embeds_of;

    let note = Path::new("Проекты/Идея.typ");
    let text = concat!(
        "#image(\"Идея.assets/board-aaaaaaaa.svg\")\n",       // прежняя раскладка
        "#image(\"assets/Идея/board-bbbbbbbb.svg\")\n",       // новая, рядом с заметкой
        "#image(\"../assets/Идея/board-cccccccc.svg\")\n",    // новая, в корне vault
        "#image(\"Фото.png\")\n",                             // обычная картинка
        "#image(\"assets/Идея/моя-картинка.svg\")\n",         // картинка в папке ассетов
        "#image(\"assets/Другая/board-dddddddd.svg\")\n",     // снимок ЧУЖОЙ заметки
    );

    let found = embeds_of(note, text);
    let ids: Vec<&str> = found.iter().map(|one| one.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["board-aaaaaaaa", "board-bbbbbbbb", "board-cccccccc"],
        "обе раскладки распознаются, чужое — нет"
    );

    // Пути разрешены относительно корня vault, включая подъём через `../`.
    let paths: Vec<String> = found
        .iter()
        .map(|one| one.snapshot_path.to_string_lossy().replace('\\', "/"))
        .collect();
    assert_eq!(
        paths,
        vec![
            "Проекты/Идея.assets/board-aaaaaaaa.svg".to_string(),
            "Проекты/assets/Идея/board-bbbbbbbb.svg".to_string(),
            "assets/Идея/board-cccccccc.svg".to_string(),
        ]
    );
}

/// Прогоняет таблицу адресов, попадающих в текст заметки.
pub fn check_markup_cases() {
    for case in MARKUP_CASES {
        let actual = markup_path(
            case.placement,
            Path::new(case.note),
            &marks(case.sections),
            &embed_id(),
        );
        assert_eq!(
            actual.as_deref(),
            case.expected,
            "{} ({:?}, {})",
            case.why,
            case.placement,
            case.note
        );
        // Путь внутри документа Typst, а не путь файловой системы.
        assert!(actual.is_none_or(|markup| !markup.contains('\\')));
    }
}
