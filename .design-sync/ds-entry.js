// Точка входа дизайн-системы: только компоненты, живущие на пропсах, без vault и wasm.
// Копируется в ui/dist при сборке (см. buildCmd в config.json).
export { Icon, IconSprite } from "./icons/Icons.js";
export { I18nProvider } from "./i18n/index.js";
export { ContextMenuProvider } from "./menu/ContextMenu.js";
export { ViewSwitch } from "./workspace/ViewSwitch.js";
export { PaneDivider } from "./workspace/PaneDivider.js";
export { CommandPalette } from "./workspace/CommandPalette.js";
export { WelcomeScreen } from "./workspace/WelcomeScreen.js";
export { VaultSwitcher } from "./workspace/VaultSwitcher.js";
export { TabStrip } from "./workspace/tabs/TabStrip.js";
export { CloseRefusedNotice } from "./workspace/tabs/CloseRefusedNotice.js";
export { SyncIndicator } from "./sync/SyncIndicator.js";
export { MoveNotice } from "./vault-tree/MoveNotice.js";
export { ErrorBanner } from "./vault-tree/ErrorBanner.js";
export { AccessStateBanner } from "./vault-tree/AccessStateBanner.js";
export { BulkChangeIndicator } from "./vault-tree/BulkChangeIndicator.js";
export { BacklinksConfirmDialog } from "./vault-tree/BacklinksConfirmDialog.js";
export { LargeImageDialog } from "./attachments/LargeImageDialog.js";
export { InsertBoardDialog } from "./boards/InsertBoardDialog.js";
export { CompletionPopup } from "./completion/CompletionPopup.js";
