//! Проверка орфографии (фича 030) — общая таблица для обоих таргетов.
//!
//! Файлы `spellcheck_native.rs` / `spellcheck_wasm.rs` только вызывают эти
//! функции своим harness'ом (принцип II). Словари вшиты тестам фичей
//! `embedded-dictionaries`.

#![allow(dead_code)]

use typsidian_core::highlight::HighlightWindow;
use typsidian_core::spellcheck::{
    misspelled_in, spell_add_word, spell_configure, spell_remove_word, spell_suggest, SpellChecker,
    SpellError, SpellLanguage, Verdict,
};
use typsidian_core::vault::vault_settings::{read_vault_settings, write_vault_settings};

use super::MemoryVault;

/// Эталонная заметка (SC-001): три опечатки в тексте и те же «опечатки» во
/// всех местах, которые не проверяются (FR-002).
pub const REFERENCE: &str = r#"#set text(lang: "ru", hyphenate: true)
#let првоерка = "резултатах conductivty"

= Введение <введение-првоерка>

Цель работы — првоерка модели теплопроводности при $T > 300 "резултатах" + првоерка$.
Методика описана в [[Методика првоерка]] и опирается на @првоерка; см. @рис-стенд.
Измерения выполнены на стенде `ТС-2М првоерка` по ГОСТ 7076–99, всё — в резултатах.
// комментарий: првоерка резултатах
/* блочный комментарий conductivty */

```
код првоерка резултатах conductivty
```

- *Thermal conductivty* measured with a guarded hot plate.
- Данные: https://example.org/conductivty-првоерка, ёмкость и емкость — оба верны.
#figure(image("стенд.png"), caption: [Схема стенда])
"#;

/// Слова, подчёркнутые в `text`.
fn underlined(text: &str, windows: &[HighlightWindow], checker: &SpellChecker) -> Vec<String> {
    let units: Vec<u16> = text.encode_utf16().collect();
    misspelled_in(text, windows, checker)
        .chunks(2)
        .map(|pair| String::from_utf16(&units[pair[0] as usize..pair[1] as usize]).unwrap())
        .collect()
}

// ── US1 ─────────────────────────────────────────────────────────────────────

/// Подчёркнуты ровно опечатки текста: не формулы, код, вызовы, метки, ссылки,
/// вики-ссылки, адреса и комментарии (SC-001, FR-001, FR-002).
pub fn reference_note_underlines_only_prose() {
    let checker = SpellChecker::default();
    assert_eq!(
        underlined(REFERENCE, &[], &checker),
        ["првоерка", "резултатах", "conductivty"]
    );
}

/// Русский и английский в одной заметке; выключенный язык и выключенная
/// проверка ничего не подчёркивают (FR-004, US3).
pub fn languages_follow_the_alphabet_and_settings() {
    let text = "Опечтка и typoo в одной строке.";
    let mut checker = SpellChecker::default();
    assert_eq!(underlined(text, &[], &checker), ["Опечтка", "typoo"]);

    checker.configure(true, vec![SpellLanguage::Ru], Vec::new());
    assert_eq!(underlined(text, &[], &checker), ["Опечтка"]);

    checker.configure(false, SpellLanguage::ALL.to_vec(), Vec::new());
    assert!(underlined(text, &[], &checker).is_empty());
}

/// Не ложные ошибки: дефис, ё/е, аббревиатуры, числа, две азбуки, одна буква,
/// апостроф (Edge Cases).
pub fn edge_cases_are_not_false_errors() {
    let checker = SpellChecker::default();
    for word in [
        "по-русски",
        "кто-нибудь",
        "ёмкость",
        "емкость",
        "Ёлка",
        "don't",
        "isn’t",
        "Москва",
        "e-mail",
    ] {
        assert_eq!(checker.verdict(word), Verdict::Correct, "{word}");
    }
    // Аббревиатуры не проверяются вовсе — ни верными, ни ошибками.
    for word in ["ГОСТ", "НИИ", "12abc", "ТС2М", "Wi-Fi-сеть", "я", "x"] {
        assert_eq!(checker.verdict(word), Verdict::Skip, "{word}");
    }
    assert_eq!(checker.verdict("москва"), Verdict::Wrong);
    assert_eq!(checker.verdict("по-руски"), Verdict::Wrong);
    let text = "Стенд ТС-2М, ГОСТ 7076–99, 25 °C, x и y.";
    assert!(underlined(text, &[], &checker).is_empty());
}

/// Подсказки содержат исправление, не больше пяти (FR-003).
pub fn suggestions_contain_the_fix() {
    let checker = SpellChecker::default();
    let ru = spell_suggest(&checker, "првоерка");
    assert!(ru.iter().any(|word| word == "проверка"), "{ru:?}");
    assert!(ru.len() <= 5);
    let en = spell_suggest(&checker, "conductivty");
    assert!(en.iter().any(|word| word == "conductivity"), "{en:?}");
    assert!(spell_suggest(&checker, "12").is_empty());
}

/// Проверяются только окна: ошибка за их пределами не подчёркнута.
pub fn only_windows_are_checked() {
    let text = "Перва строка.\nВторая строка.\nТретя строка.\n";
    let checker = SpellChecker::default();
    let second_line_end = "Перва строка.\nВторая строка.".encode_utf16().count();
    let window = HighlightWindow {
        from: 0,
        to: second_line_end,
    };
    assert_eq!(underlined(text, &[window], &checker), ["Перва"]);
    assert_eq!(underlined(text, &[], &checker), ["Перва", "Третя"]);
}

/// ≥ 95% намеренных опечаток эталонного текста подчёркнуты (SC-003).
pub fn most_deliberate_typos_are_found() {
    let typos = [
        "превет",
        "малоко",
        "сабака",
        "кампьютер",
        "извените",
        "пожалуста",
        "сдесь",
        "севодня",
        "очень-очень-очнь",
        "граматика",
        "калидор",
        "здраствуйте",
        "teh",
        "recieve",
        "seperate",
        "definately",
        "occured",
        "untill",
        "wich",
        "becuase",
    ];
    let checker = SpellChecker::default();
    let text = typos.join(" ");
    let found = underlined(&text, &[], &checker);
    assert!(
        found.len() * 100 >= typos.len() * 95,
        "найдено {} из {}: {found:?}",
        found.len(),
        typos.len()
    );
}

// ── US2 ─────────────────────────────────────────────────────────────────────

/// «Добавить в словарь» — слово в файле vault, не подчёркнуто без учёта
/// регистра; «убрать» — снова подчёркнуто; файл отсортирован (FR-005).
pub async fn vault_dictionary_adds_and_removes_words() {
    let mut vault = MemoryVault::new();
    let mut checker = SpellChecker::default();
    let text = "Typsidian собирает заметки в Syncthing.";
    assert_eq!(underlined(text, &[], &checker), ["Typsidian", "Syncthing"]);

    let config = spell_add_word(&mut vault, &mut checker, "typsidian")
        .await
        .unwrap();
    assert_eq!(config.words, ["typsidian"]);
    spell_add_word(&mut vault, &mut checker, "Syncthing")
        .await
        .unwrap();
    spell_add_word(&mut vault, &mut checker, " TYPSIDIAN ")
        .await
        .unwrap();
    assert!(underlined(text, &[], &checker).is_empty());
    let file = String::from_utf8(vault.bytes_of(".typsidian/dictionary.txt").unwrap()).unwrap();
    assert_eq!(file, "Syncthing\ntypsidian\n");

    let config = spell_remove_word(&mut vault, &mut checker, "Typsidian")
        .await
        .unwrap();
    assert_eq!(config.words, ["Syncthing"]);
    assert_eq!(underlined(text, &[], &checker), ["Typsidian"]);

    for bad in ["", "два слова", "R2D2", "a/b"] {
        assert_eq!(
            spell_add_word(&mut vault, &mut checker, bad).await,
            Err(SpellError::InvalidWord),
            "{bad:?}"
        );
    }

    let mut read_only = MemoryVault::new().read_only();
    assert_eq!(
        spell_add_word(&mut read_only, &mut checker, "слово").await,
        Err(SpellError::ReadOnly)
    );
}

/// `spell_configure` читает словарь, изменённый снаружи (синхронизация), и
/// настройки vault: выключено, языки (US2, US3).
pub async fn configure_reads_dictionary_and_settings() {
    let mut vault =
        MemoryVault::new().with_file(".typsidian/dictionary.txt", "# свои слова\nТипсидиан\n\n");
    let mut checker = SpellChecker::default();
    let config = spell_configure(&vault, &mut checker).await;
    assert!(config.enabled);
    assert_eq!(config.languages, SpellLanguage::ALL);
    assert_eq!(config.words, ["Типсидиан"]);
    assert_eq!(checker.verdict("типсидиан"), Verdict::Correct);

    let mut settings = read_vault_settings(&vault).await.unwrap();
    settings.spellcheck_enabled = Some(false);
    settings.spellcheck_languages = Some(vec!["en".into(), "xx".into()]);
    write_vault_settings(&mut vault, &settings).await.unwrap();
    let config = spell_configure(&vault, &mut checker).await;
    assert!(!config.enabled);
    assert_eq!(config.languages, [SpellLanguage::En]);
    assert!(underlined("Опечтка typoo", &[], &checker).is_empty());
}
