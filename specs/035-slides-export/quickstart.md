# Quickstart: Экспорт презентации

1. Заметка-презентация: 3 слайда, у второго 2 шага (`#pagebreak()` с одним `<pdfpc>`-номером),
   у первого заметки докладчика.
2. «Показ ▾ → Экспорт презентации…» → PDF слайдов, последний шаг → 3 страницы; все шаги → 4.
3. PDF со страницами заметок → 3 страницы, на первой — заметки под слайдом.
4. PDF для pdfpc → `Доклад.pdf` (4 стр.) и `Доклад.pdfpc` с заметками.
5. PNG 2× → `Доклад.slides/01.png…03.png`; SVG все шаги → `01.svg, 02-1.svg, 02-2.svg, 03.svg`.
6. Тесты: `cargo test --test slides_export_native`, `wasm-pack test` (`slides_export_wasm.rs`);
   сценарий `scripts/dev/scenarios/slides-export.mjs`.
