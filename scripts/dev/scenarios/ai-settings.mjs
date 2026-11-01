// Обучение с ИИ (фича 050, US4): включение с подтверждением, свой адрес без
// шифрования, отказ соединения и запись в журнале отправленного.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/ai-settings.mjs --opfs-vault --shot /путь/ai.png
//
// Адрес вне устройства (192.0.2.0/24 — документационная сеть) без шифрования
// требует подтверждения (FR-010); loopback его не требует. Порт 9 браузер
// отклоняет ещё до соединения: запрос гарантированно не дойдёт до модели, а ядро
// обязано показать отказ и записать попытку в журнал до отправки (FR-012).
function check(page, ok, label) {
  page.log(`${ok ? "✓" : "✗"} ${label}`);
  if (!ok) throw new Error(`не выполнено: ${label}`);
}

const ENDPOINT = "http://192.0.2.1:9/v1";
const MODEL = "probe-model";

async function fill(page, selector, text) {
  await page.click(selector);
  await page.type(text);
  // Поля сохраняются по потере фокуса.
  await page.click(".ts-settings__head h2");
}

export default async (page) => {
  await page.click("text=Продолжить"); // экран выбора места (037)
  await page.click("text=Открыть папку…");
  await page.waitForElement("text=Настройки", { timeout: 30000 });
  await page.click("text=Настройки");
  await page.click("text=Обучение с ИИ");

  await page.waitForElement("text=Включить для этого vault");
  const off = await page.text();
  check(page, off.includes("Пока выключено"), "по умолчанию выключено");

  await page.click("text=Включить для этого vault");
  await page.waitForElement("[role=alertdialog]");
  check(page, (await page.text("[role=alertdialog]")).includes("журнале"), "подтверждение перечисляет, что уходит");
  await page.click("text=Включить");
  await page.waitForElement("text=Свой адрес");

  await page.click("text=Свой адрес");
  await page.waitForElement('input[placeholder^="http"]');
  await fill(page, 'input[placeholder^="http"]', ENDPOINT);
  await fill(page, 'input[list="ts-ai-models"]', MODEL);

  await page.click("text=Проверить");
  await page.waitForElement("text=Всё равно подключить");
  check(page, true, "незашифрованный адрес требует подтверждения");
  await page.click("text=Всё равно подключить");
  await page.sleep(300);

  await page.click("text=Проверить");
  await page.waitForElement(".ts-ai__error", { timeout: 35000 });
  const failure = await page.text(".ts-ai__error");
  page.log(failure);
  check(
    page,
    failure.includes("Нет связи") || failure.includes("Браузер не пропустил"),
    "отказ соединения показан понятным текстом",
  );

  await page.click("text=Журнал отправленного");
  await page.waitForElement(".ts-ai__journal table");
  const journal = await page.text(".ts-ai__journal");
  page.log(journal.trim().split("\n").slice(0, 8).join("\n"));
  check(page, journal.includes(MODEL), "попытка записана в журнал");
  check(page, journal.includes(ENDPOINT), "в журнале виден адрес");
  check(page, !journal.includes("оборвано"), "у записи есть итог");
};
