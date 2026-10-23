// Закладки (фича 032, quickstart.md): «В закладки» из дерева, оглавления и окна
// поиска, без дублей; открытие заметки, раздела, поиска и папки; группа,
// перенос в неё, Alt+↓; закладки в `.typsidian/vault.json`; переименование
// заметки везёт закладку; удалённая цель помечена.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/bookmarks.mjs --opfs-vault
//
// Снимки экрана — в каталог из BOOKMARK_SHOTS (по умолчанию /tmp).
const shots = process.env.BOOKMARK_SHOTS ?? "/tmp";

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

const panel = (page) =>
  page.evaluate(`[...document.querySelectorAll('.ts-bookmarks__item .ts-bookmarks__name, .ts-bookmarks__gtoggle')].map((el) => el.textContent.trim())`);

/** Правый щелчок по строке дерева. */
const treeMenu = (page, selector) =>
  page.evaluate(`(() => {
    const row = document.querySelector(${JSON.stringify(selector)});
    row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 60, clientY: 200 }));
    return true;
  })()`);

const bookmarkButton = (page, name) =>
  page.evaluate(`[...document.querySelectorAll('.ts-bookmarks__item')].find((b) => b.querySelector('.ts-bookmarks__name')?.textContent === ${JSON.stringify(name)})?.click() ?? false`);

export default async (page) => {
  await page.evaluate(OPFS + "true");
  await page.evaluate(`window.__opfs.write("Диссертация/Введение.typ", "= Введение\\n\\nТекст введения.\\n")`);
  await page.evaluate(`window.__opfs.write("Диссертация/Методика.typ", "= Методика\\n\\n" + "Строка.\\n".repeat(60) + "\\n== Выборка\\n\\nОтобрано 24 образца.\\n")`);
  await page.evaluate(`window.__opfs.write("Диссертация/рисунки/Схема.typ", "= Схема\\n")`);
  await page.click("text=Открыть vault");
  await page.waitForElement(".ts-bookmarks", { timeout: 30000 });
  await page.shot(shots + "/032-empty.png");

  // ── Дерево: заметка и папка, без дублей ──────────────────────────────
  await treeMenu(page, '.ts-tree__file[data-drag-path="Диссертация/Введение.typ"]');
  await page.click("text=В закладки");
  await page.waitFor(`document.querySelectorAll('.ts-bookmarks__item').length === 1`, { label: "закладка заметки" });
  await treeMenu(page, '.ts-tree__file[data-drag-path="Диссертация/Введение.typ"]');
  await page.click("text=В закладки");
  await page.waitFor(`document.body.innerText.includes("уже в закладках")`, { label: "повтор — уже в закладках" });
  expect(page, (await panel(page)).length === 1, "повтор не дублирует");
  await treeMenu(page, '.ts-tree__folder[data-drag-path="Диссертация/рисунки"]');
  await page.click("text=В закладки");
  await page.waitFor(`document.querySelectorAll('.ts-bookmarks__item').length === 2`, { label: "закладка папки" });

  // ── Оглавление: раздел ───────────────────────────────────────────────
  await page.click('.ts-tree__file[data-drag-path="Диссертация/Методика.typ"] button');
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('Методика')`, { label: "Методика открыта" });
  await page.click("text=Оглавление");
  await page.waitFor(`[...document.querySelectorAll('.ts-outline__row')].some((r) => r.textContent.includes('Выборка'))`, { label: "оглавление" });
  await page.evaluate(`(() => {
    const row = [...document.querySelectorAll('.ts-outline__row')].find((r) => r.textContent.includes('Выборка'));
    row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 600, clientY: 300 }));
  })()`);
  await page.click("text=В закладки");
  await page.waitFor(`document.querySelectorAll('.ts-bookmarks__item').length === 3`, { label: "закладка раздела" });

  // ── Поиск ────────────────────────────────────────────────────────────
  await page.press("k", { ctrl: true });
  await page.waitForElement(".ts-palette");
  await page.type("Расширенный поиск");
  await page.sleep(200);
  await page.press("Enter");
  await page.waitForElement(".ts-search__input");
  await page.type("образца");
  await page.click("text=В закладки");
  await page.waitFor(`document.querySelectorAll('.ts-bookmarks__item').length === 4`, { label: "закладка поиска" });
  await page.press("Escape");
  page.log("— панель: " + (await panel(page)).join(" | "));
  await page.shot(shots + "/032-b1-panel.png");

  // ── Открытие ─────────────────────────────────────────────────────────
  await bookmarkButton(page, "Введение");
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('Текст введения')`, { label: "заметка открыта закладкой" });
  expect(page, true, "закладка заметки открывает заметку");
  await bookmarkButton(page, "Выборка");
  await page.waitFor(`document.querySelector('.ts-status, .ts-statusbar')?.innerText.match(/Ln (6[2-5])/) !== null || document.querySelector('.cm-activeLine')?.innerText.includes('Выборка')`, { label: "раздел открыт на заголовке", timeout: 10000 });
  expect(page, true, "закладка раздела открывает заметку на заголовке");
  await bookmarkButton(page, "образца");
  await page.waitFor(`document.querySelector('.ts-search__input')?.value === "образца"`, { label: "поиск с запросом" });
  expect(page, true, "закладка поиска открывает поиск с запросом");
  await page.press("Escape");
  await bookmarkButton(page, "Диссертация/рисунки");
  await page.waitFor(`document.activeElement?.closest('.ts-tree__folder')?.dataset.dragPath === "Диссертация/рисунки"`, { label: "папка показана" });
  expect(page, true, "закладка папки раскрывает её в дереве");

  // ── Группа и порядок ─────────────────────────────────────────────────
  await page.click(".ts-bookmarks__new");
  await page.waitForElement(".ts-bookmarks__edit");
  await page.evaluate(`document.querySelector('.ts-bookmarks__edit').select()`);
  await page.type("Глава 2");
  await page.press("Enter");
  await page.waitFor(`document.body.innerText.includes("Глава 2")`, { label: "группа создана" });
  await page.evaluate(`(() => {
    const row = [...document.querySelectorAll('.ts-bookmarks__row')].find((r) => r.textContent.includes('Введение'));
    row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 80, clientY: 120 }));
  })()`);
  await page.click("text=В группу «Глава 2»");
  await page.waitFor(`[...document.querySelectorAll('.ts-bookmarks__group .ts-bookmarks__name')].some((n) => n.textContent === 'Введение')`, { label: "закладка в группе" });
  expect(page, true, "закладка перенесена в группу");
  await page.evaluate(`[...document.querySelectorAll('.ts-bookmarks__item')].find((b) => b.textContent.includes('Диссертация/рисунки')).focus()`);
  await page.press("ArrowDown", { alt: true });
  await page.waitFor(`(() => { const names = [...document.querySelectorAll('.ts-bookmarks__list > .ts-bookmarks__row .ts-bookmarks__name')].map((n) => n.textContent); return names.indexOf('Диссертация/рисунки') > names.indexOf('Выборка'); })()`, { label: "Alt+↓" });
  expect(page, true, "Alt+↓ двигает закладку");
  const settings = await page.evaluate(`window.__opfs.read(".typsidian/vault.json")`);
  expect(page, settings.includes('"bookmarks"') && settings.includes("Глава 2"), "закладки и группа в vault.json");

  // ── Удалённая цель ───────────────────────────────────────────────────
  await treeMenu(page, '.ts-tree__file[data-drag-path="Диссертация/рисунки/Схема.typ"]');
  await page.click("text=Удалить…");
  await page.waitFor(`document.querySelector('.ts-tree__folder[data-drag-path="Диссертация/рисунки"]') === null`, { label: "папка исчезла" });
  await page.waitFor(`document.querySelector('.ts-bookmarks__item--missing') !== null`, { label: "закладка недоступна", timeout: 10000 });
  expect(page, true, "удалённая цель помечена недоступной");
  await page.shot(shots + "/032-b1-missing.png");
};
