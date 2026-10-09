//! Бенчмарк предпросмотра: пороги SC-001, SC-002 и SC-006 на документе в
//! 50 страниц — заявленном потолке гарантий (spec.md → Success Criteria).
//!
//! `harness = false` по образцу бенчмарков фичи 002: меряются сквозные
//! сценарии, а не микрооперации. Встроенный `#[bench]` потребовал бы nightly, а
//! criterion — целую зависимость ради одного `Instant::now()`.
//!
//! Пороги проверяются как условие прохождения. Измеряется ядро; к его времени
//! интерфейс добавляет известные постоянные — паузу дебаунса и такт опроса
//! связанных файлов, — поэтому бюджет ядра здесь меньше порога из спеки ровно
//! на них. SC-003 (отклик на ввод во время сборки) ядром не измерить: его
//! держит сборка вне потока интерфейса, а проверяет сценарий
//! `scripts/dev/scenarios/preview-timeout.mjs`.
//!
//! Прогон: `cargo bench --bench preview_scale`.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::time::{Duration, Instant};
    use typsidian_core::preview::{
        compile_note, compile_note_resolved, dependency_mark, CompileOutcome, FontSet, PackageCache,
    };
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;

    /// Пауза дебаунса в интерфейсе (`PREVIEW_DEBOUNCE_MS`).
    const DEBOUNCE: Duration = Duration::from_millis(250);
    /// Такт опроса связанных файлов в интерфейсе (`DEPENDENCY_POLL_MS`).
    const POLL: Duration = Duration::from_millis(1000);

    /// SC-001: вид не позднее 2 с после открытия заметки.
    const FIRST_VIEW: Duration = Duration::from_millis(2000);
    /// SC-002: обновлённый вид не позднее 1 с после остановки набора.
    const AFTER_TYPING: Duration = Duration::from_millis(1000);
    /// SC-006: правка связанного файла видна не позднее 2 с.
    const AFTER_DEPENDENCY: Duration = Duration::from_millis(2000);

    /// Столько правок подряд — «долгая сессия» для проверки вытеснения кэша.
    const SESSION_EDITS: usize = 40;
    /// Столько связанных файлов у заметки в замере SC-006.
    const DEPENDENCIES: usize = 40;

    const NOTE: &str = "заметка.typ";

    /// Документ на ~50 страниц.
    fn reference_note(pages: usize, edit: usize) -> String {
        let mut text = String::new();
        for page in 1..=pages {
            text.push_str(&format!("= Раздел {page}\n\n"));
            for para in 1..=6 {
                // Правка — в середине документа: это худший случай для
                // пересборки, страницы после неё переверстываются.
                let mark = if page == pages / 2 && para == 3 {
                    format!(" Правка {edit}.")
                } else {
                    String::new()
                };
                text.push_str(&format!(
                    "Абзац {para} раздела {page} с формулой $x^2 + y^2 = z^2$ и достаточным \
                     количеством слов, чтобы строка переносилась естественным образом.{mark}\n\n"
                ));
            }
            if page < pages {
                text.push_str("#pagebreak()\n\n");
            }
        }
        text
    }

    fn median(mut times: Vec<Duration>) -> Duration {
        times.sort();
        times[times.len() / 2]
    }

    let fonts = FontSet::builtin();

    // ── SC-001: первая сборка ──────────────────────────────────────────────
    let text = reference_note(50, 0);
    let started = Instant::now();
    let first = compile_note(NOTE, &text, 0, fonts.clone()).expect("сборка");
    let cold = started.elapsed();
    assert_eq!(first.outcome, CompileOutcome::Succeeded);
    println!(
        "документ:                    {} КБ, {} стр.",
        text.len() / 1024,
        first.pages.len()
    );
    println!("первая сборка (SC-001):      {cold:?} — бюджет {FIRST_VIEW:?}");

    // ── SC-002 и вытеснение кэша: сессия правок ────────────────────────────
    let mut times = Vec::with_capacity(SESSION_EDITS);
    for edit in 1..=SESSION_EDITS {
        let text = reference_note(50, edit);
        let started = Instant::now();
        let document = compile_note(NOTE, &text, edit as u64, fonts.clone()).expect("пересборка");
        times.push(started.elapsed());
        assert_eq!(document.outcome, CompileOutcome::Succeeded);
    }
    let early = median(times[..SESSION_EDITS / 4].to_vec());
    let late = median(times[SESSION_EDITS * 3 / 4..].to_vec());
    let slowest = *times.iter().max().expect("правки были");
    println!(
        "пересборка после правки:     медиана {:?}, худшая {slowest:?} — бюджет ядра {:?}",
        median(times.clone()),
        AFTER_TYPING - DEBOUNCE
    );
    println!("  начало сессии / её конец:  {early:?} / {late:?}");

    // ── SC-006: правка связанного файла ────────────────────────────────────
    let dir = tempfile::TempDir::new().expect("временный каталог");
    let root = dir.path();
    let mut imports = String::new();
    for index in 0..DEPENDENCIES {
        std::fs::write(
            root.join(format!("часть-{index:02}.typ")),
            format!("#let часть{index} = [Часть {index}]\n"),
        )
        .expect("связанный файл");
        imports.push_str(&format!("#import \"часть-{index:02}.typ\": часть{index}\n"));
    }
    let with_imports = format!("{imports}\n{}", reference_note(50, 0));

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let (mark_time, rebuild_time) = runtime.block_on(async {
        let backend = NativeBackend::open(root).await.expect("открытие vault");
        let packages = PackageCache::new();
        let built =
            compile_note_resolved(&backend, &packages, NOTE, &with_imports, 1, fonts.clone())
                .await
                .expect("сборка");
        assert_eq!(
            built.outcome,
            CompileOutcome::Succeeded,
            "{:?}",
            built.diagnostics
        );
        assert_eq!(built.dependencies.len(), DEPENDENCIES);
        let before = dependency_mark(&backend, &built.dependencies).await;

        std::fs::write(
            root.join("часть-07.typ"),
            "#let часть7 = [Часть седьмая, исправленная]\n",
        )
        .expect("правка связанного файла");

        // Такт опроса: отметка обязана измениться, иначе правку не заметят.
        let started = Instant::now();
        let after = dependency_mark(&backend, &built.dependencies).await;
        let mark_time = started.elapsed();
        assert_ne!(before, after, "правка связанного файла не видна в отметке");

        let started = Instant::now();
        let rebuilt =
            compile_note_resolved(&backend, &packages, NOTE, &with_imports, 2, fonts.clone())
                .await
                .expect("пересборка");
        assert_eq!(rebuilt.outcome, CompileOutcome::Succeeded);
        (mark_time, started.elapsed())
    });
    println!(
        "отметка {DEPENDENCIES} связанных файлов:  {mark_time:?}; пересборка после правки одного: \
         {rebuild_time:?} — бюджет ядра {:?}",
        AFTER_DEPENDENCY - POLL - DEBOUNCE
    );

    // ── Пороги ─────────────────────────────────────────────────────────────
    assert!(cold <= FIRST_VIEW, "SC-001: первая сборка {cold:?}");
    assert!(
        slowest + DEBOUNCE <= AFTER_TYPING,
        "SC-002: пересборка {slowest:?} плюс пауза {DEBOUNCE:?}"
    );
    // Вытеснение кэша мемоизации не должно делать конец долгой сессии
    // медленнее её начала: двукратный запас — на шум загруженной машины.
    assert!(
        late <= early * 2,
        "пересборка деградирует за сессию: {early:?} → {late:?}"
    );
    assert!(
        mark_time + rebuild_time + POLL + DEBOUNCE <= AFTER_DEPENDENCY,
        "SC-006: отметка {mark_time:?} и пересборка {rebuild_time:?}"
    );
    println!("пороги SC-001, SC-002, SC-006 выдержаны");
}

#[cfg(target_arch = "wasm32")]
fn main() {}
