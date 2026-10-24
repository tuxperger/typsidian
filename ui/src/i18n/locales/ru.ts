/**
 * Русский каталог (фича 010). Полный: объявлен типом [`Catalog`], поэтому
 * отсутствие любого ключа эталона — ошибка компиляции (FR-028).
 */

import type { Catalog } from "./en.js";
import { common } from "./ru/common.js";
import { app } from "./ru/app.js";
import { preview } from "./ru/preview.js";
import { settings } from "./ru/settings.js";
import { insert } from "./ru/insert.js";
import { elements } from "./ru/elements.js";
import { editor } from "./ru/editor.js";
import { boards } from "./ru/boards.js";
import { workspace } from "./ru/workspace.js";
import { palette } from "./ru/palette.js";
import { errors } from "./ru/errors.js";
import { tree } from "./ru/tree.js";
import { exporting } from "./ru/export.js";
import { templates } from "./ru/templates.js";
import { search } from "./ru/search.js";
import { graph } from "./ru/graph.js";
import { shell } from "./ru/shell.js";
import { sync } from "./ru/sync.js";
import { links } from "./ru/links.js";
import { slides } from "./ru/slides.js";
import { snapshots } from "./ru/snapshots.js";
import { spellcheck } from "./ru/spellcheck.js";
import { embeds } from "./ru/embeds.js";
import { bookmarks } from "./ru/bookmarks.js";
import { tasks } from "./ru/tasks.js";
import { properties } from "./ru/properties.js";
import { importing } from "./ru/import.js";
import { capture } from "./ru/capture.js";
import { attachments } from "./ru/attachments.js";
import { sources } from "./ru/sources.js";
import { completion } from "./ru/completion.js";
import { outline } from "./ru/outline.js";
import { server } from "./ru/server.js";

export const ru: Catalog = {
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
  ...embeds,
  ...bookmarks,
  ...tasks,
  ...properties,
  ...importing,
  ...capture,
  ...attachments,
  ...sources,
  ...completion,
  ...outline,
  ...server,
};
