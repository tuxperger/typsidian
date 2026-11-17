// Фоновое исследование в уроке (фича 056, T010): настройки (E), урок идёт, пока исследование
// идёт (A), готовый результат с источниками и заметкой (B), чтение PDF с оценкой (C), отмена,
// прерванное с «Повторить» (D), запрос учителя запускается один раз, при выключенном поиске
// в запросах нет `web_search` (SC-003), закрытая папка, узкий экран и клавиатурный фокус.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/ai-research.mjs --opfs-vault --timeout 300000
//
// Сеть к модели подменена: запросы исследования (в теле есть `submit_research`) берут
// ответы из очереди `window.__research`, остальные — из `window.__plan`, как в ai-knowledge.mjs.
// Снимки: RESEARCH_SHOTS (по умолчанию /tmp/claude-1000), research-*.png.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const shots = process.env.RESEARCH_SHOTS ?? "/tmp/claude-1000";
const PDF_BASE64 = readFileSync(
  fileURLToPath(new URL("../../../packages/core/tests/fixtures/pdf/text.pdf", import.meta.url)),
).toString("base64");

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
  window.__research = [];
  const sse = (name, data) => "event: " + name + "\\ndata: " + JSON.stringify(data) + "\\n\\n";
  const start = (input) => sse("message_start", { type: "message_start", message: { usage: { input_tokens: input, output_tokens: 1 } } });
  const stop = (reason, extra = {}) => [
    sse("message_delta", { type: "message_delta", delta: { stop_reason: reason }, usage: { output_tokens: 70, ...extra } }),
    sse("message_stop", { type: "message_stop" }),
  ];
  window.__reply = ({ text = "", tools = [], input = 1200 } = {}) => {
    const out = [start(input)];
    let index = 0;
    if (text !== "") {
      out.push(sse("content_block_start", { type: "content_block_start", index, content_block: { type: "text", text: "" } }));
      out.push(sse("content_block_delta", { type: "content_block_delta", index, delta: { type: "text_delta", text } }));
      out.push(sse("content_block_stop", { type: "content_block_stop", index }));
      index += 1;
    }
    for (const tool of tools) {
      out.push(sse("content_block_start", { type: "content_block_start", index, content_block: { type: "tool_use", id: "toolu_" + index, name: tool.name, input: {} } }));
      out.push(sse("content_block_delta", { type: "content_block_delta", index, delta: { type: "input_json_delta", partial_json: JSON.stringify(tool.input) } }));
      out.push(sse("content_block_stop", { type: "content_block_stop", index }));
      index += 1;
    }
    out.push(...stop(tools.length ? "tool_use" : "end_turn"));
    return { chunks: out };
  };
  // Ответ исследования: поиск, найденные страницы, текст с цитатами и сдача результата.
  window.__rreply = ({ web = true, summary, paragraphs, sources, found = sources }) => {
    const out = [start(3000)];
    let index = 0;
    if (web) {
      out.push(sse("content_block_start", { type: "content_block_start", index, content_block: { type: "server_tool_use", id: "srvtoolu_1", name: "web_search", input: {} } }));
      out.push(sse("content_block_delta", { type: "content_block_delta", index, delta: { type: "input_json_delta", partial_json: JSON.stringify({ query: "импульс" }) } }));
      out.push(sse("content_block_stop", { type: "content_block_stop", index }));
      index += 1;
      out.push(sse("content_block_start", { type: "content_block_start", index, content_block: { type: "web_search_tool_result", tool_use_id: "srvtoolu_1", content: found.map((s) => ({ type: "web_search_result", url: s.url, title: s.title, page_age: null, encrypted_content: "x" })) } }));
      out.push(sse("content_block_stop", { type: "content_block_stop", index }));
      index += 1;
    }
    out.push(sse("content_block_start", { type: "content_block_start", index, content_block: { type: "text", text: "" } }));
    out.push(sse("content_block_delta", { type: "content_block_delta", index, delta: { type: "text_delta", text: summary } }));
    if (web) {
      for (const s of sources) {
        out.push(sse("content_block_delta", { type: "content_block_delta", index, delta: { type: "citations_delta", citation: { type: "web_search_result_location", url: s.url, title: s.title, cited_text: "…", encrypted_index: "x" } } }));
      }
    }
    out.push(sse("content_block_stop", { type: "content_block_stop", index }));
    index += 1;
    out.push(sse("content_block_start", { type: "content_block_start", index, content_block: { type: "tool_use", id: "toolu_s", name: "submit_research", input: {} } }));
    out.push(sse("content_block_delta", { type: "content_block_delta", index, delta: { type: "input_json_delta", partial_json: JSON.stringify({ paragraphs, sources: sources.map((s) => ({ title: s.title, ...(s.url ? { url: s.url } : {}), ...(s.pages ? { pages: s.pages } : {}) })) }) } }));
    out.push(sse("content_block_stop", { type: "content_block_stop", index }));
    out.push(...stop("tool_use", web ? { server_tool_use: { web_search_requests: 1 } } : {}));
    return { chunks: out };
  };
  window.fetch = async (input, init) => {
    const url = typeof input === "string" ? input : String(input?.url ?? input);
    if (!url.includes("api.anthropic.com")) return realFetch(input, init);
    const body = String(init?.body ?? (typeof input?.clone === "function" ? await input.clone().text() : ""));
    calls.push({ url, body });
    const isResearch = body.includes("submit_research");
    const item = (isResearch ? window.__research : window.__plan).shift();
    if (!item) return new Response("{}", { status: 500 });
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

const WRITE_FILES = `(async (files, bytes) => {
  const root = await (await navigator.storage.getDirectory()).getDirectoryHandle("vault", { create: true });
  const put = async (path, data) => {
    const parts = path.split("/");
    let dir = root;
    for (const part of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(part, { create: true });
    const file = await dir.getFileHandle(parts[parts.length - 1], { create: true });
    const stream = await file.createWritable();
    await stream.write(data);
    await stream.close();
  };
  for (const [path, text] of Object.entries(files)) await put(path, text);
  for (const [path, base64] of Object.entries(bytes)) {
    await put(path, Uint8Array.from(atob(base64), (c) => c.charCodeAt(0)));
  }
  return true;
})`;

const readVault = (path) => `(async () => {
  const root = await (await navigator.storage.getDirectory()).getDirectoryHandle("vault");
  const parts = ${JSON.stringify(path)}.split("/");
  let dir = root;
  for (const part of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(part);
  return await (await (await dir.getFileHandle(parts[parts.length - 1])).getFile()).text();
})()`;
const listVault = (folder) => `(async () => {
  let dir = await (await navigator.storage.getDirectory()).getDirectoryHandle("vault");
  for (const part of ${JSON.stringify(folder)}.split("/")) dir = await dir.getDirectoryHandle(part);
  const names = [];
  for await (const [name] of dir.entries()) names.push(name);
  return names;
})()`;

const openPalette = async (page, query) => {
  await page.click(".ts-cmdfield");
  await page.waitForElement(".ts-palette");
  await page.type(query);
  await page.sleep(200);
  await page.press("Enter");
};
const planLesson = (page, items) =>
  page.evaluate(`window.__plan.push(...${JSON.stringify(items)}.map((i) => ({ ...window.__reply(i.reply), delay: i.delay })))`);
const planResearch = (page, items) =>
  page.evaluate(`window.__research.push(...${JSON.stringify(items)}.map((i) => ({ ...window.__rreply(i.reply), delay: i.delay })))`);
const researchCalls = (page) =>
  page.evaluate(`window.__modelCalls.filter((c) => c.body.includes("submit_research")).length`);
const press = (page, scope, label) =>
  page.evaluate(`(() => {
    const b = [...document.querySelectorAll(${JSON.stringify(scope + " button")})].find((x) => x.textContent.trim() === ${JSON.stringify(label)});
    if (!b) throw new Error("нет кнопки " + ${JSON.stringify(label)});
    b.click();
  })()`);
const sw = (page, label) =>
  page.evaluate(`(() => {
    const row = [...document.querySelectorAll(".ts-srow")].find((r) => r.textContent.includes(${JSON.stringify(label)}));
    const input = row?.querySelector("input");
    return input ? { present: true, on: input.checked, value: input.value, hint: row.querySelector(".ts-srow__hint")?.textContent ?? "" } : { present: false };
  })()`);
const cards = (page) =>
  page.evaluate(`[...document.querySelectorAll(".ts-research")].map((c) => ({ status: c.dataset.status, text: c.innerText }))`);
const lessonIdle = (page) =>
  page.waitFor(`!document.querySelector(".ts-lesson__status")`, { timeout: 40000, label: "ход урока закончен" });
const askInput = async (page, text) => {
  await page.click(".ts-lesson__q textarea");
  await page.type(text);
  await page.press("Enter", { ctrl: true });
};
const openSettingsAi = async (page) => {
  await page.click("text=Настройки");
  await page.waitForElement("text=Обучение с ИИ");
  await page.sleep(400);
  await page.click("text=Обучение с ИИ");
  await page.waitFor(`document.querySelector(".ts-settings")?.innerText.includes("Поиск в вебе для исследований") || document.querySelector(".ts-settings")?.innerText.includes("Включить для этого vault")`, { label: "раздел «Обучение с ИИ»" });
};
const researchBar = (page) => page.evaluate(`document.querySelector(".ts-research__bar")?.innerText ?? ""`);
const webButton = (page) =>
  page.evaluate(`(() => { const b = [...document.querySelectorAll(".ts-research__bar button")].find((x) => x.textContent.trim() === "В вебе"); return b ? { disabled: b.disabled, title: b.title } : null; })()`);
const askWeb = async (page, question) => {
  await page.click(".ts-research__input");
  await page.type(question);
  // Настройки вкладка перечитывает раз в две секунды.
  await page.waitFor(`[...document.querySelectorAll(".ts-research__bar button")].some((x) => x.textContent.trim() === "В вебе" && !x.disabled)`, { timeout: 10000, label: "«В вебе» доступна" });
  await press(page, ".ts-research__bar", "В вебе");
};

export default async (page) => {
  await page.addInitScript(FAKE_MODEL);
  await page.reload();
  await page.evaluate(
    `${WRITE_FILES}(${JSON.stringify({ "Физика/Импульс.typ": "= Импульс\nИмпульс равен массе на скорость.\n" })}, ${JSON.stringify({ "Литература/Ландау.pdf": PDF_BASE64, "Личное/Выписка.pdf": PDF_BASE64 })})`,
  );

  await page.click("text=Продолжить");
  await page.click("text=Открыть папку…");
  await page.waitForElement("text=Настройки", { timeout: 30000 });

  // ── Включаем ИИ; закрываем папку «Личное» ──
  await openSettingsAi(page);
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
  await page.click('input[placeholder="Личное/Дневник"]');
  await page.type("Личное");
  await page.click("text=Добавить папку");
  await page.waitForElement('button[aria-label*="Личное"]');

  // ── E: настройки исследований ──
  await page.waitForElement("text=Поиск в вебе для исследований");
  const web = await sw(page, "Поиск в вебе для исследований");
  const par = await sw(page, "Одновременных исследований");
  const pdf = await sw(page, "Учитель может сам предлагать изучить PDF");
  check(page, web.present && !web.on && web.hint.includes("уходят в открытый интернет"), "E: поиск в вебе выключен по умолчанию, подсказка про интернет");
  check(page, par.present && par.value === "2" && par.hint.includes("ждут в очереди"), "E: одновременных исследований — 2");
  check(page, pdf.present && !pdf.on, "E: «Учитель может сам предлагать изучить PDF» выключено");
  await page.evaluate(`[...document.querySelectorAll(".ts-srow")].find((r) => r.textContent.includes("Поиск в вебе для исследований")).scrollIntoView({ block: "center" })`);
  await page.shot(`${shots}/research-E.png`);
  await page.press("Escape");
  await page.sleep(300);

  // ── Урок; поиск выключен ──
  await openPalette(page, "Новый урок");
  await page.waitForElement(".ts-lesson-dialog textarea");
  await page.type("Понять, почему сохраняется импульс");
  await page.click("text=Сразу с урока");
  await planLesson(page, [{ reply: { text: "Начнём с тележек.", tools: [{ name: "ask_question", input: { question: "Что будет, если толкнуть тележку?" } }] } }]);
  await page.click("text=Начать урок");
  await page.waitForElement(".ts-lesson__q", { timeout: 40000 });
  await lessonIdle(page);

  check(page, (await page.evaluate(`Boolean(document.querySelector(".ts-research__input"))`)), "поле «Исследовать…» под лентой");
  let wb = await webButton(page);
  check(page, wb !== null && wb.disabled, "поиск выключен: «В вебе» недоступна");
  let bar = await researchBar(page);
  check(page, bar.includes("Поиск в вебе выключен") && bar.includes("Включить в настройках"), "объяснение и ссылка в настройки");
  await page.shot(`${shots}/research-D-off.png`);
  check(
    page,
    (await page.evaluate(`window.__modelCalls.every((c) => !c.body.includes("web_search"))`)),
    "SC-003: при выключенном поиске в запросах нет web_search",
  );

  // ── C: PDF — выбор, оценка, «Изучить» ──
  await page.click(".ts-research__input");
  await page.type("Что такое центр инерции?");
  await press(page, ".ts-research__bar", "В PDF…");
  await page.waitForElement(".ts-lesson-dialog input[placeholder]");
  await page.shot(`${shots}/research-C-pick.png`);
  // Закрытая папка: отказ оценки.
  await page.type("Личное/Выписка.pdf");
  await press(page, ".ts-lesson-dialog", "Далее");
  await page.waitFor(`document.querySelector(".ts-lesson-dialog")?.innerText.includes("в закрытой папке")`, { label: "отказ закрытой папки" });
  check(page, (await page.text(".ts-lesson-dialog")).includes("«Личное/Выписка.pdf» в закрытой папке — не отправлено."), "D: закрытая папка — «не отправлено»");
  await page.shot(`${shots}/research-D-denied.png`);
  check(page, (await researchCalls(page)) === 0, "закрытый PDF модели не отправлен");
  await press(page, ".ts-lesson-dialog", "Какой PDF изучить");
  await page.waitForElement(".ts-lesson-dialog input[placeholder]");
  await page.click(".ts-lesson-dialog input[placeholder]");
  await page.press("a", { ctrl: true });
  await page.type("Литература/Ландау.pdf");
  await press(page, ".ts-lesson-dialog", "Далее");
  await page.waitFor(`Boolean(document.querySelector(".ts-import__kv"))`, { label: "оценка PDF" });
  let est = await page.text(".ts-lesson-dialog");
  check(page, est.includes("Литература/Ландау.pdf") && est.includes("страниц") && est.includes("Примерная стоимость"), "C: файл, объём со страницами, стоимость");
  await page.shot(`${shots}/research-C.png`);
  await planResearch(page, [
    { reply: { web: false, summary: "Центр инерции движется равномерно (с. 1).", paragraphs: ["Центр инерции замкнутой системы движется равномерно и прямолинейно (с. 1)."], sources: [{ title: "Ландау.pdf", pages: [1] }] }, delay: 500 },
  ]);
  await press(page, ".ts-lesson-dialog", "Изучить");
  await page.waitForElement(".ts-research");
  await page.waitFor(`document.querySelector(".ts-research")?.dataset.status === "Done"`, { timeout: 40000, label: "PDF изучен" });
  let list = await cards(page);
  check(page, list.length === 1 && list[0].text.includes("PDF") && list[0].text.includes("Что такое центр инерции?"), "карточка PDF с вопросом");
  check(page, list[0].text.includes("с. 1") && list[0].text.includes("Литература/Ландау.pdf"), "B: страницы и файл в источниках");
  check(page, !list[0].text.includes("найдено моделью"), "у PDF нет пометки «найдено моделью»");
  const body1 = await page.evaluate(`window.__modelCalls.filter((c) => c.body.includes("submit_research"))[0].body`);
  check(page, !body1.includes("web_search"), "запрос PDF без инструмента поиска");
  await page.shot(`${shots}/research-B-pdf.png`);

  // ── Включаем поиск в вебе ──
  await openSettingsAi(page);
  await page.click("text=Поиск в вебе для исследований");
  await page.waitFor(`document.querySelector(".ts-srow input[role=switch]") && [...document.querySelectorAll(".ts-srow")].find((r) => r.textContent.includes("Поиск в вебе для исследований")).querySelector("input").checked`, { label: "поиск включён" });
  await page.press("Escape");
  await page.sleep(500);
  wb = await webButton(page);
  check(page, wb !== null && wb.disabled, "«В вебе» недоступна, пока вопрос пуст");

  // ── A: веб-исследование идёт, урок отвечает ──
  const SOURCES = [
    { title: "Relativistic momentum", url: "https://en.wikipedia.org/wiki/Momentum" },
    { title: "Lecture 12: Collisions", url: "https://ocw.mit.edu/lecture-12" },
  ];
  await planResearch(page, [
    { reply: { summary: "Сохраняется релятивистский импульс p = γmv.", paragraphs: ["Сохраняется релятивистский импульс p = γmv; классическое mv при скоростях, близких к c, не сохраняется.", "Энергия и импульс образуют четырёхвектор."], sources: SOURCES }, delay: 900 },
  ]);
  await askWeb(page, "Как импульс сохраняется в релятивистских столкновениях?");
  await page.waitFor(`document.querySelectorAll(".ts-research").length === 2 && [...document.querySelectorAll(".ts-research")].some((c) => c.dataset.status === "Running" || c.dataset.status === "Queued")`, { timeout: 20000, label: "карточка «идёт»" });
  list = await cards(page);
  const running = list.find((c) => c.status === "Running" || c.status === "Queued");
  check(page, running.text.includes("ВЕБ") && running.text.includes("Отменить"), "A: карточка «ВЕБ … идёт» с «Отменить»");
  await page.shot(`${shots}/research-A.png`);
  // Урок отвечает, пока исследование идёт.
  await planLesson(page, [{ reply: { text: "Верно, импульс сохраняется.", tools: [{ name: "ask_question", input: { question: "А что если масса B вдвое больше?" } }] } }]);
  await askInput(page, "Тележка поедет назад.");
  await page.waitFor(`document.querySelector(".ts-lesson__feed")?.innerText.includes("Верно, импульс сохраняется.") && !document.querySelector(".ts-lesson__status")`, { timeout: 40000, label: "учитель ответил" });
  const during = await cards(page);
  check(page, during.some((c) => c.status === "Running" || c.status === "Done"), "урок ответил, исследование не потеряно");
  await page.waitFor(`[...document.querySelectorAll(".ts-research")].every((c) => c.dataset.status === "Done")`, { timeout: 40000, label: "веб-исследование готово" });
  list = await cards(page);
  const webCard = list.find((c) => c.text.includes("ВЕБ"));
  check(page, webCard.text.includes("Relativistic momentum") && webCard.text.includes("en.wikipedia.org"), "B: источники ссылками с хостом");
  check(page, webCard.text.includes("найдено моделью — проверяйте источники"), "B: пометка «найдено моделью»");
  const links = await page.evaluate(`[...document.querySelectorAll(".ts-research__src a")].map((a) => ({ href: a.href, target: a.target, rel: a.rel }))`);
  check(page, links.length === 2 && links.every((l) => l.target === "_blank" && l.rel.includes("noopener")), "B: ссылки открываются во внешней вкладке без opener");
  await page.shot(`${shots}/research-B.png`);

  // Заметка: открывается и собирается в предпросмотре.
  const notes = (await page.evaluate(listVault("Уроки/Исследования"))).filter((name) => name.includes("Relativ") || name.includes("Как импульс") || !name.includes("Что такое"));
  check(page, notes.length >= 1, `заметка лежит в «Уроки/Исследования/» (${notes.join(", ")})`);
  await page.evaluate(`(() => {
    const card = [...document.querySelectorAll(".ts-research")].find((c) => c.innerText.includes("ВЕБ") && c.innerText.includes("Relativistic"));
    [...card.querySelectorAll("button")].find((b) => b.textContent.trim() === "Открыть заметку").click();
  })()`);
  await page.waitFor(`Boolean(document.querySelector(".cm-editor")) && /найдено моделью/i.test(document.querySelector(".cm-content")?.innerText ?? "")`, { timeout: 30000, label: "заметка открыта во вкладке" });
  await page.waitFor(`/собрано/.test(document.body.innerText) && Boolean(document.querySelector(".ts-preview canvas, .ts-preview svg, .ts-preview img, .ts-preview [class*=page]"))`, { timeout: 40000, label: "предпросмотр заметки собран" });
  await page.shot(`${shots}/research-note.png`);
  const noteText = await page.evaluate(readVault(`Уроки/Исследования/${notes[0]}`));
  check(page, noteText.includes("wikipedia.org") && noteText.includes("#link("), "в заметке источники ссылками");
  await openPalette(page, "Уроки");
  await page.waitForElement(".ts-lessons__row");
  await page.click(".ts-lessons__row");
  await page.waitForElement(".ts-lesson__feed");

  // ── Запрос учителя: запускается один раз ──
  const before = await researchCalls(page);
  await planResearch(page, [
    { reply: { summary: "Квантовый импульс сохраняется.", paragraphs: ["В квантовой механике сохраняется оператор импульса."], sources: [{ title: "Quantum momentum", url: "https://example.org/q" }] }, delay: 300 },
  ]);
  await planLesson(page, [{ reply: { text: "Пока посмотрю про квантовый случай.", tools: [{ name: "research", input: { question: "Закон сохранения в квантовой механике" } }, { name: "ask_question", input: { question: "Готовы дальше?" } }] } }]);
  await askInput(page, "Да, понятно.");
  await page.waitFor(`document.querySelector(".ts-lesson__feed")?.innerText.includes("Закон сохранения в квантовой механике")`, { timeout: 40000, label: "карточка от учителя" });
  await page.waitFor(`[...document.querySelectorAll(".ts-research")].some((c) => c.innerText.includes("Закон сохранения") && c.dataset.status === "Done")`, { timeout: 40000, label: "исследование учителя готово" });
  await page.sleep(3500);
  check(page, (await researchCalls(page)) === before + 1, "запрос учителя запущен один раз");
  await lessonIdle(page);

  // ── Отмена ──
  await planResearch(page, [
    { reply: { summary: "Долгий ответ.", paragraphs: ["Не должен дойти."], sources: SOURCES }, delay: 2500 },
  ]);
  await askWeb(page, "История закона сохранения импульса");
  await page.waitFor(`[...document.querySelectorAll(".ts-research")].some((c) => c.innerText.includes("История закона") && c.dataset.status === "Running")`, { timeout: 20000, label: "долгое исследование идёт" });
  await press(page, ".ts-research[data-status=Running]", "Отменить");
  await page.waitFor(`[...document.querySelectorAll(".ts-research")].some((c) => c.innerText.includes("История закона") && c.dataset.status === "Cancelled")`, { timeout: 20000, label: "карточка «отменено»" });
  check(page, true, "D: отменено");
  await page.shot(`${shots}/research-D-cancelled.png`);

  // ── Перезагрузка посреди: «прервано» и «Повторить» ──
  await planResearch(page, [
    { reply: { summary: "Не дойдёт.", paragraphs: ["Не дойдёт."], sources: SOURCES }, delay: 4000 },
  ]);
  await askWeb(page, "Релятивистский импульс фотона");
  await page.waitFor(`[...document.querySelectorAll(".ts-research")].some((c) => c.innerText.includes("фотона") && c.dataset.status === "Running")`, { timeout: 20000, label: "исследование идёт перед перезагрузкой" });
  await page.reload();
  await page.sleep(1500);
  if (!(await page.evaluate(`Boolean(document.querySelector(".ts-cmdfield"))`))) {
    await page.click("text=Продолжить").catch(() => undefined);
    await page.click("text=Открыть папку…").catch(() => undefined);
  }
  await page.waitForElement(".ts-lesson__feed", { timeout: 8000 }).catch(async () => {
    await openPalette(page, "Уроки");
    await page.waitForElement(".ts-lessons__row");
    await page.click(".ts-lessons__row");
    await page.waitForElement(".ts-lesson__feed");
  });
  await page.waitFor(`[...document.querySelectorAll(".ts-research")].some((c) => c.innerText.includes("фотона") && c.dataset.status === "Interrupted")`, { timeout: 20000, label: "карточка «прервано»" });
  const interrupted = (await cards(page)).find((c) => c.status === "Interrupted");
  check(page, interrupted.text.includes("Повторить") && interrupted.text.includes("частичного результата нет"), "D: прервано, «Повторить», пояснение");
  await page.shot(`${shots}/research-D-interrupted.png`);
  await planResearch(page, [
    { reply: { summary: "Импульс фотона p = E/c.", paragraphs: ["Импульс фотона p = E/c."], sources: [{ title: "Photon", url: "https://example.org/photon" }] }, delay: 200 },
  ]);
  const beforeRetry = await researchCalls(page);
  await press(page, ".ts-research[data-status=Interrupted]", "Повторить");
  await page.waitFor(`[...document.querySelectorAll(".ts-research")].filter((c) => c.innerText.includes("фотона") && c.dataset.status === "Done").length === 1`, { timeout: 40000, label: "повтор завершён" });
  check(page, (await researchCalls(page)) === beforeRetry + 1, "повтор — один новый запрос");

  // ── Узкий экран и фокус ──
  await page.viewport(390, 800);
  await page.sleep(500);
  const overflow = await page.evaluate(
    `document.querySelector(".ts-lesson") ? document.querySelector(".ts-lesson").scrollWidth - document.querySelector(".ts-lesson").clientWidth : -1`,
  );
  check(page, overflow <= 0, `на 390 px урок не уходит за край (избыток ${overflow})`);
  await page.evaluate(`document.querySelector(".ts-research__input").focus()`);
  await page.press("Tab");
  const focus = await page.evaluate(`(() => { const a = document.activeElement; const s = getComputedStyle(a); return { text: a.textContent.trim(), outline: s.outlineStyle + " " + s.outlineWidth }; })()`);
  page.log(`фокус после Tab: ${JSON.stringify(focus)}`);
  check(page, focus.text.length > 0 && !focus.outline.startsWith("none"), "клавиатурный фокус виден на кнопке поля");
  await page.shot(`${shots}/research-narrow.png`);
};
