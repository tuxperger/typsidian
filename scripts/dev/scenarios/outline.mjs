// Оглавление заметки (фича 015, quickstart.md): режим «Структура» колонки
// заметки, переход, подсветка текущего раздела, свёрнутость, фильтр, виды
// пунктов и боковая шторка узкого окна.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/outline.mjs --opfs-vault --shot /tmp/015.png
//
// OUTLINE_PERF=1 — замеры T046 на заметке в 10 000 строк с 2 000 заголовков.

const NOTE = `= Введение
Первый абзац введения.

== Постановка задачи
Текст постановки.

#figure(
  rect(width: 2cm),
  caption: [Схема установки],
) <fig:схема>

== Обзор литературы
Обзор.

= Методы
Описание методов.

== Введение в методику
Подробности.

=== Калибровка
Калибровка датчиков.

#figure(
  table(columns: 2)[a][b],
  caption: [Результаты замеров],
)

Отдельная метка <итог>

= Заключение
Итоги работы.
`;

const EMPTY = `Просто текст без заголовков.
`;

const BROKEN = `= Раздел с ошибкой
#неизвестная-функция()

== Подраздел
Текст.
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

// Видимые строки оглавления: [уровень, текст, подсвечена, свёрнута].
const ROWS = `JSON.stringify([...document.querySelectorAll(".ts-outline__tree [role=treeitem]")].map((li) => [
  Number(li.getAttribute("aria-level")),
  li.querySelector(".ts-outline__text")?.textContent ?? "",
  li.getAttribute("aria-current") === "true",
  li.getAttribute("aria-expanded"),
]))`;

const rows = async (page) => JSON.parse(await page.evaluate(ROWS));

// Номер строки каретки (с единицы) — по тексту активной строки редактора.
const CARET_LINE = `(() => {
  const line = document.querySelector(".ts-editor .cm-activeLine");
  return line ? line.textContent : null;
})()`;

async function openNote(page, name) {
  await page.click(`text=${name}`);
  await page.waitForElement(".ts-editor .cm-content", { timeout: 60000 });
  await page.sleep(300);
}

// Поставить каретку в начало строки с текстом `needle`.
const caretTo = (needle) => `(() => {
  const lines = [...document.querySelectorAll(".ts-editor .cm-line")];
  const line = lines.find((el) => el.textContent.includes(${JSON.stringify(needle)}));
  if (!line) return false;
  const range = document.createRange();
  range.setStart(line.firstChild ?? line, 0);
  range.collapse(true);
  const sel = window.getSelection();
  sel.removeAllRanges();
  sel.addRange(range);
  document.querySelector(".ts-editor .cm-content").focus();
  return true;
})()`;

export default async (page) => {
  let failures = 0;
  await page.evaluate(write("Глава.typ", NOTE));
  await page.evaluate(write("Пустая.typ", EMPTY));
  await page.evaluate(write("Ошибка.typ", BROKEN));

  await page.click("text=Открыть vault");
  await openNote(page, "Глава");

  // ── US1: открытие сочетанием и строение ────────────────────────────────
  await page.click(".ts-editor .cm-content");
  const opened = Date.now();
  await page.press("O", { ctrl: true, shift: true, code: "KeyO" });
  let viaShortcut = true;
  try {
    await page.waitForElement(".ts-outline__tree", { timeout: 3000 });
  } catch {
    viaShortcut = false;
    await page.click("text=Оглавление");
    await page.waitForElement(".ts-outline__tree", { timeout: 10000 });
  }
  const openMs = Date.now() - opened;
  failures += check(page, viaShortcut, "Ctrl+Shift+O открывает оглавление (браузер отдаёт сочетание странице)");
  page.log(`  оглавление показано за ${openMs} мс`);

  let shown = await rows(page);
  const expected = [
    [1, "Введение"],
    [2, "Постановка задачи"],
    [2, "Обзор литературы"],
    [1, "Методы"],
    [2, "Введение в методику"],
    [3, "Калибровка"],
    [1, "Заключение"],
  ];
  failures += check(
    page,
    JSON.stringify(shown.map(([level, text]) => [level, text])) === JSON.stringify(expected),
    `порядок и уровни заголовков: ${JSON.stringify(shown.map(([l, t]) => `${l}:${t}`))}`,
  );
  const badge = await page.evaluate(`document.querySelector(".ts-notepanel__tag")?.textContent`);
  failures += check(page, badge === "7", `счётчик в шапке — 7 (${badge})`);

  // ── US1: переход к последнему пункту ───────────────────────────────────
  const jumpStart = Date.now();
  await page.click('.ts-outline__tree [role=treeitem][title="Заключение"]');
  await page.waitFor(`(${CARET_LINE}) === "= Заключение"`, { label: "каретка на «= Заключение»", timeout: 3000 });
  page.log(`  каретка на месте за ${Date.now() - jumpStart} мс`);
  const focusInEditor = await page.evaluate(
    `Boolean(document.activeElement?.closest(".ts-editor"))`,
  );
  failures += check(page, focusInEditor, "после перехода фокус в редакторе");

  // ── US2: подсветка текущего раздела ────────────────────────────────────
  await page.evaluate(caretTo("Подробности."));
  await page.sleep(400);
  shown = await rows(page);
  const current = shown.find((row) => row[2]);
  failures += check(page, current?.[1] === "Введение в методику", `подсвечен раздел под кареткой (${current?.[1]})`);

  await page.evaluate(caretTo("Первый абзац"));
  await page.sleep(400);
  shown = await rows(page);
  failures += check(page, shown.find((row) => row[2])?.[1] === "Введение", "каретка в первом разделе — подсвечено «Введение»");

  // ── US2: правка без сохранения ─────────────────────────────────────────
  await page.evaluate(caretTo("Итоги работы."));
  await page.press("End");
  await page.press("Enter");
  const typed = Date.now();
  await page.type("== Благодарности");
  await page.waitFor(`${ROWS}.includes("Благодарности")`, { label: "новый заголовок в колонке", timeout: 3000 });
  page.log(`  правка видна через ${Date.now() - typed} мс после набора`);
  shown = await rows(page);
  failures += check(page, shown.at(-1)?.[0] === 2 && shown.at(-1)?.[1] === "Благодарности", "дописанный заголовок — последним, уровень 2");

  // ── US3: свёрнутость ───────────────────────────────────────────────────
  await page.evaluate(`document.querySelector(".ts-outline__tree [role=treeitem] .ts-outline__chevron")?.click()`);
  await page.sleep(300);
  shown = await rows(page);
  failures += check(
    page,
    !shown.some(([, text]) => text === "Постановка задачи") && shown[0]?.[3] === "false",
    "свёрнутый раздел «Введение» скрыл подзаголовки",
  );
  await page.press("x");
  await page.sleep(500);
  await page.press("Backspace");
  await page.sleep(500);
  shown = await rows(page);
  failures += check(page, !shown.some(([, text]) => text === "Постановка задачи"), "свёрнутость пережила правку текста");

  // ── US3: фильтр ────────────────────────────────────────────────────────
  await page.click(".ts-outline__filter");
  await page.type("введ");
  await page.sleep(300);
  shown = await rows(page);
  const matched = shown.map(([, text]) => text);
  failures += check(
    page,
    matched.includes("Введение") && matched.includes("Введение в методику") && matched.includes("Методы") && !matched.includes("Заключение"),
    `фильтр «введ» оставил подходящие с родителями: ${JSON.stringify(matched)}`,
  );
  const marks = await page.evaluate(`document.querySelectorAll(".ts-outline__match").length`);
  failures += check(page, marks >= 2, `совпадения подсвечены (${marks})`);
  await page.press("Escape");
  await page.sleep(300);
  const cleared = await page.evaluate(`document.querySelector(".ts-outline__filter")?.value`);
  failures += check(page, cleared === "", "Esc очищает непустой фильтр");

  // ── US4: виды пунктов ──────────────────────────────────────────────────
  await page.click(".ts-outline__kinds");
  await page.click("text=Рисунки и таблицы");
  await page.click(".ts-outline__kinds");
  await page.click("text=Метки");
  await page.sleep(400);
  // Развернуть «Введение» обратно, чтобы рисунок был виден.
  await page.evaluate(`document.querySelector(".ts-outline__tree [role=treeitem] .ts-outline__chevron--closed")?.click()`);
  await page.sleep(300);
  shown = await rows(page);
  const texts = shown.map(([, text]) => text);
  failures += check(page, texts.some((text) => text.includes("Схема установки")), "рисунок по подписи в оглавлении");
  failures += check(page, texts.some((text) => text.includes("Результаты замеров")), "таблица по подписи в оглавлении");
  failures += check(page, texts.some((text) => text.includes("итог")), "отдельная метка <итог> в оглавлении");
  const figureRow = shown.find(([, text]) => text.includes("Схема установки"));
  failures += check(page, figureRow?.[0] === 3, `рисунок под разделом уровня 2 — уровень 3 (${figureRow?.[0]})`);

  await page.sleep(800); // запись состояния колонки — с паузой 400 мс
  await page.reload();
  await page.click("text=Открыть vault").catch(() => undefined);
  await openNote(page, "Глава");
  await page.waitForElement(".ts-outline__tree", { timeout: 15000 }).catch(() => undefined);
  shown = await rows(page);
  failures += check(
    page,
    shown.some(([, text]) => text.includes("Схема установки")),
    "колонка, режим и виды пережили перезагрузку",
  );

  // ── FR-012: пустые состояния ───────────────────────────────────────────
  await openNote(page, "Пустая");
  await page.waitForElement(".ts-outline__empty", { timeout: 5000 });
  const empty = await page.text(".ts-outline__empty");
  failures += check(page, empty.includes("нет заголовков"), `заметка без заголовков: «${empty.trim()}»`);

  await openNote(page, "Ошибка");
  await page.waitFor(`${ROWS}.includes("Подраздел")`, { label: "оглавление заметки с ошибкой", timeout: 5000 });
  failures += check(page, true, "оглавление строится и при ошибке сборки (FR-005)");

  // ── Боковая шторка узкого окна (макет 1b) ──────────────────────────────
  await openNote(page, "Глава");
  await page.viewport(1000, 800);
  await page.sleep(500);
  const column = await page.evaluate(`Boolean(document.querySelector(".ts-notepanel:not(.ts-notepanel-drawer__panel)"))`);
  failures += check(page, !column, "на средней ширине встроенной колонки нет");
  await page.click(".ts-editor .cm-content");
  await page.press("O", { ctrl: true, shift: true, code: "KeyO" });
  await page.waitForElement(".ts-notepanel-drawer", { timeout: 5000 });
  const filterFocused = await page.evaluate(`document.activeElement?.classList.contains("ts-outline__filter")`);
  failures += check(page, filterFocused, "шторка открылась с фокусом в фильтре");
  await page.shot(process.env.OUTLINE_SHOT_DRAWER ?? "/tmp/015-drawer.png").catch(() => undefined);
  await page.click('.ts-outline__tree [role=treeitem][title="Методы"]');
  await page.sleep(300);
  const drawerGone = await page.evaluate(`!document.querySelector(".ts-notepanel-drawer")`);
  failures += check(page, drawerGone, "переход закрыл шторку");
  await page.waitFor(`(${CARET_LINE}) === "= Методы"`, { label: "каретка на «= Методы»", timeout: 3000 });
  await page.viewport(1440, 900);
  await page.sleep(300);

  if (process.env.OUTLINE_PERF === "1") failures += await perf(page);

  if (failures > 0) throw new Error(`не выполнено проверок: ${failures}`);
};

// Задержка отклика на нажатие (SC-004): время от `keydown` до следующего кадра,
// медиана по 200 нажатиям с интервалом 50 мс.
const LATENCY_PROBE = `(() => {
  window.__lat = [];
  window.__latOn = (event) => {
    if (event.key !== "a" && event.key !== "Backspace") return;
    const t0 = performance.now();
    requestAnimationFrame(() => window.__lat.push(performance.now() - t0));
  };
  window.addEventListener("keydown", window.__latOn, true);
})()`;
const LATENCY_MEDIAN = `(() => {
  window.removeEventListener("keydown", window.__latOn, true);
  const xs = [...window.__lat].sort((a, b) => a - b);
  return xs.length ? xs[Math.floor(xs.length / 2)] : null;
})()`;

async function latency(page) {
  await page.evaluate(LATENCY_PROBE);
  for (let i = 0; i < 100; i += 1) {
    await page.press("a");
    await page.sleep(50);
    await page.press("Backspace");
    await page.sleep(50);
  }
  return page.evaluate(LATENCY_MEDIAN);
}

async function perf(page) {
  let failures = 0;
  const blocks = [];
  for (let i = 1; i <= 2000; i += 1) blocks.push(`== Раздел ${i}\nТекст раздела ${i}.\nЕщё строка.\nИ ещё одна.\n`);
  await page.evaluate(write("Большая.typ", `= Большая заметка\n${blocks.join("")}`));
  await page.reload();
  await page.click("text=Открыть vault").catch(() => undefined);
  await page.waitForElement("text=Большая", { timeout: 30000 });

  // SC-001: от открытия заметки (текст в редакторе) до оглавления; колонка в
  // режиме «Структура» уже показана. Меряется в странице: опрос `waitFor` идёт
  // раз в 100 мс и сам по себе съел бы половину бюджета.
  await page.evaluate(`(() => {
    window.__sc001 = null;
    let opened = null;
    const tick = () => {
      if (opened === null && document.querySelector(".ts-editor .cm-content")?.textContent.includes("Большая заметка")) {
        opened = performance.now();
      }
      if (opened !== null && document.querySelectorAll(".ts-outline__tree [role=treeitem]").length > 1000) {
        window.__sc001 = performance.now() - opened;
        return;
      }
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  })()`);
  await page.click("text=Большая");
  const sc001 = await page.waitFor(`window.__sc001`, { label: "оглавление большой заметки", timeout: 30000 });
  failures += check(page, sc001 <= 200, `SC-001: оглавление 10 000 строк за ${sc001.toFixed(0)} мс после открытия (≤200)`);

  // SC-002 и FR-004: переход к далёкому заголовку, предпросмотр догоняет.
  await page.waitFor(`document.querySelectorAll(".ts-preview__page").length > 5`, {
    label: "предпросмотр большой заметки",
    timeout: 120000,
  });
  const before = await page.evaluate(`document.querySelector(".ts-preview")?.scrollTop ?? 0`);
  // SC-002: от щелчка по пункту до каретки на месте — тоже в странице.
  await page.evaluate(`(() => {
    window.__sc002 = null;
    const row = document.querySelector('.ts-outline__tree [role=treeitem][title="Раздел 1500"]');
    row.addEventListener("click", () => {
      const t0 = performance.now();
      const tick = () => {
        if ((${CARET_LINE}) === "== Раздел 1500") window.__sc002 = performance.now() - t0;
        else requestAnimationFrame(tick);
      };
      tick();
    }, { capture: true, once: true });
  })()`);
  await page.click('.ts-outline__tree [role=treeitem][title="Раздел 1500"]');
  const sc002 = await page.waitFor(`window.__sc002`, { label: "каретка на «Раздел 1500»", timeout: 3000 });
  failures += check(page, sc002 <= 200, `SC-002: каретка на месте за ${sc002.toFixed(0)} мс (≤200)`);
  await page.sleep(1000);
  const after = await page.evaluate(`document.querySelector(".ts-preview")?.scrollTop ?? 0`);
  failures += check(page, after > before + 1000, `FR-004: предпросмотр прокручен к заголовку (${before} → ${after})`);

  // SC-003: от остановки набора до правки в колонке.
  await page.press("End");
  await page.evaluate(`(() => {
    window.__sc003 = null;
    let last = null;
    document.querySelector(".ts-editor .cm-content").addEventListener("input", () => {
      last = performance.now();
    }, true);
    const tick = () => {
      if (last !== null && ${ROWS}.includes("Раздел 1500 правка")) {
        window.__sc003 = performance.now() - last;
        return;
      }
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  })()`);
  await page.type(" правка");
  const sc003 = await page.waitFor(`window.__sc003`, { label: "правка в колонке", timeout: 5000 });
  failures += check(page, sc003 <= 500, `SC-003: правка видна через ${sc003.toFixed(0)} мс после остановки (≤500)`);

  // SC-004: задержка нажатия с открытым оглавлением и без него.
  const open = await latency(page);
  await page.press("O", { ctrl: true, shift: true, code: "KeyO" });
  await page.sleep(500);
  const closed = await latency(page);
  const ratio = closed > 0 ? open / closed : 1;
  page.log(`  задержка нажатия: с оглавлением ${open?.toFixed(1)} мс, без — ${closed?.toFixed(1)} мс`);
  failures += check(page, ratio <= 1.1 || open - closed < 2, `SC-004: разница ≤10 % (×${ratio.toFixed(2)})`);
  return failures;
}
