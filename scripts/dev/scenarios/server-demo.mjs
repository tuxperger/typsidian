// Демо-сервер (фича 039, quickstart.md): стартовый экран → «Сервер» → «Попробовать
// без регистрации» → vault «Демо» с примерами → пилюля остатка в шапке → заметки
// примеров собираются без ошибок → вход паролем закрыт, удалённый репозиторий
// выключен.
//
// Нужны запущенные `typsidian-server serve --demo` и `pnpm dev` (он проксирует
// `/api` на сервер):
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/server-demo.mjs --timeout 90000 --shot /tmp/demo.png
//
// Селекторы — по классам: язык интерфейса headless-браузера зависит от окружения.

const NOTES = ["Начните здесь", "Обзор проектов", "Методика", "Задачи недели", "Доклад"];

export default async (page) => {
  await page.waitForElement(".ts-server-choice");
  await page.click("label.ts-server-choice:has(input[value=server])");
  await page.click(".ts-server-card button[type=submit]");

  // Экран входа демо: одна кнопка, формы входа нет (D1).
  await page.waitForElement(".ts-server-demo__try");
  if (await page.evaluate(`!!document.querySelector("input[name=password]")`)) {
    throw new Error("на демо-сервере есть форма входа");
  }
  page.log("— вход: " + (await page.text(".ts-server-card")));
  if (process.env.DEMO_SHOT_LOGIN) await page.shot(process.env.DEMO_SHOT_LOGIN);
  await page.click(".ts-server-demo__try");

  // Рабочая область с примерами и пилюлей остатка (D2).
  await page.waitForElement(".ts-demo-pill", { timeout: 60000 });
  await page.waitForElement("text=Начните здесь", { timeout: 30000 });
  await page.sleep(2500);
  page.log("— пилюля: " + (await page.text(".ts-demo-pill")));

  for (const note of NOTES) {
    if (note === "Методика") {
      // Папка дерева, а не слово в тексте заметки.
      await page.evaluate(`[...document.querySelectorAll("[role=tree] [role=treeitem]")]
        .find((item) => item.textContent.trim().startsWith("Исследование"))?.click()`);
      await page.sleep(800);
    }
    await page.click(`text=${note}`);
    await page.sleep(4000);
    const problems = await page.evaluate(
      `document.querySelectorAll("button.ts-diagnostics__place").length`,
    );
    page.log(`— ${note}: ошибок сборки ${problems}`);
    if (problems > 0) {
      page.log(await page.text(".ts-diagnostics"));
      throw new Error(`заметка «${note}» собирается с ошибками`);
    }
    if (note === "Обзор проектов" && process.env.DEMO_SHOT_WORK) {
      await page.shot(process.env.DEMO_SHOT_WORK);
    }
  }

  // Вход паролем закрыт; удалённый репозиторий выключен.
  const closed = await page.evaluate(`(async () => {
    const me = await (await fetch("/api/me")).json();
    const login = await fetch("/api/login", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ login: "anna", password: "правильный пароль" }),
    });
    const vaults = await fetch("/api/vaults", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Typsidian-CSRF": me.csrf },
      body: JSON.stringify({ name: "Ещё" }),
    });
    return { login: login.status, createVault: vaults.status, expiresAt: me.expiresAt };
  })()`);
  page.log("— закрытые точки: " + JSON.stringify(closed));
  if (closed.login !== 404 || closed.createVault !== 403) throw new Error("вход или vault не закрыты");

  await page.click(".ts-sync-pill");
  await page.sleep(1500);
  const connect = await page.evaluate(
    `[...document.querySelectorAll(".ts-sync__section .ts-btn")].find((b) => b.textContent.includes("…") && b.closest("[aria-label]")?.querySelector(".ts-dialog__hint"))?.disabled`,
  );
  page.log("— «Подключить…» выключено: " + connect);
  if (process.env.DEMO_SHOT_SYNC) await page.shot(process.env.DEMO_SHOT_SYNC);
};
