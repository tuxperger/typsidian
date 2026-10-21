# Specification Quality Checklist: Application Settings

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-08-08
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Notes

- Items marked incomplete require spec updates before `/speckit-clarify` or `/speckit-plan`
- **Iteration 1 (2026-08-08)**: три открытых маркера [NEEDS CLARIFICATION] — область действия
  настроек, момент вступления в силу смены режима редактирования, содержание категории
  «Дополнительно».
- **Iteration 2 (2026-08-08)**: все три сняты решениями пользователя и записаны в раздел
  Clarifications. Спека переписана с последовательной нумерацией FR-001…FR-044. Ключевые
  добавления, вытекающие из решений:
  - настройки принадлежат устройству, хранилище лежит вне vault, список vault — часть настроек
    (FR-010…FR-012); vault остаётся самодостаточным без них;
  - принудительное сохранение перед сменой режима не молчаливое (FR-029), а два отказных пути
    блокируют переключение — неудавшееся сохранение (FR-030) и неразрешённый конфликт
    (FR-031); без них выбор «сохранить и переключить» терял бы данные ровно в тех случаях,
    ради которых он и выбран;
  - смена режима без открытой заметки происходит без шага сохранения (FR-032);
  - раздел «Дополнительно» разделён на обслуживание (FR-036…FR-038) и диагностику
    (FR-039…FR-043); сброс к умолчаниям не трогает список vault и файлы vault (FR-038, SC-011);
  - журналы не содержат текста заметок и не уходят с устройства (FR-041, FR-042, SC-012) — это
    прямое следствие принципа IX конституции.
- **Iteration 3 (2026-08-09, `/speckit-clarify`)**: задано и принято 5 вопросов, записаны в
  `### Session 2026-08-09`. Уточнены FR-013, FR-020, FR-023, FR-025, FR-026, FR-035, FR-037;
  добавлен SC-014, расширены SC-004…SC-006. Существенное для планирования: у режима
  редактирования появилось третье наблюдаемое состояние (обычный текстовый ввод после отказа
  полноценного режима), конфиг не перечитывается автоматически, хранилище настроек следует
  правилу «диск — источник истины», запись о vault опознаётся по пути, сброс к умолчаниям —
  операция «всё или ничего». Все 16 пунктов чеклиста проходят (16/16 → 16/16, состояний не
  изменилось).
- **Iteration 4 (2026-08-09, второй `/speckit-clarify` после `/speckit-analyze`)**: задано и
  принято 3 вопроса по находке D1 — идентичность записи о vault по пути неприменима на
  веб-клиенте. Уточнены FR-018, FR-020, FR-023, Key Entities «Запись о vault» и сценарии US2
  (добавлены AS9, AS10, переномерован AS11). Более ранняя кларификация этой же даты «по пути к
  папке» помечена как пересмотренная, а не удалена. Все 16 пунктов проходят
  (16/16 → 16/16, состояний не изменилось).
- **Внимание: артефакты плана отстали от спеки.** `plan.md`, `research.md` §10,
  `data-model.md`, `contracts/settings-store-trait.md` и задачи US2 в `tasks.md` по-прежнему
  описывают идентичность по пути. Требуется повторный `/speckit-plan` либо ручная правка до
  `/speckit-implement`.
- **Гейт конституции пройден**: фича управляет режимом моста к внешнему редактору (FR-025…FR-035), и
  требуемый принципом V отдельный `/speckit-clarify` выполнен 2026-08-09 до планирования.
- Упоминание внешнего редактора в спеке — название режима, видимое пользователю в интерфейсе, а не выбор
  технологии реализации; принцип III не нарушен.
