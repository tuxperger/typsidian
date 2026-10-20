// Импорт из Obsidian (фича 021, quickstart.md §2): команда палитры → выбор папки
// (C1) → ход с отменой (C2) → продолжение (C4) → отчёт (C3); нет API выбора
// папки → C5. Источник — packages/core/tests/fixtures/obsidian/, скопированный в
// OPFS отдельной папкой «obsidian-src»; `showDirectoryPicker` подменяется так же,
// как `--opfs-vault` подменяет его для vault: выдаёт хендл этой папки.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/import.mjs --opfs-vault --shot /tmp/import.png
//
// `IMPORT_SHOTS=/каталог` — снимки шагов для сверки с макетом C1–C5.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const FIXTURES = resolve(
  fileURLToPath(new URL(".", import.meta.url)),
  "../../../packages/core/tests/fixtures/obsidian",
);

/** Все файлы папки: относительный путь через `/` и содержимое в base64. */
function collect(dir) {
  const files = [];
  const walk = (current) => {
    for (const entry of readdirSync(current)) {
      const full = join(current, entry);
      if (statSync(full).isDirectory()) walk(full);
      else {
        files.push({
          path: relative(dir, full).split(sep).join("/"),
          base64: readFileSync(full).toString("base64"),
        });
      }
    }
  };
  walk(dir);
  return files;
}

export default async (page) => {
  const shots = process.env.IMPORT_SHOTS;
  const shot = async (name) => {
    if (shots) await page.shot(`${shots}/${name}.png`);
  };
  const failures = [];
  const check = (label, ok) => {
    page.log(`${ok ? "✓" : "✗"} ${label}`);
    if (!ok) failures.push(label);
  };

  // Источник в OPFS и подмена выбора папки для него.
  const files = collect(FIXTURES);
  await page.evaluate(`(async () => {
    const root = await navigator.storage.getDirectory();
    const src = await root.getDirectoryHandle("obsidian-src", { create: true });
    for (const file of ${JSON.stringify(files)}) {
      const parts = file.path.split("/");
      let dir = src;
      for (const part of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(part, { create: true });
      const handle = await dir.getFileHandle(parts.at(-1), { create: true });
      const stream = await handle.createWritable();
      await stream.write(Uint8Array.from(atob(file.base64), (c) => c.charCodeAt(0)));
      await stream.close();
    }
    const vault = await root.getDirectoryHandle("vault", { create: true });
    const note = await vault.getFileHandle("корень.typ", { create: true });
    const stream = await note.createWritable();
    await stream.write("= Корень\\nСвоя заметка vault.\\n");
    await stream.close();
  })()`);

  /** Список файлов папки OPFS с размерами: сверка «источник не изменён». */
  const listing = (name) =>
    page.evaluate(`(async () => {
      const root = await navigator.storage.getDirectory();
      const out = [];
      const walk = async (dir, prefix) => {
        for await (const [name, handle] of dir.entries()) {
          if (handle.kind === "directory") await walk(handle, prefix + name + "/");
          else out.push(prefix + name + ":" + (await handle.getFile()).size);
        }
      };
      await walk(await root.getDirectoryHandle(${JSON.stringify(name)}), "");
      return out.sort();
    })()`);
  const typFiles = async () => (await listing("vault")).filter((e) => e.includes("из-obsidian/") && e.split(":")[0].endsWith(".typ"));

  await page.click("text=Открыть vault");
  await page.waitForElement("text=корень");
  await page.sleep(1500);

  const before = await listing("obsidian-src");
  page.log(`источник: ${before.length} файлов`);

  const openImportDialog = async () => {
    await page.click(".ts-btn--cmd");
    await page.waitForElement(".ts-palette");
    await page.type("Obsidian");
    await page.sleep(300);
    await page.press("Enter");
  };

  // ── C5: нет доступа к папкам ──────────────────────────────────────────
  await page.evaluate(`window.__realPicker = window.showDirectoryPicker; delete window.showDirectoryPicker; true`);
  await openImportDialog();
  await page.waitForElement(".ts-import");
  check("C5: причина названа", (await page.text(".ts-import")).includes("Chrome или Edge"));
  check("C5: нет кнопки выбора папки", !(await page.evaluate(`!![...document.querySelectorAll('.ts-import button')].find((b) => b.textContent.includes('Выбрать'))`)));
  check("C5: фокус на «Закрыть»", await page.evaluate(`document.activeElement?.textContent === 'Закрыть'`));
  await shot("c5-unavailable");
  await page.press("Escape");
  await page.waitFor(`!document.querySelector('.ts-import')`, { label: "C5 закрыт по Escape" });

  // ── C1: настройка ─────────────────────────────────────────────────────
  await page.evaluate(`(() => {
    window.__pickMode = null;
    window.showDirectoryPicker = async (options) => {
      window.__pickMode = options?.mode ?? null;
      const root = await navigator.storage.getDirectory();
      return root.getDirectoryHandle("obsidian-src");
    };
    return true;
  })()`);
  await openImportDialog();
  await page.waitForElement(".ts-import");
  check("C1: фокус на «Выбрать…»", await page.evaluate(`document.activeElement?.textContent === 'Выбрать…'`));
  check("C1: «Импортировать» выключена до выбора папки", await page.evaluate(`document.querySelector('.ts-import .ts-btn--accent').disabled`));
  await shot("c1-empty");
  await page.click("text=Выбрать…");
  await page.waitFor(`Boolean(document.querySelector('.ts-import__facts b'))`, { label: "числа обзора" });
  check("C1: папка открыта только для чтения", (await page.evaluate(`window.__pickMode`)) === "read");
  check(
    "C1: импорт идёт в своём воркере (T026)",
    await page.evaluate(`performance.getEntriesByType("resource").some((entry) => entry.name.includes("import-worker"))`),
  );
  const facts = await page.text(".ts-import__facts");
  page.log("обзор: " + facts.replace(/\n/g, " "));
  check("C1: .obsidian и .trash не считаются", facts.includes("без .obsidian/ и .trash/"));
  await shot("c1-setup");
  await page.click("text=Изменить");
  await page.waitForElement(".ts-import__input");
  await page.sleep(400);
  await shot("c1-destination");
  await page.click("text=Готово");

  // ── C2: ход и отмена ──────────────────────────────────────────────────
  // Нулевые паузы цикла растягиваем, чтобы успеть нажать «Отменить» посреди.
  await page.evaluate(`(() => {
    const real = window.setTimeout.bind(window);
    window.__slow = true;
    window.setTimeout = (fn, ms, ...rest) => real(fn, window.__slow && !ms ? 150 : ms, ...rest);
    return true;
  })()`);
  await page.click(".ts-import .ts-btn--accent");
  await page.waitForElement(".ts-import-card");
  check("C2: диалог закрылся, карточка на месте", !(await page.evaluate(`!!document.querySelector('.ts-import')`)));
  await page.waitFor(`/^[4-9] \\//.test(document.querySelector('.ts-import-card__count')?.textContent ?? '')`, { label: "шаги идут" });
  await shot("c2-progress");
  // UI жив во время импорта: вкладка с заметкой открывается.
  await page.click(".ts-import-card__actions .ts-btn--ghost");
  await page.waitForElement(".ts-export-pill");
  check("C2: свёрнута в шапку", await page.evaluate(`!document.querySelector('.ts-import-card') && document.querySelector('.ts-export-pill').textContent.includes('импорт')`));
  await shot("c2-minimized");
  await page.click(".ts-export-pill");
  await page.waitForElement(".ts-import-card");
  const cancelAt = Date.now();
  await page.click("text=Отменить");
  await page.waitFor(`document.querySelector('.ts-import-card')?.textContent.includes('Импорт отменён')`, { label: "итог отмены" });
  check(`C2: отмена за ${Date.now() - cancelAt} мс (<= 2000)`, Date.now() - cancelAt <= 2000);
  const afterCancel = await page.text(".ts-import-card");
  page.log("карточка:\n" + afterCancel);
  await shot("c2-cancelled");
  const partial = await typFiles();
  check(`отмена: перенесена часть заметок (${partial.length})`, partial.length > 0);
  await page.click("text=Закрыть");

  // ── C4: продолжение ───────────────────────────────────────────────────
  await openImportDialog();
  await page.waitForElement(".ts-import");
  await page.click("text=Выбрать…");
  await page.waitFor(`document.querySelector('.ts-import')?.textContent.includes('уже импортировали')`, { label: "экран продолжения" });
  check("C4: экран продолжения", true);
  page.log("C4: " + (await page.text(".ts-import")).replace(/\n/g, " "));
  await shot("c4-resume");
  await page.evaluate(`window.__slow = false; true`);
  await page.click("text=Продолжить импорт");
  await page.waitFor(`document.querySelector('.ts-import-card')?.textContent.includes('Импорт завершён')`, { label: "импорт завершён" });
  await page.sleep(300);
  page.log("итог:\n" + (await page.text(".ts-import-card")));
  await shot("c3-done");
  const all = await typFiles();
  const names = all.map((e) => e.split(":")[0]);
  check(`после продолжения без дублей (${all.length} файлов)`, new Set(names).size === names.length && !names.some((n) => n.includes("(импорт)")));
  check("дерево папок сохранено", names.some((n) => n.includes("Ссылки/")));
  check("файл с «?» в имени переименован", names.some((n) => n.includes("Что за имя")) && !names.some((n) => n.includes("?")));

  // ── C3: отчёт ─────────────────────────────────────────────────────────
  await page.click("text=Открыть отчёт");
  await page.sleep(500);
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('Импорт из Obsidian')`, { label: "отчёт открыт" });
  await page.sleep(2500);
  const report = await page.evaluate(`document.querySelector('.cm-content')?.innerText ?? ''`);
  page.log("отчёт:\n" + report);
  check("C3: в отчёте есть потери и пропущенное", report.includes("С потерями") && report.includes("Пропущено"));
  check("C3: отчёт собирается", !(await page.evaluate(`!!document.querySelector('.ts-preview [role=alert], .ts-diagnostics--error')`)));
  await shot("c3-report");

  // ── Импортированная заметка со ссылкой, картинкой и формулой собирается ─
  await page.evaluate(`(() => {
    const item = [...document.querySelectorAll('.ts-tree__file > button:first-child, .ts-tree__folder > button:first-child')]
      .find((b) => b.textContent.includes('из-obsidian'));
    item?.click();
    return true;
  })()`);
  await page.sleep(400);

  // ── Источник не изменён (SC-005) ──────────────────────────────────────
  const after = await listing("obsidian-src");
  check("SC-005: источник не изменён", JSON.stringify(before) === JSON.stringify(after));

  // ── Узкий вид и клавиатура ────────────────────────────────────────────
  await page.viewport(390, 800);
  await page.sleep(300);
  // На узком экране кнопки палитры в шапке нет — открываем клавишей.
  await page.press("k", { ctrl: true });
  await page.waitForElement(".ts-palette");
  await page.type("Obsidian");
  await page.sleep(300);
  await page.press("Enter");
  await page.waitForElement(".ts-import");
  await page.click("text=Выбрать…");
  await page.waitFor(`Boolean(document.querySelector('.ts-import__facts b')) || document.querySelector('.ts-import')?.textContent.includes('уже импортировали')`, { label: "обзор на узком экране" });
  page.log("узкий вид, диалог: " + (await page.text(".ts-import")).replace(/\n/g, " "));
  await shot("narrow-dialog");
  const width = await page.evaluate(`document.querySelector('.ts-import').getBoundingClientRect().width`);
  check(`узкий вид: диалог помещается (${Math.round(width)} px)`, width <= 390);
  // Карточка хода на узком экране: идём по готовому назначению с паузами.
  await page.evaluate(`window.__slow = true; true`);
  await page.click(".ts-import .ts-btn--accent");
  await page.waitFor(`/^[2-9] \\//.test(document.querySelector('.ts-import-card__count')?.textContent ?? '')`, { label: "карточка на узком экране" });
  await shot("narrow-card");
  const card = await page.evaluate(`(() => { const r = document.querySelector('.ts-import-card').getBoundingClientRect(); return { left: r.left, right: r.right }; })()`);
  check(`узкий вид: карточка внутри окна (${Math.round(card.left)}–${Math.round(card.right)})`, card.left >= 0 && card.right <= 390);
  await page.click("text=Отменить");
  await page.waitFor(`document.querySelector('.ts-import-card')?.textContent.includes('Закрыть')`, { label: "итог на узком экране" });
  await page.viewport(1440, 900);

  if (failures.length > 0) throw new Error("не сошлось:\n- " + failures.join("\n- "));
};
