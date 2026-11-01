// Склеивает шрифты, токены и app.css в один файл: конвертер не копирует относительные
// @import из cssEntry, а Claude Design получает только замыкание styles.css.
import { readFileSync, writeFileSync } from 'node:fs';
const dir = 'ui/dist/styles/';
const app = readFileSync(dir + 'app.css', 'utf8').replace(/^@import\s+"\.\/tokens\/[^"]+";\s*$/gm, '');
const tokens = ['colors', 'typography', 'spacing', 'elevation'].map((n) => readFileSync(`${dir}tokens/${n}.css`, 'utf8'));
writeFileSync(dir + 'ds.css', [readFileSync('.design-sync/fonts.css', 'utf8'), ...tokens, app].join('\n'));
