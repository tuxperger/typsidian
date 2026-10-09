// Предпросмотр (фича 005): связанные файлы, переход к месту ошибки, удержание
// места при пересборке.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/preview.mjs --opfs-vault
const write = (name, text) => `(async () => {
  const root = await navigator.storage.getDirectory();
  const vault = await root.getDirectoryHandle("vault", { create: true });
  const file = await vault.getFileHandle(${JSON.stringify(name)}, { create: true });
  const stream = await file.createWritable();
  await stream.write(${JSON.stringify(text)});
  await stream.close();
})()`;

const paragraphs = Array.from({ length: 60 }, (_, i) => `Абзац номер ${i + 1}. #lorem(40)`);
const NOTE = `#import "оформление.typ": метка\n\n= Заметка\n\n${paragraphs.join("\n\n")}\n\n#метка\n\n#read("данные.txt")\n`;

const pageCount = `document.querySelectorAll(".ts-preview__page").length`;
const previewText = `[...document.querySelectorAll(".ts-textlayer text")].map((t) => t.textContent).join(" ")`;

function check(page, ok, label) {
  page.log(`${ok ? "✓" : "✗"} ${label}`);
  if (!ok) throw new Error(`не выполнено: ${label}`);
}

export default async (page) => {
  await page.evaluate(write("оформление.typ", "#let метка = [МЕТКА-ОДИН]\n"));
  await page.evaluate(write("данные.txt", "ДАННЫЕ-ОДИН"));
  await page.evaluate(write("заметка.typ", NOTE));

  await page.click("text=Открыть vault");
  await page.click("text=заметка");
  await page.waitFor(`${pageCount} > 1`, { label: "страницы предпросмотра", timeout: 60000 });
  check(page, (await page.evaluate(previewText)).includes("МЕТКА-ОДИН"), "импорт вошёл в сборку");

  // Удержание места: прокрутить к середине, вставить абзац в начало текста.
  const seen = await page.evaluate(`(() => {
    const box = document.querySelector(".ts-preview");
    box.scrollTop = box.scrollHeight / 2;
    const top = box.getBoundingClientRect().top;
    const run = [...box.querySelectorAll(".ts-textlayer text")]
      .find((el) => el.getBoundingClientRect().bottom > top);
    return { text: run.textContent, offset: run.getBoundingClientRect().top - top };
  })()`);
  await page.click(".ts-editor .cm-content");
  await page.press("Home", { ctrl: true });
  await page.type("#lorem(300)\n\n");
  await page.sleep(2500);
  const after = await page.evaluate(`(() => {
    const box = document.querySelector(".ts-preview");
    const top = box.getBoundingClientRect().top;
    const run = [...box.querySelectorAll(".ts-textlayer text")]
      .find((el) => el.getBoundingClientRect().bottom > top);
    return { text: run.textContent, offset: run.getBoundingClientRect().top - top, scrollTop: box.scrollTop };
  })()`);
  page.log(`  до: «${seen.text}» @${seen.offset.toFixed(1)}  после: «${after.text}» @${after.offset.toFixed(1)}`);
  check(page, after.text === seen.text, "после вставки выше на экране тот же фрагмент");
  check(page, Math.abs(after.offset - seen.offset) < 2, "и на том же месте экрана");

  // Связанный файл изменился на диске — предпросмотр пересобирается сам.
  await page.evaluate(write("оформление.typ", "#let метка = [МЕТКА-ДВА]\n"));
  await page.waitFor(`(${previewText}).includes("МЕТКА-ДВА")`, {
    label: "пересборка после правки связанного файла",
    timeout: 15000,
  });
  check(page, true, "правка связанного файла пересобрала предпросмотр");

  // То же для файла, которого нет в индексе vault (не заметка и не доска):
  // событий о нём нет, правку замечает отметка зависимостей.
  await page.evaluate(write("данные.txt", "ДАННЫЕ-ДВА-И-ДЛИННЕЕ"));
  await page.waitFor(`(${previewText}).includes("ДАННЫЕ-ДВА")`, {
    label: "пересборка после правки файла данных",
    timeout: 15000,
  });
  check(page, true, "правка файла вне индекса пересобрала предпросмотр");

  // Ошибка в связанном файле: место названо, по нему можно перейти.
  await page.evaluate(write("оформление.typ", "#let метка = [МЕТКА]\n#let = сломано\n"));
  await page.waitForElement("button.ts-diagnostics__place", { timeout: 15000 });
  const place = await page.text("button.ts-diagnostics__place");
  check(page, place.startsWith("оформление.typ:2:"), `место ошибки названо: ${place}`);
  check(page, (await page.evaluate(pageCount)) > 1, "прежний вид остался на экране");
  await page.click("button.ts-diagnostics__place");
  await page.waitFor(
    `document.querySelector(".ts-editor .cm-content")?.textContent.includes("сломано")`,
    { label: "связанный файл открыт в редакторе" },
  );
  check(page, true, "переход открыл связанный файл");
};
