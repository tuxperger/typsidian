# Typsidian UI: как строить экраны

Тёмный инструмент для заметок на Typst. Интерфейс на русском, плотный, управляется с клавиатуры.

## Обёртка

Все компоненты рисуются внутри `I18nProvider` и `ContextMenuProvider`: без первого тексты берутся не на том языке, без второго не открываются меню. `IconSprite` монтируется один раз в корне: без него `Icon` пуст.

```jsx
const { I18nProvider, ContextMenuProvider, IconSprite, ViewSwitch } = window.TypsidianUI;
<I18nProvider choice="ru">
  <ContextMenuProvider>
    <IconSprite />
    <div style={{ background: "var(--steel-950)", color: "var(--steel-200)", fontFamily: "var(--font-sans)", padding: 16 }}>
      <ViewSwitch value="side" onChange={setView} />
    </div>
  </ContextMenuProvider>
</I18nProvider>
```

Страница не тёмная сама по себе: фон `var(--steel-950)` задаёт корневой контейнер экрана.

## Стили

Компоненты стилизуются своими классами `ts-*`, новые придумывать не нужно. Для своей раскладки используй inline-стили на токенах, цвета и шрифты числами не вписывай.

| Назначение | Токены |
| --- | --- |
| Фон приложения, панели | `--steel-950`, `--steel-900` |
| Границы | `--steel-800`, элементы управления `--steel-700` |
| Текст | основной `--steel-200`, `--steel-100`; вторичный `--steel-400`, `--steel-500` |
| Акцент | `--orange-500`, наведение `--orange-600` |
| Несохранённые правки (только они) | `--hivis-500` |
| Шрифты | интерфейс `--font-sans`, код и пути `--font-mono`, текст листа `--font-serif` |
| Форма | `--radius-md`, модальные окна `--radius-lg` и `--shadow-xl` |

Кнопки: `ts-btn`, тихая `ts-btn ts-btn--quiet`. Сегменты: `ts-segmented`.

## Где смотреть

Токены и стили: `styles.css` и `_ds_bundle.css`. API и примеры: `components/<группа>/<Имя>/<Имя>.d.ts` и `<Имя>.prompt.md`.

## Ограничения

Тексты компонентов берутся из каталогов приложения и в пропсы не передаются. Данные (`Note`, `TabView`, `device`) в реальном приложении приходят от ядра; в макетах передавай простые объекты, как в примерах `.prompt.md`. Режима Vim в редакторе нет, плашку NORMAL/INSERT не рисуй.
