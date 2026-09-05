//! Бенчмарк масштаба (T092): подтверждает SC-002 и SC-003 на 10 000 заметок.
//!
//! `harness = false` и ручной замер вместо criterion — осознанно: измеряется не
//! микрооперация, требующая статистики по тысячам итераций, а два однократных
//! сквозных сценария (холодное и повторное открытие vault), каждый из которых
//! стоит секунды и трогает диск. Плюс это не добавляет внешнюю зависимость ради
//! одного `Instant::now()`.
//!
//! Прогон: `cargo bench --bench vault_scale`. Числа зависят от оборудования —
//! пороги ниже взяты из spec.md (эталон: SSD, современный потребительский
//! класс), поэтому на медленном диске или под нагрузкой бенч честно сообщит о
//! непрохождении, а не подгонит результат.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::time::{Duration, Instant};
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;

    /// SC-002: полный обход 10 000 заметок.
    const FULL_SCAN_BUDGET: Duration = Duration::from_secs(10);
    /// SC-002: дерево пригодно для навигации.
    const NAVIGABLE_BUDGET: Duration = Duration::from_secs(3);
    /// SC-003: повторное открытие за счёт персистентного кэша.
    const REOPEN_BUDGET: Duration = Duration::from_secs(1);
    const NOTE_COUNT: usize = 10_000;
    /// Заметки раскладываются по подпапкам, а не плоско: обход дерева и
    /// вывод иерархии из путей — часть измеряемого пути (FR-003).
    const NOTES_PER_FOLDER: usize = 100;

    let dir = tempfile::TempDir::new().expect("временный каталог");
    let root = dir.path();

    let generation_started = Instant::now();
    for i in 0..NOTE_COUNT {
        let folder = root.join(format!("folder-{:03}", i / NOTES_PER_FOLDER));
        std::fs::create_dir_all(&folder).expect("создание подпапки");
        // Содержимое со ссылкой: индексация ссылок (US5) идёт тем же чтением,
        // что и снимок файла, и обязана входить в измеряемое время.
        std::fs::write(
            folder.join(format!("note-{i:05}.typ")),
            format!("= Заметка {i}\n\nсм. [[note-{:05}]]\n", (i + 1) % NOTE_COUNT),
        )
        .expect("запись заметки");
    }
    eprintln!(
        "подготовка {NOTE_COUNT} заметок: {:?}",
        generation_started.elapsed()
    );

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");

    let cold = runtime.block_on(async {
        let started = Instant::now();
        let backend = NativeBackend::open(root).await.expect("открытие vault");
        let opened = started.elapsed();
        let listed = backend.list().await.len();
        (opened, started.elapsed(), listed)
    });
    let (cold_open, cold_total, cold_count) = cold;
    assert_eq!(cold_count, NOTE_COUNT, "индекс должен содержать все заметки");

    // Дать фоновой записи кэша завершиться — иначе повторное открытие
    // измеряло бы не быстрый путь, а тот же холодный обход.
    std::thread::sleep(Duration::from_millis(500));

    let warm = runtime.block_on(async {
        let started = Instant::now();
        let backend = NativeBackend::open(root).await.expect("повторное открытие");
        let opened = started.elapsed();
        let listed = backend.list().await.len();
        (opened, listed)
    });
    let (warm_open, warm_count) = warm;

    println!("SC-002 холодное открытие ({NOTE_COUNT} заметок): {cold_open:?} (бюджет навигации {NAVIGABLE_BUDGET:?})");
    println!("SC-002 полный обход: {cold_total:?} (бюджет {FULL_SCAN_BUDGET:?})");
    println!("SC-003 повторное открытие: {warm_open:?} (бюджет {REOPEN_BUDGET:?}), заметок {warm_count}");

    let mut failures = Vec::new();
    if cold_open > NAVIGABLE_BUDGET {
        failures.push(format!(
            "SC-002: дерево пригодно для навигации за {cold_open:?}, бюджет {NAVIGABLE_BUDGET:?}"
        ));
    }
    if cold_total > FULL_SCAN_BUDGET {
        failures.push(format!(
            "SC-002: полный обход за {cold_total:?}, бюджет {FULL_SCAN_BUDGET:?}"
        ));
    }
    if warm_open > REOPEN_BUDGET {
        failures.push(format!(
            "SC-003: повторное открытие за {warm_open:?}, бюджет {REOPEN_BUDGET:?}"
        ));
    }
    if warm_count != NOTE_COUNT {
        failures.push(format!(
            "повторное открытие вернуло {warm_count} заметок вместо {NOTE_COUNT}"
        ));
    }

    if failures.is_empty() {
        println!("все бюджеты выдержаны");
    } else {
        for failure in &failures {
            eprintln!("НЕ ВЫДЕРЖАН — {failure}");
        }
        std::process::exit(1);
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    // Бенчмарк меряет нативный обход файловой системы; web-таргет измеряется
    // сценарием 7 quickstart.md в браузере (research.md §3).
}
