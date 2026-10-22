// Замена по vault (фича 027, quickstart.md): предпросмотр каждого совпадения,
// снятие отметок, замена, открытая заметка с несохранёнными правками, отмена,
// состояния R3 и телефон R4.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/replace.mjs --opfs-vault --shot /tmp/027.png
//
// Снимки — в каталог из REPLACE_SHOTS (по умолчанию /tmp).

const shots = process.env.REPLACE_SHOTS ?? "/tmp";

const FILES = {
  "Диссертация/Введение.typ":
    "= Введение\n\nНаучный руководитель — д.т.н. Иванов А. П.\nМетодика предложена Ивановым в [[Методика]].\n#cite(<Иванов2024>)\n",
  "Диссертация/Результаты.typ":
    "= Результаты\n\nСравнение с данными Иванов (2024).\nРяд Иванов—Петрова сходится при t > 3.\n",
  "Заметки/Встреча 14.10.typ": "= Встреча\n\n- Иванов: прислать замечания к главе 2\n",
};

const write = (path, text) => `(async () => {
  let dir = await (await navigator.storage.getDirectory()).getDirectoryHandle("vault", { create: true });
  const parts = ${JSON.stringify(path)}.split("/");
  for (const part of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(part, { create: true });
  const file = await dir.getFileHandle(parts.at(-1), { create: true });
  const stream = await file.createWritable();
  await stream.write(${JSON.stringify(text)});
  await stream.close();
})()`;

const read = (path) => `(async () => {
  let dir = await (await navigator.storage.getDirectory()).getDirectoryHandle("vault");
  const parts = ${JSON.stringify(path)}.split("/");
  for (const part of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(part);
  return (await (await dir.getFileHandle(parts.at(-1))).getFile()).text();
})()`;

function expect(page, ok, label) {
  page.log(`  ${ok ? "ok" : "НЕ СОШЛОСЬ"}: ${label}`);
  if (!ok) throw new Error(`не сошлось: ${label}`);
}

const editorText = `[...document.querySelectorAll(".ts-editor .cm-line")].map((l) => l.textContent).join("\\n")`;

/** Вводит текст в поле по селектору, заменяя прежний. */
async function fill(page, selector, text) {
  await page.evaluate(`(() => {
    const input = document.querySelector(${JSON.stringify(selector)});
    input.focus();
    input.select();
  })()`);
  await page.press("Backspace");
  await page.type(text);
}

const checks = `[...document.querySelectorAll(".ts-replace__match input")]`;

export default async (page) => {
  for (const [path, text] of Object.entries(FILES)) await page.evaluate(write(path, text));
  await page.click("text=Открыть vault");
  await page.waitForElement("text=Введение");
  await page.sleep(2500);

  // Открытая заметка с несохранёнными правками: замена обязана уйти в редактор.
  await page.click("text=Результаты");
  await page.waitForElement(".ts-editor .cm-content");
  await page.click(".cm-content");
  await page.press("End", { ctrl: true });
  await page.type("\nНесохранённое про Иванов.");

  // Mod+Shift+H — поиск сразу в режиме замены.
  await page.press("H", { ctrl: true, shift: true, code: "KeyH" });
  await page.waitForElement("#ts-replace-input");
  await fill(page, ".ts-search__input", "Иванов");
  await fill(page, "#ts-replace-input", "Иванова");
  await page.waitFor(`${checks}.length > 0`, { label: "предпросмотр" });
  await page.sleep(400);

  const count = await page.evaluate(`${checks}.length`);
  page.log(`совпадений в предпросмотре: ${count}`);
  expect(page, count === 7, "каждое совпадение перечислено (7)");
  expect(
    page,
    await page.evaluate(`document.querySelectorAll(".ts-replace__del").length === 7 && document.querySelectorAll(".ts-replace__ins").length === 7`),
    "у каждого совпадения «было → станет»",
  );
  expect(page, (await page.text(".ts-search__results")).includes("замена пойдёт в редактор"), "открытая заметка помечена");

  // Снять совпадение в #cite и «Ивановым» — замена их не тронет.
  for (const line of ["ым в [[", "2024>)"]) {
    await page.evaluate(`(() => {
      const row = [...document.querySelectorAll(".ts-replace__match")].find((r) => r.textContent.includes(${JSON.stringify(line)}));
      row.querySelector("input").click();
    })()`);
  }
  await page.sleep(200);
  expect(page, (await page.text(".ts-search__hints")).includes("Заменить отмеченные (5)"), "отмечено 5 из 7");
  await page.shot(shots + "/027-r1-preview.png");

  await page.press("Enter", { ctrl: true });
  await page.waitFor(`document.body.innerText.includes("Заменено 5")`, { label: "итог замены" });
  await page.shot(shots + "/027-r2-done.png");

  const intro = await page.evaluate(read("Диссертация/Введение.typ"));
  expect(page, intro.includes("д.т.н. Иванова А. П."), "отмеченное заменено");
  expect(page, intro.includes("Ивановым") && intro.includes("<Иванов2024>"), "снятое не тронуто");
  const meeting = await page.evaluate(read("Заметки/Встреча 14.10.typ"));
  expect(page, meeting.includes("- Иванова: прислать"), "замена в закрытой заметке");
  const editor = await page.evaluate(editorText);
  expect(page, editor.includes("Несохранённое про Иванова."), "открытая заметка заменена в редакторе");
  expect(page, editor.includes("данными Иванова (2024)"), "в редакторе заменены все отмеченные");

  // Отмена пакета.
  await page.click("text=Отменить замену");
  await page.waitFor(`document.body.innerText.includes("Возвращено к тексту до замены")`, { label: "итог отмены" });
  expect(page, (await page.evaluate(read("Диссертация/Введение.typ"))) === FILES["Диссертация/Введение.typ"], "отмена вернула текст файла");
  expect(page, (await page.evaluate(editorText)).includes("Несохранённое про Иванов."), "отмена вернула текст редактора");

  // Состояния R3: неверное выражение, ничего не найдено.
  await page.click("text=regex");
  await fill(page, ".ts-search__input", "Иванов(");
  await page.waitFor(`document.body.innerText.includes("Выражение не разобрано")`, { label: "ошибка выражения" });
  expect(page, await page.evaluate(`document.querySelector(".ts-replace__foot .ts-btn--accent").disabled`), "при ошибке заменять нечем");
  await page.shot(shots + "/027-r3-bad.png");
  await page.click("text=regex");
  await fill(page, ".ts-search__input", "Сидоров");
  await page.waitFor(`document.body.innerText.includes("заменять нечего")`, { label: "нечего заменять" });

  // Телефон R4.
  await fill(page, ".ts-search__input", "Иванов");
  await page.waitFor(`${checks}.length > 0`, { label: "предпросмотр снова" });
  await page.viewport(390, 760);
  await page.sleep(500);
  await page.shot(shots + "/027-r4-phone.png");
  await page.viewport(1440, 900);
};
