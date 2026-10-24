// Серверный режим web (фича 037, quickstart.md): стартовый экран → «Сервер» → вход →
// «Мои vault» → создать и открыть vault → заметка от «другой вкладки» приходит сама →
// правка → перезагрузка — текст на месте, на сервере.
//
// Нужны запущенные `typsidian-server serve` (аккаунт anna, пароль SERVER_PASSWORD) и
// `pnpm dev` (он проксирует `/api` на сервер):
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/server-mode.mjs --timeout 60000 --shot /tmp/server.png
//
// Селекторы — по классам, а не по тексту: язык интерфейса headless-браузера
// зависит от окружения.
const PASSWORD = process.env.SERVER_PASSWORD ?? "правильный пароль";
const VAULT = `Диссертация ${Date.now() % 100000}`;

export default async (page) => {
  await page.waitForElement(".ts-server-choice");
  await page.click("label.ts-server-choice:has(input[value=server])");
  await page.click(".ts-server-card button[type=submit]");

  // Неверный пароль — сообщение без уточнения.
  await page.click("input[name=username]");
  await page.type("anna");
  await page.click("input[name=password]");
  await page.type("неверный пароль!");
  await page.click(".ts-server-card button[type=submit]");
  await page.waitForElement(".ts-server-alert");
  page.log("— неверный вход: " + (await page.text(".ts-server-alert")));

  await page.evaluate(`(() => {
    const input = document.querySelector("input[name=password]");
    const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
    set.call(input, "");
    input.dispatchEvent(new Event("input", { bubbles: true }));
  })()`);
  await page.click("input[name=password]");
  await page.type(PASSWORD);
  await page.click(".ts-server-card button[type=submit]");

  await page.waitForElement(".ts-server-list__create input");
  await page.click(".ts-server-list__create input");
  await page.type(VAULT);
  await page.click(".ts-server-list__create button");
  await page.waitFor(
    `[...document.querySelectorAll(".ts-server-list__row b")].some((b) => b.textContent === ${JSON.stringify(VAULT)})`,
    { label: "новый vault в списке" },
  );
  page.log("— мои vault:\n" + (await page.text(".ts-server-card")));
  if (process.env.SERVER_SHOT_LIST) await page.shot(process.env.SERVER_SHOT_LIST);
  // Строка именно этого vault: список отсортирован, и прежние прогоны оставляют свои.
  await page.evaluate(`[...document.querySelectorAll(".ts-server-list__row")]
    .find((row) => row.querySelector("b")?.textContent === ${JSON.stringify(VAULT)})
    .querySelector(".ts-btn--outline").click()`);

  // Рабочая область открылась.
  await page.waitForElement(".ts-tree, [role=tree]", { timeout: 60000 });
  await page.sleep(1500);

  // Заметка из «другой вкладки» — прямой вызов API без id вкладки: сервер разошлёт
  // её всем вкладкам, и дерево обновится само (US4).
  const created = await page.evaluate(`(async () => {
    const me = await (await fetch("/api/me")).json();
    const base = "/api/vaults/" + encodeURIComponent(${JSON.stringify(VAULT)}) + "/invoke/";
    const call = (cmd, args) => fetch(base + cmd, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Typsidian-CSRF": me.csrf },
      body: JSON.stringify(args),
    }).then((r) => r.status);
    return [await call("create", { path: "глава.typ" }),
            await call("write", { path: "глава.typ", content: "= Введение\\n" })];
  })()`);
  page.log("— создание из другой вкладки: " + JSON.stringify(created));
  await page.waitForElement("text=глава", { timeout: 10000 });
  page.log("— дерево обновилось само");

  await page.click("text=глава");
  await page.waitForElement(".cm-content");
  await page.sleep(800);
  await page.click(".cm-content");
  await page.press("End", { ctrl: true });
  await page.type("Текст с сервера.\n");
  // Автосохранение.
  await page.sleep(3000);
  if (process.env.SERVER_SHOT_EDITOR) await page.shot(process.env.SERVER_SHOT_EDITOR);

  await offline(page);

  await page.reload();
  await page.waitForElement(".ts-tree, [role=tree]", { timeout: 60000 });
  const text = await page.evaluate(`(async () => {
    const me = await (await fetch("/api/me")).json();
    const r = await fetch("/api/vaults/" + encodeURIComponent(${JSON.stringify(VAULT)}) + "/invoke/read", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Typsidian-CSRF": me.csrf },
      body: JSON.stringify({ path: "глава.typ" }),
    });
    return r.json();
  })()`);
  page.log("— после перезагрузки на сервере:\n" + text);
  if (!String(text).includes("Текст с сервера.")) throw new Error("правка не сохранилась на сервере");
  if (process.env.SERVER_STOP_CMD && !String(text).includes("Без связи."))
    throw new Error("правка без связи потерялась");

  // Настройки → «Аккаунт»: входы этого аккаунта.
  // Кнопка настроек — последняя переключаемая в шапке (текст зависит от языка).
  await page.evaluate(`[...document.querySelectorAll("button.ts-btn[aria-pressed]")].at(-1).click()`);
  await page.waitForElement(".ts-settings__navitem");
  await page.click(".ts-settings__navitem");
  await page.waitForElement(".ts-server-sessions__row");
  page.log("— аккаунт:\n" + (await page.text(".ts-server-account")));
  if (process.env.SERVER_SHOT_ACCOUNT) await page.shot(process.env.SERVER_SHOT_ACCOUNT);
};

// Обрыв связи (FR-010, SC-004) — если сценарию дали команды остановки и запуска
// сервера: SERVER_STOP_CMD, SERVER_START_CMD.
export async function offline(page) {
  const { execSync } = await import("node:child_process");
  const stop = process.env.SERVER_STOP_CMD;
  const start = process.env.SERVER_START_CMD;
  if (!stop || !start) return;
  await page.click("text=глава");
  await page.waitForElement(".cm-content");
  execSync(stop);
  await page.click(".cm-content");
  await page.press("End", { ctrl: true });
  await page.type("Без связи.\n");
  await page.waitForElement(".ts-server-banner", { timeout: 20000 });
  page.log("— баннер: " + (await page.text(".ts-server-banner")));
  if (process.env.SERVER_SHOT_OFFLINE) await page.shot(process.env.SERVER_SHOT_OFFLINE);
  execSync(start);
  // «Повторить», если очередной повтор ещё не вернул связь сам.
  await page.evaluate(`document.querySelector(".ts-server-banner button")?.click()`);
  await page.waitFor(`!document.querySelector(".ts-server-banner")`, { label: "баннер исчез", timeout: 30000 });
  await page.sleep(2000);
}
