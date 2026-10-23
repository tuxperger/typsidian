// Локальные снимки заметок (фича 029, quickstart.md): снимок при открытии и при
// внешнем изменении, окно «История заметки» с различиями, возврат версии байт в
// байт со снимком «перед возвратом», «Недавно удалённые» с восстановлением на
// прежнее место и под новым именем, раздел настроек «Снимки заметок» с очисткой.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/snapshots.mjs --opfs-vault
//
// Снимки экрана — в каталог из SNAPSHOTS_SHOTS (по умолчанию /tmp).
const shots = process.env.SNAPSHOTS_SHOTS ?? "/tmp";

const NOTE = "Диссертация/Введение.typ";
const FIRST = "= Введение\n\nПостановка задачи.\nТребуется оценить образцы.\n";
const SECOND = "= Введение\n\nЗадача.\n";

const OPFS = `
  window.__opfs = {
    async root() { return (await navigator.storage.getDirectory()).getDirectoryHandle("vault", { create: true }); },
    async dir(path, create) {
      let dir = await this.root();
      for (const part of path.split("/").filter(Boolean)) dir = await dir.getDirectoryHandle(part, { create });
      return dir;
    },
    async write(path, text) {
      const at = path.lastIndexOf("/");
      const dir = await this.dir(at === -1 ? "" : path.slice(0, at), true);
      const file = await dir.getFileHandle(path.slice(at + 1), { create: true });
      const stream = await file.createWritable();
      await stream.write(text);
      await stream.close();
    },
    async read(path) {
      try {
        const at = path.lastIndexOf("/");
        const dir = await this.dir(at === -1 ? "" : path.slice(0, at), false);
        return await (await (await dir.getFileHandle(path.slice(at + 1))).getFile()).text();
      } catch { return null; }
    },
  };
`;

const expect = (page, condition, what) => {
  if (!condition) throw new Error("не сошлось: " + what);
  page.log("  ok: " + what);
};

const read = (page, path) => page.evaluate(`window.__opfs.read(${JSON.stringify(path)})`);
const editorText = (page) => page.evaluate(`document.querySelector('.cm-content')?.innerText ?? ''`);
const snapRows = (page) =>
  page.evaluate(`[...document.querySelectorAll('.ts-history__snap:not(.ts-history__snap--current)')].map((row) => row.textContent)`);

const openPalette = async (page, query) => {
  await page.press("k", { ctrl: true });
  await page.waitForElement(".ts-palette");
  await page.type(query);
  await page.sleep(200);
  await page.press("Enter");
};

/** Удаление заметки из контекстного меню дерева; подтверждение принимает browser.mjs. */
const deleteFromTree = async (page, path) => {
  await page.evaluate(`(() => {
    const row = document.querySelector('.ts-tree__file[data-drag-path=${JSON.stringify(path)}]');
    row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: 40, clientY: 40 }));
  })()`);
  await page.click("text=Удалить…");
  await page.waitFor(`window.__opfs.read(${JSON.stringify(path)}).then((text) => text === null)`, { label: `${path} удалена` });
};

export default async (page) => {
  await page.evaluate(OPFS + "true");
  await page.evaluate(`window.__opfs.write(${JSON.stringify(NOTE)}, ${JSON.stringify(FIRST)})`);
  await page.click("text=Открыть vault");
  await page.waitForElement("text=Введение", { timeout: 30000 });

  // ── US1: снимок при открытии и при внешнем изменении ──────────────────
  await page.click("text=Введение");
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('Постановка задачи')`, { label: "заметка открыта" });
  await page.waitFor(`window.__opfs.read(".typsidian/snapshots/index.json").then((text) => text !== null)`, { label: "снимок при открытии" });
  await page.evaluate(`window.__opfs.write(${JSON.stringify(NOTE)}, ${JSON.stringify(SECOND)})`);
  await page.waitFor(`!document.querySelector('.cm-content')?.innerText.includes('Постановка задачи')`, { timeout: 10000, label: "внешнее изменение подхвачено" });
  // Несохранённый черновик: он и станет снимком «перед возвратом» (текст,
  // совпадающий с последним снимком, ядро второй раз не снимает).
  await page.click(".cm-content");
  await page.press("End", { ctrl: true });
  await page.type("черновик");

  await openPalette(page, "История заметки");
  await page.waitForElement(".ts-history");
  await page.waitFor(`document.querySelectorAll('.ts-history__snap:not(.ts-history__snap--current)').length === 2`, { label: "два снимка" });
  const rows = await snapRows(page);
  page.log("— история:\n  " + rows.join("\n  "));
  expect(page, rows[0].includes("внешнее изменение") && rows[1].includes("открытие"), "новые первыми, с причинами");
  expect(page, rows[1].includes("+4") && rows[0].includes("−2"), "+/− строк у снимков");

  // Различия старшего снимка с текущим текстом и возврат.
  await page.evaluate(`document.querySelectorAll('.ts-history__snap:not(.ts-history__snap--current)')[1].click()`);
  await page.waitFor(`document.querySelector('.ts-history__diff')?.innerText.includes("Требуется оценить образцы.")`, { label: "различия выбранного снимка" });
  expect(page, true, "различия показывают стёртую строку");
  await page.shot(shots + "/029-s1-history.png");
  await page.click("text=Вернуть эту версию");
  await page.waitFor(`document.body.innerText.includes("Возвращена версия")`, { label: "итог возврата" });
  await page.shot(shots + "/029-s1-restored.png");
  expect(page, (await editorText(page)).includes("Требуется оценить образцы."), "редактор получил прежний текст");
  await page.waitFor(`window.__opfs.read(${JSON.stringify(NOTE)}).then((text) => text === ${JSON.stringify(FIRST)})`, { timeout: 10000, label: "файл равен снимку байт в байт" });
  expect(page, true, "файл равен снимку байт в байт (SC-002)");
  await page.waitFor(`[...document.querySelectorAll('.ts-history__snap')].some((row) => row.textContent.includes("перед возвратом"))`, { label: "снимок «перед возвратом»" });
  expect(page, true, "в истории появился «перед возвратом»");
  await page.viewport(390, 760);
  await page.sleep(300);
  await page.shot(shots + "/029-s4-phone-history.png");
  await page.viewport(1440, 900);
  await page.press("Escape");
  await page.waitFor(`!document.querySelector('.ts-history')`, { label: "окно закрыто" });
  await page.click(".cm-content");
  await page.press("z", { ctrl: true });
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('черновик')`, { label: "Ctrl+Z вернул черновик" });
  expect(page, true, "Ctrl+Z отменяет возврат одной правкой");

  // ── US2: удалённая заметка ─────────────────────────────────────────────
  // Удаление снимает то, что лежит на диске: после Ctrl+Z это текст с черновиком.
  await page.waitFor(`window.__opfs.read(${JSON.stringify(NOTE)}).then((text) => text?.includes("черновик"))`, { timeout: 10000, label: "черновик сохранён" });
  const beforeDelete = await read(page, NOTE);
  await deleteFromTree(page, NOTE);
  await openPalette(page, "Недавно удалённые");
  await page.waitForElement(".ts-deleted");
  await page.waitFor(`document.querySelectorAll('.ts-deleted__row').length === 1`, { label: "одна удалённая" });
  page.log("— удалённые:\n" + (await page.text(".ts-deleted")));
  await page.shot(shots + "/029-s2-deleted.png");
  await page.evaluate(`[...document.querySelectorAll('.ts-deleted__row button')].find((b) => b.textContent === "Восстановить").click()`);
  await page.waitFor(`window.__opfs.read(${JSON.stringify(NOTE)}).then((text) => text === ${JSON.stringify(beforeDelete)})`, { label: "восстановлена на прежнем месте" });
  expect(page, true, "заметка восстановлена на прежнем месте с последним текстом");
  if (await page.evaluate(`document.querySelector('.ts-deleted') !== null`)) await page.press("Escape");

  // Тот же путь занят: удалить, создать заново снаружи, восстановить под новым именем.
  await deleteFromTree(page, NOTE);
  await page.evaluate(`window.__opfs.write(${JSON.stringify(NOTE)}, "= Новая\\n")`);
  await page.waitFor(`document.querySelector('.ts-tree__file[data-drag-path=${JSON.stringify(NOTE)}]') !== null`, { timeout: 10000, label: "новая заметка в дереве" });
  await openPalette(page, "Недавно удалённые");
  await page.waitFor(`document.querySelectorAll('.ts-deleted__row').length === 1`, { label: "снова одна удалённая" });
  await page.evaluate(`[...document.querySelectorAll('.ts-deleted__row button')].find((b) => b.textContent === "Восстановить").click()`);
  await page.waitForElement(".ts-deleted__rename");
  const suggested = await page.evaluate(`document.querySelector('.ts-deleted__field').value`);
  expect(page, suggested === "Диссертация/Введение (2).typ", "занятый путь — предложено новое имя: " + suggested);
  await page.shot(shots + "/029-s2-occupied.png");
  await page.evaluate(`document.querySelector('.ts-deleted__rename button[type=submit]').click()`);
  await page.waitFor(`window.__opfs.read("Диссертация/Введение (2).typ").then((text) => text === ${JSON.stringify(beforeDelete)})`, { label: "восстановлена под новым именем" });
  expect(page, (await read(page, NOTE)) === "= Новая\n", "новая заметка на прежнем пути не тронута");
  if (await page.evaluate(`document.querySelector('.ts-deleted') !== null`)) await page.press("Escape");

  // ── US3: настройки, объём, очистка ─────────────────────────────────────
  await page.click("text=Настройки");
  await page.click("text=Снимки заметок");
  await page.waitFor(`document.body.innerText.includes("Сейчас:")`, { label: "объём снимков" });
  await page.shot(shots + "/029-s3-settings.png");
  await page.click("text=Очистить");
  await page.waitFor(`document.querySelector('[role=alertdialog]') !== null`, { label: "подтверждение очистки" });
  await page.click("text=Удалить снимки");
  await page.waitFor(`document.body.innerText.includes("Удалено")`, { label: "итог очистки" });
  const index = await read(page, ".typsidian/snapshots/index.json");
  expect(page, index === null || !index.includes("Введение"), "опись пуста после очистки");
  expect(page, (await read(page, NOTE)) === "= Новая\n", "заметки не изменились");

  // Телефон.
  await page.viewport(390, 760);
  await page.sleep(300);
  await page.shot(shots + "/029-phone-settings.png");
  await page.viewport(1440, 900);
};
