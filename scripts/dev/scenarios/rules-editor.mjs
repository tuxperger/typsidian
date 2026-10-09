// Редактор правил исключения (фича 012): vault в приватном хранилище браузера
// → панель синхронизации → правила истории → правка → сохранение.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/rules-editor.mjs --opfs-vault --shot /tmp/rules.png
export default async (page) => {
  // Заметка и свой `.gitignore` — до открытия vault.
  await page.evaluate(`(async () => {
    const root = await navigator.storage.getDirectory();
    const vault = await root.getDirectoryHandle("vault", { create: true });
    const write = async (name, text) => {
      const file = await vault.getFileHandle(name, { create: true });
      const stream = await file.createWritable();
      await stream.write(text);
      await stream.close();
    };
    await write("заметка.typ", "= Заметка\\n");
    await write(".gitignore", "*.pdf\\n");
  })()`);

  await page.click("text=Открыть vault");
  await page.waitForElement("text=Синхронизация", { timeout: 30000 });
  await page.click("text=Синхронизация");
  await page.waitForElement("text=Открыть", { timeout: 30000 });
  page.log("— панель синхронизации:\n" + (await page.text(".ts-sync")));

  await page.click(".ts-sync-rules-list li:first-child button");
  await page.waitForElement(".ts-sync-rules-editor .cm-content");
  await page.sleep(600);
  page.log("— редактор:\n" + (await page.text(".ts-sync-rules-editor")));

  // Правка: своя строка в начало файла, затем сохранение.
  await page.click(".ts-sync-rules-editor .cm-content");
  await page.press("Home", { ctrl: true });
  await page.type("черновики/\n");
  await page.sleep(600);
  // Без правил приложения сохранение спрашивает подтверждение и ничего не пишет.
  await page.click("text=Сохранить");
  await page.waitForElement("text=Сохранить без них");
  page.log("— подтверждение:\n" + (await page.text(".ts-sync-rules-editor")));
  await page.click("text=Вернуться к правке");

  // Возврат блока приложения и обычное сохранение.
  await page.click("text=Вернуть правила приложения");
  await page.sleep(800);
  if (process.env.RULES_SHOT) await page.shot(process.env.RULES_SHOT);
  await page.click("text=Сохранить");
  await page.sleep(1200);
  page.log("— после сохранения:\n" + (await page.text(".ts-sync-rules-editor")));

  const saved = await page.evaluate(`(async () => {
    const root = await navigator.storage.getDirectory();
    const vault = await root.getDirectoryHandle("vault");
    const file = await (await vault.getFileHandle(".gitignore")).getFile();
    return file.text();
  })()`);
  page.log("— .gitignore на диске:\n" + saved);
};
