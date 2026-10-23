//! Бенчмарк экспорта в HTML (фича 034, T007).
//!
//! SC-002: экспорт папки из 100 заметок — не дольше 30 с. Меряется весь путь
//! пакета: сборка каждой заметки в HTML со ссылками на соседей и запись
//! страницы, затем оглавление. Заметки — с заголовками, формулой, списком и
//! ссылками `[[…]]` на две соседние.
//!
//! Ручной замер и `harness = false` — по тем же причинам, что у `vault_scale`.
//!
//! Прогон: `cargo bench --bench html_export_scale`.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::time::{Duration, Instant};
    use typsidian_core::export::{export_note_html, export_site_index, ExportOutcome, SiteLabels};
    use typsidian_core::preview::{FontSet, PackageCache};
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;
    use typsidian_core::vault::vault_settings::VaultOverrides;

    const BUDGET: Duration = Duration::from_secs(30);
    const NOTES: usize = 100;

    let dir = tempfile::TempDir::new().expect("временный каталог");
    std::fs::create_dir_all(dir.path().join("Сайт")).expect("папка");
    for n in 0..NOTES {
        let text = format!(
            "#set text(lang: \"ru\")\n= Заметка {n}\n\n{}\n\nСм. [[заметка {}]] и [[заметка {}]].\n\n$ sum_(i=1)^n i = n(n+1)/2 $\n\n- первый пункт\n- второй пункт\n",
            "Абзац текста заметки. ".repeat(30),
            (n + 1) % NOTES,
            (n + NOTES - 1) % NOTES,
        );
        std::fs::write(dir.path().join(format!("Сайт/заметка {n}.typ")), text).expect("запись");
    }
    let site: Vec<String> = (0..NOTES)
        .map(|n| format!("Сайт/заметка {n}.typ"))
        .collect();
    let labels = SiteLabels {
        contents: "Оглавление".into(),
        outside: "вне экспорта".into(),
    };
    let overrides = VaultOverrides {
        auto_download_templates: Some(false),
        ..VaultOverrides::default()
    };

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut backend = runtime
        .block_on(NativeBackend::open(dir.path()))
        .expect("открытие vault");
    let packages = PackageCache::new();

    let started = Instant::now();
    for path in &site {
        let report = runtime.block_on(export_note_html(
            &mut backend,
            &packages,
            path,
            None,
            FontSet::builtin(),
            &overrides,
            &site,
            &labels,
        ));
        assert!(
            matches!(report.outcome, ExportOutcome::Written { .. }),
            "{path}: {:?}",
            report.diagnostics
        );
    }
    runtime
        .block_on(export_site_index(&mut backend, &overrides, &site, &labels))
        .expect("оглавление");
    let took = started.elapsed();
    println!("SC-002 экспорт в HTML, {NOTES} заметок: {took:?} (бюджет {BUDGET:?})");
    if took > BUDGET {
        eprintln!("НЕ ВЫДЕРЖАН — {took:?}, бюджет {BUDGET:?}");
        std::process::exit(1);
    }
    println!("бюджет выдержан");
}

#[cfg(target_arch = "wasm32")]
fn main() {}
