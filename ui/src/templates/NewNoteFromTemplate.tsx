"use client";

import type { Template } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { useI18n } from "../i18n/index.js";

// Выбор заготовки в момент создания заметки (T024, FR-020, FR-022, FR-024).
//
// Макет «Typsidian Утилиты» § Шаблоны объясняет форму прямо: «в меню „+“ шаблон
// выбирается сразу под пунктом „Заметка Typst“ — вторым уровнем, без отдельного
// шага после создания». Отсюда SC-001: от намерения до заполненной заметки — не
// больше трёх действий.
//
// Имя и расположение задаются **существующими средствами 001**: после выбора
// открывается тот же инлайн-ввод имени в дереве, что и у пустой заметки
// (FR-022). Своей формы ввода здесь нет и быть не должно — она была бы вторым
// местом, знающим правила имён.

/** Сколько заготовок показывается вторым уровнем меню создания. */
export const MENU_TEMPLATE_LIMIT = 6;

export interface NewNoteFromTemplateProps {
  templates: Template[];
  /**
   * Выбор сделан. `null` — «Пустая заметка»: отказ от выбора создаёт обычную
   * пустую заметку (FR-024), а не отменяет создание.
   */
  onChoose: (template: Template | null) => void;
  /** Открыть галерею целиком — когда заготовок больше, чем помещается в меню. */
  onOpenGallery?: () => void;
}

/**
 * Второй уровень меню создания: список заготовок и «Пустая заметка».
 *
 * Список подрезан до [`MENU_TEMPLATE_LIMIT`]: меню создания — это быстрый путь,
 * а не галерея. Всё остальное открывается галереей, где есть фильтр и превью.
 */
export function NewNoteFromTemplate({
  templates,
  onChoose,
  onOpenGallery,
}: NewNoteFromTemplateProps): React.JSX.Element {
  const { t } = useI18n();
  const shown = templates.slice(0, MENU_TEMPLATE_LIMIT);
  const hidden = templates.length - shown.length;

  return (
    <div className="ts-create__sub" role="group" aria-label={t("templates.menu.fromTemplate")}>
      {shown.map((template) => (
        <button
          key={template.path}
          type="button"
          role="menuitem"
          className="ts-create__subrow"
          onClick={() => onChoose(template)}
        >
          <span className="ts-create__dot" aria-hidden="true" />
          {stemOf(template.name)}
        </button>
      ))}

      <button
        type="button"
        role="menuitem"
        className="ts-create__subrow ts-create__subrow--muted"
        onClick={() => onChoose(null)}
      >
        {t("templates.menu.blank")}
      </button>

      {hidden > 0 && onOpenGallery && (
        <button
          type="button"
          role="menuitem"
          className="ts-create__subrow ts-create__subrow--muted"
          onClick={onOpenGallery}
        >
          {t("templates.menu.more", { count: hidden })}
        </button>
      )}
    </div>
  );
}

/** Имя заготовки без расширения — в меню оно только мешает. */
export function stemOf(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(0, dot) : name;
}

/**
 * Наполняет только что созданную заметку содержимым заготовки
 * (FR-021, FR-023).
 *
 * Отдельной операции ядра для этого нет и не нужно: композиция сводится к
 * `read_template` плюс существующему `write` (research.md §7,
 * contracts/template-library-trait.md). Правил здесь нет — только порядок.
 *
 * **Файл заготовки не изменяется ни при каком исходе** (инвариант 1): его
 * только читают. И **связи между заготовкой и созданной заметкой не
 * возникает** (инвариант 2): позже изменённая заготовка эту заметку не тронет,
 * потому что тронуть её нечему.
 *
 * Отказ пробрасывается вызывающему: он же показывает причину пользователю — и
 * причину называет ядро.
 */
export async function fillFromTemplate(
  client: VaultClient,
  notePath: string,
  template: Template,
): Promise<void> {
  const content = await client.readTemplate(template.path);
  // `expected: undefined` — заметка только что создана этим же потоком, и
  // сравнивать её с диском не с чем.
  await client.write(notePath, content, undefined);
}
