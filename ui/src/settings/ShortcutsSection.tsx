"use client";

import { useState } from "react";
import type {
  CommandId,
  ResolvedBinding,
  Shortcut,
} from "../wasm-core/typsidian_core.js";

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
    <section aria-label="Команды Typsidian">
      {standalone && <h2>Кейбиндинги</h2>}
      <span className="ts-settings__label">Команды Typsidian</span>

      <p>
        Нажмите «Изменить» и затем само сочетание. Нужен модификатор: ⌘ или Ctrl,
        либо Alt. Escape, Enter, Tab и стрелки заняты интерфейсом.
      </p>

      {error !== null && <p role="alert">{error}</p>}

      {bindings.length === 0 ? (
        <p role="status">Читаем настройки папки заметок…</p>
      ) : (
        <ul className="ts-keys">
          {bindings.map((binding) => (
            <li key={binding.command} className="ts-keys__row">
              <span className="ts-keys__title">{binding.title}</span>

              {/* Столбец фиксированной ширины: иначе подпись сочетания
                  прыгала бы по горизонтали от длины названия команды. */}
              <span className="ts-keys__combo">
                {recording === binding.command ? (
                  <span className="ts-keys__recording" role="status">
                    нажмите сочетание…
                  </span>
                ) : binding.display === "" ? (
                  <span className="ts-keys__empty">не назначено</span>
                ) : (
                  <kbd className="ts-keys__kbd">{binding.display}</kbd>
                )}

                {/* Конфликт не запрещён, а показан: запрет заставил бы
                    расставлять сочетания в определённом порядке, и поменять
                    два местами стало бы нельзя. Строкой под сочетанием, а не
                    рядом, — иначе появление пометки сдвигало бы кнопки. */}
                {binding.conflicts_with.length > 0 && (
                  <span className="ts-keys__clash" role="note">
                    занято ещё одной командой
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
                  {recording === binding.command ? "Отмена" : "Изменить"}
                </button>

                <button
                  type="button"
                  className="ts-btn ts-btn--quiet"
                  disabled={binding.shortcut === undefined}
                  onClick={() => onClear(binding.command)}
                >
                  Снять
                </button>

                {/* Возврат к умолчанию гаснет там, где возвращать нечего:
                    живая кнопка, ничего не меняющая, хуже погашенной. */}
                <button
                  type="button"
                  className="ts-btn ts-btn--quiet"
                  disabled={binding.is_default}
                  onClick={() => onReset(binding.command)}
                >
                  Умолчание
                </button>
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
