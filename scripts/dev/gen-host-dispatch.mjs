#!/usr/bin/env node
// Обёртки команд `packages/host/src/commands.rs` (фича 037, research.md §1, T002).
//
// Из каждой `pub fn` верхнего уровня строятся:
// - `packages/shell/src/commands.rs` — `#[tauri::command]` с прежним именем и
//   аргументами, где `&Vault` снова `State<'_, Vault>`;
// - `packages/host/src/dispatch.rs` — `dispatch(&Host, имя, JSON)` для сервера:
//   аргументы в camelCase, как их шлёт клиент Tauri, ответ — то, что вернула бы
//   команда Tauri.
//
// Руками эти файлы не правят. `--check` — гейт свежести (`gates.sh fast`).

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

import { body, head, scanItems } from "./rust-items.mjs";

const COMMANDS = "packages/host/src/commands.rs";
const SHELL_OUT = "packages/shell/src/commands.rs";
const DISPATCH_OUT = "packages/host/src/dispatch.rs";

const STATES = new Map(
  [
    "Vault",
    "TemplatePreviews",
    "Sources",
    "Preview",
    "Speller",
    "HighlightSessions",
    "PropertiesReindex",
    "ImportSource",
  ].map((s) => [s, s.replace(/[A-Z]/g, (c, k) => (k ? "_" : "") + c.toLowerCase())]),
);

/** Делит список по запятым верхнего уровня (вне `<>`, `()`, `[]`). */
function splitTop(text) {
  const parts = [];
  let depth = 0;
  let cur = "";
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if ("<([{".includes(c)) depth++;
    else if (">)]}".includes(c) && !(c === ">" && text[i - 1] === "-")) depth--;
    if (c === "," && depth === 0) {
      parts.push(cur);
      cur = "";
    } else cur += c;
  }
  if (cur.trim()) parts.push(cur);
  return parts.map((p) => p.trim()).filter(Boolean);
}

function parseCommand(item) {
  const text = body(item);
  const m = /^pub (async )?fn (\w+)\(/.exec(text);
  if (!m) return null;
  let i = m[0].length;
  let depth = 1;
  while (depth > 0) {
    if (text[i] === "(") depth++;
    else if (text[i] === ")") depth--;
    i++;
  }
  const params = splitTop(text.slice(m[0].length, i - 1)).map((p) => {
    const [, name, type] = /^(?:mut\s+)?(\w+)\s*:\s*([\s\S]+)$/.exec(p);
    const state = /^&(\w+)$/.exec(type.trim());
    return {
      name,
      type: type.replace(/\s+/g, " ").trim(),
      state: state && STATES.has(state[1]) ? state[1] : null,
    };
  });
  const rest = text.slice(i);
  const ret = /^\s*->\s*([\s\S]*?)\s*(?:where\b[\s\S]*)?\{/.exec(rest);
  const returns = ret ? ret[1].replace(/\s+/g, " ").trim() : null;
  return { async: !!m[1], name: m[2], params, returns };
}

const isResult = (t) => t !== null && /^(Result|Answer|\w+Answer)\b/.test(t);

const commands = scanItems(readFileSync(COMMANDS, "utf8"))
  .filter((item) => head(item).kind === "fn")
  .map(parseCommand)
  .filter(Boolean);

const HEADER = `// @generated scripts/dev/gen-host-dispatch.mjs из packages/host/src/commands.rs.
// Не править руками: \`nix develop -c node scripts/dev/gen-host-dispatch.mjs\`.
`;

const shell = [
  HEADER,
  `//! Обёртки Tauri над командами \`typsidian_host\` (фича 037): имена и аргументы
//! прежние, тела — вызов общей реализации.

#![allow(clippy::too_many_arguments)]

use tauri::State;
use typsidian_host::prelude::*;
use typsidian_host::*;
`,
];
for (const c of commands) {
  const sig = c.params
    .map((p) => (p.state ? `${p.name}: State<'_, ${p.state}>` : `${p.name}: ${p.type}`))
    .join(", ");
  const args = c.params.map((p) => (p.state ? `&${p.name}` : p.name)).join(", ");
  shell.push(`
#[tauri::command]
pub ${c.async ? "async " : ""}fn ${c.name}(${sig})${c.returns ? ` -> ${c.returns}` : ""} {
    typsidian_host::commands::${c.name}(${args})${c.async ? ".await" : ""}
}
`);
}

const dispatch = [
  HEADER,
  `//! Вызов команды по имени — для сервера (фича 037, research.md §1).
//!
//! Аргументы — объект JSON с полями в camelCase, как их шлёт клиент Tauri; ответ —
//! значение команды или её отказ, сериализованные так же, как их отдал бы Tauri.

#![allow(clippy::too_many_lines)]

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;


use crate::*;

/// Ответ команды: \`Ok\` — значение, \`Err\` — отказ (ядра, «vault не открыт» или
/// разбора аргументов).
pub type Reply = Result<Value, Value>;

/// Имена всех команд таблицы.
pub const COMMANDS: &[&str] = &[
${commands.map((c) => `    "${c.name}",`).join("\n")}
];

fn args<A: DeserializeOwned>(value: Value) -> Result<A, Value> {
    serde_json::from_value(value).map_err(|error| {
        serde_json::json!({ "kind": "InvalidArguments", "message": error.to_string() })
    })
}

fn value<T: Serialize>(value: T) -> Reply {
    serde_json::to_value(value).map_err(|error| {
        serde_json::json!({ "kind": "Unserializable", "message": error.to_string() })
    })
}

fn answer<T: Serialize, E: Serialize>(result: Result<T, E>) -> Reply {
    match result {
        Ok(ok) => value(ok),
        Err(error) => Err(value(error)?),
    }
}

/// Вызывает команду \`name\`; \`None\` — такой команды нет.
pub async fn dispatch(host: &Host, name: &str, arguments: Value) -> Option<Reply> {
    Some(match name {`,
];
for (const c of commands) {
  const plain = c.params.filter((p) => !p.state);
  const call = c.params
    .map((p) => (p.state ? `&host.${STATES.get(p.state)}` : `a.${p.name}`))
    .join(", ");
  const invoke = `commands::${c.name}(${call})${c.async ? ".await" : ""}`;
  const wrap = isResult(c.returns)
    ? `answer(${invoke})`
    : c.returns === null
      ? `{ ${invoke}; value(()) }`
      : `value(${invoke})`;
  if (plain.length === 0) {
    dispatch.push(`        "${c.name}" => {
            let _ = arguments;
            ${wrap}
        }`);
  } else {
    dispatch.push(`        "${c.name}" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
${plain.map((p) => `                ${p.name}: ${p.type},`).join("\n")}
            }
            match args::<A>(arguments) {
                Ok(a) => ${wrap},
                Err(error) => Err(error),
            }
        }`);
  }
}
dispatch.push(`        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Каждая команда \`host::commands\` есть в таблице: без vault она отвечает
    /// отказом или значением, но не «нет такой команды».
    #[tokio::test]
    async fn every_command_is_dispatched() {
        let host = Host::default();
        for name in COMMANDS {
            assert!(
                dispatch(&host, name, serde_json::json!({ "__unknown": true }))
                    .await
                    .is_some(),
                "{name}"
            );
        }
        assert!(dispatch(&host, "open_vault", Value::Null).await.is_none());
    }
}
`);

// Через rustfmt — файлы и так должны быть в его виде, и `--check` сравнивает точно.
const fmt = (text) => execFileSync("rustfmt", ["--edition", "2021", "--emit", "stdout"], { input: text, encoding: "utf8" });

const outputs = [
  [SHELL_OUT, fmt(shell.join(""))],
  [DISPATCH_OUT, fmt(dispatch.join("\n"))],
];

if (process.argv.includes("--check")) {
  let stale = false;
  for (const [path, text] of outputs) {
    let current = "";
    try {
      current = readFileSync(path, "utf8");
    } catch {}
    if (current !== text) {
      console.error(`${path} устарел: nix develop -c node scripts/dev/gen-host-dispatch.mjs`);
      stale = true;
    }
  }
  process.exit(stale ? 1 : 0);
}
for (const [path, text] of outputs) writeFileSync(path, text);
console.log(`команд: ${commands.length}`);
