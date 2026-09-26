# Specification Quality Checklist: Template Download

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-09-07
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

- Проверено грепом: ни адреса реестра, ни формата архива, ни названий протоколов и крейтов в
  спеке нет — всё это решения `plan.md` (принцип III).
- **Первая сетевая фича проекта.** Принцип VII конституции («корректная деградация в офлайне»)
  впервые применяется к непустому множеству и требует **тестов**, а не деклараций: FR-012…
  FR-014 и SC-005, SC-006, SC-009 написаны так, чтобы каждый проверялся.
- **Принцип IX проверен и не нарушается**: обмен односторонний, наружу уходит только имя и
  версия шаблона, которые пользователь сам написал в тексте подключения. Закреплено FR-021.
- **Отступление от ответа пользователя, внесённое сознательно.** Ограничение распаковки папкой
  версии (FR-019) пользователь среди желаемых гарантий не выбрал. Оно включено всё равно и
  объяснено в разделе Clarifications: распаковка, способная записать файл мимо назначения, —
  дефект реализации, а не урезанный вариант поведения. Предел размера архива при этом
  пользовательской гарантией не сделан, как он и просил, — он наследуется от уже принятых в
  проекте пределов.
- **Известное напряжение, разрешённое в спеке.** «Автоматически при отсутствии» и «показывать,
  что и откуда тянется» плохо совмещаются буквально: при автозагрузке нет момента «до».
  Разрешено так: загрузка идёт сама, отчёт даётся в диагностике сборки (FR-006, FR-007), а
  выключатель (FR-009) возвращает контроль тому, кому автоматизм не нужен.
- К обсуждению на `/speckit-clarify`: чем именно считать «явное действие» из FR-010 при
  выключенной автозагрузке — кнопкой в диагностике, командой палитры или пунктом настроек.
  **Решено при реализации (2026-09-25)**: кнопка «Загрузить шаблон» у замечания об
  отсутствующем шаблоне в предпросмотре. Какой шаблон предложить, ядро сообщает полем
  `Diagnostic.template`, а не текстом.

## Прогон quickstart

Сценарии — [quickstart.md](../quickstart.md). `[x]` — прогнано и сошлось, `[ ]` — не прогнано.

**2026-09-25, автоматически и на стенде:**

- [x] Таблица `install_*` — native 26/26, wasm32 (headless chrome) 26/26
- [x] Годность своих адресов — `placement_native` / `placement_wasm`, 20/20 на каждом
- [x] UI на стенде (chromium, поддельные клиенты, `ui/dist`): раздел шаблонов в панели папки,
  выключатель, свои адреса с отказом ядра без потери ввода, перечень с полным отпечатком,
  кнопка явной загрузки и пересборка после неё — 16/16, ошибок React нет
- [x] Сценарий 1 на **native-ядре с живым реестром** (временный тест поверх `NativeBackend`,
  удалён после прогона): `@preview/example:0.1.0` загружен (5 файлов), документ собран,
  повторная сборка без единого замечания; отпечаток двух независимых загрузок совпал
  (`76fa172f6728…`, FR-024)
- [x] Сценарий 7, native-ядро: недостижимый адрес → «сети нет: <адрес> недостижим» с кнопкой
  явной загрузки; неотвечающий хост обрывается ровно на 30 с (FR-014); 404 →
  `VersionNotFound`

**Не прогонялось руками в собранных приложениях:**

- [ ] Сценарии 1–8 в web-сборке (в том числе офлайн через DevTools — исход `Blocked`)
- [ ] Сценарии 1–8 в desktop-сборке (Tauri), включая выключенную сеть и vault только для чтения
