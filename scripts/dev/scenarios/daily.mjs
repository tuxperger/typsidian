// Быстрый захват и ежедневные заметки (фича 022, quickstart.md §2): «Сегодня» создаёт
// дневник/<дата>.typ с кареткой в конце и повтор открывает ту же заметку; ‹ › ходят
// через пропущенный день; «Во входящие» дописывает запись в конец входящие.typ, не
// теряя несохранённых правок открытой заметки; смена вида названия в настройках D6
// даёт новое имя, старые заметки остаются на месте.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/daily.mjs --opfs-vault --shot /tmp/daily.png
//
// Снимки: каталог из переменной DAILY_SHOTS (по умолчанию /tmp).
const shots = process.env.DAILY_SHOTS ?? "/tmp";

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
    async list(path) {
      try {
        const names = [];
        for await (const [name] of (await this.dir(path, false)).entries()) names.push(name);
        return names.sort();
      } catch { return []; }
    },
  };
`;

const expect = (page, condition, what) => {
  if (!condition) throw new Error("не сошлось: " + what);
  page.log("  ok: " + what);
};

const pad = (n) => String(n).padStart(2, "0");
const iso = (d) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
const dotted = (d) => `${pad(d.getDate())}.${pad(d.getMonth() + 1)}.${d.getFullYear()}`;
const daysAgo = (n) => {
  const d = new Date();
  d.setDate(d.getDate() - n);
  return d;
};

const editorText = (page) => page.evaluate(`document.querySelector('.cm-content')?.innerText ?? ''`);
const read = (page, path) => page.evaluate(`window.__opfs.read(${JSON.stringify(path)})`);
const list = (page, path) => page.evaluate(`window.__opfs.list(${JSON.stringify(path)})`);

// Каретка в последней строке редактора и фокус в нём.
const caretAtEnd = (page) =>
  page.evaluate(`(() => {
    const content = document.querySelector('.cm-content');
    if (document.activeElement !== content) return false;
    const node = getSelection().anchorNode;
    const lines = [...content.querySelectorAll('.cm-line')];
    const last = lines[lines.length - 1];
    return Boolean(node && last && last.contains(node));
  })()`);

const openPalette = async (page, query) => {
  await page.press("k", { ctrl: true });
  await page.waitForElement(".ts-palette");
  await page.type(query);
  await page.sleep(200);
  await page.press("Enter");
};

export default async (page) => {
  const today = new Date();
  const past = daysAgo(3);
  await page.evaluate(OPFS + "true");
  // Папки `дневник/` в начале нет: «Сегодня» в свежем vault обязано завести её само
  // (quickstart.md §2, 022 T021). Старая заметка для ‹ › кладётся позже, снаружи.
  await page.evaluate(`window.__opfs.write("входящие.typ", "= Входящие\\n")`);
  await page.click("text=Открыть vault");
  await page.waitForElement("text=входящие");
  await page.sleep(1500);

  // ── US2: «Сегодня» ────────────────────────────────────────────────────
  await page.click(".ts-daily__today");
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes(${JSON.stringify(iso(today))})`, { label: "открыта заметка на сегодня" });
  expect(page, (await read(page, `дневник/${iso(today)}.typ`)) !== null, `создан дневник/${iso(today)}.typ`);
  await page.sleep(300);
  expect(page, await caretAtEnd(page), "каретка в конце, фокус в редакторе");
  await page.click(".ts-daily__today");
  await page.sleep(700);
  const files = await list(page, "дневник");
  expect(page, files.length === 1, "повторное «Сегодня» не создаёт вторую заметку: " + files.join(", "));
  await page.waitForElement(".ts-daily__date");
  await page.shot(shots + "/daily-d4.png");

  // ── ‹ › через пропущенный день ───────────────────────────────────────
  // Заметка трёхдневной давности появляется снаружи — vault узнаёт о ней опросом.
  await page.evaluate(`window.__opfs.write("дневник/${iso(past)}.typ", "= ${iso(past)}\\nСтарая запись.\\n")`);
  await page.waitFor(`document.body.innerText.includes(${JSON.stringify(iso(past))})`, { timeout: 8000, label: "старая заметка видна в дереве" });
  // Соседи считаются при открытии ежедневной заметки: переоткрыть сегодняшнюю.
  await page.click(".ts-daily__today");
  await page.sleep(700);
  await page.click("[aria-label='Предыдущая ежедневная заметка']");
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('Старая запись')`, { label: "‹ открыл заметку трёхдневной давности" });
  expect(page, true, "‹ перешла через пропущенные дни");
  await page.click("[aria-label='Следующая ежедневная заметка']");
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes(${JSON.stringify(iso(today))})`, { label: "› вернул сегодняшнюю" });
  expect(page, true, "› вернул к сегодняшней");

  // ── US1: «Во входящие», несохранённые правки открытой заметки ────────
  await page.click("text=входящие");
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('Входящие')`, { label: "открыты входящие" });
  await page.click(".cm-content");
  await page.press("End", { ctrl: true });
  await page.type("\nнесохранённая правка");
  await openPalette(page, "Во входящие");
  await page.waitForElement(".ts-capture");
  await page.shot(shots + "/capture-d1.png");
  await page.type("записать на потом");
  await page.click(".ts-capture__note");
  await page.type("к докладу");
  await page.shot(shots + "/capture-d1-filled.png");
  await page.click(".ts-capture__primary");
  await page.waitFor(`!document.querySelector('.ts-capture')`, { label: "лист закрыт после записи" });
  await page.sleep(1200);
  const inbox = await editorText(page);
  expect(page, inbox.includes("несохранённая правка") && inbox.includes("записать на потом") && inbox.includes("к докладу"), "в открытой заметке и правка, и захват");
  const onDisk = await read(page, "входящие.typ");
  expect(page, onDisk.includes("несохранённая правка") && onDisk.includes("записать на потом"), "оба на диске после автосохранения");
  page.log("входящие.typ:\n" + onDisk);

  // ── Захват в закрытую заметку: «Сегодня» как назначение ──────────────
  await openPalette(page, "Во входящие");
  await page.waitForElement(".ts-capture");
  await page.type("мысль на день");
  await page.click(".ts-capture__dest button:nth-child(2)");
  await page.sleep(300);
  await page.shot(shots + "/capture-daily.png");
  await page.click(".ts-capture__primary");
  await page.waitFor(`!document.querySelector('.ts-capture')`, { label: "лист закрыт" });
  await page.sleep(800);
  const todayText = await read(page, `дневник/${iso(today)}.typ`);
  expect(page, todayText.includes("мысль на день"), "захват дописан в ежедневную заметку");

  // ── US3: настройки D6 ────────────────────────────────────────────────
  await page.click("text=Настройки");
  await page.waitForElement(".ts-settings");
  await page.click("text=Захват и дневник");
  await page.waitForElement(".ts-capture-settings");
  await page.sleep(500);
  await page.shot(shots + "/capture-d6.png");
  const hint = await page.text(".ts-capture-settings");
  expect(page, hint.includes(`дневник/${iso(today)}.typ`), "подсказка «Сегодня» с текущим видом названия");
  await page.evaluate(`(() => {
    const select = [...document.querySelectorAll('.ts-capture-settings select')].find((s) => [...s.options].some((o) => o.value === 'DD.MM.YYYY'));
    const set = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set;
    set.call(select, 'DD.MM.YYYY');
    select.dispatchEvent(new Event('change', { bubbles: true }));
  })()`);
  await page.waitFor(`document.querySelector('.ts-capture-settings')?.innerText.includes(${JSON.stringify(`дневник/${dotted(today)}.typ`)})`, { label: "образец названия пересчитан" });
  await page.shot(shots + "/capture-d6-dotted.png");
  expect(page, true, "вид названия сменился");
  await page.click("text=Закрыть");
  await page.click(".ts-daily__today");
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes(${JSON.stringify(dotted(today))})`, { label: "новая заметка с новым названием" });
  const after = await list(page, "дневник");
  expect(page, after.includes(`${dotted(today)}.typ`) && after.includes(`${iso(today)}.typ`), "старые заметки на месте, новая создана: " + after.join(", "));

  // ── Узкий вид ────────────────────────────────────────────────────────
  await page.viewport(390, 760);
  await page.sleep(500);
  await page.shot(shots + "/daily-narrow.png");
  await openPalette(page, "Во входящие");
  await page.waitForElement(".ts-capture");
  await page.shot(shots + "/capture-narrow.png");
};
