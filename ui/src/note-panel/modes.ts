import type { ReactNode } from "react";
import type { NotePanelMode } from "../wasm-core/typsidian_core.js";
import type { MessageKey } from "../i18n/index.js";

// Реестр режимов колонки заметки (фича 013, contracts/note-panel.md).
//
// Оболочка `NotePanel` режимов не знает: ей отдают список описаний. Фича 015
// добавляет сюда «Структуру», 017 — «Источники», не меняя оболочку: достаточно
// варианта в `NotePanelMode` ядра и ещё одного описания в списке, который
// собирает `Workspace`.

export interface NotePanelModeSpec {
  /** Идентификатор режима — тип ядра, он же значение `Settings.note_panel.mode`. */
  id: NotePanelMode;
  /** Название в шапке колонки. */
  title: MessageKey;
  /** Счётчик активного режима в шапке («5 · 2»); `null` — счётчика нет. */
  badge: string | null;
  /** Тело режима. Вызывается, только пока режим активен. */
  render: () => ReactNode;
}
