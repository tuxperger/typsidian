// Предпросмотр (фича 005): сборка идёт вне потока интерфейса и обрывается по
// пределу времени, после чего предпросмотр восстанавливается.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/preview-timeout.mjs --opfs-vault --timeout 40000
const previewText = `[...document.querySelectorAll(".ts-textlayer text")].map((t) => t.textContent).join(" ")`;

const LONG = "\n#for i in range(100000) { for j in range(100000) { for k in range(1000) { } } }\n";

function check(page, ok, label) {
  page.log(`${ok ? "✓" : "✗"} ${label}`);
  if (!ok) throw new Error(`не выполнено: ${label}`);
}

export default async (page) => {
  await page.evaluate(`(async () => {
    const root = await navigator.storage.getDirectory();
    const vault = await root.getDirectoryHandle("vault", { create: true });
    const file = await vault.getFileHandle("заметка.typ", { create: true });
    const stream = await file.createWritable();
    await stream.write("= Привет\\n");
    await stream.close();
    const other = await vault.getFileHandle("быстрая.typ", { create: true });
    const second = await other.createWritable();
    await second.write("= Быстрая заметка\\n");
    await second.close();
  })()`);

  await page.click("text=Открыть vault");
  await page.click("text=заметка");
  await page.waitFor(`(${previewText}).includes("Привет")`, { label: "первая сборка", timeout: 60000 });

  // Вычисление на часы: три вложенных цикла. `#while true {}` не годится — его
  // Typst обрывает сам, счётчиком итераций; рекурсивная функция тоже — её
  // вызовы мемоизируются.
  await page.click(".ts-editor .cm-content");
  await page.press("End", { ctrl: true });
  await page.type(LONG);
  await page.sleep(2000);

  // Пока она идёт, страница отвечает: поток интерфейса не занят.
  const startedAt = Date.now();
  const frames = await page.evaluate(
    `new Promise((done) => { let n = 0; const t0 = performance.now();
       const tick = () => { n += 1; performance.now() - t0 < 1000 ? requestAnimationFrame(tick) : done(n); };
       requestAnimationFrame(tick); })`,
  );
  check(page, frames > 20, `во время сборки интерфейс рисует кадры (${frames} за секунду, ответ за ${Date.now() - startedAt} мс)`);

  await page.waitFor(`document.querySelector(".ts-failure")?.innerText.includes("10")`, {
    label: "сообщение о пределе времени",
    timeout: 20000,
  });
  page.log("  " + (await page.text(".ts-failure")).replace(/\n+/g, " | "));
  check(page, (await page.evaluate(previewText)).includes("Привет"), "прежний вид остался на экране");

  // Исправление: новая сборка поднимает новый воркер и проходит.
  await page.click(".ts-editor .cm-content");
  await page.press("a", { ctrl: true });
  await page.type("= Снова работает\n");
  await page.waitFor(`(${previewText}).includes("Снова работает")`, {
    label: "сборка после обрыва",
    timeout: 30000,
  });
  check(page, true, "после обрыва предпросмотр восстановился");

  // Отмена: посреди долгой сборки открыть другую заметку. Её вид обязан
  // появиться раньше, чем истёк бы предел первой сборки.
  await page.click(".ts-editor .cm-content");
  await page.press("End", { ctrl: true });
  await page.type(LONG);
  await page.sleep(1500);
  const switchedAt = Date.now();
  await page.click("text=быстрая");
  await page.waitFor(`(${previewText}).includes("Быстрая заметка")`, {
    label: "вид другой заметки",
    timeout: 8000,
  });
  const waited = Date.now() - switchedAt;
  check(page, waited < 8000, `переход к другой заметке отменил сборку (вид через ${waited} мс)`);
};
