"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import type {
  CommandId,
  ResolvedBinding,
  VaultOverrides,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";

/**
 * Действующие сочетания команд приложения.
 *
 * До этого хука сочетания были зашиты в трёх разных местах — `Workspace`,
 * `CreateFileMenu` и `NoteEditor`, — и каждое сверяло клавиши по-своему. Здесь
 * они сходятся в одно, но **правила остаются в ядре**: что такое допустимое
 * сочетание, какое у команды умолчание и как оно подписывается, решает
 * `settings::keybindings`. Отсюда уходит один вызов, обратно приезжает готовый
 * разбор.
 *
 * Сверка нажатия идёт здесь, а не в ядре, по единственной причине: на desktop и
 * Android ядро живёт за границей процесса, и спрашивать его на каждое нажатие
 * клавиши нельзя. Поэтому она и сведена к сравнению полей на равенство —
 * это сверка, а не правило.
 */
export interface ShortcutsState {
  /** Разбор всех команд в порядке, заданном ядром. */
  bindings: ResolvedBinding[];
  /** Нажали ли сочетание этой команды. */
  matches: (command: CommandId, event: KeyboardEvent) => boolean;
  /** Подпись сочетания для меню и палитры; пустая строка — сочетание снято. */
  displayOf: (command: CommandId) => string;
}

/**
 * Перекрытия, которых ещё нет: пока настройки vault читаются, команды обязаны
 * работать на умолчаниях, а не молчать. Пустой набор — это ровно «ничего не
 * переназначено», и ядро отвечает на него умолчаниями.
 */
export const NOTHING_OVERRIDDEN: VaultOverrides = {
  assets_placement: undefined,
  sections: [],
  theme: undefined,
  editor_font_size: undefined,
  editor_mode_preference: undefined,
  editor_config_path: undefined,
  auto_download_templates: undefined,
  template_sources: [],
  keybindings: [],
  export_folder: undefined,
};

export function useShortcuts(
  client: VaultClient | null,
  overrides: VaultOverrides | null,
): ShortcutsState {
  const [bindings, setBindings] = useState<ResolvedBinding[]>([]);

  useEffect(() => {
    if (client === null) {
      setBindings([]);
      return;
    }
    // Гонка та же, что у предпросмотра и поиска: ответ на устаревшие настройки
    // не должен вытеснить более новый.
    let alive = true;
    void (async () => {
      try {
        const resolved = await client.keybindings(
          overrides ?? NOTHING_OVERRIDDEN,
          isMac(),
        );
        if (alive) setBindings(resolved);
      } catch {
        // Разбор сочетаний не удался — оставляем те, что уже есть. Обнулять их
        // значило бы отнять у пользователя клавиатуру из-за одного сбоя
        // чтения настроек.
      }
    })();
    return () => {
      alive = false;
    };
  }, [client, overrides]);

  const byCommand = useMemo(
    () => new Map(bindings.map((binding) => [binding.command, binding])),
    [bindings],
  );

  const matches = useCallback(
    (command: CommandId, event: KeyboardEvent): boolean => {
      const binding = byCommand.get(command);
      const shortcut = binding?.shortcut;
      if (binding === undefined || shortcut === undefined) return false;

      // `Mod` — ⌘ на macOS и Ctrl на остальных; какая именно клавиша нажата,
      // здесь не различается намеренно: одно и то же сочетание обязано
      // работать на обеих платформах.
      if (shortcut.mod_key !== (event.metaKey || event.ctrlKey)) return false;
      if (shortcut.shift !== event.shiftKey) return false;
      if (shortcut.alt !== event.altKey) return false;

      // Физическая клавиша важнее названной: при кириллической раскладке
      // `event.key` для той же клавиши даёт «л», а не «k». Имя кода посчитало
      // ядро — здесь только сравнение.
      if (binding.code !== "" && event.code === binding.code) return true;
      return shortcut.key === event.key.toLowerCase();
    },
    [byCommand],
  );

  const displayOf = useCallback(
    (command: CommandId): string => byCommand.get(command)?.display ?? "",
    [byCommand],
  );

  // Один объект, пока сочетания те же: его получают `memo`-компоненты (полоса
  // вкладок, T070), и новый объект на каждую отрисовку их бы перерисовывал.
  return useMemo(() => ({ bindings, matches, displayOf }), [bindings, matches, displayOf]);
}

/**
 * Подсказка кнопки с сочетанием, если оно есть.
 *
 * Снятое сочетание не должно оставлять после себя висящий разделитель — а он
 * и оставался бы, будь подсказка склеена шаблонной строкой на месте.
 */
export function hintWith(action: string, shortcut: string): string {
  return shortcut === "" ? action : `${action} · ${shortcut}`;
}

/**
 * macOS ли это. Влияет только на подпись сочетания (⌘ против Ctrl) — само
 * сочетание от платформы не зависит.
 *
 * `navigator.platform` объявлен устаревшим, поэтому смотрим на `userAgent`. Вне
 * браузера (серверный рендер Next.js) ответ «нет»: подпись всё равно не видна.
 */
function isMac(): boolean {
  if (typeof navigator === "undefined") return false;
  return /Mac|iPhone|iPad|iPod/.test(navigator.userAgent);
}
