"use client";

import { useCallback, useState } from "react";
import type { DeviceSettingsClient } from "../settings/device-settings-client.js";
import { useI18n } from "../i18n/index.js";
import { Icon } from "../icons/Icons.js";
import { useContextMenu, type ContextMenuItem } from "../menu/ContextMenu.js";
import { vaultDisplayName } from "./knownVaults.js";

/**
 * Имя открытого vault в шапке и меню переключения (фича 043, FR-003; экран 1
 * Typsidian.pdf).
 *
 * Меню — общее контекстное меню приложения, открытое под кнопкой: свой вид
 * выпадающего списка был бы третьим видом меню в одном окне. Список берётся из
 * хранилища устройства в момент открытия — его меняет и панель настроек, и
 * вести здесь вторую копию значило бы показывать устаревшее.
 */
export function VaultSwitcher({
  rootName,
  device,
  onOpenKnown,
  onOpenFolder,
  onManage,
}: {
  rootName: string;
  device: DeviceSettingsClient | undefined;
  /** Открыть известный vault; не передан — записи показываются без перехода. */
  onOpenKnown: ((id: string) => void) | undefined;
  onOpenFolder: () => void;
  /** Управление списком в настройках; не передан — пункта нет. */
  onManage: (() => void) | undefined;
}): React.JSX.Element {
  const { t } = useI18n();
  const contextMenu = useContextMenu();
  const [busy, setBusy] = useState(false);

  const open = useCallback(
    async (button: HTMLButtonElement) => {
      setBusy(true);
      let items: ContextMenuItem[] = [];
      try {
        const snapshot = device === undefined ? null : await device.snapshot();
        const current = snapshot?.settings.current_vault;
        items = (snapshot?.knownVaults ?? []).map((record) => {
          const unavailable = record.availability === "unavailable";
          const isCurrent = record.id === current;
          return {
            label: vaultDisplayName(record),
            shortcut: unavailable ? t("workspace.vault.unavailable") : record.location,
            checked: isCurrent,
            disabled: unavailable || isCurrent || onOpenKnown === undefined,
            run: () => onOpenKnown?.(record.id),
          };
        });
      } catch {
        // Хранилище устройства недоступно — список не показать, но открыть
        // папку можно по-прежнему.
        items = [];
      } finally {
        setBusy(false);
      }
      const rect = button.getBoundingClientRect();
      contextMenu.open(new MouseEvent("click", { clientX: rect.left, clientY: rect.bottom + 4 }), [
        ...items,
        { kind: "separator" },
        { label: t("workspace.vault.openFolder"), run: onOpenFolder },
        ...(onManage === undefined ? [] : [{ label: t("workspace.vault.manage"), run: onManage }]),
      ]);
    },
    [contextMenu, device, onManage, onOpenFolder, onOpenKnown, t],
  );

  return (
    <button
      type="button"
      className="ts-vault-switch"
      title={rootName}
      aria-haspopup="menu"
      aria-label={t("workspace.vault.switch", { name: rootName })}
      disabled={busy}
      onClick={(event) => void open(event.currentTarget)}
    >
      <Icon name="folder" size={13} />
      <span className="ts-vault-switch__name">{rootName}</span>
      <Icon name="down" size={11} />
    </button>
  );
}
