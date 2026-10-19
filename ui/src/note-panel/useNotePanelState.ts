"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { NotePanelMode, NotePanelState, OutlineKinds } from "../wasm-core/typsidian_core.js";
import type { PanelModeId } from "./modes.js";
import type { DeviceSettingsClient } from "../settings/device-settings-client.js";
import { changeOf } from "../settings/useSettings.js";

// Состояние колонки заметки в настройках устройства (фича 013, FR-021,
// research.md §12). Что значит «негодное значение» и куда уходит правка,
// решило ядро (`Settings.note_panel`, `apply::route`): здесь только чтение
// снимка один раз и запись с паузой.

/** Пауза перед записью: переключатели нажимают подряд, файл на каждый щелчок не пишется. */
export const NOTE_PANEL_PERSIST_DEBOUNCE_MS = 400;

/** Умолчания совпадают с ядром (`NotePanelState::default`): колонка скрыта, упоминания свёрнуты. */
const DEFAULT_MODE: NotePanelMode = "links";
/** Оглавление по умолчанию показывает только заголовки. */
const NO_KINDS: OutlineKinds = { figures: false, labels: false };

export interface NotePanelStateHandle {
  shown: boolean;
  mode: PanelModeId;
  mentionsExpanded: boolean;
  /** Виды пунктов оглавления кроме заголовков (фича 015, FR-010). */
  outlineKinds: OutlineKinds;
  toggle: () => void;
  setShown: (shown: boolean) => void;
  setMode: (mode: PanelModeId) => void;
  setMentionsExpanded: (expanded: boolean) => void;
  setOutlineKinds: (kinds: OutlineKinds) => void;
}

interface Resolved {
  shown: boolean;
  mode: PanelModeId;
  mentionsExpanded: boolean;
  outlineKinds: OutlineKinds;
}

function resolved(state: NotePanelState | undefined): Resolved {
  return {
    shown: state?.shown ?? false,
    mode: state?.mode ?? DEFAULT_MODE,
    mentionsExpanded: state?.mentions_expanded ?? false,
    outlineKinds: state?.outline_kinds ?? NO_KINDS,
  };
}

/**
 * Колонка заметки: показана ли, какой режим, раскрыты ли упоминания.
 *
 * Окно читает снимок устройства один раз при монтировании и дальше пишет своё;
 * чужие записи в текущее окно не приезжают. Без `device` состояние живёт в
 * памяти окна. Первый кадр — умолчания (колонка скрыта): сохранённое приезжает
 * эффектом, как у `usePaneLayout`.
 */
export function useNotePanelState(
  device: DeviceSettingsClient | undefined,
): NotePanelStateHandle {
  const [state, setState] = useState<Resolved>(() => resolved(undefined));
  // Пользователь успел тронуть состояние раньше ответа хранилища — тогда
  // сохранённое не применяется (то же правило, что у `usePaneLayout`).
  const touched = useRef(false);

  useEffect(() => {
    if (device === undefined) return undefined;
    let cancelled = false;
    void (async () => {
      try {
        const snapshot = await device.snapshot();
        if (cancelled) return;
        if (!touched.current) setState(resolved(snapshot.settings.note_panel));
      } catch {
        // Хранилище не ответило: остаются умолчания, окно работает в памяти.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [device]);

  useEffect(() => {
    if (device === undefined || !touched.current) return undefined;
    const timer = window.setTimeout(() => {
      const next: NotePanelState = {
        shown: state.shown,
        mode: state.mode,
        mentions_expanded: state.mentionsExpanded,
        outline_kinds: state.outlineKinds,
      };
      // Отказ записи не ломает окно: состояние уже применено, потеряется
      // только его память.
      void device.apply(changeOf({ note_panel: next })).catch(() => undefined);
    }, NOTE_PANEL_PERSIST_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [device, state]);

  const update = useCallback((patch: Partial<Resolved>) => {
    touched.current = true;
    setState((current) => ({ ...current, ...patch }));
  }, []);

  const toggle = useCallback(() => {
    touched.current = true;
    setState((current) => ({ ...current, shown: !current.shown }));
  }, []);

  return {
    ...state,
    toggle,
    setShown: useCallback((shown: boolean) => update({ shown }), [update]),
    setMode: useCallback((mode: PanelModeId) => update({ mode }), [update]),
    setMentionsExpanded: useCallback(
      (mentionsExpanded: boolean) => update({ mentionsExpanded }),
      [update],
    ),
    setOutlineKinds: useCallback(
      (outlineKinds: OutlineKinds) => update({ outlineKinds }),
      [update],
    ),
  };
}
