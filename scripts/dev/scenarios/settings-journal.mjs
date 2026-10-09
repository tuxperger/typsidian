// Журнал приложения (фича 004, FR-040): раздел «Дополнительно» панели настроек
// показывает события настроек и vault, без значений и путей.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/settings-journal.mjs --opfs-vault
function check(page, ok, label) {
  page.log(`${ok ? "✓" : "✗"} ${label}`);
  if (!ok) throw new Error(`не выполнено: ${label}`);
}

export default async (page) => {
  await page.click("text=Открыть vault");
  await page.waitForElement("text=Настройки", { timeout: 30000 });
  await page.click("text=Настройки");
  await page.click("text=Дополнительно");
  await page.click("text=Показать журнал");
  await page.waitForElement(".ts-journal");
  const journal = await page.text(".ts-journal");
  page.log(journal.trim().split("\n").slice(-6).join("\n"));
  check(page, journal.includes("SettingsLoaded"), "загрузка настроек записана");
  check(page, journal.includes("VaultAdded"), "добавление vault записано");
  check(page, journal.includes("VaultSwitched"), "переход к vault записан");
};
