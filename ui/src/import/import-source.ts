import type { ImportSourceTarget } from "../vault-client.js";
import { basename } from "../paths.js";

// Выбор исходной папки импорта (фича 021, FR-001): единственное, что в импорте
// различается между оболочками. Web спрашивает `showDirectoryPicker` в режиме
// чтения, рабочий стол и Android — диалог или обзор папок и получают путь.
// `Workspace` знает только этот тип и не ветвится по платформе.

/** Выбранная исходная папка: как её показать и чем открыть. */
export interface ImportSourceChoice {
  /** Имя папки для заголовков и отчёта. */
  name: string;
  /** Что показать в поле «Откуда»: путь либо имя папки браузера. */
  label: string;
  target: ImportSourceTarget;
}

/**
 * Жест выбора исходной папки, который оболочка передаёт рабочей области.
 *
 * `available: false` — платформа не умеет (браузер без File System Access API):
 * команда остаётся в палитре, а диалог называет причину (C5), не прячет её.
 */
export interface ImportSourcePicker {
  available: boolean;
  /** `null` — диалог закрыт без выбора. */
  pick: () => Promise<ImportSourceChoice | null>;
}

/** Выбор по пути: рабочий стол и Android. */
export function pathImportSource(path: string): ImportSourceChoice {
  return { name: basename(path), label: path, target: { kind: "path", path } };
}

/** Выбор по хендлу папки: web. */
export function handleImportSource(handle: FileSystemDirectoryHandle): ImportSourceChoice {
  return { name: handle.name, label: handle.name, target: { kind: "handle", handle } };
}

/**
 * Собирает [`ImportSourcePicker`] из функции «дай путь папки»: рабочий стол
 * (диалог ОС) и Android (обзор папок) различаются только ею.
 */
export function pathImportPicker(pickPath: () => Promise<string | null>): ImportSourcePicker {
  return {
    available: true,
    pick: async () => {
      const path = await pickPath();
      return path === null ? null : pathImportSource(path);
    },
  };
}
