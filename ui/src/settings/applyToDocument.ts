import { useEffect } from "react";
import type { Settings, Theme } from "../wasm-core/typsidian_core.js";

/**
 * Имена CSS-переменных, которыми настройки внешнего вида доходят до остального
 * интерфейса. Объявлены здесь один раз, потому что их читает не только панель.
 *
 * Кто читает `FONT_SIZE_VARIABLE` на самом деле: правило `.ts-editor .cm-editor`
 * в `ui/src/styles/app.css`. Связь идёт каскадом, а не пропсом, и это
 * существенно — редактор перерисовывать для смены размера не нужно, поэтому
 * FR-017 «без переоткрытия» выполняется само собой.
 */
export const THEME_ATTRIBUTE = "data-theme";
export const FONT_SIZE_VARIABLE = "--typsidian-editor-font-size";

/**
 * Применяет внешний вид к корню документа (T044, FR-017, SC-002).
 *
 * Почему через CSS-переменные и атрибут на `<html>`, а не пропсами вглубь
 * дерева: значения нужны и редактору, и панели предпросмотра, лежащим в разных
 * ветках дерева. Переменная на корне доходит до обоих каскадом, а проп
 * пришлось бы протаскивать через всё, что между ними.
 *
 * Оговорка, которая была здесь раньше — «компонента редактора ещё не
 * существует» — снята: `ui/src/editor/CodeEditor.tsx` появился и размер
 * потребляет. Что **не** следует за настройкой: цвета самого редактора. Панель
 * редактора тёмная всегда, по макету, и темой не переключается — это решение,
 * а не пропуск.
 *
 * Стоимость применения — установка двух свойств на один узел, поэтому бюджет
 * SC-002 (≤200 мс от действия пользователя) определяется не этим шагом, а
 * временем возврата `apply` из ядра.
 */
export function applyAppearanceToDocument(settings: Settings): void {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  root.setAttribute(THEME_ATTRIBUTE, themeName(settings.theme));
  if (settings.editor_font_size !== undefined) {
    root.style.setProperty(FONT_SIZE_VARIABLE, `${settings.editor_font_size}px`);
  }
}

function themeName(theme: Theme | undefined): string {
  switch (theme) {
    case "light":
      return "light";
    case "dark":
      return "dark";
    case undefined:
      // Значения ещё не загружены — держим светлую тему по умолчанию, чтобы не
      // моргать тёмным на первом кадре.
      return "light";
    default: {
      const exhaustive: never = theme;
      return exhaustive;
    }
  }
}

/**
 * Держит корень документа в соответствии с настройками. Отдельный хук, чтобы
 * применение работало и от изменения через панель, и от внешней правки файла:
 * оба пути приводят к новому `settings`, и разницы между ними здесь нет
 * (FR-013, FR-006).
 */
export function useAppearance(settings: Settings | null): void {
  const theme = settings?.theme;
  const fontSize = settings?.editor_font_size;

  useEffect(() => {
    if (!settings) return;
    applyAppearanceToDocument(settings);
    // Зависимости — сами значения, а не объект: `settings` пересоздаётся на
    // каждый опрос ядра, и по нему эффект срабатывал бы раз в секунду впустую.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [theme, fontSize]);
}
