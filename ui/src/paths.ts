// Работа с путями на стороне UI.
//
// Отдельный модуль, а не функция внутри `vault-client-web.ts`: тот импортирует
// `WebBackend` **значением** и потому тянет за собой WASM. Оболочка рабочего
// стола не имеет права его импортировать (см. шапку `vault-client-web.ts`), а
// правило «последний сегмент пути» нужно обоим клиентам.

/**
 * Последний сегмент пути — имя папки vault.
 *
 * В вебе `rootPath` и так возвращает одно имя (File System Access API полного
 * пути не даёт), но на рабочем столе это абсолютный путь, и хлебным крошкам
 * нужна его хвостовая часть. Разделители оба: путь приходит от ОС, а не от нас.
 */
export function basename(path: string): string {
  const trimmed = path.replace(/[/\\]+$/, "");
  const cut = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
  return cut === -1 ? trimmed : trimmed.slice(cut + 1);
}
