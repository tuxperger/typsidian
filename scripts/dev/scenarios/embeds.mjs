// Встраивание заметок (фича 031, quickstart.md): команда «Встроить заметку»
// вставляет `#import` и `#embed("")` и открывает дополнение; раздел встраивается
// в предпросмотр без соседних; правка источника обновляет предпросмотр; битые
// встраивания подчёркнуты и показаны плашкой; цикл обрывается плашкой.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/embeds.mjs --opfs-vault
//
// Снимки экрана — в каталог из EMBED_SHOTS (по умолчанию /tmp).
const shots = process.env.EMBED_SHOTS ?? "/tmp";

const METHOD = `#set text(lang: "ru")
= Методика

Вводная часть методики.

== Выборка

Отобрано двадцать четыре образца из трёх партий.

== Выводы

Соседний раздел, его во встройке быть не должно.

$ q = -lambda nabla T $ <eq-fourier>
`;
const INTRO = `#set text(lang: "ru")
= Введение

Цель работы — оценить теплопроводность образцов.
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

const previewText = `[...document.querySelectorAll(".ts-textlayer text")].map((t) => t.textContent).join(" ")`;
const waitPreview = (page, condition, label) =>
  page.waitFor(`(() => { const text = ${previewText}; return ${condition}; })()`, { label, timeout: 30000 });

export default async (page) => {
  await page.evaluate(OPFS + "true");
  await page.evaluate(`window.__opfs.write("Диссертация/Методика.typ", ${JSON.stringify(METHOD)})`);
  await page.evaluate(`window.__opfs.write("Диссертация/Введение.typ", ${JSON.stringify(INTRO)})`);
  await page.click("text=Открыть vault");
  await page.waitForElement("text=Введение", { timeout: 30000 });
  await page.click("text=Введение");
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('Цель работы')`, { label: "заметка открыта" });
  await waitPreview(page, `text.includes("Цель")`, "предпросмотр собран");

  // ── Команда и дополнение ─────────────────────────────────────────────
  await page.click(".cm-content");
  await page.press("End", { ctrl: true });
  await page.press("k", { ctrl: true });
  await page.waitForElement(".ts-palette");
  await page.type("Встроить заметку");
  await page.sleep(200);
  await page.press("Enter");
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('#embed("")')`, { label: "вставлен вызов" });
  expect(page, (await page.evaluate(`document.querySelector('.cm-content').innerText`)).includes('#import "/.typsidian/lib/embed.typ": embed'), "вставлен #import");
  await page.waitFor(`window.__opfs.read(".typsidian/lib/embed.typ").then((text) => text?.startsWith("// typsidian-embed v1"))`, { label: "библиотека в vault" });
  expect(page, true, "библиотека записана в vault");
  await page.type("Мет");
  await page.waitFor(`document.body.innerText.includes("Методика")`, { label: "дополнение: заметка" });
  await page.type("одика#Вы");
  await page.waitFor(`document.body.innerText.includes("Выборка") && document.body.innerText.includes("Выводы")`, { label: "дополнение: разделы", timeout: 15000 });
  await page.shot(shots + "/031-e2-completion.png");
  await page.press("Enter");
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('#embed("Методика#Выборка")')`, { label: "раздел выбран" });
  expect(page, true, "дополнение вставило раздел");

  // ── Предпросмотр ─────────────────────────────────────────────────────
  await waitPreview(page, `text.includes("Отобрано")`, "раздел во встройке");
  const shown = await page.evaluate(previewText);
  expect(page, !shown.includes("Соседний") && !shown.includes("Вводная"), "только раздел, без соседних");

  await page.evaluate(`window.__opfs.write("Диссертация/Методика.typ", ${JSON.stringify(METHOD.replace("двадцать четыре", "тридцать два"))})`);
  await waitPreview(page, `text.includes("тридцать")`, "правка источника видна");
  expect(page, true, "правка источника обновила предпросмотр");

  // ── Ошибки ───────────────────────────────────────────────────────────
  await page.press("End", { ctrl: true });
  await page.type('\n\n#embed("Результаты")\n\n#embed("Методика#Погрешность")\n');
  await waitPreview(page, `text.includes("Результаты") && text.includes("Погрешность")`, "плашки ошибок");
  await page.waitFor(`document.querySelectorAll('.cm-content .ts-mark-note').length >= 2`, { label: "битые встраивания подчёркнуты", timeout: 15000 });
  expect(page, true, "битые встраивания подчёркнуты и показаны плашками");
  await page.shot(shots + "/031-e1-editor-preview.png");

  // Цикл: «Методика» встраивает «Введение».
  await page.evaluate(`window.__opfs.write("Диссертация/Методика.typ", ${JSON.stringify(METHOD.replace("Отобрано", '#import "/.typsidian/lib/embed.typ": embed\n#embed("Введение")\nОтобрано'))})`);
  await waitPreview(page, `text.includes("цикл") || text.includes("cycle")`, "плашка цикла");
  expect(page, true, "цикл обрывается плашкой");
  await page.shot(shots + "/031-e3-cycle.png");
};
