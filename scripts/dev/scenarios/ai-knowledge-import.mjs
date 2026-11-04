// Заметки в граф знаний (фича 055, T006, T008, T009): меню дерева (A), подтверждение с
// пропусками (B), ход по файлам (C), итог по файлам (D) и «Повторить», повтор неизменённой
// папки (0 запросов), правка одной заметки (1 запрос), отмена посреди, сбой 503 → «Сбой» и
// «Повторить», устаревшая оценка, выбор «Добавить заметки…» в виде графа, узкий экран и
// клавиатурный фокус; во время импорта ввод в редактор не блокируется (SC-003).
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/ai-knowledge-import.mjs --opfs-vault --timeout 300000
//
// Сеть к модели подменена очередью `window.__plan`, как в ai-knowledge.mjs. PDF с текстом и без
// него берутся из фикстур ядра. Снимки: KNOWLEDGE_SHOTS (по умолчанию /tmp/claude-1000), import-*.png.
import { readFileSync } from "node:fs";

const shots = process.env.KNOWLEDGE_SHOTS ?? "/tmp/claude-1000";
const fixture = (name) => readFileSync(new URL(`../../../packages/core/tests/fixtures/pdf/${name}`, import.meta.url)).toString("base64");

function check(page, ok, label) {
  page.log(`${ok ? "✓" : "✗"} ${label}`);
  if (!ok) throw new Error(`не выполнено: ${label}`);
}

const FAKE_MODEL = `
(() => {
  const realFetch = window.fetch.bind(window);
  const calls = [];
  window.__modelCalls = calls;
  window.__plan = [];
  const sse = (name, data) => "event: " + name + "\\ndata: " + JSON.stringify(data) + "\\n\\n";
  window.__reply = ({ text = "", tools = [], stop = true, input = 1200 } = {}) => {
    const out = [sse("message_start", { type: "message_start", message: { usage: { input_tokens: input, output_tokens: 1 } } })];
    let index = 0;
    if (text !== "") {
      out.push(sse("content_block_start", { type: "content_block_start", index, content_block: { type: "text", text: "" } }));
      for (let at = 0; at < text.length; at += 4000) {
        out.push(sse("content_block_delta", { type: "content_block_delta", index, delta: { type: "text_delta", text: text.slice(at, at + 4000) } }));
      }
      out.push(sse("content_block_stop", { type: "content_block_stop", index }));
      index += 1;
    }
    for (const tool of tools) {
      const args = JSON.stringify(tool.input);
      out.push(sse("content_block_start", { type: "content_block_start", index, content_block: { type: "tool_use", id: "toolu_" + index, name: tool.name, input: {} } }));
      out.push(sse("content_block_delta", { type: "content_block_delta", index, delta: { type: "input_json_delta", partial_json: args } }));
      out.push(sse("content_block_stop", { type: "content_block_stop", index }));
      index += 1;
    }
    if (stop) {
      out.push(sse("message_delta", { type: "message_delta", delta: { stop_reason: tools.length ? "tool_use" : "end_turn" }, usage: { output_tokens: 70 } }));
      out.push(sse("message_stop", { type: "message_stop" }));
    }
    return { chunks: out };
  };
  window.fetch = async (input, init) => {
    const url = typeof input === "string" ? input : String(input?.url ?? input);
    if (!url.includes("api.anthropic.com")) return realFetch(input, init);
    const body = init?.body ?? (typeof input?.clone === "function" ? await input.clone().text() : "");
    calls.push({ url, body: String(body ?? "") });
    const item = window.__plan.shift();
    if (!item) return new Response("{}", { status: 500 });
    if (item.status) return new Response(JSON.stringify({ error: { type: "rate_limit_error", message: "x" } }), { status: item.status, headers: { "content-type": "application/json" } });
    const encoder = new TextEncoder();
    const stream = new ReadableStream({
      async start(controller) {
        for (const chunk of item.chunks) {
          controller.enqueue(encoder.encode(chunk));
          await new Promise((done) => setTimeout(done, item.delay ?? 40));
        }
        controller.close();
      },
    });
    return new Response(stream, { status: 200, headers: { "content-type": "text/event-stream" } });
  };
})();
`;

// Файл — текст или { base64 } для PDF.
const WRITE_FILES = `(async (files) => {
  const root = await (await navigator.storage.getDirectory()).getDirectoryHandle("vault", { create: true });
  for (const [path, content] of Object.entries(files)) {
    const parts = path.split("/");
    let dir = root;
    for (const part of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(part, { create: true });
    const file = await dir.getFileHandle(parts[parts.length - 1], { create: true });
    const stream = await file.createWritable();
    await stream.write(typeof content === "string" ? content : Uint8Array.from(atob(content.base64), (c) => c.charCodeAt(0)));
    await stream.close();
  }
  return true;
})`;

const SECRET = "СЕКРЕТ-ИЗ-ЗАКРЫТОЙ-ПАПКИ-2718";
const SEEDED = {
  "Черновик.typ": "= Черновик\nМесто для ввода во время импорта.\n",
  "Физика/Механика/Импульс.typ": "= Импульс\nИмпульс тела равен произведению массы на скорость. ИМПУЛЬС-ТЕКСТ.\n",
  "Физика/Механика/Энергия.typ": "= Энергия\nКинетическая энергия равна половине массы на квадрат скорости. ЭНЕРГИЯ-ТЕКСТ.\n",
  "Физика/Механика/Работа.typ": "= Работа\nРабота силы равна произведению силы на путь. РАБОТА-ТЕКСТ.\n",
  "Физика/Механика/Схема.board": "{}",
  "Физика/Механика/Скан задачника.pdf": { base64: fixture("no-text.pdf") },
  "Физика/Механика/Ландау, гл. 2.pdf": { base64: fixture("text.pdf") },
  "Физика/Механика/Личное/Мысли.typ": `= Мысли\n${SECRET}\n`,
  "Физика/Оптика/Линзы.typ": "= Линзы\nЛинза собирает лучи. ЛИНЗЫ-ТЕКСТ.\n",
  "Физика/Оптика/Призма.typ": "= Призма\nПризма раскладывает свет. ПРИЗМА-ТЕКСТ.\n",
  "Физика/Оптика/Зеркала.typ": "= Зеркала\nУгол падения равен углу отражения. ЗЕРКАЛА-ТЕКСТ.\n",
  "Физика/Сбой/Один.typ": "= Один\nЗаметка для проверки сбоя. СБОЙ-ТЕКСТ.\n",
};

const openPalette = async (page, query) => {
  await page.click(".ts-cmdfield");
  await page.waitForElement(".ts-palette");
  await page.type(query);
  await page.sleep(200);
  await page.press("Enter");
};

const plan = (page, items) =>
  page.evaluate(`window.__plan.push(...${JSON.stringify(items)}.map((i) => i.reply ? { ...window.__reply(i.reply), delay: i.delay } : i))`);

const press = (page, scope, label) =>
  page.evaluate(`(() => {
    const b = [...document.querySelectorAll(${JSON.stringify(scope + " button")})].find((x) => x.textContent.trim() === ${JSON.stringify(label)});
    if (!b) throw new Error("нет кнопки " + ${JSON.stringify(label)});
    b.click();
  })()`);

const findings = (propositions, chains = [], delay = 400) => ({
  reply: { tools: [{ name: "submit_findings", input: { propositions, chains } }] },
  delay,
});
const imports = (page) =>
  page.evaluate(`window.__modelCalls.map((c) => c.body).filter((b) => b.includes("submit_findings") && b.includes("tool_choice"))`);

/** Контекстное меню строки дерева: событие `contextmenu` на строке с путём. */
const contextMenuOn = async (page, path) => {
  await page.evaluate(`(() => {
    const row = document.querySelector('[data-drag-path=${JSON.stringify(path)}]');
    if (!row) throw new Error("нет строки дерева " + ${JSON.stringify(path)});
    const r = row.querySelector("button").getBoundingClientRect();
    row.querySelector("button").dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: r.x + 20, clientY: r.y + 8 }));
  })()`);
  await page.waitForElement(".ts-ctxmenu");
};
const menuItems = (page) => page.evaluate(`[...document.querySelectorAll(".ts-ctxmenu [role=menuitem]")].map((b) => b.textContent.trim())`);
const addToGraphFrom = async (page, path) => {
  await contextMenuOn(page, path);
  await page.click("text=Добавить в граф знаний…");
};
const waitConfirm = (page) => page.waitForElement(".ts-import[role=dialog]", { timeout: 20000 });
const waitDone = (page, title = "Импорт завершён") =>
  page.waitFor(`document.querySelector(".ts-import-card")?.innerText.includes(${JSON.stringify(title)})`, { timeout: 90000, label: title });
const rowOf = (page, name) =>
  page.evaluate(`(() => {
    const li = [...document.querySelectorAll(".ts-import__file")].find((x) => x.querySelector(".ts-import__name").textContent.trim() === ${JSON.stringify(name)});
    return li ? { state: li.dataset.state, text: li.innerText.replace(/\\s+/g, " ").trim(), retry: Boolean(li.querySelector("button")) } : null;
  })()`);
const openFolder = (page, path) =>
  page.evaluate(`(() => {
    const row = document.querySelector('[data-drag-path=${JSON.stringify(path)}]');
    if (row && row.getAttribute("aria-expanded") === "false") row.querySelector("button").click();
  })()`);

export default async (page) => {
  await page.addInitScript(FAKE_MODEL);
  await page.reload();
  await page.evaluate(`${WRITE_FILES}(${JSON.stringify(SEEDED)})`);
  await page.click("text=Продолжить");
  await page.click("text=Открыть папку…");
  await page.waitForElement("text=Настройки", { timeout: 30000 });

  // ── Включаем ИИ, закрываем папку «Личное» ──
  await page.click("text=Настройки");
  await page.waitForElement("text=Обучение с ИИ");
  await page.sleep(400);
  await page.click("text=Обучение с ИИ");
  await page.click("text=Включить для этого vault");
  await page.waitForElement("[role=alertdialog]");
  await page.click("text=Включить");
  await page.waitForElement("input[type=password]");
  await page.click("input[type=password]");
  await page.type("sk-ant-test");
  await page.click("text=Сохранить");
  await page.waitForElement("text=Заменить");
  await page.click('input[list="ts-ai-models"]');
  await page.press("a", { ctrl: true });
  await page.type("claude-sonnet-5-5");
  await page.click(".ts-settings__head h2");
  await page.click('input[aria-label="Папки, закрытые для агента"]');
  await page.type("Физика/Механика/Личное");
  await page.click("text=Добавить папку");
  await page.waitFor(`document.body.innerText.includes("Личное/ ✕")`, { label: "тег закрытой папки" });
  await page.press("Escape");
  await page.sleep(300);

  // ── Вид «Граф знаний»: кнопка «Добавить заметок…» есть и у пустого графа ──
  await openPalette(page, "Граф знаний");
  await page.waitForElement(".ts-kg");
  await page.waitFor(`!document.querySelector(".ts-kg__empty")?.textContent.includes("Загружаем")`, { label: "граф загружен" });
  check(page, (await page.text(".ts-kg__bar")).includes("Добавить заметки…"), "в виде графа знаний есть «Добавить заметки…»");
  await page.shot(`${shots}/import-view-empty.png`);
  await openPalette(page, "Вернуться в Workspace");
  await page.waitFor(`!document.querySelector(".ts-kg")`, { label: "вид закрыт" });

  // ── A: контекстное меню дерева ──
  await openFolder(page, "Физика");
  await openFolder(page, "Физика/Механика");
  await page.waitForElement('[data-drag-path="Физика/Механика"]');
  await contextMenuOn(page, "Физика/Механика");
  let items = await menuItems(page);
  check(page, items.includes("Добавить в граф знаний…"), `A: у папки есть «Добавить в граф знаний…» (${items.join(" | ")})`);
  await page.shot(`${shots}/import-A-folder-menu.png`);
  await page.press("Escape");
  await page.waitFor(`!document.querySelector(".ts-ctxmenu")`, { label: "меню закрыто" });
  await page.waitForElement('[data-drag-path="Физика/Механика/Импульс.typ"]');
  await contextMenuOn(page, "Физика/Механика/Импульс.typ");
  items = await menuItems(page);
  check(page, items.includes("Добавить в граф знаний…"), "A: у заметки есть «Добавить в граф знаний…»");
  await page.press("Escape");
  await page.waitFor(`!document.querySelector(".ts-ctxmenu")`, { label: "меню закрыто" });
  // Кнопка «⋯» папки — то же через клавиатурно доступное меню.
  await page.evaluate(`document.querySelector('[data-drag-path="Физика/Механика"] > button[aria-haspopup=menu]').click()`);
  await page.waitForElement('[data-drag-path="Физика/Механика"] [role=menu]');
  check(page, (await page.text('[data-drag-path="Физика/Механика"] [role=menu]')).includes("Добавить в граф знаний…"), "A: пункт есть и в меню «⋯» папки");
  await page.evaluate(`document.querySelector('[data-drag-path="Физика/Механика"] > button[aria-haspopup=menu]').click()`);
  check(page, !(await page.evaluate(`[...document.querySelectorAll('[role=tree] > li')].some((li) => !li.dataset.dragPath && li.querySelector("button[aria-haspopup=menu]"))`)), "A: у корня vault меню импорта нет");

  // ── B: подтверждение до отправки ──
  await addToGraphFrom(page, "Физика/Механика");
  await waitConfirm(page);
  await page.waitFor(`document.querySelector(".ts-import .ts-import__kv") !== null`, { label: "оценка показана" });
  let dialog = await page.text(".ts-import");
  check(page, dialog.includes("Добавить «Физика/Механика» в граф знаний?"), "B: заголовок «Добавить «Физика/Механика» в граф знаний?»");
  check(page, /Файлов уйдёт модели\s*4 \(3 заметки, 1 PDF\)/.test(dialog), `B: «4 (3 заметки, 1 PDF)» (${dialog.replace(/\n/g, " ")})`);
  check(page, dialog.includes("Объём") && dialog.includes("единиц"), "B: объём в единицах");
  check(page, /Примерная стоимость\s*(≈ \$|< \$)\S+ · claude-sonnet-5-5/.test(dialog), "B: стоимость и модель");
  check(page, /Пропущено\s*3 файла/.test(dialog), "B: «Пропущено 3 файла»");
  check(page, dialog.includes("Текст файлов попадёт в журнал отправленного"), "B: подсказка про журнал");
  await page.click("text=Показать");
  const skipped = await page.text(".ts-import__skipped");
  check(page, skipped.includes("Личное/Мысли.typ") && skipped.includes("папка закрыта для агента"), "B: запрещённая подпапка в списке пропущенных с причиной");
  check(page, skipped.includes("Схема.board") && skipped.includes("не поддерживается"), "B: доска — «не поддерживается»");
  check(page, skipped.includes("Скан задачника.pdf") && skipped.includes("нет текстового слоя"), "B: PDF без текста — «нет текстового слоя»");
  await page.shot(`${shots}/import-B-confirm.png`);
  // Отмена не отправляет ничего.
  await press(page, ".ts-import", "Отмена");
  await page.waitFor(`!document.querySelector(".ts-import")`, { label: "подтверждение закрыто" });
  check(page, (await imports(page)).length === 0, "B: «Отмена» — модели ничего не ушло");

  // ── C: ход по файлам; редактор не заблокирован ──
  await page.evaluate(`document.querySelector('[data-drag-path="Черновик.typ"] button').click()`);
  await page.waitForElement(".cm-content");
  await plan(page, [
    findings([{ key: "a", text: "Импульс тела равен произведению массы на скорость" }]),
    findings([{ key: "a", text: "Кинетическая энергия равна половине массы на квадрат скорости" }]),
    findings([{ key: "a", text: "Работа силы равна произведению силы на путь" }]),
    findings([{ key: "a", text: "Во втором законе Ньютона сила равна произведению массы на ускорение" }]),
  ]);
  await addToGraphFrom(page, "Физика/Механика");
  await waitConfirm(page);
  await page.waitFor(`document.querySelector(".ts-import .ts-import__kv") !== null`, { label: "оценка показана" });
  await press(page, ".ts-import", "Добавить");
  await page.waitFor(`document.querySelector(".ts-import-card [role=progressbar]") !== null`, { timeout: 20000, label: "карточка хода" });
  check(page, !(await page.evaluate(`Boolean(document.querySelector(".ts-import[role=dialog]"))`)), "C: подтверждение закрылось, карточка хода не модальная");
  let card = await page.text(".ts-import-card");
  check(page, /Импорт в граф знаний/.test(card) && card.includes("Отменить"), "C: заголовок хода и «Отменить»");
  // Ввод в редактор во время импорта (SC-003).
  await page.click(".cm-content");
  await page.type("ВВОД-ВО-ВРЕМЯ-ИМПОРТА");
  check(page, (await page.text(".cm-content")).includes("ВВОД-ВО-ВРЕМЯ-ИМПОРТА"), "C: ввод в заметку во время импорта работает");
  check(page, await page.evaluate(`document.querySelector(".ts-import-card") !== null`), "C: импорт при этом продолжается");
  await page.waitFor(`document.querySelectorAll(".ts-import__file").length >= 7`, { label: "строки файлов" });
  await page.waitFor(`[...document.querySelectorAll(".ts-import__file")].some((li) => li.dataset.state === "Running" || li.dataset.state === "Done")`, { timeout: 30000, label: "первый файл пошёл" });
  await page.shot(`${shots}/import-C-progress.png`);
  await waitDone(page);

  // ── D: итог ──
  card = await page.text(".ts-import-card");
  const calls1 = await imports(page);
  check(page, calls1.length === 4, `D: ровно 4 запроса: 3 заметки и PDF (получено ${calls1.length})`);
  check(page, !calls1.some((b) => b.includes(SECRET)), "SC-001: текст из закрытой папки модели не ушёл");
  check(page, ["ИМПУЛЬС-ТЕКСТ", "ЭНЕРГИЯ-ТЕКСТ", "РАБОТА-ТЕКСТ"].every((m) => calls1.some((b) => b.includes(m))), "D: в запросах — тексты трёх заметок");
  for (const name of ["Физика/Механика/Импульс.typ", "Физика/Механика/Энергия.typ", "Физика/Механика/Работа.typ", "Физика/Механика/Ландау, гл. 2.pdf"]) {
    const row = await rowOf(page, name);
    check(page, row?.state === "Done" && row.text.includes("готово"), `D: «${name}» — готово`);
  }
  const nopdf = await rowOf(page, "Физика/Механика/Скан задачника.pdf");
  check(page, nopdf?.state === "Skipped" && nopdf.text.includes("пропущен: нет текстового слоя"), "D: скан — «пропущен: нет текстового слоя»");
  const board = await rowOf(page, "Физика/Механика/Схема.board");
  check(page, board?.state === "Skipped" && board.text.includes("пропущен: не поддерживается"), "D: доска — «пропущен: не поддерживается»");
  const secret = await rowOf(page, "Физика/Механика/Личное/Мысли.typ");
  check(page, secret?.state === "Skipped" && secret.text.includes("пропущен: папка закрыта для агента"), "D: закрытая папка — «пропущен: папка закрыта для агента»");
  check(page, card.includes("В граф добавлено 4 утверждения"), `D: «В граф добавлено 4 утверждения» (${card.split("\n")[1]})`);
  check(page, card.includes("Открыть граф") && card.includes("Закрыть"), "D: «Открыть граф» и «Закрыть»");
  await page.shot(`${shots}/import-D-done.png`);
  await press(page, ".ts-import-card", "Закрыть");
  await page.waitFor(`!document.querySelector(".ts-import-card")`, { label: "карточка закрыта" });

  // ── Повтор неизменённой папки: 0 запросов ──
  await addToGraphFrom(page, "Физика/Механика");
  await waitConfirm(page);
  await page.waitFor(`document.querySelector(".ts-import .ts-import__kv") !== null`, { label: "оценка показана" });
  dialog = await page.text(".ts-import");
  check(page, dialog.includes("уже в графе") && dialog.includes("Отправлять нечего"), "повтор: оценка говорит, что всё уже в графе");
  await page.shot(`${shots}/import-B-nothing-new.png`);
  await press(page, ".ts-import", "Добавить");
  await waitDone(page);
  check(page, (await imports(page)).length === 4, "повтор неизменённой: новых запросов 0");
  await press(page, ".ts-import-card", "Закрыть");

  // ── Правка одной заметки → 1 запрос ──
  await page.evaluate(`document.querySelector('[data-drag-path="Физика/Механика/Работа.typ"] button').click()`);
  await page.waitFor(`document.querySelector(".cm-content")?.innerText.includes("РАБОТА-ТЕКСТ")`, { label: "заметка «Работа» открыта" });
  await page.click(".cm-content");
  await page.press("End", { ctrl: true });
  await page.type("\nМощность — это работа за единицу времени. МОЩНОСТЬ-ТЕКСТ.");
  await page.sleep(2500);
  await plan(page, [findings([{ key: "a", text: "Мощность — работа за единицу времени" }])]);
  await addToGraphFrom(page, "Физика/Механика");
  await waitConfirm(page);
  await page.waitFor(`document.querySelector(".ts-import .ts-import__kv") !== null`, { label: "оценка показана" });
  await press(page, ".ts-import", "Добавить");
  await waitDone(page);
  const calls2 = await imports(page);
  check(page, calls2.length === 5 && calls2[4].includes("МОЩНОСТЬ-ТЕКСТ"), `правка одной заметки — ровно 1 новый запрос (всего ${calls2.length})`);
  await press(page, ".ts-import-card", "Закрыть");

  // ── Отмена посреди ──
  await plan(page, [
    findings([{ key: "a", text: "Линза собирает параллельные лучи в фокусе" }], [], 1200),
    findings([{ key: "a", text: "Призма раскладывает белый свет в спектр" }], [], 1200),
    findings([{ key: "a", text: "Угол падения равен углу отражения" }], [], 1200),
  ]);
  const beforeCancel = (await imports(page)).length;
  await openFolder(page, "Физика/Оптика");
  await addToGraphFrom(page, "Физика/Оптика");
  await waitConfirm(page);
  await page.waitFor(`document.querySelector(".ts-import .ts-import__kv") !== null`, { label: "оценка показана" });
  await press(page, ".ts-import", "Добавить");
  await page.waitFor(`window.__modelCalls.length > ${beforeCancel + 0} && document.querySelector(".ts-import-card [role=progressbar]") !== null`, { timeout: 20000, label: "первый запрос оптики ушёл" });
  await press(page, ".ts-import-card", "Отменить");
  await waitDone(page, "Импорт отменён");
  const afterCancel = (await imports(page)).length - beforeCancel;
  check(page, afterCancel < 3, `отмена: ушло ${afterCancel} из 3 запросов, остальные не отправлены`);
  const pending = await page.evaluate(`[...document.querySelectorAll(".ts-import__file")].filter((li) => li.dataset.state === "Pending").length`);
  check(page, pending >= 1, `отмена: ${pending} файла(ов) отмечены «не отправлен»`);
  check(page, (await page.text(".ts-import-card")).includes("не отправлен"), "отмена: в итоге написано «не отправлен»");
  await page.shot(`${shots}/import-D-cancelled.png`);
  await press(page, ".ts-import-card", "Закрыть");

  // ── Сбой 503 → «Сбой» и «Повторить» ──
  await openFolder(page, "Физика/Сбой");
  await page.evaluate("window.__plan.length = 0");
  await plan(page, [{ status: 503 }, { status: 503 }, { status: 503 }, { status: 503 }]);
  await addToGraphFrom(page, "Физика/Сбой/Один.typ");
  await waitConfirm(page);
  await page.waitFor(`document.querySelector(".ts-import .ts-import__kv") !== null`, { label: "оценка показана" });
  check(page, (await page.text(".ts-import")).includes("Добавить «Физика/Сбой/Один.typ» в граф знаний?"), "заголовок для одного файла");
  await press(page, ".ts-import", "Добавить");
  await waitDone(page, "Импорт");
  await page.waitFor(`document.querySelector(".ts-import-card")?.innerText.includes("Закрыть")`, { timeout: 60000, label: "итог сбоя" });
  const failedRow = await rowOf(page, "Физика/Сбой/Один.typ");
  check(page, failedRow?.state === "Failed" && failedRow.text.includes("сбой:") && failedRow.retry, `сбой: файл в состоянии «Сбой» с «Повторить» (${failedRow?.text})`);
  await page.shot(`${shots}/import-D-failed.png`);

  // ── T008: «Повторить» → новая оценка → подтверждение → импорт ──
  const beforeRetry = (await imports(page)).length;
  await page.evaluate("window.__plan.length = 0");
  await plan(page, [findings([{ key: "a", text: "Заметка после сбоя повторно обработана" }])]);
  await page.click('button[aria-label="Повторить «Физика/Сбой/Один.typ»"]');
  await waitConfirm(page);
  await page.waitFor(`document.querySelector(".ts-import .ts-import__kv") !== null`, { label: "новая оценка показана" });
  await press(page, ".ts-import", "Добавить");
  await waitDone(page);
  const retried = await rowOf(page, "Физика/Сбой/Один.typ");
  check(page, retried?.state === "Done", "T008: после «Повторить» файл — «готово»");
  check(page, (await imports(page)).length === beforeRetry + 1, "T008: повтор отправил один запрос");

  // ── Устаревшая оценка: файл изменился между подтверждением и запуском ──
  await press(page, ".ts-import-card", "Закрыть");
  await page.evaluate(`${WRITE_FILES}(${JSON.stringify({ "Физика/Сбой/Два.typ": "= Два\nНовая заметка. ДВА-ТЕКСТ.\n" })})`);
  await addToGraphFrom(page, "Физика/Сбой");
  await waitConfirm(page);
  await page.waitFor(`document.querySelector(".ts-import .ts-import__kv") !== null`, { label: "оценка показана" });
  await page.evaluate(`${WRITE_FILES}(${JSON.stringify({ "Физика/Сбой/Два.typ": "= Два\nЗаметка изменилась. ДВА-ИЗМЕНЕНО.\n" })})`);
  await press(page, ".ts-import", "Добавить");
  await page.waitFor(`document.querySelector(".ts-import .ts-lesson__bad")?.innerText.includes("Файлы изменились")`, { timeout: 20000, label: "«Файлы изменились»" });
  check(page, (await page.text(".ts-import")).includes("Оценить заново"), "устаревшая оценка: «Файлы изменились. Оцените заново.» и кнопка «Оценить заново»");
  await page.shot(`${shots}/import-B-stale.png`);
  await press(page, ".ts-import", "Оценить заново");
  await page.waitFor(`document.querySelector(".ts-import .ts-import__kv") !== null && !document.querySelector(".ts-import .ts-lesson__bad")`, { label: "новая оценка без ошибки" });
  await press(page, ".ts-import", "Отмена");

  // ── «Добавить заметки…» в виде графа ──
  await openPalette(page, "Граф знаний");
  await page.waitForElement(".ts-kg");
  await page.waitFor(`!document.querySelector(".ts-kg__empty")?.textContent.includes("Загружаем")`, { label: "граф загружен" });
  const bar = await page.text(".ts-kg__bar");
  check(page, /\d+ утвержд/.test(bar), `граф получил утверждения из заметок (${bar.replace(/\n/g, " ")})`);
  await page.click("text=Добавить заметки…");
  await page.waitForElement(".ts-import[role=dialog]");
  await page.type("Призма");
  await page.sleep(300);
  const rowsPicked = await page.evaluate(`[...document.querySelectorAll(".ts-import__pickrow")].map((x) => x.innerText.trim())`);
  check(page, rowsPicked.some((x) => x.includes("Призма.typ")), "выбор: поиск находит заметку");
  check(page, await page.evaluate(`document.querySelector(".ts-import .ts-btn--accent").disabled`), "выбор: «Далее» недоступна, пока ничего не выбрано");
  await page.evaluate(`document.querySelector(".ts-import__pickrow input").click()`);
  await page.shot(`${shots}/import-pick.png`);
  await press(page, ".ts-import", "Далее");
  await waitConfirm(page);
  await page.waitFor(`document.querySelector(".ts-import .ts-import__kv") !== null`, { label: "оценка выбора" });
  dialog = await page.text(".ts-import");
  check(page, dialog.includes("Призма.typ"), "выбор → подтверждение для выбранного файла");
  await press(page, ".ts-import", "Отмена");
  await page.waitFor(`!document.querySelector(".ts-import")`, { label: "закрыто" });

  // ── «Открыть граф» из итога ──
  await openPalette(page, "Вернуться в Workspace");
  await page.waitFor(`!document.querySelector(".ts-kg")`, { label: "вид закрыт" });
  await plan(page, [findings([{ key: "a", text: "Призма раскладывает белый свет в спектр цветов" }])]);
  await addToGraphFrom(page, "Физика/Оптика/Призма.typ");
  await waitConfirm(page);
  await page.waitFor(`document.querySelector(".ts-import .ts-import__kv") !== null`, { label: "оценка показана" });
  await press(page, ".ts-import", "Добавить");
  await waitDone(page);

  // ── Узкий экран и клавиатурный фокус ──
  await page.viewport(390, 800);
  await page.sleep(500);
  const over = await page.evaluate(`Math.max(document.documentElement.scrollWidth - document.documentElement.clientWidth, 0)`);
  check(page, over <= 0, `узко: карточка итога не выходит за край (избыток ${over})`);
  const box = await page.evaluate(`(() => { const r = document.querySelector(".ts-import-card").getBoundingClientRect(); return { left: r.left, right: r.right, w: innerWidth }; })()`);
  check(page, box.left >= 0 && box.right <= box.w, `узко: карточка внутри экрана (${Math.round(box.left)}…${Math.round(box.right)} из ${box.w})`);
  await page.shot(`${shots}/import-narrow-done.png`);
  await page.evaluate(`document.querySelector(".ts-import-card .ts-btn--accent").focus()`);
  await page.press("Tab", { shift: true });
  const focus = await page.evaluate(`(() => { const el = document.activeElement; const s = getComputedStyle(el); return { inCard: Boolean(el.closest(".ts-import-card")), outline: s.outlineStyle, visible: el.matches(":focus-visible") }; })()`);
  check(page, focus.inCard && focus.outline !== "none" && focus.visible, `узко: клавиатурный фокус в карточке виден (${JSON.stringify(focus)})`);
  await page.evaluate(`document.querySelector(".ts-import-card .ts-btn--accent").click()`);
  await page.waitForElement(".ts-kg", { timeout: 20000 });
  check(page, true, "D: «Открыть граф» открывает вид графа знаний");
  await page.waitFor(`!document.querySelector(".ts-import-card")`, { label: "карточка ушла" });
  await page.shot(`${shots}/import-narrow-graph.png`);
  await page.viewport(1280, 800);
  await page.sleep(400);
  await page.shot(`${shots}/import-graph.png`);
};
