// Проверка орфографии (фича 030, quickstart.md): подчёркнуты только опечатки
// текста, а не разметка; исправление из меню и отмена Ctrl+Z; «Добавить в
// словарь» снимает подчёркивание и пишет `.typsidian/dictionary.txt`; раздел
// настроек «Орфография»: словарь списком, «убрать», языки, выключатель.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/spellcheck.mjs --opfs-vault
//
// Снимки экрана — в каталог из SPELL_SHOTS (по умолчанию /tmp).
const shots = process.env.SPELL_SHOTS ?? "/tmp";

const NOTE = "Диссертация/Введение.typ";
const TEXT = `#set text(lang: "ru")

= Введение <введение-првоерка>

Цель работы — првоерка модели при $T > 300 "резултатах"$, проект Typsidian.
Методика описана в [[Методика првоерка]] и опирается на @првоерка.
Стенд \`ТС-2М првоерка\` по ГОСТ 7076–99, всё — в резултатах.

- *Thermal conductivty* measured with a guarded hot plate.
`;

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

const underlined = (page) =>
  page.evaluate(`[...document.querySelectorAll('.cm-content .ts-misspelled')].map((el) => el.textContent)`);

const waitUnderlined = (page, words, label) =>
  page.waitFor(
    `JSON.stringify([...document.querySelectorAll('.cm-content .ts-misspelled')].map((el) => el.textContent)) === ${JSON.stringify(JSON.stringify(words))}`,
    { label, timeout: 15000 },
  );

/** Правый щелчок по подчёркнутому слову. */
const rightClickWord = (page, word) =>
  page.evaluate(`(() => {
    const el = [...document.querySelectorAll('.cm-content .ts-misspelled')].find((e) => e.textContent === ${JSON.stringify(word)});
    const box = el.getBoundingClientRect();
    el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: box.left + box.width / 2, clientY: box.top + box.height / 2 }));
    return true;
  })()`);

export default async (page) => {
  await page.evaluate(OPFS + "true");
  await page.evaluate(`window.__opfs.write(${JSON.stringify(NOTE)}, ${JSON.stringify(TEXT)})`);
  await page.click("text=Открыть vault");
  await page.waitForElement("text=Введение", { timeout: 30000 });
  await page.click("text=Введение");
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('Цель работы')`, { label: "заметка открыта" });

  // ── US1: только текст ────────────────────────────────────────────────
  await waitUnderlined(page, ["првоерка", "Typsidian", "резултатах", "conductivty"], "подчёркнуты опечатки текста");
  expect(page, true, "подчёркнуты ровно опечатки текста: " + (await underlined(page)).join(", "));
  await page.shot(shots + "/030-o1-underlined.png");

  // Исправление из меню.
  await rightClickWord(page, "првоерка");
  await page.waitForElement("text=Исправить «првоерка»", { timeout: 15000 });
  await page.shot(shots + "/030-o1-menu.png");
  await page.click("text=проверка");
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('Цель работы — проверка')`, { label: "слово исправлено" });
  expect(page, true, "исправление из меню");
  await page.press("z", { ctrl: true });
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('Цель работы — првоерка')`, { label: "Ctrl+Z вернул" });
  expect(page, true, "Ctrl+Z отменяет исправление");

  // ── US2: словарь vault ───────────────────────────────────────────────
  await waitUnderlined(page, ["првоерка", "Typsidian", "резултатах", "conductivty"], "снова подчёркнуто");
  await rightClickWord(page, "Typsidian");
  await page.waitForElement("text=Добавить в словарь", { timeout: 15000 });
  await page.click("text=Добавить в словарь");
  await waitUnderlined(page, ["првоерка", "резултатах", "conductivty"], "Typsidian не подчёркнуто");
  const dictionary = await page.evaluate(`window.__opfs.read(".typsidian/dictionary.txt")`);
  expect(page, dictionary === "Typsidian\n", "слово в .typsidian/dictionary.txt");

  // ── US3: настройки ───────────────────────────────────────────────────
  await page.click("text=Настройки");
  await page.click("text=Орфография");
  await page.waitFor(`document.body.innerText.includes("Словарь vault · 1 слово")`, { label: "словарь в настройках" });
  await page.shot(shots + "/030-o3-settings.png");
  await page.evaluate(`document.querySelector('[aria-label="Убрать «Typsidian» из словаря"]').click()`);
  await page.waitFor(`document.body.innerText.includes("Словарь vault · пусто")`, { label: "слово убрано" });
  expect(page, (await page.evaluate(`window.__opfs.read(".typsidian/dictionary.txt")`)) === "", "файл словаря пуст");

  await page.evaluate(`document.querySelector('#ts-spellcheck-en').click()`);
  await page.waitFor(`window.__opfs.read(".typsidian/vault.json").then((t) => t?.includes('"ru"') && !t.includes('"en"'))`, { label: "языки в vault.json" });
  await page.click("text=Закрыть");
  await waitUnderlined(page, ["првоерка", "резултатах"], "без English латиница не проверяется");
  expect(page, true, "снятый язык не проверяется: Typsidian и conductivty без подчёркивания");

  await page.click("text=Настройки");
  await page.click("text=Орфография");
  await page.evaluate(`document.querySelector('#ts-spellcheck-enabled').click()`);
  await page.waitFor(`window.__opfs.read(".typsidian/vault.json").then((t) => t?.includes('"enabled": false'))`, { label: "выключено в vault.json" });
  await page.click("text=Закрыть");
  await waitUnderlined(page, [], "выключено — подчёркиваний нет");
  expect(page, true, "выключенная проверка ничего не подчёркивает");

  // Телефон: меню долгим нажатием — то же меню редактора.
  await page.click("text=Настройки");
  await page.click("text=Орфография");
  await page.evaluate(`document.querySelector('#ts-spellcheck-enabled').click()`);
  await page.click("text=Закрыть");
  await page.viewport(390, 760);
  await waitUnderlined(page, ["првоерка", "резултатах"], "снова включено");
  await rightClickWord(page, "резултатах");
  await page.waitForElement("text=Исправить «резултатах»", { timeout: 15000 });
  await page.shot(shots + "/030-o2-phone.png");
  await page.press("Escape");
  await page.viewport(1440, 900);
};
