"use client";

import { useState } from "react";
import type {
  CommandId,
  ResolvedBinding,
  Shortcut,
} from "../wasm-core/typsidian_core.js";
import { useI18n, type MessageKey } from "../i18n/index.js";

/**
 * Название команды на языке интерфейса. Карта исчерпывающая: команда, которую
 * ядро добавит без подписи, не соберётся (фича 010, принцип IV).
 */
const COMMAND_TITLE: Record<CommandId, MessageKey> = {
  "command-palette": "palette.label",
  search: "workspace.search",
  templates: "workspace.templates",
  "new-file": "tree.menu.createNote",
  "new-board": "tree.menu.createBoard",
  "insert-menu": "editor.insertElementHint",
  "element-params": "workspace.elementParams",
  "toggle-graph": "tree.graph.title",
  "rebuild-preview": "workspace.rebuildPreview",
  "export-pdf": "workspace.exportPdf",
  "export-center": "workspace.exportCenter",
  "sync-open-panel": "sync.command.openPanel",
  "sync-snapshot": "sync.command.snapshot",
  "sync-file-history": "sync.command.fileHistory",
  "sync-push": "sync.command.push",
  "sync-pull": "sync.command.pull",
  "sync-resolve-conflicts": "sync.command.resolveConflicts",
  "toggle-note-panel": "notePanel.command",
  "tab-close": "workspace.tabs.close",
  "tab-next": "workspace.tabs.next",
  "tab-previous": "workspace.tabs.previous",
  "split-editor": "workspace.tabs.split",
  "tab-move-to-other-group": "workspace.tabs.moveToOther",
  "navigate-back": "workspace.tabs.back",
  "navigate-forward": "workspace.tabs.forward",
  // Команда фичи 015 уже объявлена в ядре; её поведение в окне — 015 T020.
  "toggle-outline": "notePanel.outlineCommand",
  // Фича 019: «с текущего слайда»; другие варианты запуска — пункты меню «Показ».
  "start-show": "slides.menu.fromCurrent",
  // Фича 022: открыть ежедневную заметку на сегодня.
  today: "capture.command.today",
  // Фича 016: изображение из файла к каретке.
  "insert-image": "attachments.command.insertImage",
  // Фича 017: цитата поиском и колонка «Источники».
  "insert-citation": "sources.command.insertCitation",
  "toggle-sources": "sources.command.toggle",
  // Фича 018: переход к определению и ручной вызов дополнения.
  "goto-definition": "completion.command.goto",
  "trigger-completion": "completion.command.trigger",
};

/**
 * Сочетания клавиш команд приложения.
 *
 * Ни одного решения о том, что такое допустимое сочетание, здесь нет: список
 * команд, их подписи, умолчания, проверка годности и конфликты приезжают
 * разобранными из ядра (`settings::keybindings`). Этот файл записывает нажатие
 * и показывает ответ.
 *
 * Хранится назначенное в настройках **vault**, а не устройства: панель
 * настроек устройства существует только в web-сборке, а до файла внутри папки
 * заметок добираются все три оболочки.
 */
export interface ShortcutsSectionProps {
  /**
   * Раздел стоит сам по себе (панель папки desktop/Android) — тогда у него
   * свой заголовок «Кейбиндинги». В панели web он идёт внутри раздела
   * «Кейбиндинги» группой «Команды Typsidian», как в макете «Утилиты», и
   * второго заголовка страницы там быть не должно.
   */
  standalone?: boolean;
  /** Разбор всех команд; пустой список — настройки ещё читаются. */
  bindings: ResolvedBinding[];
  /** Причина отказа последней операции, названная ядром. */
  error: string | null;
  onAssign: (command: CommandId, shortcut: Shortcut) => void;
  /** Снять сочетание: команда останется в палитре, но не на клавишах. */
  onClear: (command: CommandId) => void;
  /** Вернуть умолчание ядра. */
  onReset: (command: CommandId) => void;
}

export function ShortcutsSection({
  bindings,
  standalone = false,
  error,
  onAssign,
  onClear,
  onReset,
}: ShortcutsSectionProps): React.JSX.Element {
  const { t } = useI18n();
  // Команда, которая сейчас ждёт нажатия. `null` — никто не ждёт.
  const [recording, setRecording] = useState<CommandId | null>(null);

  const capture = (command: CommandId, event: React.KeyboardEvent): void => {
    // Пока зажаты только модификаторы, сочетания ещё нет. Спрашиваем об этом
    // саму платформу, а не сверяем со списком имён: список модификаторов —
    // знание ядра, и его копия здесь разошлась бы с ним.
    //
    // Приведение — про узость типа, а не про подмену: `getModifierState`
    // объявлен списком известных модификаторов, но по спецификации принимает
    // любую строку и на незнакомую отвечает `false`, что нам и нужно.
    type ModifierName = Parameters<typeof event.getModifierState>[0];
    if (event.getModifierState(event.key as ModifierName)) return;

    event.preventDefault();
    // Нажатие не должно доехать до глобального слушателя: иначе запись
    // сочетания заодно открывала бы команду, которую записывают.
    event.stopPropagation();

    if (event.key === "Escape") {
      // Escape назначить нельзя (ядро откажет), поэтому он означает отмену
      // записи — самое ожидаемое его поведение здесь.
      setRecording(null);
      return;
    }

    setRecording(null);
    onAssign(command, {
      // `Mod` — один модификатор на обе платформы; какая именно клавиша нажата,
      // не различается намеренно.
      mod_key: event.metaKey || event.ctrlKey,
      shift: event.shiftKey,
      alt: event.altKey,
      // Приведением к канону занимается ядро: `Shortcut::normalize`.
      key: event.key,
    });
  };

  return (
    <section aria-label={t("settings.shortcuts.label")}>
      {standalone && <h2>{t("settings.section.keybindings")}</h2>}
      <span className="ts-settings__label">{t("settings.shortcuts.label")}</span>

      <p>{t("settings.shortcuts.hint")}</p>

      {error !== null && <p role="alert">{error}</p>}

      {bindings.length === 0 ? (
        <p role="status">{t("settings.shortcuts.loading")}</p>
      ) : (
        <ul className="ts-keys">
          {bindings.map((binding) => (
            <li key={binding.command} className="ts-keys__row">
              <span className="ts-keys__title">{t(COMMAND_TITLE[binding.command])}</span>

              {/* Столбец фиксированной ширины: иначе подпись сочетания
                  прыгала бы по горизонтали от длины названия команды. */}
              <span className="ts-keys__combo">
                {recording === binding.command ? (
                  <span className="ts-keys__recording" role="status">
                    {t("settings.shortcuts.recording")}
                  </span>
                ) : binding.display === "" ? (
                  <span className="ts-keys__empty">{t("settings.shortcuts.unassigned")}</span>
                ) : (
                  <kbd className="ts-keys__kbd">{binding.display}</kbd>
                )}

                {/* Конфликт не запрещён, а показан: запрет заставил бы
                    расставлять сочетания в определённом порядке, и поменять
                    два местами стало бы нельзя. Строкой под сочетанием, а не
                    рядом, — иначе появление пометки сдвигало бы кнопки. */}
                {binding.conflicts_with.length > 0 && (
                  <span className="ts-keys__clash" role="note">
                    {t("settings.shortcuts.conflict")}
                  </span>
                )}
              </span>

              <span className="ts-keys__actions">
                <button
                  type="button"
                  className="ts-btn"
                  aria-pressed={recording === binding.command}
                  onClick={() =>
                    setRecording((current) =>
                      current === binding.command ? null : binding.command,
                    )
                  }
                  onKeyDown={(event) => {
                    if (recording === binding.command) capture(binding.command, event);
                  }}
                >
                  {recording === binding.command ? t("common.cancel") : t("settings.shortcuts.change")}
                </button>

                <button
                  type="button"
                  className="ts-btn ts-btn--quiet"
                  disabled={binding.shortcut === undefined}
                  onClick={() => onClear(binding.command)}
                >
                  {t("settings.shortcuts.clear")}
                </button>

                {/* Возврат к умолчанию гаснет там, где возвращать нечего:
                    живая кнопка, ничего не меняющая, хуже погашенной. */}
                <button
                  type="button"
                  className="ts-btn ts-btn--quiet"
                  disabled={binding.is_default}
                  onClick={() => onReset(binding.command)}
                >
                  {t("settings.shortcuts.reset")}
                </button>
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
