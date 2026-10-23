/**
 * Эталонный каталог (фича 010, research.md §2): английский.
 *
 * Тип [`Catalog`] выводится отсюда, и каждый поставляемый язык объявлен этим
 * типом — забытый ключ или параметр другой формы роняют `tsc` (FR-028).
 * Значение — строка или функция от типизированных параметров; имена
 * пользователя (файлы, пути, шаблоны) приходят параметром и не меняются (FR-005).
 *
 * Ключи группируются по области интерфейса (`common.*`, `tree.*`, `editor.*`…),
 * и каждая область лежит в своём файле `en/<область>.ts`.
 */

import type { Translation } from "./define.js";
import { common } from "./en/common.js";
import { app } from "./en/app.js";
import { preview } from "./en/preview.js";
import { settings } from "./en/settings.js";
import { insert } from "./en/insert.js";
import { elements } from "./en/elements.js";
import { editor } from "./en/editor.js";
import { boards } from "./en/boards.js";
import { workspace } from "./en/workspace.js";
import { palette } from "./en/palette.js";
import { errors } from "./en/errors.js";
import { tree } from "./en/tree.js";
import { exporting } from "./en/export.js";
import { templates } from "./en/templates.js";
import { search } from "./en/search.js";
import { graph } from "./en/graph.js";
import { shell } from "./en/shell.js";
import { sync } from "./en/sync.js";
import { links } from "./en/links.js";
import { slides } from "./en/slides.js";
import { snapshots } from "./en/snapshots.js";
import { spellcheck } from "./en/spellcheck.js";
import { properties } from "./en/properties.js";
import { importing } from "./en/import.js";
import { capture } from "./en/capture.js";
import { attachments } from "./en/attachments.js";
import { sources } from "./en/sources.js";
import { completion } from "./en/completion.js";
import { outline } from "./en/outline.js";

export const en = {
  ...common,
  ...app,
  ...preview,
  ...settings,
  ...insert,
  ...elements,
  ...editor,
  ...boards,
  ...workspace,
  ...palette,
  ...errors,
  ...tree,
  ...exporting,
  ...templates,
  ...search,
  ...graph,
  ...shell,
  ...sync,
  ...links,
  ...slides,
  ...snapshots,
  ...spellcheck,
  ...properties,
  ...importing,
  ...capture,
  ...attachments,
  ...sources,
  ...completion,
  ...outline,
} as const;

/** Набор переводов одного языка: те же ключи и те же параметры, что у эталона. */
export type Catalog = Translation<typeof en>;

export type MessageKey = keyof Catalog;

/** Параметры ключа: кортеж из одного объекта или пустой. */
export type MessageParams<K extends MessageKey> = Catalog[K] extends (params: infer P) => string
  ? [params: P]
  : [];
