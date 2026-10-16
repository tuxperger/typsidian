// Свойства заметок и выборки (фича 020, quickstart.md §2, §5): тег и «статус» через
// колонку «Свойства» → запись в тексте; клик по тегу → поиск с условием → заметки;
// «Выборка» в меню вставки → блок с таблицей; неоднозначная запись → плашка B2.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/properties.mjs --opfs-vault --shot /tmp/props.png
//
// Снимки: каталог из переменной PROPS_SHOTS (по умолчанию /tmp).
const shots = process.env.PROPS_SHOTS ?? "/tmp";

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
  };
`;

const record = (extra) =>
  `#metadata((tags: ("проект",), статус: "в работе"${extra})) <properties>\n`;

const expect = (page, condition, what) => {
  if (!condition) throw new Error("не сошлось: " + what);
  page.log("  ok: " + what);
};

const openNote = async (page, name) => {
  await page.evaluate(`(() => {
    const item = [...document.querySelectorAll('.ts-tree__file > button:first-child')]
      .find((button) => button.querySelector('.ts-tree__name')?.textContent === ${JSON.stringify(name)});
    item.setAttribute('data-pick', '1');
  })()`);
  await page.click("[data-pick='1']");
  await page.sleep(400);
  await page.evaluate(`document.querySelector('[data-pick]')?.removeAttribute('data-pick')`);
};

const editorText = (page) => page.evaluate(`document.querySelector('.cm-content')?.innerText ?? ''`);

export default async (page) => {
  await page.evaluate(OPFS + "true");
  await page.evaluate(`window.__opfs.write("проекты/первая.typ", "= Первая\\nТекст.\\n")`);
  await page.evaluate(`window.__opfs.write("проекты/вторая.typ", ${JSON.stringify(record("") + "= Вторая\nТекст.\n")})`);
  await page.evaluate(`window.__opfs.write("проекты/третья.typ", ${JSON.stringify(record("") + "= Третья\nТекст.\n")})`);
  await page.evaluate(`window.__opfs.write("сложная.typ", ${JSON.stringify('#let базовые = ("а",)\n#metadata((tags: базовые + ("срочно",), статус: "в работе")) <properties>\n= Сложная\n')})`);
  await page.click("text=Открыть vault");
  await page.waitForElement("text=первая");
  await page.sleep(2500);

  // ── US1: свойства через колонку ────────────────────────────────────────
  await openNote(page, "первая");
  await page.click("text=Панель заметки");
  await page.waitForElement(".ts-notepanel");
  await page.click("text=Свойства");
  await page.waitForElement(".ts-props");
  await page.shot(shots + "/props-b1-empty.png");

  await page.click("[aria-label='Добавить тег']");
  await page.type("проект");
  await page.press("Enter");
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('tags')`, { label: "запись tags в тексте" });
  expect(page, true, "тег записан в текст");

  await page.click("text=+ Добавить свойство");
  await page.type("статус");
  await page.click("text=Добавить");
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('статус')`, { label: "запись статус в тексте" });
  await page.waitForElement("[aria-label='Значение статус']");
  await page.click("[aria-label='Значение статус']");
  await page.type("в работе");
  await page.press("Enter");
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('в работе')`, { label: "значение статуса в тексте" });
  page.log("текст заметки:\n" + (await editorText(page)));
  await page.sleep(1500);
  const noGlyph = await page.evaluate(`!document.querySelector('.ts-preview')?.innerText.includes('проект')`);
  expect(page, noGlyph, "запись свойств в предпросмотре не выводится");
  await page.shot(shots + "/props-b1.png");

  // ── US2: клик по тегу → поиск с условием ───────────────────────────────
  await page.click("[title='Заметки с тегом проект']");
  await page.waitForElement(".ts-search-cond");
  await page.waitFor(`document.querySelectorAll('.ts-search__hit').length === 3`, { label: "три заметки с тегом" });
  expect(page, true, "по тегу найдено три заметки");
  await page.shot(shots + "/props-b3.png");

  // ── US3: Mod+Enter → форма выборки ────────────────────────────────────
  await page.press("Enter", { ctrl: true });
  await page.waitForElement(".ts-dialog--params");
  await page.waitFor(`document.body.innerText.includes('Сейчас подходят 3')`, { label: "счётчик «подходят 3»" });
  await page.shot(shots + "/props-b4.png");
  await page.click(".ts-dialog--params .ts-btn--accent");
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('#select')`, { label: "блок #select вставлен" });
  page.log("текст после вставки:\n" + (await editorText(page)));
  expect(page, (await editorText(page)).includes("select.typ"), "добавлен #import библиотеки");
  await page.waitFor(`document.querySelectorAll('.ts-preview__page').length > 0 && document.querySelector('.ts-preview')?.innerText.includes('вторая')`, { timeout: 15000, label: "таблица выборки в предпросмотре" });
  await page.shot(shots + "/props-b5.png");
  // 020 T026: несохранённые правки после вставки — не конфликт, пометки на вкладке нет.
  expect(page, await page.evaluate(`!document.querySelector(".ts-tab__tag--warn")`), "на вкладке нет пометки «конфликт»");
  // SC-002: тег четвёртой заметке снаружи - строка в таблице без правки блока.
  const t0 = Date.now();
  await page.evaluate(`window.__opfs.write("проекты/четвёртая.typ", ${JSON.stringify(record("") + "= Четвёртая\\n")})`);
  await page.waitFor(`document.querySelector('.ts-preview')?.innerText.includes('четвёртая')`, { timeout: 10000, label: "четвёртая строка в таблице" });
  page.log(`замер: строка появилась через ${Date.now() - t0} мс (SC-002: <= 2000)`);

  // ── Неоднозначная запись (B2) ─────────────────────────────────────────
  await openNote(page, "сложная");
  await page.waitForElement(".ts-props__warn");
  await page.shot(shots + "/props-b2.png");
  const before = await editorText(page);
  expect(page, !(await page.evaluate(`!!document.querySelector('.ts-props [aria-label="Добавить тег"]')`)), "форма выключена");
  expect(page, before.includes("базовые"), "файл не менялся");
};
