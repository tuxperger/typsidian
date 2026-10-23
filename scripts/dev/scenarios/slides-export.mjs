// Экспорт презентации (фича 035, quickstart.md): окно из палитры; PDF слайдов
// (последний шаг и все шаги), PDF со страницами заметок, PDF + .pdfpc, PNG и
// SVG картинки — файлы в папке экспорта vault.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/slides-export.mjs --opfs-vault
//
// Снимки экрана — в каталог из SLIDES_SHOTS (по умолчанию /tmp).
const shots = process.env.SLIDES_SHOTS ?? "/tmp";

let deck = "#set page(width: 16cm, height: 9cm)\n";
[1, 2, 2, 3].forEach((number, i) => {
  if (i > 0) deck += "#pagebreak()\n";
  deck += `Страница ${i}\n#metadata((t: "LogicalSlide", v: ${number})) <pdfpc>\n`;
  if (i === 0) deck += `#metadata((t: "Note", v: "Сказать про стенд")) <pdfpc>\n`;
});

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
    async size(path) {
      try {
        const at = path.lastIndexOf("/");
        const dir = await this.dir(at === -1 ? "" : path.slice(0, at), false);
        return (await (await dir.getFileHandle(path.slice(at + 1))).getFile()).size;
      } catch { return null; }
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

const exportWith = async (page, format, allSteps = false) => {
  await page.evaluate(`[...document.querySelectorAll('.ts-slides-export__format')].find((l) => l.textContent.includes(${JSON.stringify(format)})).querySelector('input').click()`);
  if (allSteps) await page.click("text=все шаги");
  await page.click("text=Экспортировать");
  await page.waitFor(`document.querySelector('.ts-slides-export [role=status]')?.innerText.includes("Готово")`, { label: `экспорт: ${format}`, timeout: 30000 });
  return page.evaluate(`document.querySelector('.ts-slides-export [role=status]').innerText`);
};

export default async (page) => {
  await page.evaluate(OPFS + "true");
  await page.evaluate(`window.__opfs.write("Доклад.typ", ${JSON.stringify(deck)})`);
  await page.click("text=Открыть vault");
  await page.waitForElement("text=Доклад", { timeout: 30000 });
  await page.click("text=Доклад");
  await page.waitFor(`document.querySelector('.cm-content')?.innerText.includes('Страница 0')`, { label: "заметка открыта" });

  await page.press("k", { ctrl: true });
  await page.waitForElement(".ts-palette");
  await page.type("Экспорт презентации");
  await page.sleep(200);
  await page.press("Enter");
  await page.waitForElement(".ts-slides-export");

  let status = await exportWith(page, "PDF слайдов");
  expect(page, status.includes("3 страницы") && status.includes("export/Доклад.pdf"), "PDF слайдов: 3 страницы");
  await page.shot(shots + "/035-p1-dialog.png");

  status = await exportWith(page, "PDF со страницами заметок");
  expect(page, status.includes("export/Доклад.notes.pdf"), "PDF со страницами заметок");
  expect(page, (await page.evaluate(`window.__opfs.size("export/Доклад.notes.pdf")`)) > 1000, "файл заметок не пуст");

  status = await exportWith(page, "PDF для pdfpc");
  expect(page, status.includes("4 страницы") && status.includes("export/Доклад.pdfpc"), "PDF для pdfpc: все шаги и .pdfpc");
  const pdfpc = JSON.parse(await page.evaluate(`window.__opfs.read("export/Доклад.pdfpc")`));
  expect(page, pdfpc.pages.length === 4 && pdfpc.pages[0].note === "Сказать про стенд", ".pdfpc с заметками");

  status = await exportWith(page, "Картинки SVG", true);
  expect(page, status.includes("export/Доклад.slides/02-2.svg"), "SVG: все шаги с номером шага в имени");

  await page.evaluate(`[...document.querySelectorAll('.ts-slides-export__format')].find((l) => l.textContent.includes("Картинки PNG")).querySelector('input').click()`);
  await page.click("text=последний шаг");
  status = await exportWith(page, "Картинки PNG");
  expect(page, status.includes("export/Доклад.slides/01.png") && status.includes("3 страницы"), "PNG: по файлу на слайд");
  await page.shot(shots + "/035-p1-png.png");
};
