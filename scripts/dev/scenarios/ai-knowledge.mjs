// Граф знаний из уроков (фича 054, T016): настройки (D), пустой граф (F), автопополнение
// после хода учителя с длинным объяснением и полоса задания (A), строка итога в уроке (C),
// карта с узлами, развилкой, группой, поиском, колонкой утверждения и переходом к событию
// урока (A), таблица цепочек (B), удаление утверждения → цепочка неполная (E), выбор
// утверждений в новой практике и текст в запросе, ручное пополнение при выключенном
// автопополнении и отмена задания (C), чтение графа в ленте урока, удаление урока с
// «оставить вклад» (E), узкий экран и клавиатурный фокус.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/ai-knowledge.mjs --opfs-vault --timeout 240000
//
// Сеть к модели подменена очередью `window.__plan`, как в ai-practice.mjs; у элемента очереди
// можно задать `delay` — паузу между кусками потока (чтобы полоса задания успела показаться).
// Снимки: KNOWLEDGE_SHOTS (по умолчанию /tmp/claude-1000), knowledge-*.png.
const shots = process.env.KNOWLEDGE_SHOTS ?? "/tmp/claude-1000";

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
const NOW = Date.now();
const USER_SECRET = "СЕКРЕТНЫЙ-ОТВЕТ-ПОЛЬЗОВАТЕЛЯ-31415";

// Объяснение длиннее порога автозапуска (2 тыс. символов).
const explain = (tag) =>
  Array.from({ length: 30 }, (_, i) => `${tag}: пункт ${i + 1}. Объяснение про импульс и силы взаимодействия двух тел в замкнутой системе.`).join(" ");

const P_EQUAL = "Силы взаимодействия двух тел равны по модулю и противоположны по направлению";
const P_FORCE = "Сила равна скорости изменения импульса";
const P_CONST = "Импульс замкнутой системы постоянен";
const P_OPEN = "Внешних сил нет — система замкнута";
const P_SPEED = "Скорость тележек обратно пропорциональна их массам";
const P_FOURIER = "Периодический сигнал раскладывается в сумму синусоид";
const P_COEFF = "Коэффициент ряда Фурье — проекция сигнала на синусоиду";

const SEEDED = {
  "Физика/Импульс.typ": "= Импульс\nИмпульс равен массе на скорость.\n",
  "Уроки/Ряды Фурье на пальцах.lesson":
    line({ format: 1, goal: "Ряды Фурье на пальцах", language: "ru", created_at: NOW - 9000, start: "Teach" }) +
    line({ kind: "Teacher", at: NOW - 8000, text: "ФУРЬЕ-ОБЪЯСНЕНИЕ. Любой периодический сигнал можно собрать из синусоид разной частоты.", interrupted: false, model: "claude-sonnet-5-5" }) +
    line({ kind: "Teacher", at: NOW - 7000, text: "Коэффициент при синусоиде показывает, сколько этой синусоиды в сигнале.", interrupted: false, model: "claude-sonnet-5-5" }),
  // Чтение графа агентом и выбранные утверждения: строки ленты (T013).
  "Уроки/Опора и чтение графа.lesson":
    line({ format: 1, goal: "Опора и чтение графа", language: "ru", created_at: NOW - 5000, start: "Teach" }) +
    line({ kind: "Material", at: NOW - 4900, lessons: [], knowledge: { statements: [{ id: "p_x1", text: "Первое утверждение" }, { id: "p_x2", text: "Второе утверждение" }], chains: [] } }) +
    line({ kind: "KnowledgeRead", at: NOW - 4800, query: "импульс", outcome: { kind: "Sent", count: 2 } }) +
    line({ kind: "KnowledgeRead", at: NOW - 4700, query: "секрет", outcome: { kind: "Denied" } }) +
    line({ kind: "Teacher", at: NOW - 4600, text: "Вот что нашлось в графе.", interrupted: false, model: "claude-sonnet-5-5" }),
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

const body = (page) => page.evaluate("document.body.innerText");
const settled = (page, label) =>
  page.waitFor(`!document.querySelector(".ts-lesson__status")`, { timeout: 40000, label });
const findings = (propositions, chains) => ({
  reply: { tools: [{ name: "submit_findings", input: { propositions, chains } }] },
  delay: 500,
});
const extractBodies = (page) =>
  page.evaluate(`window.__modelCalls.map((c) => c.body).filter((b) => b.includes("submit_findings") && b.includes("tool_choice"))`);

/** Утверждение карты по подписи — через список для чтения с экрана (щелчок по холсту координатами не нужен). */
const pickOnMap = (page, name) =>
  page.evaluate(`(() => {
    const b = [...document.querySelectorAll(".ts-graph__sr button")].find((x) => x.textContent.trim().startsWith(${JSON.stringify(name)}));
    if (!b) throw new Error("нет узла " + ${JSON.stringify(name)});
    b.click();
  })()`);

// Раздел «Обучение с ИИ» в настройках: первый щелчок по пункту иногда приходит до отрисовки панели.
const openAiSettings = async (page) => {
  await page.click("text=Настройки");
  await page.waitForElement("text=Обучение с ИИ");
  await page.sleep(400);
  await page.click("text=Обучение с ИИ");
  await page.waitFor(`document.querySelector(".ts-settings")?.innerText.includes("Пополнять граф") || document.querySelector(".ts-settings")?.innerText.includes("Включить для этого vault")`, { label: "раздел «Обучение с ИИ»" });
};

const openKnowledge = async (page) => {
  await openPalette(page, "Граф знаний");
  await page.waitForElement(".ts-kg");
  await page.waitFor(`!document.querySelector(".ts-kg__empty")?.textContent.includes("Загружаем")`, { label: "граф загружен" });
};
const leaveKnowledge = async (page) => {
  await openPalette(page, "Вернуться в Workspace");
  await page.waitFor(`!document.querySelector(".ts-kg")`, { label: "вид графа знаний закрыт" });
};

export default async (page) => {
  await page.addInitScript(FAKE_MODEL);
  await page.reload();
  await page.evaluate(`${WRITE_FILES}(${JSON.stringify(SEEDED)})`);

  await page.click("text=Продолжить");
  await page.click("text=Открыть папку…");
  await page.waitForElement("text=Настройки", { timeout: 30000 });

  // ── Включаем ИИ ──
  await openAiSettings(page);
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

  // ── D: настройки графа ──
  const sw = (label) =>
    page.evaluate(`(() => {
      const row = [...document.querySelectorAll(".ts-srow")].find((r) => r.textContent.includes(${JSON.stringify(label)}));
      const input = row?.querySelector("input[type=checkbox]");
      return input ? { present: true, on: input.checked, hint: row.querySelector(".ts-srow__hint")?.textContent ?? "" } : { present: false };
    })()`);
  const auto = await sw("Пополнять граф после шагов урока");
  const reads = await sw("Агент может сам читать граф");
  const mapSw = await sw("Показывать картой");
  check(page, auto.present && auto.on && auto.hint.includes("не ваши ответы"), "D: «Пополнять граф после шагов урока» включено, подсказка про ответы");
  check(page, reads.present && !reads.on && reads.hint.includes("только по вашему выбору"), "D: «Агент может сам читать граф» выключено");
  check(page, mapSw.present && mapSw.on && mapSw.hint.includes("только таблица цепочек"), "D: «Показывать картой» включено");
  await page.evaluate(`[...document.querySelectorAll(".ts-srow")].find((r) => r.textContent.includes("Показывать картой")).scrollIntoView({ block: "center" })`);
  await page.shot(`${shots}/knowledge-D.png`);
  // Выключить «Показывать картой»: в виде остаётся только таблица.
  await page.click("text=Показывать картой");
  await page.waitFor(`(async () => true)() && ![...document.querySelectorAll(".ts-srow")].find((r) => r.textContent.includes("Показывать картой")).querySelector("input").checked`, { label: "переключатель карты выключен" });
  await page.press("Escape");
  await page.sleep(300);

  // ── F: граф пуст; только таблица (карта выключена) ──
  await openKnowledge(page);
  let view = await page.text(".ts-kg");
  check(page, view.includes("Граф пополняется из уроков"), "F: пустой граф — пояснение «Граф пополняется из уроков»");
  check(page, !(await page.evaluate(`Boolean(document.querySelector(".ts-kg .ts-graph__seg"))`)), "карта выключена: переключателя «Карта / Цепочки» нет");
  check(page, !(await page.evaluate(`Boolean(document.querySelector(".ts-kg__count, .ts-kg__side"))`)), "F: у пустого графа нет счётчиков и колонки утверждения");
  await page.shot(`${shots}/knowledge-F.png`);
  await leaveKnowledge(page);
  // Карту обратно.
  await openAiSettings(page);
  await page.click("text=Показывать картой");
  await page.waitFor(`[...document.querySelectorAll(".ts-srow")].find((r) => r.textContent.includes("Показывать картой")).querySelector("input").checked`, { label: "карта включена" });
  await page.press("Escape");
  await page.sleep(300);

  // ── Первый ход учителя с длинным объяснением → автозапуск пополнения ──
  await openPalette(page, "Новый урок");
  await page.waitForElement(".ts-lesson-dialog textarea");
  await page.type("Понять, почему сохраняется импульс");
  await page.click("text=Сразу с урока");
  const TEACH_1 = explain("ПЕРВОЕ-ОБЪЯСНЕНИЕ");
  await plan(page, [
    { reply: { text: TEACH_1, tools: [{ name: "ask_question", input: { question: "Согласны?" } }] } },
    findings(
      [
        { key: "a", text: P_EQUAL },
        { key: "b", text: P_FORCE },
        { key: "c", text: P_CONST },
      ],
      [{ premises: ["a", "b"], conclusion: "c" }],
    ),
  ]);
  await page.click("text=Начать урок");
  await page.waitForElement(".ts-lesson__q", { timeout: 40000 });
  await settled(page, "первый ход закончен");
  // Полоса задания: идёт в фоне, урок не ждёт; «Отменить» на месте.
  await page.waitFor(`Boolean(document.querySelector(".ts-kg__job"))`, { timeout: 20000, label: "полоса задания в уроке" });
  const strip = await page.text(".ts-kg__job");
  check(page, strip.includes("Пополняем граф из «Понять, почему сохраняется импульс»") || strip.includes("Пополняем граф из"), "A: полоса «Пополняем граф из …»");
  check(page, strip.includes("Отменить"), "A: в полосе есть «Отменить»");
  check(page, await page.evaluate(`Boolean(document.querySelector(".ts-lesson__q"))`), "урок не ждёт: форма ответа доступна, пока идёт пополнение");
  await page.shot(`${shots}/knowledge-job.png`);
  await page.waitFor(`!document.querySelector(".ts-kg__job") && document.querySelector(".ts-kg__lesson")?.innerText.includes("В граф добавлено")`, { timeout: 30000, label: "пополнение закончилось, итог в уроке" });
  let lessonText = await page.text(".ts-lesson__feed");
  check(page, lessonText.includes("В граф добавлено 3 утверждения и 1 цепочка"), "C: строка итога «В граф добавлено 3 утверждения и 1 цепочка»");
  check(page, await page.evaluate(`[...document.querySelectorAll(".ts-kg__lesson button")].some((b) => b.textContent.trim() === "открыть")`), "C: у итога есть «открыть»");
  check(page, !(await page.evaluate(`[...document.querySelectorAll(".ts-kg__lesson button")].some((b) => b.textContent.trim() === "Пополнить граф")`)), "C: всё покрыто — «Пополнить граф» не нужна");
  await page.shot(`${shots}/knowledge-C-summary.png`);
  const extracts1 = await extractBodies(page);
  check(page, extracts1.length === 1, `ровно один запрос пополнения после хода (получено ${extracts1.length})`);
  check(page, extracts1[0].includes("ПЕРВОЕ-ОБЪЯСНЕНИЕ"), "в запросе пополнения — объяснение учителя");
  const callsAfterFirst = await page.evaluate("window.__modelCalls.length");
  await page.sleep(1500);
  check(page, (await page.evaluate("window.__modelCalls.length")) === callsAfterFirst, "повторных запросов пополнения без нового хода нет");

  // ── Второй ход: ответ пользователя не уходит на пополнение (SC-004), повтор → слияние ──
  const TEACH_2 = explain("ВТОРОЕ-ОБЪЯСНЕНИЕ");
  await plan(page, [
    { reply: { text: TEACH_2, tools: [{ name: "ask_question", input: { question: "Теперь ясно?" } }] } },
    findings(
      [
        { key: "a", text: P_CONST },
        { key: "b", text: P_OPEN },
        { key: "c", text: P_SPEED },
      ],
      [{ premises: ["b"], conclusion: "a" }],
    ),
  ]);
  await page.click(".ts-lesson__q textarea");
  await page.type(USER_SECRET);
  await page.press("Enter", { ctrl: true });
  await page.waitFor(`document.querySelector(".ts-lesson__feed")?.innerText.includes("ВТОРОЕ-ОБЪЯСНЕНИЕ")`, { timeout: 40000, label: "второй ход" });
  await settled(page, "второй ход закончен");
  await page.waitFor(`document.querySelector(".ts-kg__lesson")?.innerText.includes("В граф добавлено 2 утверждения и 1 цепочка")`, { timeout: 40000, label: "второе пополнение" });
  lessonText = await page.text(".ts-kg__lesson");
  check(page, lessonText.includes("1 утверждение уже было в графе и объединено"), "повтор утверждения объединён, не продублирован");
  const extracts2 = await extractBodies(page);
  check(page, extracts2.length === 2, `второй ход — второй запрос пополнения (получено ${extracts2.length})`);
  check(page, extracts2[1].includes("ВТОРОЕ-ОБЪЯСНЕНИЕ") && !extracts2[1].includes(USER_SECRET), "SC-004: ответ пользователя в запрос пополнения не попал");
  check(page, !extracts2[1].includes("ПЕРВОЕ-ОБЪЯСНЕНИЕ"), "покрытое объяснение повторно не уходит");

  // ── Список уроков: кнопка «Граф знаний» и «Удалить…» у урока ──
  await openPalette(page, "Уроки");
  await page.waitForElement(".ts-lessons__row");
  const list = await page.text(".ts-lessons");
  check(page, list.includes("Граф знаний") && list.includes("Удалить…"), "список уроков: «Граф знаний» и «Удалить…»");
  await page.shot(`${shots}/knowledge-lessons.png`);
  // Кнопка «Граф знаний» в списке открывает вид на месте текста.
  await press(page, ".ts-lessons", "Граф знаний");
  await page.waitForElement(".ts-kg");
  await page.waitFor(`!document.querySelector(".ts-lessons") && !document.querySelector(".ts-kg__empty")?.textContent.includes("Загружаем")`, { label: "вид открыт из списка уроков" });

  // ── A: карта ──
  await page.waitForElement(".ts-graph__surface");
  await page.waitFor(`document.querySelectorAll(".ts-graph__sr button").length >= 5`, { label: "узлы карты" });
  view = await page.text(".ts-kg__bar");
  check(page, view.includes("5 утверждений") && view.includes("2 цепочки") && view.includes("1 источник"), `A: счётчики «5 утверждений · 2 цепочки · 1 источник» (${view.replace(/\n/g, " ")})`);
  const nodes = await page.evaluate(`[...document.querySelectorAll(".ts-graph__sr button")].map((b) => b.textContent.trim())`);
  check(page, nodes.length === 6, `A: на карте 5 утверждений и 1 развилка (узлов ${nodes.length})`);
  check(page, nodes.some((n) => n.includes(P_CONST)), "A: узел «Импульс замкнутой системы постоянен»");
  const legend = await page.text(".ts-kg__legend");
  check(page, legend.includes("Понять, почему сохраняется импульс") && legend.includes("развилка цепочки"), "A: легенда — группа по уроку и развилка");
  check(page, await page.evaluate(`document.querySelector(".ts-kg__split").getBoundingClientRect().height > 200 && document.querySelector(".ts-graph__surface").getBoundingClientRect().height > 200`), "A: холст занимает область");
  await page.shot(`${shots}/knowledge-A-map.png`);

  // Поиск: подстрока + соседи.
  await page.click(".ts-kg__search");
  await page.type("скорост");
  await page.waitFor(`document.querySelectorAll(".ts-graph__sr button").length > 0 && document.querySelectorAll(".ts-graph__sr button").length < 6`, { label: "поиск сузил карту" });
  const found = await page.evaluate(`[...document.querySelectorAll(".ts-graph__sr button")].map((b) => b.textContent.trim())`);
  check(page, found.some((n) => n.includes(P_FORCE)) && found.some((n) => n.includes(P_SPEED)), `A: поиск «скорост» находит утверждения по подстроке (${found.length} узлов)`);
  await page.shot(`${shots}/knowledge-A-search.png`);
  // Очистка поля: значение React-поля ставится родным установщиком и событием input.
  await page.evaluate(`(() => {
    const input = document.querySelector(".ts-kg__search");
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(input, "");
    input.dispatchEvent(new Event("input", { bubbles: true }));
  })()`);
  await page.waitFor(`document.querySelectorAll(".ts-graph__sr button").length === 6`, { label: "поиск сброшен" });

  // Колонка утверждения.
  await pickOnMap(page, P_CONST);
  await page.waitFor(`document.querySelector(".ts-kg__side")?.innerText.toLowerCase().includes("выводится из")`, { label: "колонка утверждения" });
  let side = await page.text(".ts-kg__side");
  check(page, side.includes(P_CONST) && side.toLowerCase().includes("выводится из"), "A: колонка — формулировка и «Выводится из»");
  check(page, side.includes(P_EQUAL) && side.includes(P_FORCE) && side.includes("это утверждение"), "A: цепочка «посылка + посылка → это утверждение»");
  check(page, side.includes(P_OPEN), "A: вторая цепочка из второго объяснения");
  check(page, side.includes("Понять, почему сохраняется импульс") && /реплика \d+/.test(side), "A: источник — урок со ссылкой и местом");
  check(page, side.includes("Удалить утверждение…"), "A: в колонке «Удалить утверждение…»");
  await page.shot(`${shots}/knowledge-A-panel.png`);

  // Переход к источнику: урок открывается и событие подсвечивается.
  const place = side.match(/реплика (\d+)/);
  await page.click(".ts-kg__side .ts-kg__link");
  await page.waitForElement(".ts-lesson__feed", { timeout: 20000 });
  await page.waitFor(`document.querySelector('[data-flash="on"]') !== null`, { timeout: 8000, label: "подсветка события" });
  const flashed = await page.evaluate(`document.querySelector('[data-flash="on"]').closest("[data-event]")?.getAttribute("data-event")`);
  check(page, place !== null && Number(flashed) === Number(place[1]) - 1, `A: переход открыл урок на событии (реплика ${place?.[1]} → data-event ${flashed})`);
  // Плавная прокрутка идёт дольше кадра: ждём, пока событие окажется в видимой части ленты.
  await page.waitFor(`(() => {
    const feed = document.querySelector(".ts-lesson__feed").getBoundingClientRect();
    const holder = document.querySelector('[data-event="${flashed}"]');
    const target = (holder.firstElementChild ?? holder).getBoundingClientRect();
    return target.top >= feed.top - 2 && target.bottom <= feed.bottom + 2;
  })()`, { timeout: 5000, label: "событие урока прокручено в видимую область ленты" });
  check(page, true, "A: событие урока прокручено в видимую область ленты");
  await page.shot(`${shots}/knowledge-A-jump.png`);

  // ── B: таблица цепочек ──
  await openKnowledge(page);
  await page.click("text=Цепочки");
  await page.waitForElement(".ts-kg__table");
  const table = await page.text(".ts-kg__table");
  check(page, table.includes("Посылки") && table.includes("Вывод") && table.includes("Источник"), "B: заголовки «Посылки / Вывод / Источник»");
  check(page, table.includes(P_EQUAL) && table.includes(P_FORCE) && table.includes(P_OPEN), "B: посылки цепочек");
  check(page, (await page.evaluate(`document.querySelectorAll(".ts-kg__table tbody tr").length`)) === 2, "B: две цепочки в таблице");
  await page.shot(`${shots}/knowledge-B-chains.png`);

  // ── E: удаление утверждения → цепочка неполная ──
  await page.evaluate(`[...document.querySelectorAll(".ts-kg__table .ts-kg__stmt")].find((b) => b.textContent.trim() === ${JSON.stringify(P_FORCE)}).click()`);
  await page.waitFor(`document.querySelector(".ts-kg__side")?.innerText.includes("Удалить утверждение…")`, { label: "колонка посылки" });
  await page.click("text=Удалить утверждение…");
  await page.waitForElement("[role=alertdialog]");
  const warn = await page.text(".ts-kg__confirm");
  check(page, warn.includes(`Удалить утверждение «${P_FORCE}»?`) && warn.includes("1 цепочка, где оно посылка, станет неполной"), "E: предупреждение «цепочка станет неполной»");
  await page.shot(`${shots}/knowledge-E-remove.png`);
  await press(page, ".ts-kg__confirm", "Удалить");
  await page.waitFor(`document.querySelector(".ts-kg__badge") !== null`, { label: "цепочка помечена неполной" });
  const afterRemove = await page.text(".ts-kg__table");
  check(page, afterRemove.includes("неполная") && !afterRemove.includes(P_FORCE), "E: цепочка неполная, удалённого утверждения в ней нет");
  await page.shot(`${shots}/knowledge-E-incomplete.png`);
  await leaveKnowledge(page);

  // ── T013: выбор утверждений в новой практике ──
  await openPalette(page, "Новая практика");
  await page.waitForElement(".ts-lesson-dialog .ts-kg__pick");
  // Кнопка «Добавить…» появляется, когда граф прочитан и в нём есть из чего выбирать.
  await page.waitForElement(".ts-kg__pick .ts-btn");
  const pickBox = await page.text(".ts-kg__pick");
  check(page, pickBox.includes("Опора из графа знаний") && pickBox.includes("Добавить…"), "T013: в диалоге практики блок «Опора из графа знаний»");
  await page.click(".ts-kg__pick .ts-btn");
  await page.waitForElement(".ts-kg__pick .ts-lesson__pick input");
  await page.type("Импульс");
  await page.waitFor(`document.querySelectorAll(".ts-kg__pick .ts-lesson__pickrow").length > 0`, { label: "результаты поиска по графу" });
  const rows = await page.evaluate(`[...document.querySelectorAll(".ts-kg__pick .ts-lesson__pickrow")].map((b) => b.textContent.trim())`);
  check(page, rows.includes(P_CONST), "T013: поиск по графу находит утверждение");
  await page.evaluate(`[...document.querySelectorAll(".ts-kg__pick .ts-lesson__pickrow")].find((b) => b.textContent.trim() === ${JSON.stringify(P_CONST)}).click()`);
  await page.waitFor(`document.querySelector(".ts-kg__pick .ts-lesson__chip")?.textContent.includes("Импульс замкнутой")`, { label: "чип выбранного" });
  check(page, (await page.text(".ts-kg__pick")).includes("уйдёт модели"), "T013: подсказка, что текст уйдёт модели");
  await page.shot(`${shots}/knowledge-pick-practice.png`);
  await plan(page, [{ reply: { tools: [{ name: "pose_problem", input: { statement: "Тележки разъезжаются после толчка. Что с суммарным импульсом?", solution: "Он остаётся нулевым." } }] } }]);
  const callsBeforePractice = await page.evaluate("window.__modelCalls.length");
  await press(page, ".ts-lesson-dialog", "Начать практику");
  await page.waitForElement(".ts-practice__form", { timeout: 40000 });
  await settled(page, "первая задача практики");
  const practiceFeed = await page.text(".ts-lesson__feed");
  check(page, practiceFeed.includes("Опора на граф знаний: 1 утверждение"), "T013: в ленте практики «Опора на граф знаний: 1 утверждение»");
  const practiceCall = await page.evaluate(`window.__modelCalls.slice(${callsBeforePractice}).map((c) => c.body).join("\\n")`);
  check(page, practiceCall.includes(P_CONST), "T013: текст выбранного утверждения ушёл в запрос практики");
  check(page, !practiceCall.includes(USER_SECRET), "в запрос практики не попали ответы чужого урока");
  await page.shot(`${shots}/knowledge-practice-feed.png`);

  // ── Чтение графа и выбранные утверждения в ленте урока ──
  await openPalette(page, "Уроки");
  await page.waitForElement(".ts-lessons__row");
  await page.waitFor(`[...document.querySelectorAll(".ts-lessons__row")].some((r) => r.textContent.includes("Опора и чтение графа"))`, { label: "урок с чтением графа" });
  await page.evaluate(`[...document.querySelectorAll(".ts-lessons__row")].find((r) => r.textContent.includes("Опора и чтение графа")).click()`);
  await page.waitFor(`document.querySelector(".ts-lesson__feed")?.innerText.includes("Вот что нашлось")`, { label: "лента урока с чтением графа" });
  const readFeed = await page.text(".ts-lesson__feed");
  check(page, readFeed.includes("Опора на граф знаний: 2 утверждения"), "лента: строка «Опора на граф знаний: 2 утверждения» у урока");
  check(page, readFeed.includes("Агент прочитал граф знаний по запросу «импульс» — отправлено 2 утверждения"), "лента: чтение графа агентом — отправлено 2 утверждения");
  check(page, readFeed.includes("запросил граф знаний по запросу «секрет» — папка закрыта для агента"), "лента: отказ чтения графа");
  await page.shot(`${shots}/knowledge-read-feed.png`);

  // ── C: ручное пополнение при выключенном автопополнении; отмена ──
  await openAiSettings(page);
  await page.click("text=Пополнять граф после шагов урока");
  await page.waitFor(`![...document.querySelectorAll(".ts-srow")].find((r) => r.textContent.includes("Пополнять граф после шагов урока")).querySelector("input").checked`, { label: "автопополнение выключено" });
  await page.press("Escape");
  await page.sleep(300);
  await openPalette(page, "Уроки");
  await page.waitForElement(".ts-lessons__row");
  await page.waitFor(`[...document.querySelectorAll(".ts-lessons__row")].some((r) => r.textContent.includes("Ряды Фурье"))`, { label: "урок про Фурье" });
  await page.evaluate(`[...document.querySelectorAll(".ts-lessons__row")].find((r) => r.textContent.includes("Ряды Фурье")).click()`);
  await page.waitFor(`document.querySelector(".ts-lesson__feed")?.innerText.includes("ФУРЬЕ-ОБЪЯСНЕНИЕ")`, { label: "лента урока про Фурье" });
  await page.waitForElement("text=Пополнить граф");
  const fillRow = await page.text(".ts-kg__lesson");
  check(page, fillRow.includes("Пополнить граф") && fillRow.includes("Ещё не в графе"), "C: при выключенном автопополнении есть «Пополнить граф» и подсказка");
  await page.shot(`${shots}/knowledge-C-button.png`);
  const callsBeforeCancel = await page.evaluate("window.__modelCalls.length");
  await plan(page, [
    findings([{ key: "a", text: P_FOURIER }], []),
  ].map((item) => ({ ...item, delay: 900 })));
  await page.click("text=Пополнить граф");
  await page.waitFor(`Boolean(document.querySelector(".ts-kg__job"))`, { timeout: 15000, label: "полоса задания после нажатия" });
  await page.click(".ts-kg__job .ts-btn");
  await page.waitFor(`!document.querySelector(".ts-kg__job")`, { timeout: 20000, label: "задание отменено" });
  await page.sleep(1200);
  check(page, (await page.evaluate("window.__modelCalls.length")) === callsBeforeCancel + 1, "C: нажатие «Пополнить граф» — один запрос");
  lessonText = await page.text(".ts-lesson__feed");
  check(page, !lessonText.includes("В граф добавлено 1 утверждение"), "C: после отмены в граф ничего не добавлено");
  // Повтор: теперь довести до конца.
  await plan(page, [
    findings(
      [
        { key: "a", text: P_FOURIER },
        { key: "b", text: P_COEFF },
      ],
      [{ premises: ["a"], conclusion: "b" }],
    ),
  ]);
  await page.waitFor(`[...document.querySelectorAll(".ts-kg__lesson button")].some((b) => b.textContent.trim() === "Пополнить граф" && !b.disabled)`, { label: "кнопка снова доступна" });
  await page.click("text=Пополнить граф");
  await page.waitFor(`document.querySelector(".ts-kg__lesson")?.innerText.includes("В граф добавлено 2 утверждения и 1 цепочка")`, { timeout: 30000, label: "ручное пополнение закончилось" });
  check(page, true, "C: ручное пополнение добавило утверждения и цепочку");
  await page.shot(`${shots}/knowledge-C-manual.png`);

  // ── E: удаление урока с «оставить вклад» ──
  await openPalette(page, "Уроки");
  await page.waitForElement(".ts-lessons__row");
  await page.evaluate(`[...document.querySelectorAll(".ts-lessons__item")].find((i) => i.textContent.includes("Ряды Фурье")).querySelector(".ts-lessons__del").click()`);
  await page.waitForElement(".ts-lessons__delbox");
  const del = await page.text("[role=alertdialog]");
  check(page, del.includes("Удалить урок «Ряды Фурье на пальцах»?") && del.includes("Оставить его вклад в графе") && del.includes("Удалить и вклад: 2 утверждения, 1 цепочка"), "E: диалог — «оставить вклад / удалить и вклад: 2 утверждения, 1 цепочка»");
  check(page, await page.evaluate(`document.querySelector('[role=alertdialog] input[type=radio]').checked`), "E: по умолчанию вклад остаётся");
  await page.shot(`${shots}/knowledge-E-delete-lesson.png`);
  await press(page, "[role=alertdialog]", "Удалить урок");
  await page.waitFor(`![...document.querySelectorAll(".ts-lessons__row")].some((r) => r.textContent.includes("Ряды Фурье"))`, { timeout: 15000, label: "урок исчез из списка" });
  await page.press("Escape");
  await page.sleep(300);
  // Удаление идёт через ядро: дерево и вкладка удалённого урока обновляются сразу.
  await page.waitFor(`!document.body.innerText.includes("Ряды Фурье")`, { timeout: 10000, label: "удалённый урок ушёл из дерева и вкладок" });
  check(page, true, "E: удалённый урок ушёл из дерева, его вкладка закрыта");
  await openKnowledge(page);
  await page.waitFor(`document.querySelectorAll(".ts-graph__sr button").length > 0`, { label: "карта" });
  await pickOnMap(page, P_COEFF);
  await page.waitFor(`document.querySelector(".ts-kg__side")?.innerText.toLowerCase().includes("источники")`, { label: "колонка Фурье" });
  side = await page.text(".ts-kg__side");
  check(page, side.includes("Ряды Фурье на пальцах") && side.includes("источник удалён"), "E: источник удалённого урока помечен «источник удалён»");
  check(page, await page.evaluate(`Boolean(document.querySelector(".ts-kg__side .ts-kg__gone")) && !document.querySelector(".ts-kg__side .ts-kg__link")`), "E: у удалённого источника зачёркнутое имя и нет ссылки");
  await page.shot(`${shots}/knowledge-E-source-removed.png`);

  // ── Узкий экран, клавиатурный фокус ──
  await page.viewport(390, 800);
  await page.sleep(500);
  await openKnowledge(page).catch(() => undefined);
  await page.waitFor(`Boolean(document.querySelector(".ts-kg"))`, { label: "вид графа знаний на телефоне" });
  await page.sleep(400);
  const over = await page.evaluate(`Math.max(document.documentElement.scrollWidth - document.documentElement.clientWidth, document.querySelector(".ts-kg").scrollWidth - document.querySelector(".ts-kg").clientWidth, 0)`);
  check(page, over <= 0, `узко: вид не выходит за край (избыток ${over})`);
  await page.shot(`${shots}/knowledge-narrow.png`);
  const phoneMode = await page.evaluate(`document.querySelector(".ts-kg .ts-graph__seg button[aria-pressed=true]")?.textContent.trim()`);
  check(page, phoneMode === "Цепочки", `узко: по умолчанию таблица цепочек (режим «${phoneMode}»)`);
  check(page, await page.evaluate(`Boolean(document.querySelector(".ts-kg__table"))`), "узко: таблица цепочек показана");
  // Фокус с клавиатуры: Tab до формулировки с видимой обводкой.
  await page.evaluate(`document.querySelector(".ts-kg__table .ts-kg__stmt").focus()`);
  await page.press("Tab");
  await page.press("Tab", { shift: true });
  const focus = await page.evaluate(`(() => { const el = document.activeElement; const s = getComputedStyle(el); return { stmt: el.classList.contains("ts-kg__stmt"), outline: s.outlineStyle, width: s.outlineWidth, visible: el.matches(":focus-visible") }; })()`);
  page.log(`фокус на формулировке: ${JSON.stringify(focus)}`);
  check(page, focus.stmt && focus.outline !== "none" && focus.visible, "узко: клавиатурный фокус на формулировке виден (outline)");
  await page.click(".ts-kg__table .ts-kg__stmt");
  await page.waitFor(`document.querySelector(".ts-kg__side")?.innerText.toLowerCase().includes("источники")`, { label: "колонка под таблицей" });
  await page.shot(`${shots}/knowledge-narrow-panel.png`);
  const overPanel = await page.evaluate(`Math.max(document.documentElement.scrollWidth - document.documentElement.clientWidth, 0)`);
  check(page, overPanel <= 0, `узко: колонка утверждения не выходит за край (избыток ${overPanel})`);
  // Планшет: карта и колонка друг под другом.
  await page.viewport(760, 800);
  await page.sleep(500);
  await page.click("text=Карта");
  await page.waitForElement(".ts-graph__surface");
  await page.shot(`${shots}/knowledge-tablet-map.png`);
  const overTablet = await page.evaluate(`Math.max(document.documentElement.scrollWidth - document.documentElement.clientWidth, 0)`);
  check(page, overTablet <= 0, `планшет: вид не выходит за край (избыток ${overTablet})`);
  await page.viewport(1280, 800);
};
