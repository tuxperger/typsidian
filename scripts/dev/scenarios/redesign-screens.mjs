// Экраны нового дизайна (фича 044, quickstart.md): палитра, колонка заметки,
// ошибка сборки, граф, настройки, окно экспорта, синхронизация. Снимки для сверки
// с артбордами `specs/044-ui-redesign-screens/design/*.dc.html`.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/redesign-screens.mjs --opfs-vault --timeout 180000
//
// `REDESIGN_SHOTS=/папка` — снимки palette.png, text-panel.png, build-error.png,
// graph.png, settings.png, export.png, sync.png.

const NOTE = `// Глава 3. Результаты эксперимента
#set text(lang: "ru", size: 12pt)
#set par(justify: true, first-line-indent: 1.25em)

= Результаты
== Тепловой режим образца

Нагрев шёл _равномерно_, без локальных перегревов: см. @fig-termo.
Максимум отклонения — *3.2%* при нагрузке 450 Вт.

== Механические свойства

#figure(rect(width: 40%, height: 2cm), caption: [Термограмма образца]) <fig-termo>

== Сравнение с моделью
`;

const FILES = [
  ["Введение/Введение.typ", "= Введение\nСм. #link(\"Результаты/Результаты.typ\")[результаты].\n"],
  ["Методология/Методология.typ", "= Методология\n"],
  ["Методология/Протокол теплового испытания №4.typ", "= Протокол\n"],
  ["Результаты/Обсуждение.typ", "= Обсуждение\nСм. #link(\"Результаты.typ\")[результаты].\n"],
  ["Результаты/Результаты.typ", NOTE],
  ["шаблоны/диссертация.typ", "#let рисунок = none\n"],
  ["Результаты/Сломанная.typ", "// Глава 3\n#import \"../шаблоны/нет такого.typ\": рисунок\n\n= Результаты\n"],
  ["План работы.typ", "= План работы\nСм. #link(\"Результаты/Результаты.typ\")[результаты].\n"],
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
const visible = (selector) =>
  `(() => { const el = document.querySelector(${JSON.stringify(selector)}); return !!el && el.getClientRects().length > 0; })()`;
const clickLabel = (label) =>
  `[...document.querySelectorAll("button")].find((el) => el.getAttribute("aria-label") === ${JSON.stringify(label)} && el.getClientRects().length > 0)?.click()`;

async function runCommand(page, query) {
  await page.click(".ts-cmdfield");
  await page.waitForElement(".ts-palette");
  await page.type(query);
  await page.sleep(200);
  await page.press("Enter");
  await page.sleep(400);
}

export default async (page) => {
  await page.viewport(1200, 750);
  await page.evaluate(writeFiles(FILES));
  await page.click("text=Продолжить");
  await page.waitForElement(".ts-start");
  await page.click("text=Открыть папку…");
  await page.waitForElement(".ts-vault-switch", { timeout: 60000 });
  await page.evaluate(
    `[...document.querySelectorAll("[data-drag-path='Результаты/Результаты.typ']")][0]?.querySelector("button")?.click()`,
  );
  await page.waitFor(`document.querySelectorAll(".ts-preview__page").length > 0`, {
    label: "сборка",
    timeout: 90000,
  });
  await page.sleep(800);

  // Артборд 1: низ дерева.
  const footer = await page.evaluate(
    `[...document.querySelectorAll(".ts-sidebar__footer button")].map((el) => el.getAttribute("aria-label") || el.textContent.trim())`,
  );
  page.log(`  низ дерева: ${footer.join(" | ")}`);
  check(
    page,
    ["Открыть граф связей", "Задачи", "Источники", "Настройки"].every((label) => footer.includes(label)) &&
      !footer.includes("Оглавление"),
    "низ дерева: граф, задачи, источники, настройки (артборд 1)",
  );

  // Артборд 3: палитра.
  await page.click(".ts-cmdfield");
  await page.waitForElement(".ts-palette");
  await page.type("резу");
  await page.sleep(300);
  check(page, await page.evaluate(visible(".ts-palette__match")), "палитра: совпадение выделено");
  check(page, await page.evaluate(visible(".ts-palette__esc")), "палитра: «Esc» в поле");
  check(page, (await page.text(".ts-palette")).includes("Результаты/"), "палитра: папка заметки справа");
  await shot(page, "palette");
  await page.press("Escape");
  await page.sleep(200);

  // Артборд 4: «Текст» с колонкой заметки справа.
  await runCommand(page, "оглавлен");
  await page.waitForElement(".ts-notepanel");
  await page.click("text=Текст");
  await page.sleep(500);
  check(
    page,
    await page.evaluate(`document.querySelector(".ts-body > .ts-notepanel") === document.querySelector(".ts-body").lastElementChild`),
    "колонка заметки у правого края",
  );
  const tabs = await page.evaluate(
    `[...document.querySelectorAll(".ts-notepanel__mode")].map((el) => el.textContent.trim())`,
  );
  page.log(`  вкладки: ${tabs.join(" | ")}`);
  check(page, tabs[0]?.startsWith("Оглавление") && tabs[1]?.startsWith("Ссылки"), "колонка: вкладки «Оглавление», «Ссылки»…");
  await shot(page, "text-panel");
  await runCommand(page, "оглавлен");
  await page.click("text=Рядом");
  await page.sleep(400);

  // Артборд 7: граф на месте текста, дерево слева.
  await page.evaluate(clickLabel("Открыть граф связей"));
  await page.waitForElement(".ts-graph");
  await page.sleep(1500);
  check(page, await page.evaluate(visible(".ts-sidebar")), "граф: дерево остаётся");
  check(page, (await page.text(".ts-header")).includes("Граф связей"), "граф: название в шапке");
  check(page, (await page.text(".ts-graph__panel")).includes("Показывать"), "граф: колонка «Показывать»");
  check(page, await page.evaluate(visible(".ts-graph__status")), "граф: сводка в строке состояния");
  await shot(page, "graph");
  await page.click("text=Рядом");
  await page.waitFor(`document.querySelector(".ts-graph") === null`, { label: "возврат из графа" });
  check(page, true, "граф: «Рядом» возвращает рабочую область");

  // Артборд 9: настройки под шапкой.
  await page.evaluate(clickLabel("Настройки"));
  await page.waitForElement(".ts-settings");
  await page.sleep(400);
  check(page, (await page.text(".ts-header")).includes("Настройки"), "настройки: название в шапке");
  check(page, (await page.text(".ts-sheet__foot")).includes("Настройки сохраняются сразу"), "настройки: строка внизу");
  await shot(page, "settings");
  await page.click(".ts-sheet__foot .ts-statusbar__action");
  await page.sleep(300);

  // Артборд 10: окно экспорта.
  await page.click(".ts-header__primary");
  await page.waitForElement(".ts-export-dialog");
  const exportText = await page.text(".ts-export-dialog");
  check(page, ["PDF", "HTML", "Слайды", "Что", "Куда"].every((part) => exportText.includes(part)), "экспорт: формат, «Что», «Куда»");
  await shot(page, "export");
  await page.press("Escape");
  await page.sleep(200);

  // Артборд 5: ошибка сборки — карточка над бледным листом, подчёркивание.
  await page.evaluate(
    `[...document.querySelectorAll("[data-drag-path='Результаты/Сломанная.typ']")][0]?.querySelector("button")?.click()`,
  );
  await page.waitForElement(".ts-failure__card", { timeout: 60000 });
  await page.sleep(800);
  check(page, await page.evaluate(visible(".ts-failure__reveal")), "ошибка: «Перейти к строке»");
  check(page, (await page.text(".ts-failure__place")).includes("строка 2"), "ошибка: место словами");
  check(page, await page.evaluate(visible(".ts-editor .ts-build-error")), "ошибка: подчёркивание в тексте");
  check(page, /\d+ ошиб/.test(await page.text(".ts-statusbar__errors")), "ошибка: число ошибок в строке состояния");
  page.log("  номер строки с ошибкой: " + (await page.evaluate(`(() => { const el = document.querySelector(".cm-lineNumbers .ts-build-error-line"); return el ? el.textContent + " " + getComputedStyle(el).color : "нет"; })()`)));
  await shot(page, "build-error");
};
