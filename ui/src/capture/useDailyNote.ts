"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { DailyDate, VaultOverrides } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import type { Translate } from "../i18n/index.js";
import { NOTHING_OVERRIDDEN } from "../workspace/useShortcuts.js";

/**
 * «Сегодня» и переход по ежедневным заметкам (фича 022, US2, FR-007…FR-009).
 *
 * Правил здесь нет (принцип I): путь по дате, создание из шаблона, распознавание
 * ежедневной заметки по пути и соседние дни — ответы ядра. Хук берёт дату по
 * часам устройства, спрашивает ядро и открывает то, что оно назвало.
 */
export interface DailyNoteInput {
  client: VaultClient | null;
  /** Настройки vault окна; ядро читает свои сами, а эти нужны чистым функциям. */
  overrides: VaultOverrides | null;
  /** Путь открытой заметки; `null` — открыта не заметка. */
  activePath: string | null;
  /** Растёт на изменения файлов: соседние дни пересчитываются. */
  revision: number;
  /** Открыть заметку: каретка в конце, фокус редактора. */
  openAtEnd: (path: string) => Promise<void>;
  /** Заметка создана «Сегодня» — дерево перечитывается. */
  onCreated: () => void;
  /** Короткое сообщение пользователю. */
  notify: (message: string) => void;
  t: Translate;
}

/** Открытая ежедневная заметка и её соседи. */
export interface DailyPosition {
  date: DailyDate;
  /** Путь предыдущей существующей ежедневной заметки; `null` — раньше нет. */
  previous: string | null;
  next: string | null;
}

export interface DailyNote {
  /** `null` — открыта не ежедневная заметка: кнопок ‹ › нет. */
  position: DailyPosition | null;
  openToday: () => Promise<void>;
  openPrevious: () => Promise<void>;
  openNext: () => Promise<void>;
}

/** Сегодняшняя дата по часам устройства в момент нажатия (Edge Cases). */
export function deviceToday(now: Date = new Date()): DailyDate {
  return { year: now.getFullYear(), month: now.getMonth() + 1, day: now.getDate() };
}

export function useDailyNote({
  client,
  overrides,
  activePath,
  revision,
  openAtEnd,
  onCreated,
  notify,
  t,
}: DailyNoteInput): DailyNote {
  const [position, setPosition] = useState<DailyPosition | null>(null);
  const overridesRef = useRef(overrides);
  overridesRef.current = overrides;

  // Ежедневная ли открытая заметка и кто её соседи. Гонка та же, что у колонки
  // связей: ответ на устаревший путь не должен вытеснить новый.
  useEffect(() => {
    if (client === null || activePath === null) {
      setPosition(null);
      return undefined;
    }
    let alive = true;
    void (async () => {
      try {
        const settings = await client.capture.settings(overridesRef.current ?? NOTHING_OVERRIDDEN);
        const date = await client.capture.dailyDateOf(settings, activePath);
        if (date === undefined) {
          if (alive) setPosition(null);
          return;
        }
        const neighbours = await client.capture.dailyNeighbours(date);
        if (alive) {
          setPosition({
            date,
            previous: neighbours.previous ?? null,
            next: neighbours.next ?? null,
          });
        }
      } catch {
        // Соседей узнать не удалось — кнопок нет; открытая заметка от этого не страдает.
        if (alive) setPosition(null);
      }
    })();
    return () => {
      alive = false;
    };
  }, [client, activePath, revision, overrides]);

  const openToday = useCallback(async () => {
    if (client === null) return;
    try {
      const opened = await client.capture.openDaily(deviceToday());
      if (opened.created) onCreated();
      await openAtEnd(opened.path);
      if (opened.template === "missing") notify(t("capture.daily.templateMissing"));
    } catch (cause) {
      notify(t("capture.daily.failed", { reason: dailyFailure(t, cause) }));
    }
  }, [client, onCreated, openAtEnd, notify, t]);

  const openPrevious = useCallback(async () => {
    if (position?.previous != null) await openAtEnd(position.previous);
  }, [position, openAtEnd]);

  const openNext = useCallback(async () => {
    if (position?.next != null) await openAtEnd(position.next);
  }, [position, openAtEnd]);

  return { position, openToday, openPrevious, openNext };
}

/** Причина отказа «Сегодня» словами; неизвестный вид — как отказ диска. */
function dailyFailure(t: Translate, cause: unknown): string {
  const kind =
    typeof cause === "object" && cause !== null && "kind" in cause
      ? (cause as { kind: unknown }).kind
      : undefined;
  switch (kind) {
    case "InvalidDate":
      return t("capture.daily.error.InvalidDate");
    case "ReadOnly":
      return t("capture.daily.error.ReadOnly");
    case "NoVault":
      return t("capture.daily.error.NoVault");
    default:
      return t("capture.daily.error.Io");
  }
}
