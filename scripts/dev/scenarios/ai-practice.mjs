// Практика с ИИ (фича 053, T010): диалог «Новая практика» (A), задача без решения в DOM
// и подсказка (B), ответ, итог, разбор, решение и «Следующая задача» (C), «Сдаюсь» (C),
// задача без зафиксированного решения (D), сводка с переходом к разбору (E), закрытая
// папка, список с меткой «ПРАКТИКА», узкая ширина.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/ai-practice.mjs --opfs-vault --timeout 120000
//
// Сеть к модели подменена очередью `window.__plan`, как в ai-lesson-more.mjs.
// Снимки: PRACTICE_SHOTS (по умолчанию /tmp/claude-1000), practice-*.png.
const shots = process.env.PRACTICE_SHOTS ?? "/tmp/claude-1000";

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
const NOW = Date.now();
const SECRET_1 = "СЕКРЕТНОЕ-РЕШЕНИЕ-ПЕРВОЙ-42";
const SECRET_2 = "СЕКРЕТНОЕ-РЕШЕНИЕ-ВТОРОЙ-77";

const SEEDED = {
  "Уроки/Понять импульс.lesson":
    line({ format: 1, goal: "Понять импульс", language: "ru", created_at: NOW - 5000 }) +
    line({ kind: "Teacher", at: NOW - 4000, text: "Импульс — масса на скорость; в замкнутой системе он сохраняется.", interrupted: false, model: "claude-sonnet-5-5" }) +
    line({ kind: "Summary", at: NOW - 3000, text: "Закон сохранения импульса: суммарный импульс замкнутой системы не меняется.", covers: 1 }),
};

// Урок в закрытой папке: в диалоге помечен и не выбирается (ревью 053, T021).
SEEDED["Секрет/Закрытый урок.lesson"] =
  line({ format: 1, goal: "Закрытый урок", language: "ru", created_at: NOW - 9000 }) +
  line({ kind: "Teacher", at: NOW - 8000, text: "СНИМОК-В-ЗАКРЫТОЙ-ПАПКЕ", interrupted: false, model: "claude-sonnet-5-5" });

const NOTES = {
  "Физика/Импульс.typ": "= Импульс\nИмпульс равен массе на скорость.\n",
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
  page.evaluate(`window.__plan.push(...${JSON.stringify(items)}.map((i) => i.reply ? window.__reply(i.reply) : i))`);

const settled = (page, label) =>
  page.waitFor(`!document.querySelector(".ts-lesson__status")`, { timeout: 40000, label });

// Кнопка по точному тексту внутри контейнера.
const press = (page, scope, label) =>
  page.evaluate(`(() => {
    const b = [...document.querySelectorAll(${JSON.stringify(scope + " button")})].find((x) => x.textContent.trim() === ${JSON.stringify(label)});
    if (!b) throw new Error("нет кнопки " + ${JSON.stringify(label)});
    b.click();
  })()`);

const body = (page) => page.evaluate("document.body.innerText");
const feed = (page) => page.text(".ts-lesson__feed");

const pose = (statement, solution) => ({
  reply: { tools: [{ name: "pose_problem", input: solution === undefined ? { statement } : { statement, solution } }] },
});
const conclude = (outcome, review) => ({ reply: { tools: [{ name: "conclude", input: { outcome, review } }] } });

const writeAnswer = async (page, text) => {
  await page.waitForElement(".ts-practice__form textarea", { timeout: 20000 });
  await page.click(".ts-practice__form textarea");
  await page.type(text);
};

export default async (page) => {
  await page.addInitScript(FAKE_MODEL);
  await page.reload();
  await page.evaluate(`${WRITE_FILES}(${JSON.stringify({ ...NOTES, ...SEEDED })})`);

  await page.click("text=Продолжить");
  await page.click("text=Открыть папку…");
  await page.waitForElement("text=Настройки", { timeout: 30000 });

  await openPalette(page, "Уроки");
  await page.waitForElement(".ts-lessons");
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
  await page.press("Escape");
  await page.sleep(300);

  // ── A: диалог «Новая практика» ──
  await openPalette(page, "Новая практика");
  await page.waitForElement(".ts-lesson-dialog .ts-practice__src");
  const dialog = await page.text(".ts-lesson-dialog");
  check(page, dialog.includes("Новая практика") && dialog.includes("Уроки и заметки") && dialog.includes("Описание темы"), "A: заголовок и выбор «Уроки и заметки / Описание темы»");
  check(page, dialog.includes("Понять импульс") && dialog.includes("урок · 1 шаг"), "A: урок в списке с числом шагов");
  check(page, dialog.includes("Закрытый урок") && dialog.includes("папка закрыта для агента"), "A: урок из закрытой папки помечен «папка закрыта для агента»");
  check(page, await page.evaluate(`[...document.querySelectorAll(".ts-practice__src")].find((l) => l.textContent.includes("Закрытый урок")).querySelector("input").disabled`), "A: закрытый урок нельзя выбрать");
  check(page, dialog.includes("Выбранное уйдёт модели") && dialog.includes("Уроки/"), "A: подсказка про журнал и папку");
  check(page, await page.evaluate(`document.querySelector(".ts-lesson-dialog .ts-btn--accent").disabled`), "A: без материала «Начать практику» недоступна");
  await page.click(".ts-practice__src");
  await page.click("text=Добавить…");
  await page.waitForElement(".ts-lesson__pick");
  await page.type("Импульс");
  await page.sleep(200);
  await page.click(".ts-lesson__pickrow");
  await page.click(".ts-lesson__pick input");
  await page.type("Дневник");
  await page.sleep(200);
  const closedRow = await page.evaluate(`(() => { const r = document.querySelector(".ts-lesson__pickrow"); return { disabled: r.disabled, text: r.textContent }; })()`);
  check(page, closedRow.disabled && closedRow.text.includes("Секрет/Дневник.typ") && closedRow.text.includes("папка закрыта для агента"), "A: закрытая заметка в поиске помечена и не выбирается");
  await page.click(".ts-lesson__pickrow");
  check(page, !(await page.text(".ts-lesson-dialog")).includes("Секрет/Дневник.typ ✕"), "A: нажатие на закрытую заметку её не прикрепляет");
  await page.shot(`${shots}/practice-A.png`);
  // Вкладка «Описание темы» показывает поле темы и прячет список.
  await press(page, ".ts-lesson-dialog", "Описание темы");
  await page.waitForElement(".ts-lesson-dialog textarea");
  check(page, !(await page.evaluate(`Boolean(document.querySelector(".ts-practice__src"))`)), "A: «Описание темы» скрывает список уроков");
  await page.shot(`${shots}/practice-A-topic.png`);
  await press(page, ".ts-lesson-dialog", "Уроки и заметки");
  check(page, (await page.evaluate(`document.querySelector(".ts-practice__src input").checked`)), "A: выбор урока сохранён при переключении");

  // ── B: первая задача; решения нет в DOM ──
  await plan(page, [pose("Две тележки разъезжаются после толчка. Что можно сказать об их скоростях?", SECRET_1)]);
  await press(page, ".ts-lesson-dialog", "Начать практику");
  await page.waitForElement(".ts-practice__form", { timeout: 40000 });
  await settled(page, "первая задача поставлена");
  let text = await body(page);
  check(page, text.includes("ЗАДАЧА 1") && text.includes("Две тележки разъезжаются"), "B: карточка «ЗАДАЧА 1» с условием");
  check(page, !text.includes(SECRET_1), "B: решение не попало в DOM (innerText)");
  check(page, !(await page.evaluate(`document.documentElement.outerHTML`)).includes(SECRET_1), "B: решения нет и в разметке");
  check(page, text.includes("Решение зафиксировано и откроется после вашего ответа") && text.includes("Решение хранится в файле практики"), "B: замок с подсказкой про файл практики");
  check(page, (await page.text(".ts-lesson__head")).includes("ПРАКТИКА"), "B: метка «ПРАКТИКА» в шапке");
  const form = await page.text(".ts-practice__form");
  check(page, ["Ваш ответ и рассуждение", "Сдаюсь", "Подсказка", "Ответить"].every((word) => form.includes(word)), "B: поле ответа и кнопки «Сдаюсь», «Подсказка», «Ответить»");
  let f = await feed(page);
  check(page, f.includes("практики · 1 урок") && f.includes("Вы прикрепили"), "материал: урок и прикреплённая заметка");
  const sent1 = await page.evaluate("window.__modelCalls.map((c) => c.body).join('\\n')");
  check(page, !sent1.includes("Личное.") && sent1.includes("Закон сохранения импульса"), "закрытая заметка модели не ушла, материал урока ушёл");
  await page.shot(`${shots}/practice-B.png`);

  // ── Подсказка: решение по-прежнему скрыто ──
  await writeAnswer(page, "Они разъезжаются, но пока не пойму как");
  await plan(page, [{ reply: { tools: [{ name: "give_hint", input: { text: "Что можно сказать о системе до толчка?" } }] } }]);
  await press(page, ".ts-practice__form", "Подсказка");
  await page.waitFor(`document.body.innerText.includes("ПОДСКАЗКА 1")`, { timeout: 30000, label: "карточка подсказки" });
  await settled(page, "подсказка получена");
  text = await body(page);
  check(page, text.includes("Что можно сказать о системе до толчка?") && text.includes("Прошу подсказку"), "B: карточка «ПОДСКАЗКА 1» и реплика пользователя");
  check(page, !text.includes(SECRET_1) && text.includes("откроется после вашего ответа"), "после подсказки решение скрыто, замок на месте");
  check(page, (await page.evaluate(`document.querySelector(".ts-practice__form textarea").value`)).includes("пока не пойму"), "подсказка не стирает написанный черновик");
  await page.shot(`${shots}/practice-B-hint.png`);

  // ── C: ответ → итог, разбор, решение ──
  await page.click(".ts-practice__form textarea");
  await page.press("a", { ctrl: true });
  await page.type("До толчка суммарный импульс ноль, значит скорости противоположны.");
  await plan(page, [conclude("Correct", "Вы сразу увидели замкнутую систему.")]);
  await page.press("Enter", { ctrl: true });
  await page.waitFor(`Boolean(document.querySelector(".ts-practice__verdict"))`, { timeout: 30000, label: "итог" });
  await settled(page, "итог вынесен");
  text = await body(page);
  check(page, text.includes("Верно") && text.includes("Вы сразу увидели замкнутую систему."), "C: итог «Верно» и разбор");
  check(page, text.includes("РЕШЕНИЕ (ЗАФИКСИРОВАНО ПРИ ПОСТАНОВКЕ)") && text.includes(SECRET_1), "C: решение показано после итога");
  check(page, text.includes("До толчка суммарный импульс ноль"), "C: ответ пользователя в ленте");
  check(page, text.includes("Следующая задача появится только по кнопке") && !(await page.evaluate(`Boolean(document.querySelector(".ts-practice__form"))`)), "C: форма ответа ушла, следующая задача только по кнопке");
  const callsAfterVerdict = await page.evaluate("window.__modelCalls.length");
  await page.sleep(800);
  check(page, (await page.evaluate("window.__modelCalls.length")) === callsAfterVerdict, "C: без нажатия новая задача не запрашивается");
  await page.shot(`${shots}/practice-C.png`);

  // ── Следующая задача: сразу задача, повтор не нужен ──
  await plan(page, [pose("Космонавт бросает ключ массой 0,5 кг со скоростью 4 м/с. Что с ним произойдёт?", SECRET_2)]);
  await press(page, ".ts-lesson__feed", "Следующая задача");
  await page.waitFor(`document.body.innerText.includes("ЗАДАЧА 2")`, { timeout: 40000, label: "вторая задача" });
  await settled(page, "вторая задача поставлена");
  await page.waitForElement(".ts-practice__form");
  text = await body(page);
  check(page, text.includes("Космонавт бросает ключ") && !text.includes(SECRET_2) && text.includes(SECRET_1), "вторая задача поставлена одним нажатием; её решение скрыто, первое видно");
  check(page, (await page.evaluate(`document.querySelectorAll(".ts-practice__prob").length`)) === 3, "лента: задача 1, итог 1, задача 2");
  check(page, (await page.evaluate("window.__plan.length")) === 0, "очередь ответов модели израсходована: повторов не было");

  // ── Сдаюсь → «Не решено» и решение ──
  await plan(page, [conclude("Correct", "Вот как это решается.")]);
  await press(page, ".ts-practice__form", "Сдаюсь");
  await page.waitFor(`document.querySelectorAll(".ts-practice__verdict").length >= 2`, { timeout: 30000, label: "итог после «Сдаюсь»" });
  await settled(page, "ход после «Сдаюсь»");
  text = await body(page);
  check(page, text.includes("Не решено") && text.includes(SECRET_2), "«Сдаюсь» → «Не решено» и решение показано");
  check(page, (await feed(page)).includes("Сдаюсь"), "реплика «Сдаюсь» в ленте");
  await page.shot(`${shots}/practice-C-giveup.png`);

  // ── D: задача без зафиксированного решения ──
  await plan(page, [pose("Задача без решения"), pose("Задача без решения")]);
  await press(page, ".ts-lesson__feed", "Следующая задача");
  await page.waitFor(`document.body.innerText.includes("ЗАДАЧА 3")`, { timeout: 40000, label: "третья задача" });
  await settled(page, "третья задача поставлена");
  await writeAnswer(page, "Не знаю, как решать.");
  await plan(page, [conclude("Partial", "Идея есть, но знак перепутан.")]);
  await page.press("Enter", { ctrl: true });
  await page.waitFor(`document.querySelectorAll(".ts-practice__verdict").length >= 3`, { timeout: 30000, label: "итог третьей" });
  await settled(page, "итог третьей");
  text = await body(page);
  check(page, text.includes("Частично") && text.includes("У этой задачи нет зафиксированного решения — итог вынесен по разбору."), "D: итог «Частично» и плашка про отсутствие решения");
  await page.shot(`${shots}/practice-D.png`);

  // ── E: сводка ──
  await press(page, ".ts-lesson__head", "Сводка ◧");
  await page.waitForElement(".ts-practice__rows");
  const sum = await page.text(".ts-plan");
  check(page, sum.includes("верно") && sum.includes("не решено") && sum.includes("частично") && sum.includes("1 подсказка"), "E: сводка — итоги и число подсказок");
  check(page, sum.includes("Идея есть, но знак перепутан.") && sum.includes("Нажатие на задачу открывает её разбор."), "E: последний разбор и пояснение");
  check(page, (await page.evaluate(`document.querySelectorAll(".ts-practice__row").length`)) === 3, "E: три строки задач");
  await page.shot(`${shots}/practice-E.png`);
  await page.evaluate(`document.querySelector(".ts-lesson__feed").scrollTop = 1e6`);
  await page.click(".ts-practice__row");
  await page.sleep(300);
  check(page, await page.evaluate(`document.querySelector('[data-event][data-flash="on"]') !== null || [...document.querySelectorAll('[data-flash="on"]')].length > 0`), "E: нажатие на задачу подсвечивает её место в ленте");

  // ── Список уроков ──
  await page.press("Escape");
  await openPalette(page, "Уроки");
  await page.waitForElement(".ts-lessons__row");
  const listText = await page.text(".ts-lessons");
  check(page, listText.includes("ПРАКТИКА") && listText.includes("Новая практика"), "список: метка «ПРАКТИКА» и кнопка «Новая практика»");
  check(page, (await page.evaluate(`document.querySelectorAll(".ts-lessons__kind").length`)) === 1, "метка только у практики");
  await page.shot(`${shots}/practice-list.png`);
  await page.press("Escape");

  // ── Узкая ширина и фокус ──
  await page.viewport(390, 800);
  await page.sleep(500);
  const over = await page.evaluate(`Math.max(document.querySelector(".ts-lesson").scrollWidth - document.querySelector(".ts-lesson").clientWidth, 0)`);
  check(page, over <= 0, `узко: вкладка практики не выходит за край (избыток ${over})`);
  check(page, await page.evaluate(`document.activeElement?.textContent === "Закрыть сводку"`), "узко: сводка — лист, фокус на «Закрыть сводку»");
  await page.shot(`${shots}/practice-narrow.png`);
  await press(page, ".ts-plan", "Закрыть сводку");
  await page.sleep(300);
  await page.shot(`${shots}/practice-narrow-feed.png`);
  const overFeed = await page.evaluate(`Math.max(document.querySelector(".ts-lesson").scrollWidth - document.querySelector(".ts-lesson").clientWidth, 0)`);
  check(page, overFeed <= 0, "узко: после закрытия сводки лента не выходит за край");
  // Клавиатурный фокус: Tab доходит до кнопки итога с видимой обводкой.
  await page.evaluate(`document.querySelector(".ts-lesson__feed").scrollTop = 1e6`);
  await page.evaluate(`[...document.querySelectorAll(".ts-lesson__feed button")].find((b) => b.textContent.trim() === "Следующая задача").focus()`);
  const outline = await page.evaluate(`getComputedStyle(document.activeElement).outlineStyle + " " + document.activeElement.matches(":focus-visible")`);
  page.log(`фокус на «Следующая задача»: ${outline}`);

  // ── Ревью 053: снимок урока из папки, закрытой после создания практики; видимый отказ ──
  await page.viewport(1280, 800);
  await page.sleep(300);
  const old = line({ format: 1, goal: "Практика старая", language: "ru", created_at: NOW - 1000, kind: "Practice" }) +
    line({
      kind: "Material",
      at: NOW - 900,
      lessons: [
        { path: "Секрет/Закрытый урок.lesson", goal: "Закрытая цель", text: "СНИМОК-В-ЗАКРЫТОЙ-ПАПКЕ", chars: 23 },
        { path: "Уроки/Понять импульс.lesson", goal: "Открытая цель", text: "СНИМОК-ОТКРЫТОГО-УРОКА", chars: 22 },
      ],
      denied: [
        { path: "Личное/Дневник.lesson", reason: "Denied" },
        { path: "Уроки/Чужая практика.lesson", reason: "NotLesson" },
      ],
    });
  await page.evaluate(`${WRITE_FILES}(${JSON.stringify({ "Уроки/Практика старая.lesson": old })})`);
  // Практика без задач при открытии сама заказывает ход: ответ готовим заранее.
  await plan(page, [pose("Задача по старой практике", "решение")]);
  const callsBefore = await page.evaluate("window.__modelCalls.length");
  await openPalette(page, "Уроки");
  await page.waitForElement(".ts-lessons__row");
  await page.waitFor(`[...document.querySelectorAll(".ts-lessons__row")].some((r) => r.textContent.includes("Практика старая"))`, { timeout: 20000, label: "старая практика в списке" });
  await page.evaluate(`[...document.querySelectorAll(".ts-lessons__row")].find((r) => r.textContent.includes("Практика старая")).click()`);
  await page.waitFor(`document.body.innerText.includes("Не вошёл в материал")`, { timeout: 30000, label: "строки отказов в ленте" });
  f = await feed(page);
  check(page, f.includes("Не вошёл в материал: Личное/Дневник.lesson") && f.includes("папка закрыта для агента"), "L1: отказ «папка закрыта» виден в ленте");
  check(page, f.includes("Не вошёл в материал: Уроки/Чужая практика.lesson") && f.includes("это не урок"), "L1: отказ «это не урок» виден в ленте");
  check(page, (await page.evaluate(`document.querySelectorAll(".ts-practice__denied").length`)) === 2, "L1: строка отказа на каждый отказанный урок");
  await page.shot(`${shots}/practice-denied.png`);
  const started = await page.evaluate(`(() => { const b = [...document.querySelectorAll(".ts-lesson button")].find((x) => ["Повторить", "Начать урок", "Начать практику"].includes(x.textContent.trim())); if (b) b.click(); return Boolean(b); })()`);
  page.log(`кнопка запуска хода: ${started ? "нажата" : "ход пошёл сам"}`);
  await page.waitFor(`window.__modelCalls.length > ${callsBefore}`, { timeout: 30000, label: "запрос по старой практике" });
  const lastBody = await page.evaluate("window.__modelCalls[window.__modelCalls.length - 1].body");
  check(page, lastBody.includes("СНИМОК-ОТКРЫТОГО-УРОКА"), "M1: снимок открытого урока ушёл в запрос");
  check(page, !lastBody.includes("СНИМОК-В-ЗАКРЫТОЙ-ПАПКЕ"), "M1: снимок урока из закрытой позже папки в запрос не ушёл");
};
