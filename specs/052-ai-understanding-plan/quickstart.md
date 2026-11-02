# Quickstart: замер и план

## Тесты ядра

```bash
nix develop -c cargo test -p typsidian-core --test ai_native lesson_plan
env -u LD_LIBRARY_PATH CARGO_PROFILE_DEV_DEBUG=0 nix develop -c \
  wasm-pack test --headless --chrome packages/core --test ai_wasm
```

Поддельный поток: два `probe_question`, `probe_done`, `plan_set` с циклом (отклонён), верный
`plan_set`, `plan_progress`, второй `plan_set` с добавленным шагом.

## В браузере

Сценарий `scripts/dev/scenarios/ai-lesson-plan.mjs`: урок с замером, ответ «Не знаю»,
итог замера, план-карта и список, история версий. Сверка с артбордами B–F макета.
