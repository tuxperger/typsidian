//! Бенчмарк состояния ссылок (фича 028, T009): SC-001 — экран «Состояние
//! ссылок» на vault из 5 000 заметок открывается не дольше 1 с.
//!
//! Меряется весь вызов `link_health` поверх `NativeBackend`: источник графа из
//! индекса, проверка существования вставок, чтение заметок с битыми ссылками
//! ради строк-примеров, сироты и настройки vault. Битых ссылок — у каждой
//! двадцатой заметки, битых вставок — у каждой десятой.
//!
//! Ручной замер и `harness = false` — по тем же причинам, что у `vault_scale`.
//! Меряется лучший из нескольких прогонов.
//!
//! Прогон: `cargo bench --bench link_health_scale`.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::time::{Duration, Instant};
    use typsidian_core::links::health::link_health;
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;

    const BUDGET: Duration = Duration::from_secs(1);
    const NOTE_COUNT: usize = 5_000;
    const LINKS_PER_NOTE: usize = 3;
    const NOTES_PER_FOLDER: usize = 100;
    const RUNS: usize = 5;

    let name = |i: usize| format!("folder-{:03}/note-{i:05}", i / NOTES_PER_FOLDER);

    let dir = tempfile::TempDir::new().expect("временный каталог");
    for i in 0..NOTE_COUNT {
        std::fs::create_dir_all(
            dir.path()
                .join(format!("folder-{:03}", i / NOTES_PER_FOLDER)),
        )
        .expect("подпапка");
        let mut text = String::from("Текст заметки со ссылками.\n");
        for k in 1..=LINKS_PER_NOTE {
            let target = (i * 7 + k * 1_103) % NOTE_COUNT;
            text.push_str(&format!("[[/{}]] ", name(target)));
        }
        if i % 20 == 0 {
            text.push_str(&format!(
                "\nсм. [[/нет/пропавшая-{}|подпись]]\n",
                (i / 20) % 50
            ));
        }
        if i % 10 == 0 {
            text.push_str(&format!("#image(\"/pics/pic-{}.png\")\n", (i / 10) % 200));
        }
        std::fs::write(dir.path().join(format!("{}.typ", name(i))), text).expect("запись заметки");
    }

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let backend = runtime
        .block_on(NativeBackend::open(dir.path()))
        .expect("открытие vault");

    let mut shape = (0, 0, 0);
    let took = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            let health = runtime.block_on(link_health(&backend, &[]));
            let took = started.elapsed();
            shape = (
                health.broken.len(),
                health.broken_links,
                health.orphans.len(),
            );
            took
        })
        .min()
        .expect("прогоны");
    let (targets, links, orphans) = shape;
    assert_eq!(targets, 50 + 200, "битые цели: 50 заметок и 200 картинок");
    assert_eq!(links, 250 + 500, "битые ссылки и вставки");

    println!(
        "SC-001 link_health, {NOTE_COUNT} заметок: {took:?} (бюджет {BUDGET:?}); \
         {targets} целей, {links} битых, {orphans} без входящих"
    );
    if took > BUDGET {
        eprintln!("НЕ ВЫДЕРЖАН — {took:?}, бюджет {BUDGET:?}");
        std::process::exit(1);
    }
    println!("бюджет выдержан");
}

#[cfg(target_arch = "wasm32")]
fn main() {}
