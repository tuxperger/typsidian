#!/usr/bin/env node
// Одноразовый перенос команд ядра из `packages/shell/src/lib.rs` в крейт
// `packages/host` (фича 037, research.md §1, T001).
//
// Команда переезжает, если ей не нужны ни `AppHandle`, ни устройство: её
// параметры — аргументы клиента и состояния открытого vault. В `host` она
// становится `pub fn` с теми же аргументами, где `State<'_, X>` заменено на
// `&X`; обёртки Tauri с прежними именами строит `gen-host-dispatch.mjs`.
// Всё, что знает про окно, устройство и обзор папок, остаётся в оболочке.
//
// Скрипт запускается один раз, на исходном `lib.rs`; повторный запуск после
// переноса ничего не найдёт и откажется.

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";

import { body, head, scanItems } from "./rust-items.mjs";
const SHELL = "packages/shell/src/lib.rs";
const src = readFileSync(SHELL, "utf8");


if (!src.includes("#[tauri::command]\nasync fn list(")) {
  console.error("lib.rs уже перенесён: команд ядра в нём нет");
  process.exit(1);
}

const headerEnd = src.search(/\n(?!\/\/!)/);
const header = src.slice(0, headerEnd + 1);
const items = scanItems(src.slice(headerEnd + 1));

// Остаются в оболочке: окно, устройство, обзор папок, импорт из папки
// (путь файловой системы приходит от клиента) и точка входа.
const SHELL_ONLY = new Set([
  "sync",
  "VAULT_EVENTS",
  "PROPERTIES_REINDEX_PROGRESS",
  "open_vault",
  "spawn_event_forwarding",
  "DirEntryView",
  "DirListing",
  "BrowseError",
  "browse_root",
  "browse_dir",
  "browse_create_dir",
  "Device",
  "LastVaultView",
  "remember_opened_vault",
  "last_opened_vault",
  "open_device_store",
  "device_store",
  "RememberedVault",
  "remember_vault",
  "last_vault",
  "DeviceSnapshot",
  "snapshot_of",
  "native_diagnostics",
  "device_settings",
  "device_journal",
  "device_settings_events",
  "AppliedSettings",
  "apply_device_settings",
  "settings_reset_change",
  "settings_change_target",
  "settings_route_change",
  "resolve_device_settings",
  "device_defaults_change",
  "reveal_device_settings",
  "add_known_vault",
  "set_known_vault_label",
  "forget_known_vault",
  "resolve_known_vault",
  "vault_reindex_properties",
  "import_open_source",
  "attachments_batch_store",
  "disable_dmabuf_renderer_on_nvidia",
  "builder",
]);

const uses = [];
const hostItems = [];
const commands = [];
const shellItems = [];
const moved = [];

for (const item of items) {
  const { kind, name } = head(item);
  if (kind === "use") {
    uses.push(item);
    continue;
  }
  let target = SHELL_ONLY.has(name) ? "shell" : "host";
  // `impl From<std::io::Error> for BrowseError` и подобные — за своим типом.
  if (kind === "impl" && /\bfor\s+(BrowseError|Device)\b/.test(body(item))) target = "shell";
  if (target === "host" && /AppHandle|tauri::(?!command)/.test(item)) {
    console.error(`элемент ${kind} ${name} знает о Tauri, а в списке оболочки его нет`);
    process.exit(1);
  }
  if (target === "shell") {
    shellItems.push(item);
    continue;
  }
  if (item.includes("#[tauri::command]")) {
    let text = item.replace(/#\[tauri::command\]\n/, "");
    text = text.replace(/^(async fn|fn) /m, "pub $1 ");
    text = text.replace(/State<'_,\s*(\w+)>/g, "&$1");
    commands.push(text);
    moved.push(name);
  } else {
    let text = item;
    if (["fn", "struct", "enum", "type", "const", "static", "trait"].includes(kind)) {
      text = text.replace(/^((?:async )?(?:fn|struct|enum|type|const|static|trait) )/m, "pub $1");
    }
    // Поля состояний читает и оболочка (открытие vault), и сервер.
    text = text.replace(/pub struct (\w+)\((?!pub )/, "pub struct $1(pub ");
    text = text.replace(/^(\s+)(fonts|packages): (?!preview::\w+::)/gm, "$1pub $2: ");
    if (kind === "macro_rules!") text = text.replace(/macro_rules!/, "#[macro_export]\nmacro_rules!");
    hostItems.push(text);
  }
}

const prelude = uses
  .filter((u) => !/use tauri::/.test(u) && !/^\s*mod /.test(u))
  .map((u) => u.replace(/(^|\n)use /g, "$1    pub use ").replace(/\n(?!    |$)/g, "\n    "))
  .join("");

const states = [
  "Vault",
  "TemplatePreviews",
  "Sources",
  "Preview",
  "Speller",
  "HighlightSessions",
  "PropertiesReindex",
  "ImportSource",
];
const snake = (s) => s.replace(/[A-Z]/g, (c, k) => (k ? "_" : "") + c.toLowerCase());

const hostLib = `//! Команды ядра для открытого vault — одна реализация для оболочки Tauri и сервера
//! (фича 037, research.md §1, конституция «Архитектурные ограничения → Сервер»).
//!
//! Здесь нет Tauri: команда — обычная функция, которой передают аргументы клиента и
//! состояния открытого vault (\`&Vault\`, \`&Preview\`, …). Оболочка (\`packages/shell\`)
//! оборачивает каждую в \`#[tauri::command]\` с прежним именем, сервер (\`apps/server\`)
//! вызывает их через [\`dispatch::dispatch\`] по имени. Обе обёртки строит
//! \`scripts/dev/gen-host-dispatch.mjs\`; руками их не правят.
//!
//! Правил про vault здесь по-прежнему нет: тела команд — получение backend из состояния
//! и вызов ядра (принцип I).

/// Имена, которыми пользуются команды. Обёртки оболочки берут отсюда же типы своих
/// сигнатур, поэтому импорты открыты.
pub mod prelude {
${prelude.replace(/\s+$/, "")}
}

use prelude::*;

/// Состояния одного открытого vault: всё, что нужно командам ядра.
///
/// Оболочка Tauri держит те же состояния по отдельности (\`.manage(...)\`); сервер —
/// один \`Host\` на пару (аккаунт, vault).
#[derive(Default)]
pub struct Host {
${states.map((s) => `    pub ${snake(s)}: ${s},`).join("\n")}
}

${hostItems.join("").trim()}

pub mod commands;
pub mod dispatch;

pub use commands::*;
`;

const hostCommands = `//! Тела команд ядра (перенесены из \`packages/shell/src/lib.rs\`, фича 037 T001).
//!
//! Каждая \`pub fn\` верхнего уровня этого файла — команда: по ним
//! \`scripts/dev/gen-host-dispatch.mjs\` строит обёртки Tauri и таблицу сервера.
//! Помощник, который командой не является, живёт в \`lib.rs\`.


use crate::*;

${commands.join("").trim()}
`;

// Оболочка: шапка, свои элементы, импорт перенесённого.
const shellUses = uses.filter((u) => /use tauri::|^\s*mod /.test(u) || /^\s*use std/.test(u)).join("");
let shellLib = `${header}
${shellUses}
mod commands;

use tokio::sync::Mutex;
use typsidian_host::prelude::*;
pub use typsidian_host::{
    Answer, Failure, HighlightSessions, ImportSource, NoVault, Preview, PropertiesReindex,
    Sources, Speller, TemplatePreviews, Vault,
};
${shellItems.join("")}`;

// Список обработчиков: перенесённые команды — из модуля обёрток.
const movedSet = new Set(moved);
shellLib = shellLib.replace(/tauri::generate_handler!\[([\s\S]*?)\]/, (all, list) =>
  "tauri::generate_handler![" +
  list.replace(/(^|\n)(\s+)(\w+),/g, (m, nl, sp, name) =>
    movedSet.has(name) ? `${nl}${sp}commands::${name},` : m,
  ) +
  "]",
);

mkdirSync("packages/host/src", { recursive: true });
writeFileSync("packages/host/src/lib.rs", hostLib);
writeFileSync("packages/host/src/commands.rs", hostCommands);
writeFileSync(SHELL, shellLib);
console.log(`перенесено команд: ${moved.length}, элементов: ${hostItems.length}, осталось в оболочке: ${shellItems.length}`);
