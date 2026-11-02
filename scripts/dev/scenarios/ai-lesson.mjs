// Сессия урока с ИИ-учителем (фича 051, US1–US2): включение ИИ, новый урок, вопрос с
// вариантами, ответ с уверенностью, ответ учителя потоком, перезагрузка с сохранённой
// перепиской, список уроков, узкий экран.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/ai-lesson.mjs --opfs-vault --shot /путь/lesson.png
//
// Сеть к модели подменена: `fetch` на api.anthropic.com отдаёт заготовленный поток
// SSE Anthropic (текст + вызов `ask_question`, затем текст). Остальные запросы идут
// как обычно. Снимки: каталог из LESSON_SHOTS (по умолчанию /tmp/claude-1000).
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
  const sse = (name, data) => "event: " + name + "\\ndata: " + JSON.stringify(data) + "\\n\\n";
  const pieces = (turn) => {
    const start = sse("message_start", { type: "message_start", message: { usage: { input_tokens: 9000, output_tokens: 1 } } });
    if (turn === 0) {
      const args = JSON.stringify({
        question: "Тележка A толкает тележку B с силой 3 Н. С какой силой B действует на A?",
        choices: ["3 Н, в противоположную сторону", "0 Н — толкает только A", "Зависит от масс тележек"],
      });
      return [
        start,
        sse("content_block_start", { type: "content_block_start", index: 0, content_block: { type: "text", text: "" } }),
        sse("content_block_delta", { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: "Начнём с того, что вы уже знаете " } }),
        sse("content_block_delta", { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: "о силах между двумя телами." } }),
        sse("content_block_stop", { type: "content_block_stop", index: 0 }),
        sse("content_block_start", { type: "content_block_start", index: 1, content_block: { type: "tool_use", id: "toolu_1", name: "ask_question", input: {} } }),
        sse("content_block_delta", { type: "content_block_delta", index: 1, delta: { type: "input_json_delta", partial_json: args.slice(0, 40) } }),
        sse("content_block_delta", { type: "content_block_delta", index: 1, delta: { type: "input_json_delta", partial_json: args.slice(40) } }),
        sse("content_block_stop", { type: "content_block_stop", index: 1 }),
        sse("message_delta", { type: "message_delta", delta: { stop_reason: "tool_use" }, usage: { output_tokens: 90 } }),
        sse("message_stop", { type: "message_stop" }),
      ];
    }
    return [
      start,
      sse("content_block_start", { type: "content_block_start", index: 0, content_block: { type: "text", text: "" } }),
      sse("content_block_delta", { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: "Верно — силы равны и противоположны. " } }),
      sse("content_block_delta", { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: "Важная путаница: эти силы приложены к разным телам, " } }),
      sse("content_block_delta", { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: "поэтому они не гасят друг друга." } }),
      sse("content_block_stop", { type: "content_block_stop", index: 0 }),
      sse("message_delta", { type: "message_delta", delta: { stop_reason: "end_turn" }, usage: { output_tokens: 70 } }),
      sse("message_stop", { type: "message_stop" }),
    ];
  };
  window.fetch = async (input, init) => {
    const url = typeof input === "string" ? input : String(input?.url ?? input);
    if (!url.includes("api.anthropic.com")) return realFetch(input, init);
    const body = init?.body ?? (typeof input?.clone === "function" ? await input.clone().text() : "");
    const turn = calls.length;
    calls.push({ url, body: String(body ?? "") });
    const encoder = new TextEncoder();
    const chunks = pieces(turn);
    // Кусками с паузами: опрос интерфейса (150 мс) должен успеть показать текст до конца потока.
    const stream = new ReadableStream({
      async start(controller) {
        for (const chunk of chunks) {
          controller.enqueue(encoder.encode(chunk));
          await new Promise((done) => setTimeout(done, window.__chunkDelay ?? 350));
        }
        controller.close();
      },
    });
    return new Response(stream, { status: 200, headers: { "content-type": "text/event-stream" } });
  };
})();
`;

async function fill(page, selector, text) {
  await page.click(selector);
  await page.press("a", { ctrl: true });
  await page.type(text);
  // Поля сохраняются по потере фокуса.
  await page.click(".ts-settings__head h2");
}

const openPalette = async (page, query) => {
  await page.click(".ts-cmdfield");
  await page.waitForElement(".ts-palette");
  await page.type(query);
  await page.sleep(200);
  await page.press("Enter");
};

export default async (page) => {
  await page.addInitScript(FAKE_MODEL);
  await page.reload();

  await page.click("text=Продолжить"); // экран выбора места (037)
  await page.click("text=Открыть папку…");
  await page.waitForElement("text=Настройки", { timeout: 30000 });

  // ── Без ИИ учитель не начинается (артборд E) ──
  await openPalette(page, "Новый урок");
  await page.waitForElement(".ts-lesson-dialog");
  await page.waitForElement("text=Открыть настройки");
  check(page, (await page.text(".ts-lesson-dialog")).includes("выключено"), "ИИ выключен: урок начать нельзя");
  check(page, (await page.evaluate("window.__modelCalls.length")) === 0, "ни одного запроса к модели");
  await page.shot(`${shots}/lesson-E.png`);
  await page.click("text=Открыть настройки");

  // ── Включаем ИИ, ключ, модель, папка уроков ──
  await page.waitForElement("text=Обучение с ИИ");
  await page.click("text=Обучение с ИИ");
  await page.click("text=Включить для этого vault");
  await page.waitForElement("[role=alertdialog]");
  await page.click("text=Включить");
  await page.waitForElement('input[type=password]');
  await page.click('input[type=password]');
  await page.type("sk-ant-test");
  await page.click("text=Сохранить");
  await page.waitForElement("text=Заменить");
  await fill(page, 'input[list="ts-ai-models"]', "claude-sonnet-5-5");
  await page.waitForElement("text=Папка уроков");
  check(page, (await page.text()).includes("Папка уроков"), "в настройках есть поле «Папка уроков»");
  await page.press("Escape");
  await page.sleep(300);

  // ── Новый урок (артборд D) ──
  await openPalette(page, "Новый урок");
  await page.waitForElement(".ts-lesson-dialog textarea");
  await page.type("Понять, почему сохраняется импульс");
  await page.shot(`${shots}/lesson-D.png`);
  await page.click("text=Сразу с урока");
  await page.click("text=Начать урок");

  // ── Первый ход потоком; вопрос с вариантами (артборд A) ──
  await page.waitForElement(".ts-lesson__stream", { timeout: 20000 });
  check(page, true, "текст учителя виден до конца потока");
  await page.waitForElement(".ts-lesson__opts", { timeout: 20000 });
  const feed = await page.text(".ts-lesson__feed");
  check(page, feed.includes("Начнём с того"), "реплика учителя в ленте");
  check(page, feed.includes("С какой силой B действует на A"), "вопрос показан");
  check(page, (await page.text(".ts-lesson__opts")).includes("Зависит от масс"), "варианты ответа показаны");
  check(
    page,
    (await page.evaluate(`document.querySelector(".ts-lesson__q button[type=submit]").disabled`)) === true,
    "«Ответить» недоступно, пока вариант не выбран",
  );

  await page.click("text=3 Н, в противоположную сторону");
  await page.click("text=Скорее да");
  await page.click(".ts-lesson__q textarea");
  await page.type("Третий закон Ньютона, но не до конца понимаю, почему тела тогда вообще двигаются.");
  await page.shot(`${shots}/lesson-A.png`);

  // ── Ответ: Ctrl+Enter, учитель отвечает потоком (артборд B) ──
  await page.evaluate("window.__chunkDelay = 700");
  await page.press("Enter", { ctrl: true });
  await page.waitForElement(".ts-lesson__status");
  await page.waitForElement(".ts-lesson__stream");
  await page.sleep(900);
  await page.shot(`${shots}/lesson-B.png`);
  check(page, (await page.text(".ts-lesson__status")).includes("Esc"), "виден «Остановить Esc»");
  await page.waitForElement(".ts-lesson__q", { timeout: 20000 }).catch(() => undefined);
  await page.waitFor(
    `document.querySelector(".ts-lesson__feed")?.innerText.includes("не гасят друг друга") && !document.querySelector(".ts-lesson__status")`,
    { timeout: 25000, label: "ответ учителя дописан" },
  );
  await page.sleep(500);
  const after = await page.text(".ts-lesson__feed");
  check(page, after.includes("Верно — силы равны"), "ответ учителя записан в ленту");
  check(page, after.includes("3 Н, в противоположную сторону"), "ответ пользователя в ленте");
  check(page, after.includes("уверенность: скорее да"), "уверенность отмечена");
  check(page, after.includes("Третий закон Ньютона"), "«думаю вслух» в ленте");
  const calls = await page.evaluate("window.__modelCalls.length");
  check(page, calls === 2, `к модели два запроса (получено ${calls})`);
  const meter = await page.text(".ts-lesson__meter");
  page.log(`счётчик: ${meter.replace(/\n/g, " ")}`);
  check(page, meter.includes("$"), "счётчик стоимости в шапке");
  check(page, (await page.evaluate("window.__modelCalls[1].body")).includes("Третий закон Ньютона"), "рассуждение ушло модели");

  // ── Перезагрузка: переписка на месте ──
  await page.reload();
  // Vault и вкладки восстанавливаются сами; если нет — открываем папку руками.
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
  const restored = await page.text(".ts-lesson__feed");
  check(page, restored.includes("Верно — силы равны") && restored.includes("Начнём с того"), "после перезагрузки переписка на месте");

  // ── Список уроков (артборд G) ──
  await openPalette(page, "Уроки");
  await page.waitForElement(".ts-lessons__row");
  const list = await page.text(".ts-lessons");
  check(page, list.includes("Понять, почему сохраняется импульс"), "урок в списке");
  check(page, list.includes("ждёт ответа"), "состояние урока в списке (ждёт ответа)");
  await page.shot(`${shots}/lesson-G.png`);
  await page.press("Escape");
  await page.sleep(300);

  // ── Остановка ответа и повтор (артборд F) ──
  await page.click(".ts-lesson__q textarea");
  await page.type("Суммарный импульс не меняется.");
  await page.evaluate("window.__chunkDelay = 1500");
  await page.press("Enter", { ctrl: true });
  await page.waitForElement(".ts-lesson__stream", { timeout: 20000 });
  const focused = await page.evaluate(`document.activeElement?.className ?? ""`);
  check(page, focused.includes("ts-lesson__stop"), "фокус на «Остановить»: Esc под рукой");
  await page.press("Escape");
  await page.waitForElement(".ts-lesson__box--err", { timeout: 20000 });
  const cut = await page.text(".ts-lesson__feed");
  check(page, cut.includes("ОТВЕТ ОБОРВАН"), "оборванная реплика помечена");
  check(page, cut.includes("Запрос отменён"), "причина обрыва словами");
  check(page, cut.includes("Суммарный импульс не меняется."), "ответ пользователя сохранён");
  await page.shot(`${shots}/lesson-F.png`);
  await page.evaluate("window.__chunkDelay = 100");
  await page.click("text=Запросить ответ снова");
  await page.waitFor(`!document.querySelector(".ts-lesson__status") && !document.querySelector(".ts-lesson__box--err")`, {
    timeout: 25000,
    label: "повтор ответа завершён",
  });
  check(page, (await page.evaluate("window.__modelCalls.length")) === 2, "повтор — ещё один запрос (счёт после перезагрузки), ответ не дублируется");

  // ── Узкий экран (артборд J) ──
  await page.viewport(390, 800);
  await page.sleep(400);
  const overflow = await page.evaluate(
    `document.querySelector(".ts-lesson") ? document.querySelector(".ts-lesson").scrollWidth - document.querySelector(".ts-lesson").clientWidth : -1`,
  );
  check(page, overflow <= 0, `на 390 px урок не уходит за край (избыток ${overflow})`);
  await page.shot(`${shots}/lesson-J.png`);
};
