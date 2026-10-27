// Конфликты синхронизации (фича 040): vault в приватном хранилище браузера →
// история → конфликтные копии Syncthing → список с отметками и «Принять
// совмещённые» → редактор слияния с подсветкой, «везде их», ручная правка.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/sync-merge.mjs --opfs-vault --timeout 60000 --shot /tmp/merge.png
//
// `MERGE_LIST_SHOT` — снимок списка конфликтов.

const BASE = [
  "= Глава 2. Методика",
  "",
  "Эксперимент проводился в три этапа.",
  "",
  "Выборка: 40 образцов стали 09Г2С.",
  "",
  "Нагрузка менялась от 10 до 50 кН.",
  "",
  "Результаты сведены в таблицу.",
  "",
].join("\n");

const OURS = BASE.replace("в три этапа", "в четыре этапа").replace("от 10 до 50", "от 10 до 60");
const THEIRS = BASE.replace("40 образцов", "42 образца").replace("от 10 до 50", "от 5 до 50");
const PLAN = "= План\n\nПервое.\n\nВторое.\n\nТретье.\n";
const COPY = "глава.sync-conflict-20261026-091400-ABCDEFG.typ";
const PLAN_COPY = "план.sync-conflict-20261026-091400-ABCDEFG.typ";

const writeFiles = (files) => `(async () => {
  const root = await navigator.storage.getDirectory();
  const vault = await root.getDirectoryHandle("vault", { create: true });
  for (const [name, text] of ${JSON.stringify(files)}) {
    const file = await vault.getFileHandle(name, { create: true });
    const stream = await file.createWritable();
    await stream.write(text);
    await stream.close();
  }
})()`;

export default async (page) => {
  await page.evaluate(
    writeFiles([
      ["глава.typ", BASE],
      ["план.typ", PLAN],
    ]),
  );
  // Экран выбора места (037): папка на устройстве.
  await page.click("text=Продолжить");
  await page.click("text=Открыть vault");
  await page.waitForElement("text=Синхронизация", { timeout: 30000 });
  await page.click("text=Синхронизация");
  await page.waitForElement("text=Включить историю…", { timeout: 30000 });
  await page.click("text=Включить историю…");
  await page.waitForElement(".ts-sync-wizard");
  for (let step = 0; step < 4; step += 1) {
    const text = await page.text(".ts-sync-wizard");
    if (text.includes("Включить и сделать первый снимок")) break;
    if (text.includes("Дальше")) await page.click("text=Дальше");
    await page.sleep(300);
  }
  // Автор снимков: поля мастера, если git-автора нет.
  const inputs = await page.evaluate(
    `document.querySelectorAll(".ts-sync-wizard input").length`,
  );
  if (inputs >= 2) {
    await page.evaluate(`document.querySelectorAll(".ts-sync-wizard input")[0].focus()`);
    await page.type("Анна");
    await page.evaluate(`document.querySelectorAll(".ts-sync-wizard input")[1].focus()`);
    await page.type("anna@example.org");
  }
  await page.click("text=Включить и сделать первый снимок");
  await page.waitFor(`!document.querySelector(".ts-sync-wizard")`, { timeout: 30000 });
  page.log("— история включена");

  // Своя правка и копии с другого устройства: одна спорит, другая совмещается сама.
  await page.evaluate(
    writeFiles([
      ["глава.typ", OURS],
      [COPY, THEIRS],
      ["план.typ", PLAN.replace("Первое.", "Первое, своё.")],
      [PLAN_COPY, PLAN.replace("Третье.", "Третье, их.")],
    ]),
  );
  // Панель перечитывает конфликты при открытии.
  await page.press("Escape");
  await page.sleep(500);
  await page.click("text=Синхронизация");
  await page.waitForElement(".ts-sync-conflicts", { timeout: 30000 });
  await page.sleep(800);
  page.log("— список:\n" + (await page.text(".ts-sync-conflicts")));
  if (process.env.MERGE_LIST_SHOT) await page.shot(process.env.MERGE_LIST_SHOT);

  await page.click("text=Принять совмещённые");
  await page.waitForElement(".ts-merge-confirm");
  page.log("— подтверждение: " + (await page.text(".ts-merge-confirm")));
  await page.click(".ts-merge-confirm .ts-btn--accent");
  await page.sleep(1500);
  page.log("— после «Принять совмещённые»:\n" + (await page.text(".ts-sync-conflicts")));

  await page.click(".ts-sync-conflicts .ts-sync-list__row");
  await page.waitForElement(".ts-sync-merge .ts-merge-legend");
  await page.waitForElement(".ts-merge-result .cm-line");
  await page.sleep(500);
  const marks = await page.evaluate(`({
    ours: [...document.querySelectorAll('[data-column="ours"] .ts-merge-line')].map((l) => l.className.replace("ts-merge-line ", "")),
    theirs: [...document.querySelectorAll('[data-column="theirs"] .ts-merge-line')].map((l) => l.className.replace("ts-merge-line ", "")),
    result: [...document.querySelectorAll(".ts-merge-result .cm-line")].map((l) => l.getAttribute("data-mark")),
  })`);
  page.log("— пометки: " + JSON.stringify(marks));

  await page.click("text=везде их");
  await page.sleep(500);
  page.log(
    "— результат после «везде их»: " +
      JSON.stringify(
        await page.evaluate(
          `[...document.querySelectorAll(".ts-merge-result .cm-line")].map((l) => [l.getAttribute("data-mark"), l.textContent])`,
        ),
      ),
  );

  // Ручная правка строки результата → ✎, затем «везде своё» просит подтверждение.
  await page.click(".ts-merge-result .cm-line:nth-child(3)");
  await page.press("End");
  await page.type(" (поправлено)");
  await page.sleep(700);
  page.log(
    "— после правки: " +
      JSON.stringify(
        await page.evaluate(
          `[...document.querySelectorAll(".ts-merge-result .cm-line")].map((l) => l.getAttribute("data-mark"))`,
        ),
      ),
  );
  await page.click("text=везде своё");
  await page.waitForElement(".ts-sync-merge .ts-merge-confirm");
  page.log("— подтверждение замены: " + (await page.text(".ts-sync-merge .ts-merge-confirm")));
};
