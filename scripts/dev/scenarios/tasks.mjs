// Задачи из заметок (фича 033, quickstart.md): окно «Задачи» со всего vault,
// отметка закрытой заметки меняет одну строку файла, открытой — через
// редактор; переход на строку; фильтры; «Переключить задачу» в редакторе.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/tasks.mjs --opfs-vault
//
// Снимки экрана — в каталог из TASK_SHOTS (по умолчанию /tmp).
const shots = process.env.TASK_SHOTS ?? "/tmp";

const pad = (n) => String(n).padStart(2, "0");
const iso = (d) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
const shift = (days) => {
  const d = new Date();
  d.setDate(d.getDate() + days);
  return iso(d);
};

const INTRO = `= Введение\n\n- [ ] Сверить цель 📅 ${shift(-3)}\n- [x] Собрать литературу\n- [ ] Дописать актуальность срок: ${shift(0)}\n  - [ ] Найти три обзора\n- обычный пункт\n\n\`\`\`\n- [ ] не задача в коде\n\`\`\`\n`;
const METHOD = `= Методика\n\n+ [ ] Пересчитать погрешность due: ${shift(4)}\n+ [x] Откалибровать термопары\n`;
const DIARY = `= Дневник\n\n- [ ] Отправить главу\n`;

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

const openTasks = async (page) => {
  await page.press("k", { ctrl: true });
  await page.waitForElement(".ts-palette");
  await page.type("Задачи");
  await page.sleep(200);
  await page.press("Enter");
  await page.waitForElement(".ts-tasks");
};

const rowTexts = (page) =>
  page.evaluate(`[...document.querySelectorAll('.ts-tasks__row .ts-tasks__text')].map((b) => b.firstChild.textContent.trim())`);

const checkRow = (page, text) =>
  page.evaluate(`[...document.querySelectorAll('.ts-tasks__row')].find((r) => r.querySelector('.ts-tasks__text').firstChild.textContent.trim() === ${JSON.stringify(text)}).querySelector('input').click()`);

export default async (page) => {
  await page.evaluate(OPFS + "true");
  await page.evaluate(`window.__opfs.write("Диссертация/Введение.typ", ${JSON.stringify(INTRO)})`);
  await page.evaluate(`window.__opfs.write("Диссертация/Методика.typ", ${JSON.stringify(METHOD)})`);
  await page.evaluate(`window.__opfs.write("дневник/сегодня.typ", ${JSON.stringify(DIARY)})`);
  await page.click("text=Открыть vault");
  await page.waitForElement("text=Введение", { timeout: 30000 });
  await page.sleep(1500);
  await page.click(".ts-tree");

  // ── Все задачи ───────────────────────────────────────────────────────
  await openTasks(page);
  await page.waitFor(`document.querySelectorAll('.ts-tasks__row').length === 5`, { label: "5 невыполненных", timeout: 15000 });
  const texts = await rowTexts(page);
  page.log("— задачи: " + texts.join(" | "));
  expect(page, !texts.some((t) => t.includes("в коде")), "задача из кода не считается");
  expect(page, (await page.evaluate(`document.querySelector('.ts-tasks__row--sub') !== null`)), "вложенная — с отступом");
  await page.shot(shots + "/033-t1-tasks.png");

  // ── Отметка закрытой заметки ─────────────────────────────────────────
  await checkRow(page, "Отправить главу");
  await page.waitFor(`window.__opfs.read("дневник/сегодня.typ").then((t) => t?.includes("- [x] Отправить главу"))`, { label: "отметка в файле" });
  const diary = await page.evaluate(`window.__opfs.read("дневник/сегодня.typ")`);
  expect(page, diary === DIARY.replace("- [ ] Отправить главу", "- [x] Отправить главу"), "изменена ровно одна строка");

  // ── Фильтры ──────────────────────────────────────────────────────────
  await page.click("text=просрочено");
  await page.waitFor(`document.querySelectorAll('.ts-tasks__row').length === 1`, { label: "просрочено" });
  expect(page, (await rowTexts(page))[0] === "Сверить цель", "фильтр «просрочено»");
  await page.click("text=любой");
  await page.click("text=по сроку");
  await page.waitFor(`document.body.innerText.includes("Просрочено") && document.body.innerText.includes("Без срока")`, { label: "группы по сроку" });
  await page.shot(shots + "/033-t2-by-due.png");
  await page.click("text=по заметкам");

  // ── Переход на строку ────────────────────────────────────────────────
  await page.evaluate(`[...document.querySelectorAll('.ts-tasks__text')].find((b) => b.textContent.includes('Пересчитать')).click()`);
  await page.waitFor(`document.querySelector('.cm-activeLine')?.innerText.includes('Пересчитать погрешность')`, { label: "заметка на строке задачи" });
  expect(page, true, "щелчок открывает заметку на строке задачи");

  // ── Отметка открытой заметки — через редактор ────────────────────────
  await openTasks(page);
  await page.waitFor(`document.querySelectorAll('.ts-tasks__row').length > 0`, { label: "список" });
  await checkRow(page, "Пересчитать погрешность");
  await page.press("Escape");
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('+ [x] Пересчитать')`, { label: "правка в редакторе" });
  await page.click(".cm-content");
  await page.press("z", { ctrl: true });
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('+ [ ] Пересчитать')`, { label: "Ctrl+Z" });
  expect(page, true, "отметка открытой заметки — правка редактора, Ctrl+Z снимает");

  // ── «Переключить задачу» ─────────────────────────────────────────────
  await page.press("End", { ctrl: true });
  await page.type("\n- новый пункт");
  await page.press("k", { ctrl: true });
  await page.waitForElement(".ts-palette");
  await page.type("Переключить задачу");
  await page.sleep(200);
  await page.press("Enter");
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('- [ ] новый пункт')`, { label: "пункт стал задачей" });
  expect(page, true, "«Переключить задачу» делает пункт задачей");
};
