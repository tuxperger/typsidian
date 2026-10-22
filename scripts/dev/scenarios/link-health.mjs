// Состояние ссылок (фича 028, quickstart.md): счётчик в подвале дерева,
// группы битых ссылок, перенаправление с сохранением подписи и отмена,
// «Создать заметку», ✕ у строки, вкладка «Без входящих» и «Намеренно».
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/link-health.mjs --opfs-vault
//
// Снимки — в каталог из HEALTH_SHOTS (по умолчанию /tmp).

const shots = process.env.HEALTH_SHOTS ?? "/tmp";

const FILES = {
  "Диссертация/Введение.typ":
    "= Введение\n\nМетодика предложена в [[Методика]] и уточнена.\n#image(\"нет.png\")\n",
  "Диссертация/Результаты.typ": "= Результаты\n\nпо [[Методика|методике]] из главы 2\n",
  "Заметки/Встреча.typ": "= Встреча\n\n- перечитать [[/Диссертация/Методика]]\n- [[Схема стенда.excalidraw]]\n",
  "Диссертация/Методология.typ": "= Методология\n",
  "Заметки/Одинокая.typ": "= Одинокая\n",
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

/** Кнопка внутри группы цели по тексту. */
const groupButton = (target, label) => `(() => {
  const group = [...document.querySelectorAll(".ts-health__target")].find((g) =>
    g.querySelector(".ts-health__name")?.textContent === ${JSON.stringify(target)});
  const button = [...(group?.querySelectorAll("button") ?? [])].find((b) => b.textContent === ${JSON.stringify(label)});
  button?.click();
  return button !== undefined;
})()`;

export default async (page) => {
  for (const [path, text] of Object.entries(FILES)) await page.evaluate(write(path, text));
  await page.click("text=Открыть vault");
  await page.waitForElement("text=Введение");

  // L3: счётчик в подвале дерева.
  await page.waitFor(`document.querySelector(".ts-tree-broken") !== null`, { label: "счётчик битых", timeout: 30000 });
  const footer = await page.text(".ts-tree-broken");
  page.log(`подвал дерева: ${footer}`);
  expect(page, footer.includes("5"), "5 битых: 3 × Методика, вставка, доска");
  await page.click(".ts-tree-broken");
  await page.waitForElement(".ts-health");
  await page.waitFor(`document.querySelectorAll(".ts-health__target").length === 3`, { label: "три группы" });
  page.log("— экран:\n" + (await page.text(".ts-health")));
  await page.shot(shots + "/028-l1-broken.png");

  // Перенаправление группы «Методика» на «Методология».
  expect(page, await page.evaluate(groupButton("Методика", "Перенаправить на…")), "кнопка «Перенаправить на…»");
  await page.waitForElement(".ts-health__picker input");
  await page.type("Методол");
  await page.sleep(200);
  await page.shot(shots + "/028-l1-picker.png");
  await page.press("Enter");
  await page.waitFor(`document.body.innerText.includes("Перенаправлено 3")`, { label: "итог перенаправления" });
  await page.waitFor(`document.querySelectorAll(".ts-health__target").length === 2`, { label: "группа ушла" });
  const results = await page.evaluate(read("Диссертация/Результаты.typ"));
  expect(page, results.includes("[[Методология|методике]]"), "подпись сохранена");
  const intro = await page.evaluate(read("Диссертация/Введение.typ"));
  expect(page, intro.includes("в [[Методология]] и"), "ссылка перенаправлена");
  expect(page, intro.includes("#image(\"нет.png\")"), "чужая группа не тронута");

  await page.click("text=Отменить");
  await page.waitFor(`!document.body.innerText.includes("Перенаправлено 3")`, { label: "отмена прошла" });
  await page.waitFor(`document.querySelectorAll(".ts-health__target").length === 3`, { label: "группа вернулась" });
  expect(page, (await page.evaluate(read("Диссертация/Результаты.typ"))) === FILES["Диссертация/Результаты.typ"], "отмена вернула текст");

  // «Создать доску» — группа исчезает.
  expect(page, await page.evaluate(groupButton("Схема стенда", "Создать доску")), "кнопка «Создать доску»");
  await page.waitFor(`document.querySelectorAll(".ts-health__target").length === 2`, { label: "группа доски ушла" });

  // ✕ у одной строки — ссылка стала текстом.
  await page.evaluate(`(() => {
    const site = [...document.querySelectorAll(".ts-health__site")].find((s) => s.textContent.includes("Результаты"));
    site.querySelector(".ts-health__x").click();
  })()`);
  await page.waitFor(`document.body.innerText.includes("Убрано 1")`, { label: "одна ссылка убрана" });
  expect(page, (await page.evaluate(read("Диссертация/Результаты.typ"))).includes("по методике из"), "осталась подпись");

  // L2: без входящих, «Намеренно».
  await page.click("text=Без входящих");
  await page.waitFor(`document.querySelectorAll(".ts-health__orphan").length > 0`, { label: "сироты" });
  page.log("— без входящих:\n" + (await page.text(".ts-health")));
  await page.shot(shots + "/028-l2-orphans.png");
  await page.evaluate(`(() => {
    const row = [...document.querySelectorAll(".ts-health__orphan")].find((r) => r.textContent.includes("Одинокая"));
    [...row.querySelectorAll("button")].find((b) => b.textContent === "Намеренно").click();
  })()`);
  await page.waitFor(`document.body.innerText.includes("скрыто намеренно: 1")`, { label: "отметка" });
  expect(
    page,
    await page.evaluate(`![...document.querySelectorAll(".ts-health__orphan")].some((r) => r.textContent.includes("Одинокая"))`),
    "заметка пропала из списка",
  );
  const settings = await page.evaluate(read(".typsidian/vault.json"));
  expect(page, settings.includes("Заметки/Одинокая.typ"), "отметка в настройках vault");

  // Телефон.
  await page.viewport(390, 760);
  await page.click("text=Битые ссылки");
  await page.sleep(500);
  await page.shot(shots + "/028-phone.png");
  await page.viewport(1440, 900);
};
