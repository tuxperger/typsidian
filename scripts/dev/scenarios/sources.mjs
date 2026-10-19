// Библиография vault (фича 017, quickstart.md): файл источников находится
// сам, режим «Источники» колонки заметки, цитирующие заметки, битая цитата
// отмечена в редакторе, палитра «Вставить цитату» и запрос списка литературы.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/sources.mjs --opfs-vault --shot /tmp/017.png

const BIB = `@book{knuth1984,
  author = {Knuth, Donald E.},
  title = {The TeXbook},
  year = {1984},
  publisher = {Addison-Wesley},
}

@article{lamport1986,
  author = {Lamport, Leslie},
  title = {LaTeX: A Document Preparation System},
  year = {1986},
  journal = {Addison-Wesley},
}

@misc{broken2000,
  title = {Незакрытая скобка,
  year = {2000},

`;

const CITING = `= Вёрстка
Классика — @knuth1984, а ещё @nosuchkey.

#bibliography("/references.bib")
`;

const PLAIN = `= Черновик
Здесь пока нет цитат.
`;

const write = (name, text) => `(async () => {
  const root = await navigator.storage.getDirectory();
  const vault = await root.getDirectoryHandle("vault", { create: true });
  const file = await vault.getFileHandle(${JSON.stringify(name)}, { create: true });
  const stream = await file.createWritable();
  await stream.write(${JSON.stringify(text)});
  await stream.close();
})()`;

function check(page, ok, label) {
  page.log(`${ok ? "✓" : "✗"} ${label}`);
  return ok ? 0 : 1;
}

async function openNote(page, name) {
  await page.click(`text=${name}`);
  await page.waitForElement(".ts-editor .cm-content", { timeout: 60000 });
  await page.sleep(300);
}

// Каретка в конец строки с текстом `needle`.
const caretToEnd = (needle) => `(() => {
  const lines = [...document.querySelectorAll(".ts-editor .cm-line")];
  const line = lines.find((el) => el.textContent.includes(${JSON.stringify(needle)}));
  if (!line) return false;
  const range = document.createRange();
  range.selectNodeContents(line);
  range.collapse(false);
  const sel = window.getSelection();
  sel.removeAllRanges();
  sel.addRange(range);
  document.querySelector(".ts-editor .cm-content").focus();
  return true;
})()`;

const editorText = `[...document.querySelectorAll(".ts-editor .cm-line")].map((l) => l.textContent).join("\\n")`;

export default async (page) => {
  let failures = 0;
  await page.evaluate(write("references.bib", BIB));
  await page.evaluate(write("Вёрстка.typ", CITING));
  await page.evaluate(write("Черновик.typ", PLAIN));

  await page.click("text=Открыть vault");
  await openNote(page, "Вёрстка");

  // ── Отметка битой цитаты (FR-006) ──────────────────────────────────────
  try {
    await page.waitForElement(".ts-editor .ts-mark--error", { timeout: 5000 });
  } catch {
    // проверка ниже назовёт провал
  }
  const marked = JSON.parse(
    await page.evaluate(
      `JSON.stringify([...document.querySelectorAll(".ts-editor .ts-mark--error")].map((el) => el.textContent))`,
    ),
  );
  failures += check(page, marked.some((text) => text.includes("nosuchkey")), `битая цитата отмечена: ${JSON.stringify(marked)}`);
  failures += check(page, !marked.some((text) => text.includes("knuth1984")), "известный ключ не отмечен");

  // ── Режим «Источники» (FR-005) ─────────────────────────────────────────
  await page.click(".ts-editor .cm-content");
  await page.press("r", { ctrl: true, alt: true, code: "KeyR" });
  try {
    await page.waitForElement(".ts-sources__row", { timeout: 5000 });
  } catch {
    page.log("✗ Ctrl+Alt+R не открыл «Источники»");
    failures += 1;
    await page.click("text=Источники");
    await page.waitForElement(".ts-sources__row", { timeout: 5000 });
  }
  await page.sleep(800);
  const rows = JSON.parse(
    await page.evaluate(
      `JSON.stringify([...document.querySelectorAll(".ts-sources__row")].map((el) => [el.querySelector(".ts-sources__key")?.textContent, el.classList.contains("ts-sources__row--uncited")]))`,
    ),
  );
  failures += check(page, rows.length === 2, `в панели два источника: ${JSON.stringify(rows)}`);
  failures += check(
    page,
    rows.some(([key, uncited]) => key === "lamport1986" && uncited) &&
      rows.some(([key, uncited]) => key === "knuth1984" && !uncited),
    "нецитируемый помечен после прохода",
  );
  const missing = await page.text(".ts-sources");
  failures += check(page, missing.includes("@nosuchkey"), "ключ без источника — в группе «Нет в файлах»");
  failures += check(
    page,
    (await page.evaluate(`document.querySelector(".ts-sources__group--problems")?.textContent ?? ""`)).includes("references.bib"),
    "негодная запись — в группе «Ошибки в файлах»",
  );

  await page.click(".ts-sources__row");
  await page.sleep(500);
  const card = await page.text(".ts-sources__card");
  failures += check(page, card.includes("Вёрстка"), "у источника видна цитирующая заметка");

  // ── Палитра и вставка (FR-003, FR-004) ─────────────────────────────────
  await openNote(page, "Черновик");
  failures += check(page, (await page.evaluate(caretToEnd("пока нет цитат"))) === true, "каретка в конце строки");
  await page.press("b", { ctrl: true, alt: true, code: "KeyB" });
  await page.waitForElement(".ts-cite__query", { timeout: 5000 });
  await page.type("lamport");
  await page.sleep(400);
  const options = await page.text(".ts-cite__list");
  failures += check(page, options.includes("lamport1986") && !options.includes("knuth1984"), "поиск сужает палитру");
  await page.press("Enter", { code: "Enter" });
  await page.sleep(400);
  const text = await page.evaluate(editorText);
  failures += check(page, text.includes("@lamport1986"), "цитата вставлена у каретки");
  try {
    await page.waitForElement(".ts-cite-offer", { timeout: 3000 });
  } catch {
    // проверка ниже назовёт провал
  }
  const offer = await page.text("body");
  failures += check(page, offer.includes("Добавить"), "предложен список литературы");
  await page.click(".ts-cite-offer .ts-btn--accent");
  await page.sleep(400);
  const withBibliography = await page.evaluate(editorText);
  failures += check(page, withBibliography.includes("#bibliography("), `список литературы добавлен: ${JSON.stringify(withBibliography)}`);

  // Своя запись событий vault не публикует — индекс цитат узнаёт о ней от
  // автосохранения.
  let cited = false;
  for (let i = 0; i < 20 && !cited; i++) {
    await page.sleep(250);
    cited = await page.evaluate(
      `[...document.querySelectorAll(".ts-sources__row")].some((el) => el.querySelector(".ts-sources__key")?.textContent === "lamport1986" && !el.classList.contains("ts-sources__row--uncited"))`,
    );
  }
  failures += check(page, cited, "вставленная цитата сняла «не цитируется»");

  if (failures > 0) throw new Error(`провалов: ${failures}`);
};
