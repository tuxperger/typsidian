# Quickstart: Проверка орфографии

1. Открыть vault, заметку с текстом из макета O1: «првоерка», «резултатах», «conductivty» в
   тексте и те же слова в `$…$`, `` `…` ``, `#set text(…)`, `<метке>`, `@ключе`, `[[ссылке]]`,
   адресе. Подчёркнуты только три слова в тексте (SC-001).
2. Правый щелчок по «првоерка» → «проверка» — слово заменено, Ctrl+Z возвращает.
3. Правый щелчок по «Typsidian» → «Добавить в словарь» — подчёркивание исчезает и в другой
   заметке; в `.typsidian/dictionary.txt` появилась строка.
4. Настройки → «Орфография»: слово видно в списке, «убрать» — снова подчёркнуто; снять
   «English» — «conductivty» не подчёркнуто; выключить проверку — подчёркиваний нет.
5. Тесты: `cargo test --test spellcheck_native`, `wasm-pack test` (`spellcheck_wasm.rs`).
6. Бенч: `cargo bench --bench spellcheck_scale` — проверка окна заметки на 10 000 строк.
7. Сценарий: `nix develop -c node scripts/dev/browser.mjs http://localhost:3000 scripts/dev/scenarios/spellcheck.mjs --opfs-vault`.
