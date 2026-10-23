#!/usr/bin/env node
// Словари орфографии для web (фича 030, research.md §1).
//
// Словари едут рядом с wasm-модулем, а не внутри него — как шрифты
// предпросмотра (`builtin-fonts.mjs`): модуль пересобирается на каждом релизе,
// а 4 МБ словарей не меняются. Источник — те же файлы, что вшиты в нативную
// сборку ядра (`packages/core/dictionaries/`). `index.json` — список, по
// которому их грузит `ui/src/spellcheck/dictionaries.ts`; версия — хэш
// содержимого, чтобы кэш устройства не держал старые словари.
//
//   node scripts/spell-dictionaries.mjs apps/web/public/dictionaries

import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const source = resolve("packages/core/dictionaries");
const target = resolve(process.argv[2] ?? "apps/web/public/dictionaries");
const languages = {
  ru: { aff: "ru_RU.aff", dic: "ru_RU.dic" },
  en: { aff: "en_US.aff", dic: "en_US.dic" },
};

const hash = createHash("sha256");
rmSync(target, { recursive: true, force: true });
mkdirSync(target, { recursive: true });
for (const files of Object.values(languages)) {
  for (const name of [files.aff, files.dic]) {
    hash.update(readFileSync(join(source, name)));
    copyFileSync(join(source, name), join(target, name));
  }
}
copyFileSync(join(source, "NOTICE.md"), join(target, "NOTICE.md"));
const version = hash.digest("hex").slice(0, 12);
writeFileSync(join(target, "index.json"), `${JSON.stringify({ version, languages })}\n`);
console.log(`spell-dictionaries: ${Object.keys(languages).length} языка → ${target}`);
