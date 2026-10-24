import { webDeviceSettings } from "@typsidian/ui/settings/device-settings-web.js";
import { webTabsStore } from "@typsidian/ui/workspace/tabs/tabs-store-web.js";
import { createWebShowHost } from "@typsidian/ui/slides/show-channel-web.js";

/**
 * Хранилище настроек устройства — `WebStore` поверх OPFS. Один объект на
 * страницу: панель подписывается на него эффектами. Общий для обоих режимов web
 * (фича 037): язык и вид — свойства браузера, а не места, где лежат заметки.
 */
export const device = webDeviceSettings();

/** Набор вкладок vault на устройстве (фича 014): тот же `WebStore`, что и у настроек. */
export const tabsStore = webTabsStore();

/** Хозяин окон показа слайдов (фича 019): `window.open` + `BroadcastChannel`. */
export const showHost = createWebShowHost();
