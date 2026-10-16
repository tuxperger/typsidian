// Подсветка Typst (фича 024, quickstart.md §2–§5): роли образца из макета,
// незакрытая пара во время набора, `Mod-/`, контраст и скорость.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/syntax-highlight.mjs --opfs-vault --shot /tmp/024.png
//
// Переменные окружения: HL_CONTRAST=1 — контраст ролей (SC-005); HL_PERF=1 —
// заметка на 5 000 строк, задержка раскраски при наборе (SC-002) и цена
// подсветки при открытии (SC-003); HL_SPLIT=1 — смена темы и разделённый
// редактор (quickstart §5).

// Текст артборда «Редактор» — тот же, что в `packages/core/tests/common/highlight_cases.rs`.
const SAMPLE = `// Глава 3. Результаты эксперимента
#import "../шаблоны/диссертация.typ": рисунок, таблица-данных
#set text(lang: "ru", size: 12pt)
#set par(justify: true, first-line-indent: 1.25em)
#show heading.where(level: 2): set block(above: 1.4em)

= Результаты
== Тепловой режим образца

Нагрев шёл _равномерно_, без локальных перегревов: см. @fig:термограмма.
Максимум отклонения — *3.2%* при нагрузке 450 Вт, а \\#7 серии отбракован.
Схема — в «Методологии» и в "протоколе №4", исходники на https://gitlab.lab.ru/тепло.

- калибровка датчиков до и после серии
- шаг измерений \`dt = 0.5 s\`
+ прогрев 15 мин
+ запись 60 мин \\
/ Термопара: датчик типа K, класс допуска 1.

$ Delta T = 1/n sum_(i=1)^n (T_i - T_0) $

#let k = calc.pow(2, 10) * 0.5  /* коэффициент из 2.3 */
#figure(
  image("рисунки/термограмма.png", width: 80%),
  caption: [Термограмма образца при *450 Вт*],
) <fig:термограмма>

#for (i, t) in данные.enumerate() [
  Замер #(i + 1): $T = #t$ °C, статус #none
]
`;

/** [фрагмент, номер вхождения, сдвиг внутри, ожидаемый класс или null — без раскраски]. */
const EXPECTED = [
  ["// Глава 3", 0, 0, "ts-syn-comment"],
  ["#import", 0, 1, "ts-syn-keyword"],
  ['"../шаблоны', 0, 2, "ts-syn-string"],
  ["text(lang", 0, 0, "ts-syn-function"],
  ["lang:", 0, 0, "ts-syn-arg-name"],
  ["12pt", 0, 0, "ts-syn-number"],
  ["true", 0, 0, "ts-syn-constant"],
  ["heading.where", 0, 0, "ts-syn-function"],
  ["heading.where", 0, 8, "ts-syn-function"],
  ["= Результаты", 0, 0, "ts-syn-heading-marker"],
  ["= Результаты", 0, 2, "ts-syn-heading"],
  ["_равномерно_", 0, 0, "ts-syn-markup-marker"],
  ["_равномерно_", 0, 1, "ts-syn-emph"],
  ["@fig:термограмма", 0, 1, "ts-syn-ref"],
  ["*3.2%*", 0, 1, "ts-syn-strong"],
  ["\\#7", 0, 0, "ts-syn-escape"],
  ['"протоколе', 0, 0, null],
  ["https://gitlab", 0, 3, "ts-syn-link"],
  ["dt = 0.5 s", 0, 0, "ts-syn-raw"],
  ["- калибровка", 0, 0, "ts-syn-list-marker"],
  ["Термопара", 0, 0, "ts-syn-list-term"],
  ["Delta", 0, 0, "ts-syn-math-ident"],
  ["^n", 0, 0, "ts-syn-math-operator"],
  ["calc.pow", 0, 0, "ts-syn-function"],
  ["/* коэффициент", 0, 0, "ts-syn-comment"],
  ["caption", 0, 0, "ts-syn-arg-name"],
  ["<fig:термограмма>", 0, 1, "ts-syn-label"],
  ["данные.enumerate", 0, 0, "ts-syn-function"],
  ["Замер", 0, 0, null],
  ["#t$", 0, 1, "ts-syn-variable"],
  ["#none", 0, 1, "ts-syn-constant"],
];

const write = (name, text) => `(async () => {
  const root = await navigator.storage.getDirectory();
  const vault = await root.getDirectoryHandle("vault", { create: true });
  const file = await vault.getFileHandle(${JSON.stringify(name)}, { create: true });
  const stream = await file.createWritable();
  await stream.write(${JSON.stringify(text)});
  await stream.close();
})()`;

// Классы раскраски символа: `nth`-е вхождение `needle` в тексте видимых строк,
// символ со сдвигом `shift`. Пустая строка — символ без раскраски; `null` —
// фрагмента нет в отрисованной части.
const CLASS_AT = `(needle, nth, shift) => {
  const lines = [...document.querySelectorAll(".ts-editor .cm-line")];
  let text = "";
  const nodes = [];
  for (const line of lines) {
    const walker = document.createTreeWalker(line, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      nodes.push({ node, at: text.length });
      text += node.data;
    }
    text += "\\n";
  }
  let at = -1;
  for (let i = 0; i <= nth; i += 1) {
    at = text.indexOf(needle, at + 1);
    if (at < 0) return null;
  }
  const target = at + shift;
  const hit = nodes.findLast((entry) => entry.at <= target);
  const el = hit?.node.parentElement?.closest("[class*='ts-syn-']");
  return el ? [...el.classList].filter((c) => c.startsWith("ts-syn-")).join(" ") : "";
}`;

function check(page, ok, label) {
  page.log(`${ok ? "✓" : "✗"} ${label}`);
  return ok;
}

async function rolesMatch(page, label) {
  let bad = 0;
  for (const [needle, nth, shift, wanted] of EXPECTED) {
    const got = await page.evaluate(`(${CLASS_AT})(${JSON.stringify(needle)}, ${nth}, ${shift})`);
    const ok = wanted === null ? got === "" : typeof got === "string" && got.split(" ").includes(wanted);
    if (!ok) {
      bad += 1;
      page.log(`  «${needle}»+${shift}: ждали ${wanted ?? "без раскраски"}, вышло ${JSON.stringify(got)}`);
    }
  }
  check(page, bad === 0, `${label}: ${EXPECTED.length - bad} из ${EXPECTED.length} фрагментов в своей роли`);
  return bad;
}

const colored = `document.querySelectorAll(".ts-editor [class*='ts-syn-']").length > 0`;

async function openNote(page, name) {
  await page.click(`text=${name}`);
  await page.waitForElement(".ts-editor .cm-content", { timeout: 60000 });
  await page.waitFor(colored, { label: "раскраска", timeout: 30000 });
  await page.sleep(300);
}

// Раскраска `index`-го редактора на странице: пары «классы — текст» по
// отрисованным строкам. Две половины разделения с одной заметкой должны дать
// одно и то же.
const PAINT_OF = `(index) => {
  const editor = document.querySelectorAll(".ts-editor")[index];
  if (!editor) return null;
  const lines = [...editor.querySelectorAll(".cm-line")];
  return JSON.stringify(lines.map((line) =>
    [...line.querySelectorAll("[class*='ts-syn-']")].map((el) => [el.className, el.textContent])));
}`;

/** Смена темы и разделённый редактор (US4, quickstart §5). */
async function themeAndSplit(page) {
  let failures = 0;
  const caret = `document.body.innerText.match(/Ln \\d+, Col \\d+/)?.[0] ?? null`;

  // Тема — через панель настроек, как её меняет пользователь.
  await page.click(".ts-editor .cm-content");
  await page.press("Home", { ctrl: true });
  await page.press("ArrowDown");
  await page.press("ArrowDown");
  await page.press("End");
  const before = await page.evaluate(caret);
  await page.click("text=Настройки");
  await page.click("text=Внешний вид");
  await page.waitForElement("select");
  const next = await page.evaluate(`(() => {
    const select = [...document.querySelectorAll("select")].find((el) => el.querySelector("option[value='dark']"));
    if (!select) return null;
    const value = select.value === "dark" ? "light" : "dark";
    Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value").set.call(select, value);
    select.dispatchEvent(new Event("change", { bubbles: true }));
    return value;
  })()`);
  await page.sleep(300);
  const theme = await page.evaluate(`document.documentElement.getAttribute("data-theme")`);
  if (!check(page, next !== null && theme === next, `тема переключена на «${theme}»`)) failures += 1;
  await page.press("Escape");
  await page.sleep(300);
  if (!(await page.evaluate(`Boolean(document.querySelector(".ts-editor .cm-content"))`))) {
    await openNote(page, "Результаты");
  }
  failures += await rolesMatch(page, "после смены темы");
  const after = await page.evaluate(caret);
  if (!check(page, before !== null && after === before, `каретка на месте после смены темы (${before} → ${after})`)) failures += 1;

  // Разделение (014): та же заметка в двух половинах, правка в левой.
  await page.click("[aria-label='Разделить редактор']");
  await page.waitFor(`document.querySelectorAll(".ts-editor .cm-content").length === 2`, { label: "вторая половина", timeout: 30000 });
  await page.waitFor(`document.querySelectorAll(".ts-editor")[1].querySelector("[class*='ts-syn-']") !== null`, { label: "раскраска второй половины", timeout: 30000 });
  await page.click(".ts-editor .cm-content");
  await page.press("Home", { ctrl: true });
  await page.type("#let x = (1, $a$) // правка\n");
  await page.sleep(500);
  const [left, right] = [await page.evaluate(`(${PAINT_OF})(0)`), await page.evaluate(`(${PAINT_OF})(1)`)];
  const edited = left?.includes("правка") && left.includes("ts-syn-keyword");
  // Высота половин разная — сравнивается общая отрисованная часть.
  const [l, r] = [JSON.parse(left ?? "[]"), JSON.parse(right ?? "[]")];
  const common = Math.min(l.length, r.length);
  const line = l.slice(0, common).findIndex((spans, i) => JSON.stringify(spans) !== JSON.stringify(r[i]));
  if (!check(page, edited && common >= 10 && line === -1, `обе половины раскрашены одинаково после правки в левой (${common} строк)`)) {
    failures += 1;
    page.log(`  строка ${line + 1}, строк ${l.length} / ${r.length}`);
    page.log(`  левая:  ${JSON.stringify(l[line])}`);
    page.log(`  правая: ${JSON.stringify(r[line])}`);
  }
  return failures;
}

/** Контраст каждой встреченной роли с фоном полотна и текущей строки (SC-005). */
async function contrast(page) {
  const rows = await page.evaluate(`(() => {
    // rgb(255, 138, 51) — каналы 0–255; color(srgb 0.51 0.68 0.83) (так
    // браузер отдаёт результат color-mix) — доли единицы.
    const lum = (css) => {
      const scale = css.startsWith("color(") ? 1 : 255;
      const [r, g, b] = css.match(/\\d+(\\.\\d+)?/g).slice(0, 3).map(Number).map((v) => {
        const c = v / scale;
        return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
      });
      return 0.2126 * r + 0.7152 * g + 0.0722 * b;
    };
    const probe = (css) => { const el = document.createElement("span"); el.style.color = css; document.body.append(el); const c = getComputedStyle(el).color; el.remove(); return c; };
    const backs = [probe("var(--steel-950)"), probe("var(--steel-900)")].map(lum);
    const seen = new Map();
    for (const el of document.querySelectorAll(".ts-editor [class*='ts-syn-']")) {
      const role = [...el.classList].find((c) => c.startsWith("ts-syn-") && !["ts-syn-strong", "ts-syn-emph", "ts-syn-heading"].includes(c)) ?? [...el.classList].join(" ");
      if (seen.has(role)) continue;
      const fore = lum(getComputedStyle(el).color);
      seen.set(role, backs.map((back) => (Math.max(fore, back) + 0.05) / (Math.min(fore, back) + 0.05)));
    }
    return [...seen].map(([role, [canvas, line]]) => ({ role, canvas, line }));
  })()`);
  let bad = 0;
  for (const { role, canvas, line } of rows) {
    const floor = role === "ts-syn-comment" ? 3.5 : 4.5;
    const ok = Math.min(canvas, line) >= floor;
    if (!ok) bad += 1;
    page.log(`  ${ok ? "✓" : "✗"} ${role.padEnd(24)} ${canvas.toFixed(2)} / ${line.toFixed(2)} (порог ${floor})`);
  }
  check(page, bad === 0, `контраст ${rows.length} ролей не ниже порога SC-005`);
  return bad;
}

/**
 * Задержка «ввод → раскрашенный символ» (SC-002), измеренная в самой странице:
 * от `beforeinput` до мутации DOM, после которой набранная `#` стоит в
 * `.ts-syn-keyword`. Рядом — то же для самого символа без раскраски: разница
 * между ними и есть цена подсветки. Обход CDP в замер не входит.
 */
async function typingLatency(page, label, presses) {
  const colored = [];
  const plain = [];
  for (let i = 0; i < presses; i += 1) {
    await page.press("Home");
    await page.evaluate(`(() => {
      window.__hl = { t0: null, text: null, colored: null };
      const content = document.querySelector(".ts-editor .cm-content");
      const caretLine = () => {
        const node = window.getSelection().focusNode;
        const el = node && (node.nodeType === 3 ? node.parentElement : node);
        return el?.closest(".cm-line") ?? null;
      };
      const look = () => {
        const state = window.__hl;
        if (state.t0 === null) return;
        const line = caretLine();
        if (line === null) return;
        const now = performance.now() - state.t0;
        if (state.text === null && line.textContent.startsWith("#")) state.text = now;
        const mark = line.querySelector(".ts-syn-keyword");
        if (state.colored === null && mark && mark.textContent.startsWith("#")) {
          state.colored = now;
          observer.disconnect();
        }
      };
      const observer = new MutationObserver(look);
      observer.observe(content, { subtree: true, childList: true, characterData: true, attributes: true });
      document.addEventListener("beforeinput", () => { window.__hl.t0 = performance.now(); }, { capture: true, once: true });
    })()`);
    await page.type("#");
    await page.waitFor("window.__hl.colored !== null", { label: "раскраска набранной #", timeout: 3000 });
    const { text, colored: done } = await page.evaluate("window.__hl");
    plain.push(text ?? done);
    colored.push(done);
    await page.press("Backspace");
  }
  const stats = (samples) => {
    const sorted = [...samples].sort((x, y) => x - y);
    return { p95: sorted[Math.ceil(sorted.length * 0.95) - 1], median: sorted[Math.floor(sorted.length / 2)] };
  };
  const c = stats(colored);
  const p = stats(plain);
  check(
    page,
    c.p95 <= 16,
    `${label}: раскраска — 95-й перцентиль ${c.p95.toFixed(1)} мс, медиана ${c.median.toFixed(1)} мс; сам символ — ${p.p95.toFixed(1)} / ${p.median.toFixed(1)} мс (порог 16 мс)`,
  );
  return c.p95 <= 16 ? 0 : 1;
}

/**
 * Каретка в начало абзаца обычного текста («Нагрев шёл…») на доле `fraction`
 * высоты заметки: `#` там — всегда переход в код, а не текст комментария или raw.
 */
async function toParagraph(page, fraction) {
  await page.evaluate(`(() => {
    const scroller = document.querySelector(".ts-editor .cm-scroller");
    scroller.scrollTop = ${fraction} * (scroller.scrollHeight - scroller.clientHeight);
  })()`);
  await page.sleep(400);
  // Отрисована только видимая часть с запасом: ближайший к середине экрана
  // абзац «Нагрев…» подтягивается в центр, и только потом по нему щёлкают.
  await page.evaluate(`(() => {
    const box = document.querySelector(".ts-editor .cm-scroller").getBoundingClientRect();
    const middle = (box.top + box.bottom) / 2;
    const lines = [...document.querySelectorAll(".ts-editor .cm-line")]
      .filter((el) => el.textContent.startsWith("Нагрев"));
    const distance = (el) => Math.abs(el.getBoundingClientRect().top - middle);
    lines.sort((a, b) => distance(a) - distance(b))[0].scrollIntoView({ block: "center" });
  })()`);
  await page.sleep(300);
  const point = await page.evaluate(`(() => {
    const box = document.querySelector(".ts-editor .cm-scroller").getBoundingClientRect();
    const middle = (box.top + box.bottom) / 2;
    const line = [...document.querySelectorAll(".ts-editor .cm-line")]
      .filter((el) => el.textContent.startsWith("Нагрев"))
      .sort((a, b) => Math.abs(a.getBoundingClientRect().top - middle) - Math.abs(b.getBoundingClientRect().top - middle))[0];
    const r = line.getBoundingClientRect();
    return { x: r.left + 20, y: r.top + 8 };
  })()`);
  await page.mouse("mousePressed", point.x, point.y);
  await page.mouse("mouseReleased", point.x, point.y);
  await page.press("Home");
}

export default async (page) => {
  let failures = 0;
  const perf = process.env.HL_PERF === "1";
  await page.evaluate(write("Результаты.typ", SAMPLE));
  if (perf) {
    const big = Array.from({ length: Math.ceil(5000 / SAMPLE.split("\n").length) }, () => SAMPLE).join("\n");
    await page.evaluate(write("Большая.typ", big));
  }

  await page.click("text=Открыть vault");
  await openNote(page, "Результаты");
  failures += await rolesMatch(page, "образец артборда «Редактор»");

  // Незакрытая скобка во время набора (US3, AS 3.2): заголовок ниже не перекрашен.
  await page.click(".ts-editor .cm-content");
  await page.press("Home", { ctrl: true });
  await page.type("#figure(");
  await page.sleep(400);
  const heading = await page.evaluate(`(${CLASS_AT})("== Тепловой", 0, 0)`);
  if (!check(page, heading === "ts-syn-heading-marker ts-syn-heading" || heading?.includes("ts-syn-heading-marker"), `незакрытая «#figure(» не тронула заголовок ниже (${heading})`)) failures += 1;
  for (let i = 0; i < 8; i += 1) await page.press("Backspace");
  await page.sleep(400);
  failures += await rolesMatch(page, "после отмены набора — как при открытии (FR-007)");

  // `Mod-/` берёт маркеры комментария из языковых данных (research §6).
  await page.press("Home", { ctrl: true });
  await page.press("/", { ctrl: true, code: "Slash" });
  await page.sleep(200);
  const first = await page.evaluate(`document.querySelector(".ts-editor .cm-line")?.textContent`);
  if (!check(page, first?.startsWith("Глава") || first?.startsWith(" Глава"), `Mod-/ снимает «// » (строка: «${first}»)`)) failures += 1;
  await page.press("z", { ctrl: true });

  if (process.env.HL_CONTRAST === "1") failures += await contrast(page);

  if (process.env.HL_SPLIT === "1") failures += await themeAndSplit(page);

  // FR-012: дерево и вкладки открывают в редакторе только заметки Typst
  // (`FileKind::Note`), а `typst` у `NoteEditor` ставится по тому же виду;
  // проверять в браузере нечего — другой файл сюда не попадает.

  if (perf) {
    const t0 = Date.now();
    await page.click("text=Большая");
    await page.waitForElement(".ts-editor .cm-content", { timeout: 60000 });
    const shown = Date.now() - t0;
    await page.waitFor(colored, { label: "раскраска большой заметки", timeout: 30000 });
    const painted = Date.now() - t0;
    if (!check(page, painted - shown <= 100, `открытие 5 000 строк: текст ${shown} мс, раскраска +${painted - shown} мс (порог 100 мс, SC-003)`)) failures += 1;
    for (const [fraction, label] of [[0, "начало заметки"], [0.5, "середина заметки"], [1, "конец заметки"]]) {
      await toParagraph(page, fraction);
      failures += await typingLatency(page, label, 70);
    }
  }

  if (failures > 0) throw new Error(`подсветка: ${failures} проверок не прошли`);
};
