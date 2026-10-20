// Импорт из Obsidian со своей историей vault (фича 021, T021, FR-012): после
// импорта предлагается снимок истории 011, и он попадает в историю.
// Источник — packages/core/tests/fixtures/obsidian/, как в `import.mjs`.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/import-snapshot.mjs --opfs-vault --shot /tmp/import-snapshot.png
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const FIXTURES = resolve(
  fileURLToPath(new URL(".", import.meta.url)),
  "../../../packages/core/tests/fixtures/obsidian",
);

function collect(dir) {
  const files = [];
  const walk = (current) => {
    for (const entry of readdirSync(current)) {
      const full = join(current, entry);
      if (statSync(full).isDirectory()) walk(full);
      else {
        files.push({
          path: relative(dir, full).split(sep).join("/"),
          base64: readFileSync(full).toString("base64"),
        });
      }
    }
  };
  walk(dir);
  return files;
}

const clickButton = (text) => `(() => {
  const button = [...document.querySelectorAll("button")].find((b) => b.textContent.trim() === ${JSON.stringify(text)} && !b.disabled);
  button?.click();
  return Boolean(button);
})()`;

export default async (page) => {
  const failures = [];
  const check = (label, ok) => {
    page.log(`${ok ? "✓" : "✗"} ${label}`);
    if (!ok) failures.push(label);
  };

  const files = collect(FIXTURES);
  await page.evaluate(`(async () => {
    const root = await navigator.storage.getDirectory();
    const src = await root.getDirectoryHandle("obsidian-src", { create: true });
    for (const file of ${JSON.stringify(files)}) {
      const parts = file.path.split("/");
      let dir = src;
      for (const part of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(part, { create: true });
      const handle = await dir.getFileHandle(parts.at(-1), { create: true });
      const stream = await handle.createWritable();
      await stream.write(Uint8Array.from(atob(file.base64), (c) => c.charCodeAt(0)));
      await stream.close();
    }
    const vault = await root.getDirectoryHandle("vault", { create: true });
    const note = await vault.getFileHandle("корень.typ", { create: true });
    const stream = await note.createWritable();
    await stream.write("= Корень\\n");
    await stream.close();
  })()`);
  await page.click("text=Открыть vault");
  await page.waitForElement("text=корень");
  await page.sleep(1500);

  // ── История vault: мастер включения ──────────────────────────────────
  await page.click(".ts-sync-pill");
  await page.waitFor(`[...document.querySelectorAll("button")].some((b) => b.textContent.includes("Включить историю"))`, { label: "панель синхронизации" });
  await page.evaluate(clickButton("Включить историю…"));
  await page.waitFor(`Boolean(document.querySelector("[aria-label='Включить историю'], .ts-dialog"))`, { label: "мастер" });
  // Шаги правил — «Дальше»; шаг автора — имя и почта, затем включение.
  for (let step = 0; step < 4; step++) {
    await page.sleep(400);
    const author = await page.evaluate(`document.querySelectorAll("[role=dialog] input").length`);
    if (author >= 2) break;
    await page.evaluate(clickButton("Дальше"));
  }
  await page.evaluate(`(() => {
    const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
    const [name, email] = document.querySelectorAll("[role=dialog] input");
    set.call(name, "Тест");
    name.dispatchEvent(new Event("input", { bubbles: true }));
    set.call(email, "test@example.com");
    email.dispatchEvent(new Event("input", { bubbles: true }));
    return true;
  })()`);
  await page.sleep(300);
  check("мастер: включение доступно", await page.evaluate(clickButton("Включить и сделать первый снимок")));
  await page.waitFor(`!document.querySelector("[role=dialog]")`, { timeout: 30000, label: "мастер закрылся" });
  await page.waitFor(`!document.querySelector(".ts-sync-pill")?.textContent.includes("выключена")`, { timeout: 30000, label: "история включена" });
  check("история включена", true);
  await page.sleep(1500);
  await page.press("Escape");
  await page.sleep(500);

  // ── Импорт ───────────────────────────────────────────────────────────
  await page.evaluate(`(() => {
    window.showDirectoryPicker = async () => (await navigator.storage.getDirectory()).getDirectoryHandle("obsidian-src");
    return true;
  })()`);
  await page.click(".ts-btn--cmd");
  await page.waitForElement(".ts-palette");
  await page.type("Obsidian");
  await page.sleep(300);
  await page.press("Enter");
  await page.waitForElement(".ts-import");
  await page.click("text=Выбрать…");
  await page.waitFor(`Boolean(document.querySelector('.ts-import__facts b'))`, { label: "обзор" });
  await page.click(".ts-import .ts-btn--accent");
  await page.waitForElement(".ts-import-card");
  await page.waitFor(`[...document.querySelectorAll(".ts-import-card button")].some((b) => b.textContent.trim() === "Сделать снимок")`, { timeout: 60000, label: "предложение снимка" });
  check("после импорта предложен снимок истории", true);
  await page.evaluate(clickButton("Сделать снимок"));
  await page.waitFor(`document.querySelector(".ts-import-card")?.textContent.includes("Снимок сделан.")`, { timeout: 20000, label: "снимок сделан" });
  check("снимок сделан", true);

  // В истории — снимок импорта.
  await page.evaluate(clickButton("Закрыть"));
  await page.click(".ts-sync-pill");
  await page.waitFor(`document.body.textContent.includes("Импорт из Obsidian")`, { timeout: 10000, label: "снимок в истории" });
  check("снимок «Импорт из Obsidian» в истории", true);

  if (failures.length > 0) throw new Error(`провалов: ${failures.length}`);
};
