//! Бенчмарк предпросмотра: пороги SC-001, SC-002, SC-003 и SC-006 и потолок
//! гарантий в 50 страниц (spec.md → Success Criteria).
//!
//! `harness = false` по образцу бенчмарков фичи 002: меряются сквозные
//! сценарии, а не микрооперации. Встроенный `#[bench]` потребовал бы nightly, а
//! criterion — целую зависимость ради одного `Instant::now()`.
//!
//! Сейчас здесь измерена только та часть конвейера, которая уже существует:
//! построение снимка и мира над ним. Замеры времени сборки и пересборки
//! добавляются задачами T053 (SC-001), T070 (SC-002, SC-003) и T108 (SC-006),
//! когда появится `preview::compile`.

use std::time::Instant;

use typsidian_core::preview::{DependencySnapshot, PreviewWorld};
use typst::foundations::Bytes;

/// Документ на ~50 страниц — заявленный потолок гарантий (SC-001).
fn reference_note(pages: usize) -> String {
    let mut text = String::new();
    for page in 1..=pages {
        text.push_str(&format!("= Раздел {page}\n\n"));
        for para in 1..=6 {
            text.push_str(&format!(
                "Абзац {para} раздела {page} с формулой $x^2 + y^2 = z^2$ и достаточным \
                 количеством слов, чтобы строка переносилась естественным образом.\n\n"
            ));
        }
        if page < pages {
            text.push_str("#pagebreak()\n\n");
        }
    }
    text
}

fn main() {
    let text = reference_note(50);
    println!("исходный текст: {} КБ", text.len() / 1024);

    let started = Instant::now();
    let id = DependencySnapshot::file_id("заметка.typ").expect("путь внутри vault");
    let mut snapshot = DependencySnapshot::new();
    snapshot.insert(id, Bytes::new(text.into_bytes()));
    let world = PreviewWorld::new(id, snapshot, Vec::new());
    let elapsed = started.elapsed();

    println!(
        "снимок и мир над ним: {:?} (бюджет SC-001 на весь путь до показа — 2 с)",
        elapsed
    );

    // Держим `world` живым до конца замера, чтобы оптимизатор не выбросил его
    // построение целиком.
    assert_eq!(world.snapshot().dependencies().len(), 1);
}
