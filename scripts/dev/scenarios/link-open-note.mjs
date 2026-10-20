// «Связать» для заметки, открытой с несохранёнными правками (фича 013, T047):
// замену применяет порт `OpenNoteEditors` в буфере редактора — правка
// пользователя не теряется, файл пишет автосохранение.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/link-open-note.mjs --opfs-vault

const OPFS = `
  window.__opfs = {
    async file(path, create) {
      const root = await (await navigator.storage.getDirectory()).getDirectoryHandle("vault", { create: true });
      return root.getFileHandle(path, { create });
    },
    async write(path, text) {
      const stream = await (await this.file(path, true)).createWritable();
      await stream.write(text);
      await stream.close();
    },
    async read(path) {
      return (await (await this.file(path, false)).getFile()).text();
    },
  };
`;

const expect = (page, condition, what) => {
  if (!condition) throw new Error("не сошлось: " + what);
  page.log("  ok: " + what);
};

const openInTree = (name) => `(() => {
  const item = [...document.querySelectorAll(".ts-sidebar *")].find((el) => el.children.length === 0 && el.textContent.trim() === ${JSON.stringify(name)});
  item?.click();
  return Boolean(item);
})()`;

const editorText = `[...document.querySelectorAll(".ts-editor .cm-line")].map((l) => l.textContent).join("\\n")`;

export default async (page) => {
  await page.evaluate(OPFS + "true");
  await page.evaluate(`(async () => {
    await window.__opfs.write("Цель.typ", "= Цель\\n");
    await window.__opfs.write("Источник.typ", "= Источник\\n\\nЗдесь упомянута Цель без ссылки.\\n");
  })()`);
  await page.click("text=Открыть vault");
  await page.waitForElement("text=Источник");

  // Источник открыт, в нём несохранённая правка в конце.
  expect(page, await page.evaluate(openInTree("Источник")), "источник открыт из дерева");
  await page.waitForElement(".ts-editor .cm-content");
  await page.sleep(500);
  await page.evaluate(`(() => {
    const lines = [...document.querySelectorAll(".ts-editor .cm-line")];
    const range = document.createRange();
    range.selectNodeContents(lines[lines.length - 1]);
    range.collapse(false);
    const sel = window.getSelection();
    sel.removeAllRanges();
    sel.addRange(range);
    document.querySelector(".ts-editor .cm-content").focus();
  })()`);
  await page.type("Правка до сохранения.");

  // Сразу — к цели, пока правка не сохранена; источник остаётся вкладкой.
  await page.evaluate(openInTree("Цель"));
  await page.sleep(600);
  await page.press("L", { ctrl: true, shift: true });
  await page.waitForElement(".ts-notepanel");
  await page.click("text=Упоминания");
  await page.waitForElement(".ts-links__link", { timeout: 10000 });
  await page.click(".ts-links__link[aria-label*='Источник']");
  await page.sleep(400);

  // Вернуться к источнику: в буфере и ссылка, и правка.
  await page.evaluate(openInTree("Источник"));
  await page.sleep(400);
  const text = await page.evaluate(editorText);
  page.log("— буфер источника:\n" + text);
  expect(page, text.includes("упомянута [[Цель]] без ссылки"), "ссылка вставлена в открытую заметку");
  expect(page, text.includes("Правка до сохранения."), "несохранённая правка на месте");

  // Автосохранение пишет обе правки.
  await page.sleep(1500);
  const file = await page.evaluate('window.__opfs.read("Источник.typ")');
  expect(page, file.includes("[[Цель]]") && file.includes("Правка до сохранения."), "файл записан автосохранением с обеими правками");
};
