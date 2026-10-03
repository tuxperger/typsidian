//! Бенчмарк цикла резолва на заметке с большим внешним шаблоном.
//!
//! Меряет не сборку самого документа, а её **повторение**: Typst обрывает
//! вычисление на первом недостающем файле, поэтому каждый файл шаблона стоит
//! отдельного прохода, а проход — это полная сборка заново
//! (`preview/snapshot.rs` § MAX_RESOLVE_ITERATIONS). Пока снимок начинался с
//! нуля при каждом вызове, эти проходы повторялись на каждое нажатие клавиши:
//! на настоящей заметке с `@preview/codly-languages:0.1.1` (86 иконок) это 97
//! сборок и 240 мс на показ вместо 4 мс.
//!
//! Здесь тот же документ собран из фикстуры: шаблон, чей `lib.typ` тянет
//! [`PART_COUNT`] файлов по одному. Сравниваются два режима, отличающиеся
//! ровно одним — переживает ли [`PackageCache`] сборку.
//!
//! `harness = false` и ручной замер — по той же причине, что и в остальных
//! бенчмарках: это сквозной сценарий, а не микрооперация.
//!
//! Прогон: `cargo bench --bench preview_resolve`.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::time::Instant;
    use typsidian_core::preview::{compile_note_resolved, CompileOutcome, FontSet, PackageCache};
    use typsidian_core::vault::backend::VaultBackend;
    use typsidian_core::vault::native::NativeBackend;

    /// Столько файлов шаблон отдаёт по одному за проход. Порядок величины
    /// взят у `codly-languages`, где иконок 86.
    const PART_COUNT: usize = 80;
    /// Во сколько раз повторный показ обязан быть быстрее первого.
    ///
    /// Порог грубый нарочно: настоящая разница — два порядка, и запас нужен
    /// только чтобы бенчмарк не мигал на загруженной машине. Падение ниже
    /// означает, что кэш шаблонов перестал переживать сборку.
    const MIN_SPEEDUP: f64 = 5.0;

    const NOTE: &str = "заметка.typ";

    let dir = tempfile::TempDir::new().expect("временный каталог");
    let root = dir.path();

    // Шаблон: манифест, точка входа и PART_COUNT кусочков. Каждый импорт в
    // `lib.typ` — отдельный промах, потому что до предыдущего компилятор не
    // доходит.
    let package = root.join(".typsidian/packages/preview/many/1.0.0");
    std::fs::create_dir_all(&package).expect("папка шаблона");
    std::fs::write(
        package.join("typst.toml"),
        "[package]\nname = \"many\"\nversion = \"1.0.0\"\nentrypoint = \"lib.typ\"\n",
    )
    .expect("манифест");

    let mut lib = String::new();
    for part in 0..PART_COUNT {
        lib.push_str(&format!("#import \"part-{part:03}.typ\": v{part:03}\n"));
        std::fs::write(
            package.join(format!("part-{part:03}.typ")),
            format!("#let v{part:03} = {part}\n"),
        )
        .expect("файл шаблона");
    }
    lib.push_str("#let итог = (\n");
    for part in 0..PART_COUNT {
        lib.push_str(&format!("  v{part:03},\n"));
    }
    lib.push_str(")\n");
    std::fs::write(package.join("lib.typ"), lib).expect("точка входа");

    let text = "#import \"@preview/many:1.0.0\": итог\n= Заметка\n#итог.len()\n";
    std::fs::write(root.join(NOTE), text).expect("заметка");

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
        let backend = NativeBackend::open(root).await.expect("открытие vault");
        let fonts = FontSet::builtin();
        let packages = PackageCache::new();

        // Первый показ: шаблон поднимается по файлу за проход.
        let started = Instant::now();
        let first = compile_note_resolved(&backend, &packages, NOTE, text, 1, fonts.clone())
            .await
            .expect("сборка");
        let cold = started.elapsed();
        assert_eq!(
            first.outcome,
            CompileOutcome::Succeeded,
            "фикстура обязана собираться: {:?}",
            first.diagnostics
        );

        // Повторный показ — то, что происходит на каждое нажатие клавиши.
        let started = Instant::now();
        let second = compile_note_resolved(&backend, &packages, NOTE, text, 2, fonts.clone())
            .await
            .expect("пересборка");
        let warm = started.elapsed();
        assert_eq!(second.outcome, CompileOutcome::Succeeded);

        // Тот же документ с пустым кэшем каждый раз — поведение до кэша, для
        // сравнения.
        let started = Instant::now();
        let _ = compile_note_resolved(&backend, &PackageCache::new(), NOTE, text, 3, fonts)
            .await
            .expect("сборка без кэша");
        let uncached = started.elapsed();

        println!("файлов в шаблоне:      {}", PART_COUNT + 2);
        println!("файлов осело в кэше:   {}", packages.len());
        println!("первый показ:          {cold:?}");
        println!("повторный показ:       {warm:?}");
        println!("показ без кэша:        {uncached:?}");

        let speedup = uncached.as_secs_f64() / warm.as_secs_f64().max(f64::EPSILON);
        println!("ускорение повтора:     ×{speedup:.1} (порог ×{MIN_SPEEDUP:.0})");
        if speedup < MIN_SPEEDUP {
            eprintln!(
                "ПОРОГ НЕ ВЫДЕРЖАН: повторный показ ускоряется лишь в {speedup:.1} раза — \
                 кэш шаблонов перестал переживать сборку"
            );
            std::process::exit(1);
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn main() {
    // Замер идёт на native: цикл резолва платформенно нейтрален, а разница
    // измеряется в проходах компилятора, а не в скорости файловой системы.
    // Одинаковость поведения на wasm32 держат зеркальные проверки в
    // `tests/preview_wasm.rs`.
}
