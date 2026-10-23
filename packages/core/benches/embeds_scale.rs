//! Бенчмарк встраивания (фича 031, T008).
//!
//! SC-001: предпросмотр заметки с 10 встраиваниями обновляется в тех же
//! пределах, что без них. Сборка самих встроенных разделов — работа Typst,
//! её стоимость та же, что у этого текста в заметке; добавляет встраивание
//! только план: чтение целей, вырезание разделов, переписывание вложенных
//! вызовов. Он меряется здесь: 10 встраиваний разделов заметок по 2 000
//! строк, у каждой ещё по одному вложенному встраиванию. Бюджет — 20 мс.
//!
//! Ручной замер и `harness = false` — по тем же причинам, что у `vault_scale`.
//!
//! Прогон: `cargo bench --bench embeds_scale`.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::time::{Duration, Instant};
    use typsidian_core::embeds::plan;
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;

    const BUDGET: Duration = Duration::from_millis(20);
    const NOTES: usize = 10;
    const SECTIONS: usize = 100;
    const RUNS: usize = 10;

    let dir = tempfile::TempDir::new().expect("временный каталог");
    let note = |n: usize| {
        let mut text = format!("= Заметка {n}\n\n");
        for s in 0..SECTIONS {
            text.push_str(&format!("== Раздел {s}\n\n"));
            for line in 0..18 {
                text.push_str(&format!("Строка {line} раздела {s} заметки {n}.\n"));
            }
            text.push('\n');
        }
        text.push_str(&format!(
            "== Ссылка\n\n#embed(\"Заметка {}#Раздел 1\")\n",
            (n + 1) % NOTES
        ));
        text
    };
    for n in 0..NOTES {
        std::fs::write(dir.path().join(format!("Заметка {n}.typ")), note(n)).expect("запись");
    }
    let main: String = (0..NOTES)
        .map(|n| format!("#embed(\"Заметка {n}#Ссылка\")\n"))
        .collect();

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let backend = runtime
        .block_on(NativeBackend::open(dir.path()))
        .expect("открытие vault");

    let mut files = 0;
    let took = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            let planned = runtime.block_on(plan(&backend, "Введение.typ", &main));
            let took = started.elapsed();
            files = planned.files.len();
            took
        })
        .min()
        .expect("прогоны");
    assert_eq!(files, NOTES * 2, "10 встраиваний и по одному вложенному");
    println!("SC-001 план {NOTES} встраиваний (+{NOTES} вложенных): {took:?} (бюджет {BUDGET:?})");
    if took > BUDGET {
        eprintln!("НЕ ВЫДЕРЖАН — {took:?}, бюджет {BUDGET:?}");
        std::process::exit(1);
    }
    println!("бюджет выдержан");
}

#[cfg(target_arch = "wasm32")]
fn main() {}
