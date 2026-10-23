// Экспорт в HTML (фича 034, quickstart.md): Центр экспорта → HTML → папка из
// трёх заметок; страницы в папке экспорта, ссылки внутри набора — на страницы,
// вне набора — помечены; `rect` — рисунком и назван в строке; оглавление;
// «Скачать архив» на web.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/html-export.mjs --opfs-vault
//
// Снимки экрана — в каталог из HTML_SHOTS (по умолчанию /tmp).
const shots = process.env.HTML_SHOTS ?? "/tmp";

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

export default async (page) => {
  await page.evaluate(OPFS + "true");
  await page.evaluate(`window.__opfs.write("Диссертация/Введение.typ", "#set text(lang: \\"ru\\")\\n= Введение\\n\\nСм. [[Методика]] и [[Обзор]].\\n\\n$ q = -lambda nabla T $\\n")`);
  await page.evaluate(`window.__opfs.write("Диссертация/Методика.typ", "= Методика\\n\\nНазад: [[Введение]].\\n\\n#rect(width: 2cm, height: 1cm)\\n")`);
  await page.evaluate(`window.__opfs.write("Диссертация/Результаты.typ", "= Результаты\\n\\nТаблица.\\n")`);
  await page.evaluate(`window.__opfs.write("Обзор.typ", "= Обзор\\n")`);
  await page.click("text=Открыть vault");
  await page.waitForElement("text=Введение", { timeout: 30000 });
  await page.sleep(1500);
  await page.click(".ts-tree");

  await page.press("k", { ctrl: true });
  await page.waitForElement(".ts-palette");
  await page.type("Центр экспорта");
  await page.sleep(200);
  await page.press("Enter");
  await page.waitForElement(".ts-export");
  await page.waitFor(`document.querySelectorAll('.ts-export__row').length >= 4`, { label: "список заметок", timeout: 15000 });

  await page.click("text=HTML");
  for (const name of ["Введение.typ", "Методика.typ", "Результаты.typ"]) {
    await page.evaluate(`[...document.querySelectorAll('.ts-export__row')].find((r) => r.querySelector('.ts-export__row-name')?.textContent.trim() === ${JSON.stringify(name)}).querySelector('.ts-export__row-check').click()`);
  }
  await page.click("text=Экспортировать 3");
  await page.waitFor(`document.body.innerText.includes("Экспорт завершён")`, { label: "пакет завершён", timeout: 60000 });
  await page.shot(shots + "/034-h1-center.png");

  const intro = await page.evaluate(`window.__opfs.read("export/Диссертация/Введение.html")`);
  const method = await page.evaluate(`window.__opfs.read("export/Диссертация/Методика.html")`);
  const index = await page.evaluate(`window.__opfs.read("export/index.html")`);
  expect(page, intro !== null && method !== null && index !== null, "страницы и оглавление записаны");
  expect(page, intro.includes('href="Методика.html"') && method.includes('href="Введение.html"'), "ссылки внутри набора — на страницы");
  expect(page, intro.includes("ts-outside"), "ссылка вне набора помечена");
  expect(page, intro.includes("<math"), "формула — MathML");
  expect(page, method.includes("<svg"), "rect — рисунком");
  expect(page, (await page.evaluate(`document.body.innerText`)).includes("рисунком: rect"), "в строке названо, что выведено рисунком");
  expect(page, index.includes("Диссертация/Результаты.html"), "оглавление со страницами");

  // Архив: перехват ссылки скачивания.
  await page.evaluate(`window.__downloads = []; const click = HTMLAnchorElement.prototype.click; HTMLAnchorElement.prototype.click = function () { if (this.download) window.__downloads.push(this.download); else click.call(this); }; true`);
  await page.click("text=Скачать архив (zip)");
  await page.waitFor(`window.__downloads.length === 1`, { label: "архив скачан" });
  expect(page, (await page.evaluate(`window.__downloads[0]`)) === "export.zip", "архив export.zip");
};
