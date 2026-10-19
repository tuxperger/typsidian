// Вложения из буфера и перетаскиванием (фича 016): синтетические `paste` и
// `drop` с файлами в редактор заметки, файлы в OPFS и текст заметки.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/attachments.mjs --opfs-vault --shot /tmp/attach.png
//
// `ATTACH_SHOTS` — каталог для снимков состояний (по умолчанию `/tmp`).

const shots = process.env.ATTACH_SHOTS ?? "/tmp";
const NOTE = "Диссертация/Глава.typ";
const EDITOR = ".ts-egroup .cm-content";

const expect = (page, condition, what) => {
  if (!condition) throw new Error("не сошлось: " + what);
  page.log("  ok: " + what);
};

/** Список файлов папки vault в OPFS. */
const filesIn = (page, folder) =>
  page.evaluate(`(async () => {
    const root = await navigator.storage.getDirectory();
    let dir = await root.getDirectoryHandle("vault");
    try {
      for (const part of ${JSON.stringify(folder)}.split("/")) dir = await dir.getDirectoryHandle(part);
    } catch { return "[]"; }
    const names = [];
    for await (const [name, handle] of dir.entries()) if (handle.kind === "file") names.push(name);
    return JSON.stringify(names.sort());
  })()`).then((json) => JSON.parse(json));

const editorText = (page) => page.evaluate(`document.querySelector(${JSON.stringify(EDITOR)})?.innerText ?? ""`);

/** Файлы для события: SVG с разным цветом и PDF. */
const FILES = `
  window.__svg = (color) => new File(
    ['<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="' + color + '"/></svg>'],
    color + ".svg", { type: "image/svg+xml" });
  window.__pdf = () => new File(["%PDF-1.4\\n%"], "отчёт.pdf", { type: "application/pdf" });
`;

export default async (page) => {
  await page.evaluate(`(async () => {
    const root = await navigator.storage.getDirectory();
    const vault = await root.getDirectoryHandle("vault", { create: true });
    const folder = await vault.getDirectoryHandle("Диссертация", { create: true });
    const file = await folder.getFileHandle("Глава.typ", { create: true });
    const stream = await file.createWritable();
    await stream.write("= Глава\\n\\nПервая строка.\\nВторая строка.\\n");
    await stream.close();
  })()`);
  await page.click("text=Открыть vault");
  await page.waitForElement("text=Диссертация", { timeout: 30000 });
  // Первый тик сверки видит записанный тестом файл; правка до него дала бы
  // ложный конфликт — свойство стенда, а не вложений.
  await page.sleep(2500);
  await page.evaluate(`[...document.querySelectorAll('.ts-tree__name')].find((n) => n.textContent === 'Глава')?.closest('button')?.click()`);
  await page.waitForElement(EDITOR, { timeout: 15000 });
  await page.sleep(500);
  await page.evaluate(FILES);

  // ── US1: картинка из буфера ───────────────────────────────────────────────
  await page.click(EDITOR);
  await page.press("End", { ctrl: true });
  await page.evaluate(`(() => {
    const data = new DataTransfer();
    data.items.add(window.__svg("red"));
    document.querySelector(${JSON.stringify(EDITOR)}).dispatchEvent(
      new ClipboardEvent("paste", { clipboardData: data, bubbles: true, cancelable: true }));
  })()`);
  await page.waitFor(`document.querySelector(${JSON.stringify(EDITOR)})?.innerText.includes('#image("attachments/Глава-')`, {
    timeout: 10000,
    label: "запись из буфера в тексте",
  });
  const first = await filesIn(page, "Диссертация/attachments");
  expect(page, first.length === 1 && /^Глава-\d{4}-\d\d-\d\d-\d{6}\.svg$/.test(first[0]), "файл из буфера в папке вложений: " + first.join(", "));
  await page.waitFor(`document.querySelector('.ts-statusbar')?.innerText.includes(${JSON.stringify("вставлено: " + first[0])})`, {
    timeout: 5000,
    label: "статус-бар «вставлено»",
  });
  expect(page, true, "статус-бар: вставлено: " + first[0]);
  await page.shot(`${shots}/attach-1a-paste.png`);

  // Отмена одним шагом: запись уходит, файл остаётся (US1 AS5).
  await page.click(EDITOR);
  await page.press("z", { ctrl: true });
  await page.sleep(300);
  expect(page, !(await editorText(page)).includes("#image("), "Ctrl+Z убрал запись одной правкой");
  expect(page, (await filesIn(page, "Диссертация/attachments")).length === 1, "файл после отмены на месте");

  // Буфер с текстом — обычная вставка текста (FR-002).
  await page.evaluate(`(() => {
    const data = new DataTransfer();
    data.setData("text/plain", "просто текст");
    data.items.add(window.__svg("blue"));
    document.querySelector(${JSON.stringify(EDITOR)}).dispatchEvent(
      new ClipboardEvent("paste", { clipboardData: data, bubbles: true, cancelable: true }));
  })()`);
  await page.sleep(600);
  expect(page, (await filesIn(page, "Диссертация/attachments")).length === 1, "буфер с текстом не создаёт файла");

  // ── US2: бросок нескольких файлов, один не изображение ───────────────────
  const drop = (body) =>
    page.evaluate(`(() => {
      const editor = document.querySelector(${JSON.stringify(EDITOR)});
      const line = [...editor.querySelectorAll('.cm-line')].find((l) => l.textContent.startsWith('Вторая'));
      const rect = line.getBoundingClientRect();
      const data = new DataTransfer();
      ${body}
      const at = { clientX: rect.left + 20, clientY: rect.top + rect.height / 2, bubbles: true, cancelable: true, dataTransfer: data };
      editor.dispatchEvent(new DragEvent("dragover", at));
      editor.dispatchEvent(new DragEvent("drop", at));
    })()`);
  await drop(`data.items.add(window.__svg("green")); data.items.add(window.__pdf()); data.items.add(window.__svg("gold"));`);
  await page.waitFor(`document.querySelector('.ts-attach-panel')?.innerText.includes('Вставлено')`, {
    timeout: 10000,
    label: "итог пачки",
  });
  const summary = await page.text(".ts-attach-panel");
  page.log("  итог: " + summary.replace(/\n/g, " · "));
  expect(page, summary.includes("Вставлено 2 из 3") && summary.includes("отчёт.pdf"), "итог называет пропущенный файл");
  const afterDrop = await filesIn(page, "Диссертация/attachments");
  expect(page, afterDrop.includes("green.svg") && afterDrop.includes("gold.svg") && !afterDrop.includes("отчёт.pdf"), "PDF не попал в vault: " + afterDrop.join(", "));
  const text = await editorText(page);
  const lines = text.split("\n");
  const at = lines.findIndex((line) => line.includes("green.svg"));
  expect(page, at >= 0 && lines[at + 1]?.includes("gold.svg") && lines[at + 2]?.startsWith("Вторая"), "записи построчно перед строкой броска");
  await page.shot(`${shots}/attach-1b-summary.png`);

  // Тот же файл ещё раз — ссылка на лежащий, без копии (FR-010, FR-011).
  await drop(`data.items.add(window.__svg("green"));`);
  await page.sleep(1500);
  expect(page, (await filesIn(page, "Диссертация/attachments")).length === afterDrop.length, "повторный бросок не добавил файла");

  // Ссылка вместо файла — сообщение, текст не меняется (research.md §15).
  const before = await editorText(page);
  await drop(`data.setData("text/html", '<img src="https://example.org/x.png">');`);
  await page.waitFor(`document.body.innerText.includes('Перетащена ссылка, а не файл')`, {
    timeout: 5000,
    label: "сообщение о ссылке",
  });
  expect(page, (await editorText(page)) === before, "ссылка не изменила текст");
};
