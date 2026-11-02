// Сессия урока с ИИ-учителем (фича 051, T022, T025): пустой список (H), список с
// состояниями (G), сжатие и отказы (F), прикреплённые и прочитанные заметки (A, D, F),
// смена языка, «Завершить урок», «урок изменился» (I).
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/ai-lesson-more.mjs --opfs-vault --timeout 60000
//
// Сеть к модели подменена очередью `window.__plan`: каждый запрос к api.anthropic.com
// берёт следующий пункт — готовый поток, код отказа или поток без `message_stop`.
// Файлы vault кладутся прямо в OPFS (корень "vault"). Снимки: LESSON_SHOTS.
const shots = process.env.LESSON_SHOTS ?? "/tmp/claude-1000";

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
          await new Promise((done) => setTimeout(done, 40));
        }
        controller.close();
      },
    });
    return new Response(stream, { status: 200, headers: { "content-type": "text/event-stream" } });
  };
})();
`;

// Файлы в OPFS-vault: { "папка/файл": "текст" }.
const WRITE_FILES = `(async (files) => {
  const root = await (await navigator.storage.getDirectory()).getDirectoryHandle("vault", { create: true });
  for (const [path, text] of Object.entries(files)) {
    const parts = path.split("/");
    let dir = root;
    for (const part of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(part, { create: true });
    const file = await dir.getFileHandle(parts[parts.length - 1], { create: true });
    const stream = await file.createWritable();
    await stream.write(text);
    await stream.close();
  }
  return true;
})`;

const line = (value) => JSON.stringify(value) + "\n";
const header = (goal, at) => line({ format: 1, goal, language: "ru", created_at: at });
const teacher = (at, text, extra = {}) =>
  line({ kind: "Teacher", at, text, interrupted: false, model: "claude-sonnet-5-5", ...extra });
const user = (at, text) => line({ kind: "User", at, text });

const NOW = Date.now();
const SEEDED = {
  "Уроки/Ждёт ответа.lesson":
    header("Что такое энтропия", NOW - 4000) +
    teacher(NOW - 3000, "Начнём с примера.", { question: { question: "Что растёт при смешивании?" } }),
  "Уроки/Оборван.lesson":
    header("Закон Ома", NOW - 3000) +
    teacher(NOW - 2500, "Сначала напряжение.", { question: { question: "Что такое напряжение?" } }) +
    user(NOW - 2000, "Разность потенциалов."),
  "Уроки/Завершён.lesson":
    header("Первый закон Ньютона", NOW - 9000) +
    teacher(NOW - 8000, "Инерция.") +
    line({ kind: "State", at: NOW - 7000, state: "Finished" }),
  "Уроки/Остановлен.lesson":
    header("Дифракция", NOW - 12000) +
    teacher(NOW - 11000, "Волны.") +
    line({ kind: "State", at: NOW - 10000, state: "Stopped" }),
};

const NOTES = {
  "Физика/Импульс.typ": "= Импульс\nИмпульс равен массе на скорость.\n",
  "Физика/Энергия.typ": "= Энергия\nЭнергия сохраняется.\n",
  "Секрет/Дневник.typ": "= Дневник\nЛичное.\n",
};

const openPalette = async (page, query) => {
  await page.click(".ts-cmdfield");
  await page.waitForElement(".ts-palette");
  await page.type(query);
  await page.sleep(200);
  await page.press("Enter");
};

const plan = (page, items) =>
  page.evaluate(`window.__plan.push(...${JSON.stringify(items.map((i) => i))}.map((i) => i.reply ? window.__reply(i.reply) : i))`);

// Вопрос с вариантами/без: ответить свободным текстом.
const answer = async (page, text) => {
  await page.waitForElement(".ts-lesson__q textarea", { timeout: 20000 });
  await page.click(".ts-lesson__q textarea");
  await page.type(text);
  await page.press("Enter", { ctrl: true });
};

const settled = (page, label) =>
  page.waitFor(`!document.querySelector(".ts-lesson__status")`, { timeout: 40000, label });

const lessonFeed = (page) => page.text(".ts-lesson__feed");

export default async (page) => {
  await page.addInitScript(FAKE_MODEL);
  await page.reload();
  await page.evaluate(`${WRITE_FILES}(${JSON.stringify(NOTES)})`);

  await page.click("text=Продолжить");
  await page.click("text=Открыть папку…");
  await page.waitForElement("text=Настройки", { timeout: 30000 });

  // ── H: пустой список ──
  await openPalette(page, "Уроки");
  await page.waitForElement(".ts-lessons__empty");
  const empty = await page.text(".ts-lessons");
  check(page, empty.includes("Уроков ещё нет"), "H: пустой список объясняет, с чего начать");
  check(page, (await page.evaluate(`document.querySelectorAll(".ts-lessons__empty .ts-btn").length`)) === 1, "H: в пустом списке есть «Новый урок»");
  await page.shot(`${shots}/lesson-H.png`);
  await page.press("Escape");
  await page.sleep(300);

  // ── Настройки: ИИ, ключ, модель, закрытая папка ──
  await page.click("text=Настройки");
  await page.waitForElement("text=Обучение с ИИ");
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
  await page.type("Секрет");
  await page.click("text=Добавить папку");
  await page.waitFor(`document.body.innerText.includes("Секрет/ ✕")`, { label: "тег папки" });
  check(page, true, "закрытая для агента папка «Секрет» добавлена в настройках");
  await page.press("Escape");
  await page.sleep(300);

  // ── D: прикрепить заметку в диалоге ──
  await openPalette(page, "Новый урок");
  await page.waitForElement(".ts-lesson-dialog textarea");
  await page.type("Понять импульс");
  await page.click("text=Добавить…");
  await page.waitForElement(".ts-lesson__pick");
  await page.type("Импульс");
  await page.sleep(200);
  const pickList = await page.text(".ts-lesson__picklist");
  check(page, pickList.includes("Физика/Импульс.typ") && !pickList.includes("Энергия"), "D: поиск по заметкам сужает список");
  await page.click(".ts-lesson__pickrow");
  const dialog = await page.text(".ts-lesson-dialog");
  check(page, dialog.includes("Физика/Импульс.typ ✕"), "D: заметка стала чипом");
  await page.shot(`${shots}/lesson-D2.png`);

  // Первый ход: учитель читает три заметки (разрешённую, закрытую, несуществующую), затем спрашивает.
  await plan(page, [
    {
      reply: {
        text: "Посмотрю ваши заметки.",
        tools: [
          { name: "read_note", input: { path: "Физика/Энергия.typ" } },
          { name: "read_note", input: { path: "Секрет/Дневник.typ" } },
          { name: "read_note", input: { path: "Физика/Нет.typ" } },
        ],
      },
    },
    { reply: { text: "Теперь вопрос.", tools: [{ name: "ask_question", input: { question: "Что такое импульс?" } }] } },
  ]);
  await page.click("text=Сразу с урока");
  await page.click("text=Начать урок");
  await page.waitForElement(".ts-lesson__q", { timeout: 30000 });
  await settled(page, "первый ход закончен");
  const feed = await lessonFeed(page);
  check(page, feed.includes("Вы прикрепили") && feed.includes("Физика/Импульс.typ"), "A: строка «Вы прикрепили … Импульс»");
  check(page, feed.includes("отправлено модели, есть в журнале"), "A: «отправлено модели, есть в журнале»");
  check(page, feed.includes("Учитель прочитал") && feed.includes("Физика/Энергия.typ"), "T025: «Учитель прочитал» Энергия");
  check(page, feed.includes("Учитель запросил") && feed.includes("папка закрыта для агента"), "F: отказ «папка закрыта для агента»");
  check(page, feed.includes("такой заметки нет"), "F: отказ «нет заметки»");
  const sent = await page.evaluate("window.__modelCalls.map((c) => c.body).join('\\n')");
  check(page, !sent.includes("Личное."), "содержимое закрытой заметки модели не ушло");
  check(page, sent.includes("Импульс равен массе"), "прикреплённая заметка ушла модели");
  await page.shot(`${shots}/lesson-A2.png`);

  // Ссылка открывает заметку.
  await page.click('button[aria-label="Открыть заметку Физика/Импульс.typ"]');
  await page.sleep(800);
  const opened = await page.evaluate(`Boolean(document.querySelector('[role=tab][aria-selected=true]')?.textContent?.includes("Импульс")) || document.body.innerText.includes("Импульс равен массе")`);
  check(page, opened, "ссылка в ленте открывает заметку");
  await page.shot(`${shots}/lesson-link.png`);
  // Назад во вкладку урока.
  await page.evaluate(`[...document.querySelectorAll('[role=tab]')].find((tab) => tab.textContent.includes("Понять импульс"))?.click()`);
  await page.waitForElement(".ts-lesson__q");

  // ── Язык учителя ──
  await page.click("text=English");
  await page.waitFor(`document.querySelector(".ts-lesson__feed").innerText.includes("язык учителя: English")`, { label: "смена языка в ленте" });
  check(page, (await page.evaluate(`document.querySelector('.ts-lesson__seg button[aria-pressed=true]').textContent`)) === "English", "язык: переключатель показывает English");
  check(page, ((await lessonFeed(page)).match(/Что такое импульс\?/g) ?? []).length === 1, "вопрос учителя не дублируется после чтения заметок и смены языка");
  await page.click("text=Русский");
  await page.waitFor(`document.querySelectorAll(".ts-lesson__sep").length >= 2`, { label: "две смены языка" });
  check(page, true, "язык: обратная смена записана в ленту");

  // ── F: отказ 429 ──
  await plan(page, [{ status: 429 }]);
  await answer(page, "Масса на скорость.");
  await page.waitForElement(".ts-lesson__box--err", { timeout: 20000 });
  await settled(page, "ход после 429 закончен");
  const rl = await lessonFeed(page);
  check(page, rl.includes("Исчерпан лимит запросов"), "F: 429 → «Исчерпан лимит запросов»");
  check(page, rl.includes("Масса на скорость."), "ответ пользователя сохранён после отказа");
  check(page, rl.includes("Запросить ответ снова"), "F: есть «Запросить ответ снова»");
  await page.shot(`${shots}/lesson-F-429.png`);

  // ── F: обрыв потока без message_stop ──
  await plan(page, [{ reply: { text: "Импульс — это величина, которую", stop: false } }]);
  await page.click("text=Запросить ответ снова");
  await page.waitFor(`document.querySelector(".ts-lesson__feed")?.innerText.includes("ОТВЕТ ОБОРВАН") && !document.querySelector(".ts-lesson__status")`, {
    timeout: 30000,
    label: "оборванная реплика",
  });
  const cut = await lessonFeed(page);
  check(page, cut.includes("Импульс — это величина, которую"), "F: полученная часть реплики сохранена");
  check(page, cut.includes("Запросить ответ снова"), "F: после обрыва потока снова есть «Запросить ответ снова»");
  await page.shot(`${shots}/lesson-F-cut.png`);

  // ── Сжатие: два длинных ответа превышают порог 8000 ──
  const longText = (mark) => `${mark} ` + "Длинное объяснение про импульс и его сохранение. ".repeat(450);
  await page.evaluate(`${WRITE_FILES}({})`);
  await page.evaluate(`(async () => {
    const root = await (await navigator.storage.getDirectory()).getDirectoryHandle("vault");
    const dir = await root.getDirectoryHandle(".typsidian");
    const handle = await dir.getFileHandle("ai.json");
    const settings = JSON.parse(await (await handle.getFile()).text());
    settings.context_budget = 8000;
    const out = await handle.createWritable();
    await out.write(JSON.stringify(settings));
    await out.close();
  })()`);
  await plan(page, [
    { reply: { text: longText("ПЕРВЫЙ"), tools: [{ name: "ask_question", input: { question: "Согласны?" } }] } },
    { reply: { text: longText("ВТОРОЙ"), tools: [{ name: "ask_question", input: { question: "Продолжим?" } }] } },
    { reply: { text: "Кратко: обсудили импульс и его сохранение." } },
    { reply: { text: "Продолжаем после сжатия.", tools: [{ name: "ask_question", input: { question: "Готовы дальше?" } }] } },
  ]);
  await page.click("text=Запросить ответ снова");
  await settled(page, "первый длинный ответ");
  await answer(page, "Да.");
  await settled(page, "второй длинный ответ");
  await answer(page, "Да, продолжим.");
  await page.waitFor(`document.querySelector(".ts-lesson__feed")?.innerText.includes("сжаты для модели")`, {
    timeout: 40000,
    label: "отметка сжатия",
  });
  await settled(page, "ход после сжатия");
  const compact = await lessonFeed(page);
  check(page, /первые события \(\d+\) сжаты для модели · полная переписка выше/.test(compact), "F: отметка «первые события (N) сжаты для модели»");
  check(page, compact.includes("ПЕРВЫЙ") && compact.includes("Продолжаем после сжатия."), "полная переписка осталась в ленте");
  await page.shot(`${shots}/lesson-F-compact.png`);

  // ── I: урок изменился в другом окне ──
  const path = await page.evaluate(`(async () => {
    const root = await (await navigator.storage.getDirectory()).getDirectoryHandle("vault");
    const dir = await root.getDirectoryHandle("Уроки");
    for await (const [name] of dir.entries()) if (name.endsWith(".lesson") && !["Ждёт ответа.lesson","Оборван.lesson","Завершён.lesson","Остановлен.lesson"].includes(name)) return "Уроки/" + name;
  })()`);
  page.log(`файл урока: ${path}`);
  await page.evaluate(`(async (path) => {
    const root = await (await navigator.storage.getDirectory()).getDirectoryHandle("vault");
    const dir = await root.getDirectoryHandle("Уроки");
    const handle = await dir.getFileHandle(path.split("/")[1]);
    const text = await (await handle.getFile()).text();
    const out = await handle.createWritable();
    await out.write(text + JSON.stringify({ kind: "Language", at: Date.now(), language: "en" }) + "\\n");
    await out.close();
  })(${JSON.stringify(path)})`);
  const before = await page.evaluate("window.__modelCalls.length");
  await plan(page, [{ reply: { text: "Не должно уйти." } }]);
  await answer(page, "Готов.");
  await page.waitForElement(".ts-lesson__ro", { timeout: 20000 });
  await page.sleep(500);
  check(page, (await page.text(".ts-lesson__ro")).includes("Урок изменился"), "I: плашка «Урок изменился…»");
  check(page, (await page.evaluate("window.__modelCalls.length")) === before, "I: запрос модели не ушёл");
  check(page, (await page.evaluate(`document.querySelector(".ts-lesson__q textarea")?.value ?? ""`)).includes("Готов."), "I: черновик ответа сохранён");
  await page.shot(`${shots}/lesson-I.png`);

  // ── Завершить урок ──
  await page.click("text=Завершить урок");
  await page.waitFor(`document.querySelector(".ts-lesson__feed").innerText.includes("Урок завершён.")`, { label: "Урок завершён." });
  check(page, (await lessonFeed(page)).includes("урок завершён"), "«Завершить урок»: метка в ленте и «Урок завершён.»");
  check(page, (await page.text(".ts-lesson__tools")).includes("Продолжить урок"), "после завершения предложено «Продолжить урок»");
  await page.shot(`${shots}/lesson-finished.png`);

  // ── G: список с состояниями ──
  await page.evaluate(`${WRITE_FILES}(${JSON.stringify(SEEDED)})`);
  await openPalette(page, "Уроки");
  await page.waitForElement(".ts-lessons__row");
  await page.waitFor(`document.querySelectorAll(".ts-lessons__row").length >= 5`, { label: "пять уроков в списке" });
  const list = await page.text(".ts-lessons");
  for (const word of ["ждёт ответа", "ответ оборван", "завершён", "остановлен"]) {
    check(page, list.includes(word), `G: в списке состояние «${word}»`);
  }
  await page.shot(`${shots}/lesson-G2.png`);
  await page.viewport(390, 800);
  await page.sleep(400);
  const over = await page.evaluate(`document.querySelector(".ts-lessons").scrollWidth - document.querySelector(".ts-lessons").clientWidth`);
  check(page, over <= 0, `G: на 390 px список не выходит за край (избыток ${over})`);
  await page.shot(`${shots}/lesson-G-narrow.png`);
};
