# Quickstart: практика

```bash
nix develop -c cargo test -p typsidian-core --test ai_native practice
```

Поддельный поток: `pose_problem` с решением → `LessonView` без решения; подсказка; ответ →
`conclude` → решение видно; без `NextProblem` вторая задача не ставится; `pose_problem` без
решения → отказ модели, затем задача с пометкой.

Браузер: `scripts/dev/scenarios/ai-practice.mjs` — новая практика по уроку, задача,
подсказка, ответ, итог, сводка; сверка с артбордами A–E.
