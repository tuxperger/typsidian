// Замер понимания и план урока (фича 052, T014): диалог «Новый урок» с выбором старта (A),
// вопрос замера с «Не знаю» (B), итог замера (C), план картой и списком (D, E),
// отметка шага и перестройка плана с историей (D, F), отказ плана (G), узкий экран,
// «Пропустить замер».
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/ai-lesson-plan.mjs --opfs-vault --timeout 120000
//
// Модель подменена так же, как в `ai-lesson-more.mjs`: очередь `window.__plan`.
// Снимки: PLAN_SHOTS (по умолчанию /tmp/claude-1000, файлы plan-*.png).
const shots = process.env.PLAN_SHOTS ?? "/tmp/claude-1000";

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

const plan = (page, items) =>
  page.evaluate(`window.__plan.push(...${JSON.stringify(items)}.map((i) => i.reply ? window.__reply(i.reply) : i))`);

const openPalette = async (page, query) => {
  await page.click(".ts-cmdfield");
  await page.waitForElement(".ts-palette");
  await page.type(query);
  await page.sleep(200);
  await page.press("Enter");
};

// Настоящий клик по кнопке с этим текстом внутри контейнера.
const press = async (page, scope, label) => {
  const found = await page.evaluate(`(() => {
    document.querySelectorAll("[data-pick]").forEach((el) => el.removeAttribute("data-pick"));
    const el = [...document.querySelectorAll(${JSON.stringify(`${scope} button`)})].find((b) => b.textContent.trim() === ${JSON.stringify(label)});
    if (!el) return false;
    el.setAttribute("data-pick", "1");
    return true;
  })()`);
  check(page, found, `кнопка «${label}» есть`);
  await page.click("[data-pick]");
};

const settled = (page, label) =>
  page.waitFor(`!document.querySelector(".ts-lesson__status")`, { timeout: 40000, label });
const feed = (page) => page.text(".ts-lesson__feed");
const count = (page, selector) => page.evaluate(`document.querySelectorAll(${JSON.stringify(selector)}).length`);

const answer = async (page, text) => {
  await page.waitForElement(".ts-lesson__q textarea", { timeout: 20000 });
  await page.click(".ts-lesson__q textarea");
  await page.type(text);
  await page.press("Enter", { ctrl: true });
};

const STEPS = [
  { id: "speed", title: "Скорость и масса", after: [], known: true },
  { id: "third", title: "Третий закон Ньютона", after: [], known: true },
  { id: "second", title: "Второй закон через импульс", after: ["speed", "third"], known: false },
  { id: "closed", title: "Замкнутая система", after: ["second"], known: false },
  { id: "goal", title: "Цель: сохранение импульса", after: ["closed"], known: false },
];
const STEPS_V2 = [
  STEPS[0],
  STEPS[1],
  { id: "bodies", title: "Силы приложены к разным телам", after: ["third"], known: false },
  { ...STEPS[2], after: ["speed", "bodies"] },
  STEPS[3],
  STEPS[4],
];

const newLesson = async (page, goal) => {
  await openPalette(page, "Новый урок");
  await page.waitForElement(".ts-lesson-dialog textarea");
  await page.type(goal);
};

export default async (page) => {
  await page.addInitScript(FAKE_MODEL);
  await page.reload();

  await page.click("text=Продолжить");
  await page.click("text=Открыть папку…");
  await page.waitForElement("text=Настройки", { timeout: 30000 });

  // ── Настройки: ИИ, ключ, модель ──
  await page.click("text=Настройки");
  await page.sleep(1000);
  await page.waitForElement("text=Обучение с ИИ");
  await page.sleep(1500);
  await page.click("text=Обучение с ИИ");
  await page.sleep(800);
  await page.click("text=Обучение с ИИ");
  await page.sleep(800);
  await page.waitForElement("text=Включить для этого vault", { timeout: 15000 });
  await page.sleep(400);
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
  await page.press("Escape");
  await page.sleep(300);

  // ── A: выбор старта ──
  await newLesson(page, "Почему сохраняется импульс");
  const dialog = await page.text(".ts-lesson-dialog");
  check(page, dialog.includes("С замера понимания") && dialog.includes("Сразу с урока"), "A: выбор «С замера понимания / Сразу с урока»");
  check(page, dialog.includes("ничего не будет объяснять"), "A: подсказка замера");
  check(page, (await page.evaluate(`document.querySelector('.ts-lesson-dialog [aria-pressed=true]').textContent`)) === "С замера понимания", "A: по умолчанию выбран замер");
  await page.shot(`${shots}/plan-A.png`);

  // ── B: вопрос замера ──
  await plan(page, [
    { reply: { text: "", tools: [{ name: "probe_question", input: { part: "второй закон Ньютона", question: "Как связаны сила, действующая на тело, и изменение его импульса?" } }] } },
  ]);
  await page.click("text=Начать урок");
  await page.waitForElement(".ts-lesson__q", { timeout: 30000 });
  await settled(page, "первый вопрос замера");
  let text = await feed(page);
  check(page, text.includes("ЧАСТЬ: второй закон Ньютона"), "B: метка «ЧАСТЬ: …» над вопросом");
  check(page, (await page.text(".ts-lesson__phase")).includes("ЗАМЕР · 1 из ~10"), "B: метка стадии «ЗАМЕР · 1 из ~10»");
  check(page, text.includes("«Не знаю» — тоже ответ") && text.includes("Пропустить замер"), "B: есть «Не знаю» и «Пропустить замер»");
  await page.shot(`${shots}/plan-B.png`);

  // «Не знаю» принимается как ответ; замер идёт дальше.
  await plan(page, [
    { reply: { text: "", tools: [{ name: "probe_question", input: { part: "замкнутая система", question: "Что значит, что система замкнута?", choices: ["Внешние силы не действуют", "Тела не движутся"] } }] } },
  ]);
  await press(page, ".ts-lesson__q", "Не знаю");
  await page.waitFor(`document.querySelector(".ts-lesson__feed")?.innerText.includes("ЧАСТЬ: замкнутая система") && !document.querySelector(".ts-lesson__status")`, { timeout: 40000, label: "второй вопрос замера" });
  text = await feed(page);
  check(page, text.includes("Не знаю"), "B: «Не знаю» записано репликой пользователя");
  check(page, (await page.text(".ts-lesson__phase")).includes("2 из ~10"), "B: счётчик замера вырос");

  // ── C: итог замера ──
  await plan(page, [
    {
      reply: {
        text: "Спасибо, этого достаточно.",
        tools: [
          {
            name: "probe_done",
            input: {
              parts: [
                { name: "Скорость и масса", state: "Known", note: "уверенно" },
                { name: "Третий закон Ньютона", state: "Known", note: "уверенно" },
                { name: "Второй закон через импульс", state: "Partial", note: "формулу помнит" },
                { name: "Замкнутая система", state: "Unknown", note: "ответил «не знаю»" },
              ],
              start: "второго закона через импульс: формулу вы помните, но связь с изменением импульса пока не видна.",
            },
          },
        ],
      },
    },
  ]);
  await page.click(".ts-lesson__opt");
  await page.press("Enter", { ctrl: true });
  await page.waitForElement(".ts-lesson__probe", { timeout: 40000 });
  await settled(page, "ход с итогом замера");
  text = await feed(page);
  check(page, text.toLowerCase().includes("с чего начнём"), "C: заголовок «С чего начнём»");
  for (const word of ["понятно", "частично", "не знакомо"]) check(page, text.includes(word), `C: состояние «${word}»`);
  check(page, text.includes("Начнём с") && text.includes("второго закона через импульс"), "C: стартовая точка");
  check(page, text.includes("Начать обучение"), "C: кнопка «Начать обучение»");
  check(page, !text.includes("Запросить ответ снова") && !text.includes("Ответ не получен"), "C: итог замера не выглядит оборванным ответом");
  check(page, (await count(page, ".ts-lesson__q textarea")) === 0, "C: формы ответа нет");
  check(page, (await count(page, ".ts-lesson__phase")) === 0, "C: метка стадии замера ушла");
  await page.shot(`${shots}/plan-C.png`);

  // ── G: отказ плана с циклом, затем верный план ──
  const cyclic = STEPS.map((s) => (s.id === "speed" ? { ...s, after: ["goal"] } : s));
  await plan(page, [
    { reply: { text: "Составлю план.", tools: [{ name: "plan_set", input: { steps: cyclic, goal: "goal", current: "second" } }] } },
    {
      reply: {
        text: "Исправил план. Начнём.",
        tools: [
          { name: "plan_set", input: { steps: STEPS, goal: "goal", current: "second" } },
          { name: "ask_question", input: { question: "Тележка A толкает тележку B с силой 3 Н. С какой силой B действует на A?", choices: ["3 Н, в противоположную сторону", "0 Н"] } },
        ],
      },
    },
  ]);
  await press(page, ".ts-lesson__probe", "Начать обучение");
  await page.waitForElement(".ts-plan__svg", { timeout: 60000 });
  await settled(page, "ход обучения");
  text = await feed(page);
  check(page, text.includes("не принят") && text.includes("Урок продолжается"), "G: служебная строка об отказе плана");
  check(page, (await count(page, ".ts-lesson__probe .ts-btn")) === 0, "C: у прошлого итога кнопок нет");
  check(page, text.includes("составлен план · 5 шагов"), "D: в ленте отмечено, что план составлен");
  check(page, (await count(page, ".ts-plan__node")) === 5, "D: на карте пять узлов");
  check(page, (await count(page, ".ts-plan__edges path")) === 4, "D: на карте четыре стрелки");
  check(page, (await count(page, ".ts-plan__node--Current")) === 1, "D: текущий шаг выделен");
  check(page, (await count(page, ".ts-plan__node--Known")) === 2, "D: известные по замеру отмечены");
  check(page, (await page.text(".ts-lesson__goal")).includes("шаг 3 из 5"), "D: «шаг 3 из 5» в шапке");
  check(page, (await count(page, ".ts-plan__chg")) === 0, "D: пометки изменения у первой версии нет");
  await page.shot(`${shots}/plan-D1.png`);

  // ── E: список ──
  await press(page, ".ts-plan__head", "Список");
  await page.waitForElement(".ts-plan__list");
  const list = await page.text(".ts-plan__list");
  check(page, (await count(page, ".ts-plan__item")) === 5, "E: в списке пять шагов");
  check(page, list.includes("← 1, 2") && list.includes("Второй закон через импульс"), "E: зависимости «← 1, 2»");
  check(page, (await page.evaluate(`document.querySelector('.ts-plan__item[aria-current=step]')?.innerText ?? ""`)).includes("Второй закон"), "E: текущий шаг помечен aria-current");
  await page.shot(`${shots}/plan-E.png`);

  // ── Отметка шага ──
  await plan(page, [
    { reply: { text: "Верно.", tools: [{ name: "plan_progress", input: { current: "closed", done: ["second"] } }, { name: "ask_question", input: { question: "Что такое замкнутая система?" } }] } },
  ]);
  await page.click(".ts-lesson__opt");
  await page.press("Enter", { ctrl: true });
  await page.waitFor(`document.querySelector(".ts-lesson__goal")?.innerText.includes("шаг 4 из 5") && !document.querySelector(".ts-lesson__status")`, { timeout: 40000, label: "plan_progress" });
  check(page, await page.evaluate(`document.querySelector('.ts-plan__item[data-state=Done]') !== null`), "D: пройденный шаг помечен без перезагрузки");
  check(page, (await count(page, ".ts-plan__item button.ts-plan__row")) >= 1, "D: у пройденного шага есть переход к месту в уроке");

  // ── Перестройка плана ──
  await plan(page, [
    {
      reply: {
        text: "Ответ показал, что вы путаете, к чему приложены силы.",
        tools: [
          { name: "plan_set", input: { steps: STEPS_V2, goal: "goal", current: "bodies", reason: "Ответ показал, что это место путает: добавлен шаг «Силы приложены к разным телам»." } },
          { name: "ask_question", input: { question: "К какому телу приложена сила реакции?" } },
        ],
      },
    },
  ]);
  await answer(page, "Не уверен, к какому.");
  await page.waitFor(`document.querySelector(".ts-plan__chg") && !document.querySelector(".ts-lesson__status")`, { timeout: 40000, label: "вторая версия плана" });
  text = await feed(page);
  check(page, text.includes("план изменён · версия 2"), "D: пометка изменения в ленте");
  check(page, (await page.text(".ts-plan__chg")).includes("добавлен шаг"), "D: причина изменения в колонке плана");
  check(page, (await page.text(".ts-lesson__goal")).includes("план изменён 1 раз"), "D: «план изменён 1 раз» в шапке");
  check(page, (await count(page, ".ts-plan__item")) === 6, "E: в списке шесть шагов");
  check(page, (await page.text(".ts-plan__list")).includes("добавлено"), "E: добавленный шаг помечен");
  await page.shot(`${shots}/plan-D2.png`);

  // Переход к месту урока по пройденному шагу.
  await page.click(".ts-plan__item[data-state=Done] button.ts-plan__row");
  await page.waitFor(`document.querySelector("[data-flash=on]") !== null`, { label: "подсветка места в ленте" });
  check(page, true, "E: нажатие на пройденный шаг прокручивает ленту и подсвечивает реплику");

  // Карта с добавленным шагом.
  await press(page, ".ts-plan__head", "Карта");
  await page.waitForElement(".ts-plan__svg");
  check(page, (await count(page, ".ts-plan__node--added")) === 1, "D: на карте добавленный шаг (жёлтым)");
  await page.shot(`${shots}/plan-D3.png`);

  // ── F: история ──
  await press(page, ".ts-plan__chg", "История");
  await page.waitForElement(".ts-plan__versions");
  check(page, (await count(page, ".ts-plan__ver")) === 2, "F: две версии в истории");
  const history = await page.text(".ts-plan__history");
  check(page, history.includes("Версия 2") && history.includes("Версия 1") && history.includes("5 шагов"), "F: версии с числом шагов");
  await page.shot(`${shots}/plan-F.png`);
  await page.click('[aria-label="Открыть версию плана 1"]');
  await page.waitForElement(".ts-plan__viewing");
  check(page, (await page.text(".ts-plan__viewing")).includes("Версия 1 из 2"), "F: открыта версия 1, помечена «не текущий план»");
  check(page, (await count(page, ".ts-plan__node")) === 5, "F: на карте версии 1 пять узлов");
  await page.shot(`${shots}/plan-F2.png`);
  await press(page, ".ts-plan__viewing", "К текущему плану");
  await page.waitFor(`document.querySelector(".ts-plan__viewing") === null`, { label: "возврат к текущему плану" });
  check(page, (await count(page, ".ts-plan__node")) === 6, "F: вернулись к текущему плану из шести шагов");

  // ── Узкий экран: план — лист со списком ──
  await page.viewport(390, 800);
  await page.waitFor(`document.querySelector(".ts-plan") === null`, { label: "колонка плана сворачивается на узком экране" });
  check(page, true, "узкий экран: колонка плана закрыта");
  await press(page, ".ts-lesson__head", "План ◧");
  await page.waitForElement(".ts-plan--sheet");
  check(page, (await count(page, ".ts-plan--sheet .ts-plan__list")) === 1, "узкий экран: лист со списком");
  check(page, (await count(page, ".ts-plan--sheet .ts-plan__svg")) === 0, "узкий экран: карты в листе нет");
  const over = await page.evaluate(`document.documentElement.scrollWidth - document.documentElement.clientWidth`);
  check(page, over <= 0, `узкий экран: нет горизонтальной прокрутки страницы (избыток ${over})`);
  await page.shot(`${shots}/plan-narrow.png`);
  await press(page, ".ts-plan--sheet", "Закрыть план");
  await page.waitFor(`document.querySelector(".ts-plan") === null`, { label: "лист закрыт" });
  await page.shot(`${shots}/plan-narrow-feed.png`);
  await page.viewport(1280, 800);
  await page.sleep(300);

  // ── «Пропустить замер» ──
  await newLesson(page, "Что такое энтропия");
  await plan(page, [
    { reply: { text: "", tools: [{ name: "probe_question", input: { part: "беспорядок", question: "Что вы знаете о беспорядке в физике?" } }] } },
    { reply: { text: "Тогда начнём с примера.", tools: [{ name: "ask_question", input: { question: "Что растёт при смешивании газов?" } }] } },
  ]);
  await page.click("text=Начать урок");
  await page.waitFor(`document.querySelector(".ts-lesson__feed")?.innerText.includes("ЧАСТЬ: беспорядок") && !document.querySelector(".ts-lesson__status")`, { timeout: 40000, label: "вопрос замера второго урока" });
  await press(page, ".ts-lesson__feed", "Пропустить замер");
  await page.waitFor(`document.querySelector(".ts-lesson__feed")?.innerText.includes("Что растёт при смешивании газов?") && !document.querySelector(".ts-lesson__status")`, { timeout: 40000, label: "учитель продолжил после пропуска" });
  text = await feed(page);
  check(page, text.includes("замер пропущен"), "пропуск: отметка «замер пропущен» в ленте");
  check(page, (await count(page, ".ts-lesson__phase")) === 0, "пропуск: метки стадии нет");
  check(page, (await count(page, ".ts-lesson__feed button")) >= 0 && !text.includes("Пропустить замер") , "пропуск: кнопки замера ушли");
  await page.shot(`${shots}/plan-skip.png`);
};
