// Дополнение меток и ссылок (фича 018, quickstart.md): список по `@` и `[[`,
// переход к определению, отметки ссылок в никуда с «Создать заметку».
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/completion.mjs --opfs-vault --shot /tmp/018.png

const MAIN = `#set heading(numbering: "1.")
= Введение <введение>
Текст введения.

#figure(rect(width: 2cm), caption: [Схема установки]) <рис-схема>

#include "часть.typ"

#for i in range(2) [ #figure(rect(), caption: [Рис #i]) #label("цикл-" + str(i)) ]

Ссылка на цикл: @цикл-1.
`;

const PART = `= Часть <часть-метка>
Подключённый текст.
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

const editorText = `[...document.querySelectorAll(".ts-editor .cm-line")].map((l) => l.textContent).join("\\n")`;

// Каретка в конец документа.
const caretToEnd = `(() => {
  const lines = [...document.querySelectorAll(".ts-editor .cm-line")];
  const line = lines[lines.length - 1];
  const range = document.createRange();
  range.selectNodeContents(line);
  range.collapse(false);
  const sel = window.getSelection();
  sel.removeAllRanges();
  sel.addRange(range);
  document.querySelector(".ts-editor .cm-content").focus();
  return true;
})()`;

// Каретка внутрь первого вхождения `needle` (после `shift` знаков).
const caretInside = (needle, shift) => `(() => {
  const lines = [...document.querySelectorAll(".ts-editor .cm-line")];
  for (const line of lines) {
    const walker = document.createTreeWalker(line, NodeFilter.SHOW_TEXT);
    let node;
    while ((node = walker.nextNode())) {
      const at = node.textContent.indexOf(${JSON.stringify(needle)});
      if (at >= 0) {
        const range = document.createRange();
        range.setStart(node, at + ${shift});
        range.collapse(true);
        const sel = window.getSelection();
        sel.removeAllRanges();
        sel.addRange(range);
        document.querySelector(".ts-editor .cm-content").focus();
        return true;
      }
    }
  }
  return false;
})()`;

async function typeSlowly(page, text) {
  for (const ch of text) {
    await page.type(ch);
    await page.sleep(40);
  }
}

export default async (page) => {
  let failures = 0;
  await page.evaluate(write("Глава.typ", MAIN));
  await page.evaluate(write("часть.typ", PART));
  await page.evaluate(write("План работ.typ", "= План\n"));

  await page.click("text=Открыть vault");
  await page.click("text=Глава");
  await page.waitForElement(".ts-editor .cm-content", { timeout: 60000 });
  // Первая сборка — она даёт связанные файлы и свидетельство.
  await page.sleep(2500);

  // ── US1: `@` — метки заметки и связанного файла ─────────────────────────
  await page.evaluate(caretToEnd);
  await typeSlowly(page, "\nСм. @рис");
  try {
    await page.waitForElement(".ts-complete", { timeout: 3000 });
  } catch {
    // проверка ниже назовёт провал
  }
  const labels = await page.text(".ts-complete");
  failures += check(page, labels.includes("рис-схема") && labels.includes("рисунок"), `@рис предлагает рис-схема с видом: ${JSON.stringify(labels.slice(0, 120))}`);
  await page.shot?.("/tmp/claude-1000/-home-apokryf-projects-typsidian/5cce620f-3317-435c-bad0-17b3f23ddd2b/scratchpad/018-1a.png");
  await page.press("Enter", { code: "Enter" });
  await page.sleep(300);
  failures += check(page, (await page.evaluate(editorText)).includes("См. @рис-схема"), "Enter подставил метку");

  await typeSlowly(page, " и @час");
  await page.sleep(400);
  const linked = await page.text(".ts-complete").catch(() => "");
  failures += check(page, linked.includes("часть-метка") && linked.includes("часть.typ"), "метка связанного файла с именем файла");
  await page.press("Escape", { code: "Escape" });
  await page.sleep(200);
  failures += check(page, !(await page.evaluate(`!!document.querySelector(".ts-complete")`)), "Escape закрыл список");

  // ── US1: `[[` — заметки vault ──────────────────────────────────────────
  await typeSlowly(page, "\nСм. [[пла");
  await page.sleep(400);
  const notes = await page.text(".ts-complete").catch(() => "");
  failures += check(page, notes.includes("План работ") && notes.includes("[[План работ]]"), "[[пла предлагает заметку и запись");
  await page.press("Tab", { code: "Tab" });
  await page.sleep(300);
  failures += check(page, (await page.evaluate(editorText)).includes("См. [[План работ]]"), "Tab подставил ссылку с ]]");

  // В коде список не появляется.
  await typeSlowly(page, "\n#let x = \"@рис");
  await page.sleep(500);
  failures += check(page, !(await page.evaluate(`!!document.querySelector(".ts-complete")`)), "в строке кода списка нет");
  await typeSlowly(page, "\"\n");

  // ── US3: отметки ───────────────────────────────────────────────────────
  await typeSlowly(page, "Битая @нет-такой и [[Новая заметка]].");
  await page.sleep(2500);
  const marked = JSON.parse(
    await page.evaluate(`JSON.stringify([...document.querySelectorAll(".ts-editor .ts-mark-note")].map((el) => el.textContent))`),
  );
  failures += check(page, marked.some((text) => text.includes("нет-такой")), `отмечена ссылка на отсутствующую метку: ${JSON.stringify(marked)}`);
  failures += check(page, marked.some((text) => text.includes("Новая заметка")), "отмечена ссылка на отсутствующую заметку");
  failures += check(page, !marked.some((text) => text.includes("цикл-1")), "метка из цикла не отмечена (свидетельство сборки)");
  failures += check(page, !marked.some((text) => text.includes("рис-схема")), "существующая метка не отмечена");

  await page.evaluate(caretInside("[[Новая", 4));
  await page.sleep(300);
  const explain = await page.text(".ts-mark-explain").catch(() => "");
  failures += check(page, explain.includes("Создать заметку"), "у каретки — пояснение с «Создать заметку»");
  await page.shot?.("/tmp/claude-1000/-home-apokryf-projects-typsidian/5cce620f-3317-435c-bad0-17b3f23ddd2b/scratchpad/018-1c.png");
  await page.click(".ts-mark-explain .ts-btn--accent");
  await page.sleep(2500);
  const after = JSON.parse(
    await page.evaluate(`JSON.stringify([...document.querySelectorAll(".ts-editor .ts-mark-note")].map((el) => el.textContent))`),
  );
  failures += check(page, !after.some((text) => text.includes("Новая заметка")), "после создания отметка исчезла");
  failures += check(page, (await page.text(".ts-tree, body")).includes("Новая заметка"), "заметка появилась в vault");

  // ── US2: переход к определению ─────────────────────────────────────────
  await page.evaluate(caretInside("@рис-схема", 3));
  await page.press("f", { ctrl: true, alt: true, code: "KeyF" });
  await page.sleep(500);
  const active = await page.evaluate(`document.querySelector(".ts-editor .cm-activeLine")?.textContent ?? ""`);
  failures += check(page, active.includes("<рис-схема>"), `переход к <рис-схема>: ${JSON.stringify(active)}`);

  await page.evaluate(caretInside("@нет-такой", 3));
  await page.press("f", { ctrl: true, alt: true, code: "KeyF" });
  await page.sleep(500);
  failures += check(page, (await page.text("body")).includes("нет-такой"), "отказ перехода назван словами");

  if (process.env.COMPLETION_PERF === "1") failures += await perf(page);

  if (failures > 0) throw new Error(`провалов: ${failures}`);
};

// Замеры SC-001 и SC-003 (T068, T069): заметка с 500 метками и метка в
// связанном файле.
async function perf(page) {
  let failures = 0;
  let big = "См. @часть-метка.\n#include \"часть.typ\"\n";
  for (let i = 0; i < 500; i++) big += `= Раздел ${i} <раздел-${i}>\nТекст раздела.\n`;
  await page.evaluate(write("Большая.typ", big));
  await page.click("text=Большая");
  await page.sleep(3000);
  await page.evaluate(caretToEnd);
  await typeSlowly(page, "\nНабор ");
  const started = Date.now();
  await page.type("@");
  await page.waitForElement(".ts-complete", { timeout: 3000 });
  const shown = Date.now() - started;
  failures += check(page, shown <= 100, `SC-001: от @ до списка ${shown} мс (≤ 100) на 500 метках`);
  await page.press("Escape", { code: "Escape" });

  // Длинная заметка рисуется не вся: к ссылке в первой строке — клавишами.
  await page.press("Home", { ctrl: true, code: "Home" });
  for (let i = 0; i < 7; i++) await page.press("ArrowRight", { code: "ArrowRight" });
  const jump = Date.now();
  await page.press("f", { ctrl: true, alt: true, code: "KeyF" });
  let arrived = false;
  while (Date.now() - jump < 3000 && !arrived) {
    arrived = (await page.evaluate(`document.querySelector(".ts-editor .cm-activeLine")?.textContent ?? ""`)).includes("<часть-метка>");
    if (!arrived) await page.sleep(20);
  }
  const took = Date.now() - jump;
  if (!arrived) page.log(`после команды: строка ${JSON.stringify(await page.evaluate(`document.querySelector(".ts-editor .cm-activeLine")?.textContent ?? ""`))}, вкладки ${JSON.stringify(await page.text(".ts-tabs, .ts-tabbar").catch(() => ""))}, статус ${JSON.stringify(await page.text(".ts-cite-offer").catch(() => ""))}`);
  failures += check(page, arrived && took <= 1000, `SC-003: переход к метке связанного файла ${took} мс (≤ 1000)`);
  return failures;
}
