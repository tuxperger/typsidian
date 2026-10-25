// Синхронизация в серверном режиме (фича 038, quickstart.md): вход → новый vault →
// панель «Синхронизация» → мастер включения истории → снимок → отказ адреса ssh и
// адреса внутренней сети.
//
// Нужны запущенные `typsidian-server serve` (аккаунт anna, пароль SERVER_PASSWORD) и
// `pnpm dev` (он проксирует `/api` на сервер):
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/server-sync.mjs --timeout 90000 --shot /tmp/server-sync.png
//
// Команды, которых не видно в интерфейсе, сценарий зовёт прямо через API — так же,
// как их зовёт клиент.
const PASSWORD = process.env.SERVER_PASSWORD ?? "правильный пароль";
const VAULT = `История ${Date.now() % 100000}`;

const api = (command, args) => `(async () => {
  const me = await (await fetch("/api/me")).json();
  const r = await fetch("/api/vaults/" + encodeURIComponent(${JSON.stringify(VAULT)}) + "/invoke/${command}", {
    method: "POST",
    headers: { "Content-Type": "application/json", "X-Typsidian-CSRF": me.csrf },
    body: JSON.stringify(${JSON.stringify(args)}),
  });
  return { status: r.status, body: await r.json().catch(() => null) };
})()`;

export default async (page) => {
  await page.waitForElement(".ts-server-choice");
  await page.click("label.ts-server-choice:has(input[value=server])");
  await page.click(".ts-server-card button[type=submit]");
  await page.click("input[name=username]");
  await page.type("anna");
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
  await page.evaluate(`[...document.querySelectorAll(".ts-server-list__row")]
    .find((row) => row.querySelector("b")?.textContent === ${JSON.stringify(VAULT)})
    .querySelector(".ts-btn--outline").click()`);
  await page.waitForElement(".ts-tree, [role=tree]", { timeout: 60000 });

  page.log("— заметка: " + JSON.stringify(await page.evaluate(api("create", { path: "глава.typ" }))));
  await page.evaluate(api("write", { path: "глава.typ", content: "= Первая\n" }));

  // Пилюля синхронизации в шапке есть: сессия открыта сервером.
  await page.waitForElement(".ts-sync-pill", { timeout: 20000 });
  page.log("— индикатор: " + (await page.text(".ts-sync-pill")));
  const capabilities = await page.evaluate(api("sync_capabilities", {}));
  page.log("— возможности: " + JSON.stringify(capabilities.body));
  if (capabilities.body?.ssh !== false || capabilities.body?.server !== true)
    throw new Error("на сервере ssh должен быть выключен");

  // Включение истории — тем же путём, что мастер: автор, затем первый снимок.
  await page.evaluate(
    api("sync_set_author", { author: { name: "Анна", email: "anna@example.org" } }),
  );
  const enabled = await page.evaluate(api("sync_enable_history", { message: "Начало" }));
  page.log("— история включена: " + enabled.status);
  if (enabled.status !== 200) throw new Error(JSON.stringify(enabled.body));

  await page.evaluate(api("write", { path: "глава.typ", content: "= Вторая\n" }));
  const snap = await page.evaluate(
    api("sync_snapshot", {
      paths: null,
      message: "Правка",
      autoPrefix: "Авто",
      flushed: true,
      confirmNotKeeper: false,
    }),
  );
  page.log("— снимок: " + snap.status);
  const history = await page.evaluate(
    api("sync_history", { path: "глава.typ", cursor: null, limit: 10 }),
  );
  page.log("— снимков заметки: " + history.body?.snapshots?.length);
  if (history.body?.snapshots?.length !== 2) throw new Error("ожидалось два снимка");

  for (const url of ["git@github.com:me/notes.git", "https://192.168.1.10/notes.git"]) {
    const refused = await page.evaluate(
      api("sync_connect_remote", { url, credentials: { user: null, token: null } }),
    );
    page.log(`— ${url}: ${refused.status} ${JSON.stringify(refused.body)}`);
    if (refused.status !== 400) throw new Error("адрес должен быть отклонён: " + url);
  }

  // История включена мимо интерфейса — после перезагрузки панель её видит.
  await page.reload();
  await page.waitForElement(".ts-sync-pill", { timeout: 60000 });
  await page.sleep(1500);
  page.log("— индикатор после перезагрузки: " + (await page.text(".ts-sync-pill")));
  await page.click(".ts-sync-pill");
  await page.sleep(1500);
  if (process.env.SERVER_SYNC_SHOT_PANEL) await page.shot(process.env.SERVER_SYNC_SHOT_PANEL);
};
