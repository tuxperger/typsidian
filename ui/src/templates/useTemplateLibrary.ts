"use client";

import { useCallback, useEffect, useState } from "react";
import type {
  Template,
  TemplateLibraryView,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";

// Состав библиотеки заготовок (T020, FR-003, SC-010).
//
// Своего вотчера здесь нет: события те же, что у дерева заметок (research.md
// §3). Второй источник событий по тем же файлам означал бы две очереди с разным
// порядком доставки и гонки между ними, а гарантия «внешнее изменение видно
// ≤2 с» уже дана фичей 001.
//
// **Событие сюда не приходит вовсе — приходит счётчик.** `pollEvents` осушает
// очередь ядра, и второй подписчик перехватывал бы события у первого, поэтому
// на поток смотрит `Workspace`. Он же спрашивает ядро, затрагивает ли событие
// библиотеку (`templateEvent`), и увеличивает счётчик, если затрагивает. Так
// ничего не теряется внутри пачки: каждое событие рассматривается отдельно, а
// счётчик только растёт.
//
// Инвалидацией **превью** этот хук не занимается: ключ там — хэш содержимого,
// он лежит в самой записи библиотеки и приезжает вместе с перечитанным
// составом (research.md §4).

export interface TemplateLibraryState {
  view: TemplateLibraryView | null;
  /** Первая загрузка ещё идёт: список пока не из чего строить. */
  isLoading: boolean;
  /** Перечитать состав вручную — сразу после сохранения заготовки, например. */
  refresh: () => Promise<void>;
}

export function useTemplateLibrary(
  client: VaultClient | null,
  /**
   * Счётчик изменений библиотеки. Растёт, когда ядро сказало, что событие
   * файла её затрагивает.
   */
  revision = 0,
): TemplateLibraryState {
  const [view, setView] = useState<TemplateLibraryView | null>(null);
  const [isLoading, setIsLoading] = useState(client !== null);

  const refresh = useCallback(async () => {
    if (!client) {
      setView(null);
      setIsLoading(false);
      return;
    }
    const next = await client.templateLibrary();
    setView(next);
    setIsLoading(false);
  }, [client]);

  useEffect(() => {
    void refresh();
    // `revision` в зависимостях намеренно: он и есть повод перечитать состав.
  }, [refresh, revision]);

  return { view, isLoading, refresh };
}

/**
 * Отбор по имени (FR-015) и группировка по вложенной папке (research.md §2).
 *
 * Отбор здесь, а не в ядре: это фильтр представления, он меняется от нажатия
 * клавиши и обращения к vault не требует. Регистр не различается — пользователь
 * ищет «формула», а не «Формула».
 */
export function groupTemplates(
  templates: Template[],
  filter: string,
): { group: string | null; templates: Template[] }[] {
  const needle = filter.trim().toLocaleLowerCase();
  const matching =
    needle === ""
      ? templates
      : templates.filter((template) =>
          template.name.toLocaleLowerCase().includes(needle),
        );

  const groups: { group: string | null; templates: Template[] }[] = [];
  for (const template of matching) {
    const group = template.group ?? null;
    const known = groups.find((entry) => entry.group === group);
    if (known) known.templates.push(template);
    else groups.push({ group, templates: [template] });
  }
  return groups;
}

/**
 * Ключ карточки в списке — путь заготовки.
 *
 * Отдельная функция, чтобы список и карточка пользовались одним ключом: если
 * они разойдутся, превью будут прилетать не в те карточки.
 */
export function templateKey(template: Template): string {
  return template.path;
}

/**
 * Ключ пересчёта превью — хэш содержимого заготовки (research.md §4).
 *
 * Именно хэш, а не путь и не время изменения: правка заготовки снаружи меняет
 * хэш, и превью пересчитывается само (FR-003, SC-010), а переименование файла
 * превью не инвалидирует. То же правило, что в кэше ядра, — и здесь оно не
 * повторяется, а используется: хэш приходит из отпечатка, посчитанного индексом.
 *
 * Пустая строка — хэша нет: файл нечитаем, и превью для него не строится.
 */
export function previewKey(template: Template): string {
  return template.content_hash?.join(",") ?? "";
}
