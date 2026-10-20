#!/usr/bin/env node
// Встроенный набор шрифтов предпросмотра для web (фича 005, T029, research.md §6).
//
// Шрифты едут рядом с wasm-модулем, а не внутри него: модуль пересобирается на
// каждом релизе, а 9,6 МБ шрифтов не меняются. Скрипт берёт файлы из того же
// `typst-assets`, что вшит в нативную сборку, и кладёт их в статику web вместе с
// `index.json` — списком, по которому их грузит `ui/src/preview/builtin-fonts.ts`.
// Список имён — только здесь: его повторяет `typst_assets::fonts()`.
//
//   node scripts/builtin-fonts.mjs apps/web/public/fonts/typst

import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

const target = resolve(process.argv[2] ?? "apps/web/public/fonts/typst");
const metadata = JSON.parse(
  execFileSync("cargo", ["metadata", "--format-version", "1", "--manifest-path", "packages/core/Cargo.toml"], {
    maxBuffer: 64 * 1024 * 1024,
  }).toString(),
);
const assets = metadata.packages.find((pkg) => pkg.name === "typst-assets");
if (!assets) throw new Error("typst-assets не найден в зависимостях ядра");
const source = join(dirname(assets.manifest_path), "files", "fonts");

// Тот же состав, что `typst_assets::fonts()`: шрифты вёрстки, без Foxit (.pfb).
const files = readdirSync(source)
  .filter((name) => /\.(otf|ttf)$/.test(name))
  .sort();
rmSync(target, { recursive: true, force: true });
mkdirSync(target, { recursive: true });
for (const name of files) copyFileSync(join(source, name), join(target, name));
writeFileSync(join(target, "index.json"), `${JSON.stringify({ version: assets.version, files })}\n`);
console.log(`builtin-fonts: ${files.length} файлов → ${target}`);
