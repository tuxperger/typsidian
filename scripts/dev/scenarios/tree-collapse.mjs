// Свернуть и развернуть папки разом (фича 042, quickstart.md): кнопки над деревом,
// пункты ветки в меню папки, раскрытие одной папки не раскрывает вложенные,
// 500 папок — замер (SC-002).
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/tree-collapse.mjs --opfs-vault --timeout 90000 --shot /tmp/tree-collapse.png
//
// `NO_FOLDERS=1` — vault только из заметок: кнопки над деревом выключены (US1 AS4).

const FILES = [
  ["Исследование/Эксперимент 3/Методика.typ", "= Методика\n"],
  ["Исследование/Эксперимент 3/Протокол нагрузки.typ", "= Протокол\n"],
  ["Исследование/Обзор литературы.typ", "= Обзор\n"],
  ["Доклады/Семинар, октябрь.typ", "= Семинар\n"],
  ["Начните здесь.typ", "= Начните здесь\n"],
  // 500 папок для SC-002: «Архив» с 499 подпапками.
  ...Array.from({ length: 499 }, (_, i) => [`Архив/Год ${String(i + 1).padStart(3, "0")}/запись.typ`, "= Запись\n"]),
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

// Имена видимых строк дерева верхнего уровня и вложенных — по порядку.
const visible = `[...document.querySelectorAll("[role=tree] [role=treeitem]")]
  .filter((el) => el.getClientRects().length > 0)
  .map((el) => el.querySelector("button")?.textContent.trim().replace(/\\s+/g, " ") ?? "")`;
const expanded = (name) => `(() => {
  const row = [...document.querySelectorAll("[role=tree] [role=treeitem][aria-expanded]")]
    .find((el) => el.querySelector("button")?.textContent.trim().startsWith(${JSON.stringify(name)}));
  return row ? row.getAttribute("aria-expanded") : "нет";
})()`;

function check(page, ok, label) {
  page.log(`${ok ? "✓" : "✗"} ${label}`);
  if (!ok) throw new Error(`не выполнено: ${label}`);
}

async function timedClick(page, label) {
  // Время до кадра, в котором дерево уже перерисовано: React применяет
  // обновление после обработчика, поэтому мерить один `click()` мало.
  return page.evaluate(`(async () => {
    const button = document.querySelector('.ts-tree__head button[aria-label=${JSON.stringify(label)}]');
    const started = performance.now();
    button.click();
    await new Promise((resolve) => requestAnimationFrame(() => setTimeout(resolve, 0)));
    return performance.now() - started;
  })()`);
}

export default async (page) => {
  if (process.env.NO_FOLDERS) {
    await page.evaluate(writeFiles([["Начните здесь.typ", "= Начните здесь\n"], ["Черновик.typ", "= Черновик\n"]]));
    await page.click("text=Продолжить");
    await page.click("text=Открыть vault");
    await page.waitForElement(".ts-tree__head", { timeout: 60000 });
    const disabled = await page.evaluate(`[...document.querySelectorAll(".ts-tree__head button")].map((b) => b.disabled)`);
    check(page, disabled.length === 2 && disabled.every(Boolean), "без папок обе кнопки выключены");
    return;
  }
  await page.evaluate(writeFiles(FILES));
  await page.click("text=Продолжить");
  await page.click("text=Открыть vault");
  await page.waitFor(`${expanded("Исследование")} !== "нет"`, { label: "дерево", timeout: 60000 });
  await page.sleep(500);

  const all = await page.evaluate(visible);
  check(page, all.length > 500, `всё раскрыто: видно ${all.length} строк`);

  // US1: свернуть всё.
  const collapseMs = await timedClick(page, "Свернуть все папки");
  const top = await page.evaluate(visible);
  page.log(`  после «Свернуть все папки»: ${top.join(" | ")}`);
  check(page, top.length === 4, "видны только элементы верхнего уровня (SC-003)");
  check(page, collapseMs <= 200, `«Свернуть все» на 500 папках за ${collapseMs.toFixed(0)} мс (SC-002)`);
  if (process.env.SHOT_COLLAPSED) await page.shot(process.env.SHOT_COLLAPSED);

  // Раскрыть одну папку обычным нажатием — вложенные остаются свёрнутыми (FR-004).
  await page.click("text=Исследование");
  await page.sleep(200);
  check(page, (await page.evaluate(expanded("Исследование"))) === "true", "«Исследование» раскрыта");
  check(page, (await page.evaluate(expanded("Эксперимент 3"))) === "false", "её подпапка осталась свёрнутой");

  // US1: развернуть всё.
  const expandMs = await timedClick(page, "Развернуть все папки");
  check(page, (await page.evaluate(visible)).length === all.length, "«Развернуть все папки» вернуло всё дерево");
  check(page, expandMs <= 500, `«Развернуть все» на 500 папках за ${expandMs.toFixed(0)} мс (SC-002)`);

  // US2: ветка из контекстного меню.
  await timedClick(page, "Свернуть все папки");
  const openMenu = (name) => page.evaluate(`(() => {
    const button = [...document.querySelectorAll("[role=tree] [role=treeitem] > button")]
      .find((el) => el.textContent.trim().startsWith(${JSON.stringify(name)}));
    const r = button.getBoundingClientRect();
    button.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: r.x + 10, clientY: r.y + 5 }));
  })()`);
  await openMenu("Исследование");
  await page.waitForElement("text=Развернуть вложенные");
  await page.click("text=Развернуть вложенные");
  await page.sleep(200);
  check(page, (await page.evaluate(expanded("Эксперимент 3"))) === "true", "«Развернуть вложенные» раскрыло подпапку");
  check(page, (await page.evaluate(expanded("Доклады"))) === "false", "остальное дерево не изменилось");
  if (process.env.SHOT_MENU) {
    await openMenu("Исследование");
    await page.sleep(200);
    await page.shot(process.env.SHOT_MENU);
    await page.press("Escape");
  }
  await openMenu("Исследование");
  await page.click("text=Свернуть вложенные");
  await page.sleep(200);
  check(page, (await page.evaluate(expanded("Исследование"))) === "false", "«Свернуть вложенные» свернуло папку");
  await page.click("text=Исследование");
  await page.sleep(200);
  check(page, (await page.evaluate(expanded("Эксперимент 3"))) === "false", "и её подпапку");

  // У папки без подпапок пунктов ветки нет (FR-005).
  await openMenu("Доклады");
  await page.sleep(300);
  const items = await page.evaluate(`[...document.querySelectorAll("[role=menuitem]")].map((el) => el.textContent.trim())`);
  check(page, items.length > 0 && !items.some((t) => t.includes("вложенные")), "у «Доклады» без подпапок пунктов ветки нет");
  await page.press("Escape");
};
