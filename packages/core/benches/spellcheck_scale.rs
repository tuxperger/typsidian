//! Бенчмарк проверки орфографии (фича 030, T010).
//!
//! SC-002: ввод с проверкой не медленнее, чем без неё, больше чем на 16 мс.
//! Проверка идёт не на пути набора, а через паузу после правки (research.md §6),
//! поэтому меряется сам запрос редактора: окно из 150 строк (видимая часть с
//! запасом) в заметке на 10 000 строк по дереву сессии подсветки. Бюджет — 5 мс,
//! с тройным запасом под wasm. Отдельно — загрузка словарей (один раз за процесс).
//!
//! Ручной замер и `harness = false` — по тем же причинам, что у `vault_scale`.
//!
//! Прогон: `cargo bench --bench spellcheck_scale`.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::time::{Duration, Instant};
    use typsidian_core::highlight::{HighlightSession, HighlightWindow};
    use typsidian_core::spellcheck::{SpellChecker, Verdict};

    const BUDGET: Duration = Duration::from_millis(5);
    const LINES: usize = 10_000;
    const WINDOW_LINES: usize = 150;
    const RUNS: usize = 20;

    let checker = SpellChecker::default();
    let started = Instant::now();
    assert_eq!(checker.verdict("проверка"), Verdict::Correct);
    assert_eq!(checker.verdict("check"), Verdict::Correct);
    let load = started.elapsed();

    let line = |i: usize| match i % 5 {
        0 => format!("== Раздел {i} <раздел-{i}>\n"),
        1 => "Измерения теплопроводности выполнены на стенде с охранным кольцом, првоерка.\n"
            .to_owned(),
        2 => "Formula $T > 300 \"K\"$ and the thermal conductivty of samples.\n".to_owned(),
        3 => "См. [[Методика]], @ivanov2021 и `код`; по-русски, ёмкость.\n".to_owned(),
        _ => "#figure(image(\"стенд.png\"), caption: [Схема стенда])\n".to_owned(),
    };
    let text: String = (0..LINES).map(line).collect();
    let session = HighlightSession::new(text.clone());

    // Окно в середине заметки, по границам строк, в UTF-16.
    let offset =
        |lines: usize| -> usize { (0..lines).map(|i| line(i).encode_utf16().count()).sum() };
    let first = LINES / 2;
    let window = HighlightWindow {
        from: offset(first),
        to: offset(first + WINDOW_LINES),
    };

    let mut found = 0;
    let took = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            let pairs = session.spelling(&[window], &checker);
            let took = started.elapsed();
            found = pairs.len() / 2;
            took
        })
        .min()
        .expect("прогоны");
    // В окне 150 строк: по опечатке в каждой второй и третьей из пяти.
    assert_eq!(found, WINDOW_LINES / 5 * 2, "опечатки окна");

    println!("загрузка словарей ru + en: {load:?}");
    println!(
        "SC-002 проверка окна {WINDOW_LINES} строк в заметке на {LINES}: {took:?} (бюджет {BUDGET:?}); \
         {found} ошибок"
    );
    if took > BUDGET {
        eprintln!("НЕ ВЫДЕРЖАН — {took:?}, бюджет {BUDGET:?}");
        std::process::exit(1);
    }
    println!("бюджет выдержан");
}

#[cfg(target_arch = "wasm32")]
fn main() {}
