"use client";

import { useState } from "react";
import type { VaultClient } from "../vault-client.js";
import { useShortcuts } from "../workspace/useShortcuts.js";
import { EditorFontSizeField } from "./EditorFontSizeField.js";
import { ShortcutsSection } from "./ShortcutsSection.js";
import { useVaultSettings } from "./useVaultSettings.js";
import { TemplatesSection } from "./TemplatesSection.js";
import type { PreviewClient } from "../preview-client.js";

/**
 * Настройки, доступные без хранилища устройства.
 *
 * Зачем отдельная панель, если есть [`SettingsPanel`]. Та работает через
 * `WebStore` — OPFS плюс IndexedDB, — и импортирует его прямо в модуле: на
 * desktop и Android она не просто показала бы не те значения, а потянула бы
 * wasm, которого в этих сборках не загружают вовсе. Поэтому кнопки настроек
 * там до сих пор не было.
 *
 * Здесь только то, что живёт **внутри папки заметок** и потому доступно всем
 * трём оболочкам через один и тот же `VaultBackend`: сочетания клавиш, размер
 * шрифта редактора и внешние шаблоны Typst (фича 007). Тема и список известных папок остаются за хранилищем
 * устройства и сюда не попадают — показать их, не имея возможности записать,
 * значило бы соврать.
 *
 * Размер шрифта в этом списке оказался не потому, что он «про папку», а
 * потому, что перекрытие уровня vault — единственное его хранилище, до
 * которого дотягиваются все три оболочки (ядро, `VaultOverrides`). Цена
 * честная и видна пользователю: размер переезжает вместе с папкой заметок.
 */
export interface VaultSettingsPanelProps {
  /** Клиент открытого vault. `null` — папка не открыта, настраивать нечего. */
  client: VaultClient | null;
  /**
   * Клиент предпросмотра (фича 007): перечень установленных внешних шаблонов.
   * `null` — сборка ещё не готова, раздел показывает только настройки.
   */
  preview?: PreviewClient | null;
}

/** Какой раздел показан. */
type VaultSettingsSection = "appearance" | "shortcuts" | "templates";

const SECTIONS: { id: VaultSettingsSection; title: string }[] = [
  { id: "appearance", title: "Внешний вид" },
  { id: "shortcuts", title: "Кейбиндинги" },
  { id: "templates", title: "Шаблоны" },
];

/**
 * Своей кнопки «Закрыть» здесь нет намеренно: панель показывается внутри
 * `.ts-sheet`, у которого она уже есть, и вторая была бы мёртвым органом.
 */
export function VaultSettingsPanel({
  client,
  preview = null,
}: VaultSettingsPanelProps): React.JSX.Element {
  const vaultSettings = useVaultSettings(client);
  const shortcuts = useShortcuts(client, vaultSettings.overrides);
  const [section, setSection] = useState<VaultSettingsSection>("appearance");

  return (
    <div className="ts-settings" aria-label="Настройки">
      <nav className="ts-settings__nav" aria-label="Разделы настроек">
        <span className="ts-eyebrow">Настройки</span>
        {SECTIONS.map((item) => (
          <button
            key={item.id}
            type="button"
            className={
              item.id === section
                ? "ts-settings__navitem ts-settings__navitem--active"
                : "ts-settings__navitem"
            }
            aria-current={item.id === section ? "page" : undefined}
            onClick={() => setSection(item.id)}
          >
            {item.title}
          </button>
        ))}
        <span className="ts-spacer" />
        <div className="ts-settings__foot">
          {client === null ? "vault не открыт" : `vault: ${client.rootName}`}
          <br />
          настройки этой папки
        </div>
      </nav>

      <div className="ts-settings__body">
        {client === null ? (
          <p role="status">
            Откройте папку с заметками — эти настройки хранятся в ней.
          </p>
        ) : section === "templates" ? (
          // Фича 007, FR-016: выключатель автозагрузки, свои адреса и перечень
          // установленного — те же, что в панели web. Всё это живёт в папке
          // заметок и доступно этой оболочке через `VaultBackend`.
          <section aria-label="Шаблоны">
            <div className="ts-settings__head">
              <h2>Шаблоны</h2>
              <span className="ts-settings__meta">настройки папки · {client.rootName}</span>
            </div>
            <TemplatesSection vaultSettings={vaultSettings} preview={preview} />
          </section>
        ) : section === "appearance" ? (
          <section aria-label="Внешний вид">
            <h2>Внешний вид</h2>

            {/* Отказ записи показывается и здесь: у папки, недоступной на
                запись, размер не сохранится, и молчать об этом нельзя
                (FR-025). */}
            {vaultSettings.error !== null && (
              <p role="alert">{vaultSettings.error}</p>
            )}

            <EditorFontSizeField
              size={vaultSettings.overrides?.editor_font_size ?? null}
              onChange={(size) => void vaultSettings.setEditorFontSize(size)}
              onClear={() => void vaultSettings.setEditorFontSize(null)}
              hint="Размер принадлежит этой папке заметок и переезжает вместе с ней."
            />
          </section>
        ) : (
          <ShortcutsSection
            standalone
            bindings={shortcuts.bindings}
            error={vaultSettings.error}
            onAssign={(command, shortcut) =>
              void vaultSettings.setKeybinding(command, shortcut)
            }
            onClear={(command) => void vaultSettings.setKeybinding(command, null)}
            onReset={(command) => void vaultSettings.resetKeybinding(command)}
          />
        )}
      </div>
    </div>
  );
}
