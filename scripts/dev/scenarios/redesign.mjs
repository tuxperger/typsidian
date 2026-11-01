// Оболочка по Typsidian.pdf (фича 043, quickstart.md): стартовый экран, рабочая
// область, вид «Текст / Рядом / Документ», меню vault, низ дерева, полоса над
// листом; телефон — дерево и документ. Снимки для сверки с PDF.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/redesign.mjs --opfs-vault --timeout 120000
//
// `REDESIGN_SHOTS=/папка` — снимки start.png, workspace.png, text.png,
// document.png, vault-menu.png, phone-tree.png, phone-document.png.

const NOTE = `// Глава 3. Результаты эксперимента
#set text(lang: "ru", size: 12pt)
#set par(justify: true, first-line-indent: 1.25em)

= Результаты
== Тепловой режим образца

Нагрев шёл _равномерно_, без локальных перегревов.
Максимум отклонения — *3.2%* при нагрузке 450 Вт, а \\#7 серии отбракован.

- калибровка датчиков до и после серии
- шаг измерений \`dt = 0.5 s\`
+ прогрев 15 мин
+ запись 60 мин
/ Термопара: датчик типа K, класс допуска 1.

$ Delta T = 1/n sum_(i=1)^n (T_i - T_0) $
`;

const FILES = [
  ["Введение/Введение.typ", "= Введение\n"],
  ["Методология/Методология.typ", "= Методология\n"],
  ["Результаты/Обсуждение.typ", "= Обсуждение\n"],
  ["Результаты/Результаты.typ", NOTE],
  ["Результаты/рисунки/схема.typ", "= Схема\n"],
  ["шаблоны/диссертация.typ", "#let рисунок = none\n"],
  ["План работы.typ", "= План работы\n"],
];

const writeFiles = (files) => `(async () => {
  const root = await navigator.storage.getDirectory();
  const vault = await root.getDirectoryHandle("vault", { create: true });
  for (const [path, text] of ${JSON.stringify(files)}) {
    const parts = path.split("/");
    let dir = vault;
    for (const part of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(part, { create: true });
    const file = await dir.getFileHandle(parts[parts.length - 1], { create: true });
    const stream = await file.createWritable();
    await stream.write(text);
    await stream.close();
  }
})()`;

function check(page, ok, label) {
  page.log(`${ok ? "✓" : "✗"} ${label}`);
  if (!ok) throw new Error(`не выполнено: ${label}`);
}

const shots = process.env.REDESIGN_SHOTS;
const shot = (page, name) => (shots ? page.shot(`${shots}/${name}.png`) : Promise.resolve());
const visible = (selector) => `(() => { const el = document.querySelector(${JSON.stringify(selector)}); return !!el && el.getClientRects().length > 0; })()`;

export default async (page) => {
  await page.viewport(1200, 750);
  await page.evaluate(writeFiles(FILES));
  await page.click("text=Продолжить");

  // Экран 2: стартовый экран.
  await page.waitForElement(".ts-start");
  check(page, (await page.text(".ts-start")).includes("Заметки и документы на Typst"), "стартовый экран: знак и описание");
  await shot(page, "start");
  await page.click("text=Открыть папку…");

  // Экран 1: рабочая область.
  await page.waitForElement(".ts-vault-switch", { timeout: 60000 });
  await page.evaluate(`[...document.querySelectorAll("[data-drag-path='Результаты/Результаты.typ']")][0]?.querySelector("button")?.click()`);
  await page.waitFor(`document.querySelectorAll(".ts-preview__page").length > 0`, { label: "сборка", timeout: 60000 });
  await page.sleep(800);

  const headerButtons = await page.evaluate(`[...document.querySelectorAll(".ts-header button")].filter((el) => el.getClientRects().length > 0 && !el.closest(".ts-segmented")).length + (document.querySelector(".ts-header .ts-segmented") ? 1 : 0)`);
  page.log(`  элементов управления в шапке: ${headerButtons}`);
  check(page, headerButtons <= 7, "в шапке не больше 7 элементов управления (SC-001)");
  check(page, await page.evaluate(visible(".ts-header .ts-cmdfield")), "поле «Команды и заметки» в шапке");
  check(page, await page.evaluate(visible(".ts-header__primary")), "главная кнопка «Экспорт PDF»");
  check(page, (await page.text(".ts-pane--preview .ts-pane__toolbar")).includes("стр. 1"), "полоса над листом: номер страницы");
  check(page, await page.evaluate(`document.querySelector(".ts-build-status")?.dataset.state === "built"`), "полоса над листом: собрано");
  check(page, !(await page.evaluate(visible(".ts-ruler-h"))), "линеек нет");
  const footer = await page.evaluate(`[...document.querySelectorAll(".ts-sidebar__footer button")].map((el) => el.getAttribute("aria-label") || el.textContent.trim())`);
  page.log(`  низ дерева: ${footer.join(" | ")}`);
  check(page, ["Открыть граф связей", "Задачи", "Источники"].every((label) => footer.includes(label)), "низ дерева: граф, задачи, источники (044)");
  await shot(page, "workspace");

  // Вид «Текст» и «Документ».
  await page.click("text=Текст");
  await page.sleep(300);
  check(page, !(await page.evaluate(visible(".ts-pane--preview"))) && (await page.evaluate(visible(".ts-groups"))), "вид «Текст»: только текст");
  await shot(page, "text");
  await page.click("text=Документ");
  await page.sleep(800);
  check(page, (await page.evaluate(visible(".ts-pane--preview"))) && !(await page.evaluate(visible(".ts-groups"))), "вид «Документ»: только лист");
  await shot(page, "document");
  await page.click("text=Рядом");
  await page.sleep(300);
  check(page, (await page.evaluate(visible(".ts-pane--preview"))) && (await page.evaluate(visible(".ts-groups"))), "вид «Рядом»: оба");

  // Меню vault.
  await page.click(".ts-vault-switch");
  await page.waitForElement("[role=menu]");
  const menu = await page.text("[role=menu]");
  check(page, menu.includes("Открыть папку…"), "меню vault: «Открыть папку…»");
  await shot(page, "vault-menu");
  await page.press("Escape");

  // Палитра из поля в шапке.
  await page.click(".ts-cmdfield");
  await page.waitForElement(".ts-palette");
  check(page, true, "поле в шапке открывает палитру");
  await page.press("Escape");

  // Телефон: экраны 3 и 4.
  await page.viewport(360, 740);
  await page.sleep(800);
  await page.evaluate(`document.querySelector(".ts-phone-note button")?.click()`);
  await page.sleep(400);
  check(page, await page.evaluate(visible(".ts-sidebar__quick")), "телефон: «Быстрая заметка» внизу дерева");
  await shot(page, "phone-tree");
  await page.evaluate(`[...document.querySelectorAll("[data-drag-path='Результаты/Результаты.typ']")][0]?.querySelector("button")?.click()`);
  await page.sleep(600);
  check(page, await page.evaluate(visible(".ts-phone-note")), "телефон: экран заметки с «Назад»");
  await page.evaluate(`[...document.querySelectorAll(".ts-phone-note .ts-segmented__item")].find((el) => el.textContent.trim() === "Документ")?.click()`);
  await page.sleep(1200);
  check(page, await page.evaluate(visible(".ts-pane--preview")), "телефон: «Документ» показывает лист");
  await shot(page, "phone-document");
};
