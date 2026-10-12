// Колонка заметки «Связи» (фича 013, quickstart.md §3): vault макета в
// приватном хранилище браузера → обратные ссылки → обновление → упоминания и
// «Связать» → устаревшее → перезагрузка → узкое окно (шторка).
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/note-links.mjs --opfs-vault --shot /tmp/links-end.png
//
// Снимки: каталог из переменной LINKS_SHOTS (по умолчанию /tmp).
const shots = process.env.LINKS_SHOTS ?? "/tmp";

const OPFS = `
  window.__opfs = {
    async dir(path, create) {
      let dir = await (await navigator.storage.getDirectory()).getDirectoryHandle("vault", { create: true });
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
      const at = path.lastIndexOf("/");
      const dir = await this.dir(at === -1 ? "" : path.slice(0, at), false);
      return (await (await dir.getFileHandle(path.slice(at + 1))).getFile()).text();
    },
  };
`;

const expect = (page, condition, what) => {
  if (!condition) throw new Error("не сошлось: " + what);
  page.log("  ok: " + what);
};

export default async (page) => {
  await page.evaluate(OPFS + "true");
  await page.evaluate(`(async () => {
    const o = window.__opfs;
    await o.write("Диссертация/Проект Альфа.typ", "= Проект Альфа\\n\\nЦель — измерить теплоперенос.\\n");
    await o.write("Диссертация/Методология.typ", "= Методология\\n\\nПостановка взята из [[Проект Альфа]] без изменений.\\nКак и в [[Проект Альфа]], шаг сетки 0,5 мм.\\nСроки — см. [[Проект Альфа]].\\n");
    await o.write("Диссертация/Результаты.typ", "= Результаты\\n\\nЭто согласуется с выводами Проект Альфа (гл. 2).\\n");
    await o.write("Заметки/Лекция_12.typ", "= Лекция 12\\n\\nПример с семинара: [[/Диссертация/Проект Альфа]]\\n");
    await o.write("Заметки/Идеи_экспериментов.typ", "= Идеи\\n\\nПроверить на данных, что остались от проект альфа.\\n");
    await o.write("Яд.typ", "= Яд\\n");
  })()`);

  await page.click("text=Открыть vault");
  await page.waitForElement("text=Проект Альфа");

  // Шаг 1 (US1): колонка по Ctrl+Shift+L, группы и числа как на макете 1a.
  await page.click("text=Проект Альфа");
  await page.sleep(500);
  await page.press("L", { ctrl: true, shift: true });
  await page.waitForElement(".ts-notepanel .ts-links__group");
  await page.sleep(500);
  const columnText = await page.text(".ts-notepanel");
  page.log("— колонка:\n" + columnText);
  expect(page, columnText.includes("Методология") && columnText.includes("Лекция_12"), "источники в колонке");
  expect(page, (await page.evaluate("document.querySelectorAll('.ts-links__mark').length")) === 4, "четыре выделенные ссылки");
  await page.shot(shots + "/1a-column.png");

  await page.click(".ts-links__row .ts-links__snip[aria-label*='Методология']");
  await page.sleep(500);
  expect(page, (await page.text(".ts-crumb")).includes("Методология"), "клик открыл Методологию");
  await page.shot(shots + "/1a-after-reveal.png");
  await page.click("text=Проект Альфа");
  await page.sleep(500);

  // Шаг 2 (US2): ссылка, дописанная снаружи, появляется сама.
  await page.evaluate(`window.__opfs.write("Диссертация/Результаты.typ", "= Результаты\\n\\nЭто согласуется с [[Проект Альфа]].\\n")`);
  await page.waitFor("document.querySelector('.ts-notepanel')?.innerText.includes('Результаты')", { timeout: 8000 });
  expect(page, true, "Результаты появились без действий пользователя");

  // Шаг 3 (US3): упоминания и «Связать». Сначала вернуть упоминание в Результаты.
  await page.evaluate(`window.__opfs.write("Диссертация/Результаты.typ", "= Результаты\\n\\nЭто согласуется с выводами Проект Альфа (гл. 2).\\n")`);
  await page.waitFor("!document.querySelector('.ts-notepanel')?.innerText.includes('Результаты')", { timeout: 8000 });
  await page.click("text=Упоминания");
  await page.waitForElement(".ts-links__link");
  await page.sleep(800);
  page.log("— упоминания:\n" + (await page.text(".ts-notepanel")));
  await page.shot(shots + "/1a-mentions.png");
  const before = await page.evaluate('window.__opfs.read("Диссертация/Результаты.typ")');
  await page.click(".ts-links__link[aria-label*='Результаты']");
  await page.waitFor("!document.querySelector('.ts-links__link[aria-label*=\"Результаты\"]')", { timeout: 8000 });
  const after = await page.evaluate('window.__opfs.read("Диссертация/Результаты.typ")');
  page.log("— файл после «Связать»:\n" + after);
  expect(page, after === before.replace("выводами Проект Альфа", "выводами [[Проект Альфа]]"), "в файле изменено только упоминание");
  await page.waitFor("document.querySelector('.ts-notepanel')?.innerText.includes('Результаты')", { timeout: 8000 });
  expect(page, true, "строка переехала в обратные ссылки");

  // Шаг 4: упоминание сдвинулось, список не успел обновиться → «устарело».
  await page.evaluate(`(async () => {
    await window.__opfs.write("Заметки/Идеи_экспериментов.typ", "= Идеи\\n\\nДобавлено.\\nПроверить на данных, что остались от проект альфа.\\n");
    document.querySelector(".ts-links__link[aria-label*='Идеи']").click();
  })()`);
  await page.sleep(700);
  const staleText = await page.text(".ts-notepanel");
  page.log("— после устаревшего:\n" + staleText);
  const idea = await page.evaluate('window.__opfs.read("Заметки/Идеи_экспериментов.typ")');
  expect(page, !idea.includes("[["), "файл не тронут при устаревшем месте");
  await page.shot(shots + "/1b-stale.png");

  // Шаг 5 (FR-021): перезагрузка — колонка показана, упоминания раскрыты.
  await page.sleep(900);
  await page.evaluate("setTimeout(() => location.reload(), 50); true");
  await page.sleep(1500);
  await page.evaluate(OPFS + "true");
  // Последний vault может открыться сам; иначе — обычная кнопка.
  await page.waitFor("document.body.innerText.includes('Проект Альфа') || document.body.innerText.includes('Открыть vault')");
  if (!(await page.evaluate("document.body.innerText.includes('Проект Альфа')"))) {
    await page.click("text=Открыть vault");
  }
  await page.waitForElement("text=Проект Альфа");
  await page.click("text=Проект Альфа");
  await page.waitForElement(".ts-notepanel .ts-links__group", { timeout: 10000 });
  expect(page, (await page.evaluate("document.querySelector('.ts-links__sechead[aria-expanded=true]') !== null")), "раздел раскрыт после перезагрузки");
  expect(page, (await page.text(".ts-notepanel")).toLowerCase().includes("упоминания"), "колонка показана после перезагрузки");

  // Короткое название (1b).
  await page.click("text=Яд");
  await page.sleep(1500);
  page.log("— короткое название:\n" + (await page.text(".ts-notepanel")));
  await page.shot(shots + "/1b-short-title.png");
  await page.click("text=Проект Альфа");

  // Шаг 6 (1c): узкое окно — колонки нет, шторка по кнопке.
  await page.click("text=Скрыть дерево файлов");
  await page.viewport(360, 720);
  await page.sleep(600);
  expect(page, (await page.evaluate("document.querySelector('.ts-notepanel') === null")), "на узком окне колонки нет");
  await page.click("text=Связи");
  await page.waitForElement(".ts-notepanel--sheet .ts-links__group");
  await page.sleep(500);
  await page.shot(shots + "/1c-sheet.png");
  await page.click(".ts-notepanel--sheet .ts-links__snip");
  await page.sleep(600);
  expect(page, (await page.evaluate("document.querySelector('.ts-notepanel') === null")), "переход закрыл шторку");
  await page.shot(shots + "/1c-after.png");
};
