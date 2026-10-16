//! Бенчмарк сбора `slides` (фича 019, T030, research.md §11): презентация из
//! 100 страниц 16:9 с заметками докладчика и логическими номерами на каждой.
//!
//! Бюджет — сбор не дороже 5 % времени сборки. Чистой «сборки без сбора» в
//! `compile_with` нет (флага не заводили, поведение не меняется), поэтому
//! меряются две величины: полная `compile_note` (со сбором) и сам сбор на
//! документе, собранном тем же путём; доля — их отношение.
//!
//! Прогон: `cargo bench --bench slides_scale`.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::time::{Duration, Instant};
    use typsidian_core::preview::{compile_note, time_slides_collect, CompileOutcome, FontSet};

    const PAGES: usize = 100;
    const RUNS: usize = 15;
    const BUDGET_PERCENT: f64 = 5.0;
    const NOTE: &str = "показ.typ";

    /// Презентация без пакетов: заголовок, пункты и разметка `<pdfpc>`.
    /// `edit` меняет текст, чтобы кэш мемоизации не отдавал готовый результат.
    fn deck(edit: usize) -> String {
        let mut text = String::from(
            "#set page(width: 16cm, height: 9cm, margin: 1cm)\n#set text(size: 18pt)\n",
        );
        for n in 1..=PAGES {
            text.push_str(&format!("= Слайд {n}\n\n"));
            for item in 1..=4 {
                let mark = if n == 1 && item == 1 {
                    format!(" (правка {edit})")
                } else {
                    String::new()
                };
                text.push_str(&format!(
                    "- Пункт {item} слайда {n}, достаточно длинный.{mark}\n"
                ));
            }
            text.push_str(&format!(
                "#metadata((t: \"Note\", v: \"Заметка докладчика к слайду {n}\")) <pdfpc>\n\
                 #metadata((t: \"LogicalSlide\", v: {n})) <pdfpc>\n"
            ));
            if n < PAGES {
                text.push_str("#pagebreak()\n");
            }
        }
        text
    }

    fn median(mut times: Vec<Duration>) -> Duration {
        times.sort();
        times[times.len() / 2]
    }

    let fonts = FontSet::builtin();

    // Прогрев: шрифты, кэши компилятора.
    for edit in 0..3 {
        let built = compile_note(NOTE, &deck(edit), 0, fonts.clone()).expect("прогрев");
        assert_eq!(
            built.outcome,
            CompileOutcome::Succeeded,
            "{:?}",
            built.diagnostics
        );
        assert_eq!(built.pages.len(), PAGES);
        assert_eq!(built.slides.len(), PAGES);
        assert!(built
            .slides
            .iter()
            .all(|s| s.logical_slide.is_some() && s.notes.is_some()));
    }

    let mut builds = Vec::with_capacity(RUNS);
    let mut collects = Vec::with_capacity(RUNS);
    for run in 0..RUNS {
        let text = deck(100 + run);
        let started = Instant::now();
        let built = compile_note(NOTE, &text, run as u64, fonts.clone()).expect("сборка");
        builds.push(started.elapsed());
        assert_eq!(built.outcome, CompileOutcome::Succeeded);

        let text = deck(1000 + run);
        collects.push(time_slides_collect(NOTE, &text, fonts.clone()).expect("сбор"));
    }

    let build = median(builds.clone());
    let collect = median(collects.clone());
    let share = collect.as_secs_f64() / build.as_secs_f64() * 100.0;
    println!("презентация:                 {PAGES} стр., {RUNS} прогонов после прогрева");
    println!(
        "полная сборка со сбором:     медиана {build:?} (мин {:?}, макс {:?})",
        builds.iter().min().unwrap(),
        builds.iter().max().unwrap()
    );
    println!(
        "сбор slides отдельно:        медиана {collect:?} (мин {:?}, макс {:?})",
        collects.iter().min().unwrap(),
        collects.iter().max().unwrap()
    );
    println!("доля сбора от сборки:        {share:.2} % — бюджет {BUDGET_PERCENT} %");
    println!(
        "порог 5 %: {}",
        if share <= BUDGET_PERCENT {
            "выполнен"
        } else {
            "НЕ выполнен"
        }
    );
}

#[cfg(target_arch = "wasm32")]
fn main() {}
