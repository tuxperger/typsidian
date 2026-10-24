import type { I18n } from "../i18n/index.js";

// «сейчас», «5 минут назад», «вчера», «12.10.2026» — подпись времени в списках
// серверного режима (фича 037, макет S3, S4).

/** `seconds` — время Unix в секундах. */
export function agoText(i18n: I18n, seconds: number): string {
  const minutes = Math.round((Date.now() / 1000 - seconds) / 60);
  if (minutes < 2) return i18n.t("server.account.now");
  if (minutes < 60) return i18n.formatRelative(-minutes, "minute");
  if (minutes < 24 * 60) return i18n.formatRelative(-Math.round(minutes / 60), "hour");
  const days = Math.round(minutes / (24 * 60));
  if (days < 7) return i18n.formatRelative(-days, "day");
  return i18n.formatDate(new Date(seconds * 1000));
}
