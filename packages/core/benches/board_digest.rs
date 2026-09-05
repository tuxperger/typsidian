//! Бенчмарк нормализации и хэша доски (T061, SC-005, research.md §10).
//!
//! Что здесь измеряется и почему именно это: SC-005 обещает отклик ≤100 мс на
//! ввод, а единственное, что приложение добавляет в цикл движка, — сериализация
//! и хэширование содержимого при автосохранении (research.md §6). Отрисовку и
//! реакцию на штрих измерить здесь нельзя: это работа движка в браузере, и она
//! проверяется сценарием 7 `quickstart.md`.
//!
//! `harness = false` и ручной замер — по той же причине, что в `vault_scale.rs`:
//! измеряется не микрооперация ради статистики, а один сценарий с понятным
//! бюджетом из спеки.
//!
//! Прогон: `cargo bench --bench board_digest`.

fn main() {
    use std::time::{Duration, Instant};
    use typsidian_core::boards::digest;

    /// Предел, до которого спека даёт гарантии (research.md §10).
    const ELEMENT_COUNT: usize = 2_000;
    /// Доля бюджета SC-005, которую позволено занять хэшированию.
    ///
    /// 100 мс — весь бюджет отклика на ввод, и хэш в нём не один: до него идут
    /// сериализация сцены движком и передача через границу WASM. Треть — предел,
    /// после которого вынос за главный поток перестаёт спасать.
    const DIGEST_BUDGET: Duration = Duration::from_millis(33);
    /// Сколько раз считать хэш: разброс на одном прогоне сравним с самим
    /// временем, а бюджет проверяется по среднему.
    const ITERATIONS: u32 = 20;

    let board = board_with(ELEMENT_COUNT);
    println!(
        "доска: {ELEMENT_COUNT} элементов, {} КБ JSON",
        board.len() / 1024
    );

    // Первый вызов вне замера: он прогревает аллокатор и кэш процессора, и его
    // время говорит не о нормализации, а о состоянии машины.
    let reference = digest(board.as_bytes()).expect("эталонная доска разбирается");

    let started = Instant::now();
    for _ in 0..ITERATIONS {
        let computed = digest(board.as_bytes()).expect("доска разбирается");
        assert_eq!(computed, reference, "хэш обязан быть детерминированным");
    }
    let average = started.elapsed() / ITERATIONS;

    println!("нормализация + BLAKE3: {average:?} в среднем за {ITERATIONS} прогонов");
    assert!(
        average <= DIGEST_BUDGET,
        "хэш доски на пределе в {ELEMENT_COUNT} элементов занял {average:?} — \
         бюджет {DIGEST_BUDGET:?} (SC-005)"
    );

    // Второе измерение того же бюджета с другой стороны: устойчивость к
    // счётчикам ревизий не должна стоить отдельного прохода по документу.
    let touched = board.replace("\"version\":1", "\"version\":2");
    let started = Instant::now();
    let touched_digest = digest(touched.as_bytes()).expect("доска разбирается");
    let touched_elapsed = started.elapsed();
    assert_eq!(
        touched_digest, reference,
        "счётчики ревизий не являются содержимым (research.md §5)"
    );
    println!("та же доска после касания движком: {touched_elapsed:?}");
}

/// Доска с заданным числом элементов, каждый — со счётчиками ревизий и полным
/// набором стилевых полей, как их пишет движок.
fn board_with(element_count: usize) -> String {
    let mut elements = String::new();
    for index in 0..element_count {
        if index > 0 {
            elements.push(',');
        }
        elements.push_str(&format!(
            r##"{{"id":"element-{index}","type":"rectangle","x":{x},"y":{y},"width":120,"height":80,
"angle":0,"strokeColor":"#1e1e1e","backgroundColor":"transparent","fillStyle":"solid",
"strokeWidth":2,"strokeStyle":"solid","roughness":1,"opacity":100,"groupIds":[],
"frameId":null,"roundness":{{"type":3}},"boundElements":[],"link":null,"locked":false,
"version":1,"versionNonce":{nonce},"updated":1700000000000,"seed":{seed},"isDeleted":false}}"##,
            x = index % 50 * 140,
            y = index / 50 * 100,
            nonce = index * 7 + 1,
            seed = index * 13 + 3,
        ));
    }
    format!(
        r##"{{"type":"excalidraw","version":2,"source":"typsidian","elements":[{elements}],
"appState":{{"viewBackgroundColor":"#ffffff","scrollX":0,"scrollY":0,"zoom":{{"value":1}}}},
"files":{{}}}}"##
    )
}
