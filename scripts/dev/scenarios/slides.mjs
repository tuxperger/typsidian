// Показ слайдов (фича 019, quickstart.md §2–§3): образец без пакетов → каретка на
// втором слайде → Ctrl+Alt+P → окно показа (popup) → листание, номер, выход →
// вид докладчика → правка при открытом показе.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/slides.mjs --opfs-vault --shot /tmp/slides.png
//
// Снимки: каталог из переменной SLIDES_SHOTS (по умолчанию /tmp). Замеры печатает
// строками «замер: …»; сценарий падает на первом несошедшемся условии.
const shots = process.env.SLIDES_SHOTS ?? "/tmp";

const OPFS = `
  window.__opfs = {
    async root() { return (await navigator.storage.getDirectory()).getDirectoryHandle("vault", { create: true }); },
    async dir(path, create) {
      let dir = await this.root();
      for (const part of path.split("/").filter(Boolean)) dir = await dir.getDirectoryHandle(part, { create });
      return dir;
    },
    async write(path, text) {
      const at = path.lastIndexOf("/");
      const dir = await this.dir(at === -1 ? "" : path.slice(0, at), true);
      const file = await dir.getFileHandle(path.slice(at + 1), { create: true });
      const stream = await file.createWritable();
      await stream.write(text);
      await stream.close();
    },
  };
`;

// quickstart.md: четыре страницы, три логических слайда (второй — в два шага), заметка
// на странице 2. Картинки нет: отсутствующий файл был бы ошибкой сборки.
const SAMPLE = `#set page(width: 16cm, height: 9cm)
= Первый
Текст первого слайда.
#metadata((t: "LogicalSlide", v: 1)) <pdfpc>
#pagebreak()
= Второй
Шаг один.
#metadata((t: "LogicalSlide", v: 2)) <pdfpc>
#metadata((t: "Note", v: "Сказать про шаги.")) <pdfpc>
#pagebreak()
= Второй
Шаг один. Шаг два.
#metadata((t: "LogicalSlide", v: 2)) <pdfpc>
#pagebreak()
= Третий
Последний слайд.
#metadata((t: "LogicalSlide", v: 3)) <pdfpc>
`;

const expect = (page, condition, what) => {
  if (!condition) throw new Error("не сошлось: " + what);
  page.log("  ok: " + what);
};

const counter = (page) =>
  page.evaluate(`document.querySelector('.ts-show__counter')?.textContent.replace(/\\s+/g, '') ?? null`);

/** Ждёт счётчик окна показа с таким текстом и возвращает, сколько это заняло. */
const waitCounter = async (popup, text, timeout = 4000) => {
  const started = Date.now();
  await popup.waitFor(
    `(document.querySelector('.ts-show__counter')?.textContent.replace(/\\s+/g, '') ?? '').startsWith(${JSON.stringify(text)})`,
    { timeout, label: "счётчик «" + text + "»" },
  );
  return Date.now() - started;
};

const caretText = (page) =>
  page.evaluate(`document.querySelector('.ts-statusbar span:nth-child(2)')?.textContent ?? null`);

export default async (page) => {
  await page.evaluate(OPFS + "true");
  await page.evaluate(`window.__opfs.write("доклады/показ.typ", ${JSON.stringify(SAMPLE)})`);
  await page.click("text=Открыть vault");
  await page.waitForElement("text=показ");
  await page.sleep(2500);

  // Открыть образец и дождаться сборки: четыре страницы в предпросмотре.
  await page.evaluate(`(() => {
    const item = [...document.querySelectorAll('.ts-tree__file > button:first-child')]
      .find((button) => button.querySelector('.ts-tree__name')?.textContent === 'показ');
    item.setAttribute('data-pick', '1');
  })()`);
  await page.click("[data-pick='1']");
  await page.waitFor(`document.querySelectorAll('.ts-preview__page').length === 4`, {
    timeout: 20000,
    label: "четыре страницы предпросмотра",
  });
  expect(page, true, "образец собран: четыре страницы");

  // Кнопка «Показ» видна у открытой заметки (A1).
  await page.waitForElement(".ts-showmenu");
  await page.shot(shots + "/slides-a1-button.png");
  await page.click("[aria-label='Варианты показа']");
  await page.waitForElement("[role=menuitem]");
  const items = await page.evaluate(`[...document.querySelectorAll('[role=menuitem]')].map((el) => el.textContent.replace(/\\s+/g, ' ').trim())`);
  page.log("пункты меню: " + JSON.stringify(items));
  expect(page, items.length === 3, "в меню три пункта (с видом докладчика)");
  await page.shot(shots + "/slides-a1-menu.png");
  await page.press("Escape");
  await page.waitFor(`!document.querySelector('[role=menu]')`, { label: "меню закрыто" });

  // Те же пункты — в палитре команд.
  await page.press("k", { ctrl: true });
  await page.type("Показ слайдов");
  await page.waitFor(`document.body.innerText.includes('Показ слайдов: с начала')`, { label: "команды показа в палитре" });
  const palette = await page.evaluate(`document.body.innerText`);
  expect(page, palette.includes("Показ слайдов: с текущего слайда") && palette.includes("Показ слайдов: с видом докладчика"), "в палитре три команды показа");
  await page.press("Escape");
  await page.sleep(200);

  // Каретка — на «Шаг один.» второго слайда (строка 7).
  await page.click(".cm-content");
  await page.press("Home", { ctrl: true });
  for (let i = 0; i < 6; i += 1) await page.press("ArrowDown");
  page.log("каретка до показа: " + (await caretText(page)));

  // ── US1: запуск с каретки ──────────────────────────────────────────────
  const startedAt = Date.now();
  await page.press("p", { ctrl: true, alt: true });
  const show = await page.popup("/show");
  await show.waitFor(`Boolean(document.querySelector('.ts-slide__paint svg'))`, { timeout: 6000, label: "первый слайд" });
  const firstSlideMs = Date.now() - startedAt;
  page.log(`замер: от команды до первого слайда ${firstSlideMs} мс (SC-001: ≤ 2000)`);
  expect(page, firstSlideMs <= 2000, "первый слайд за 2 с");

  const label = await counter(show);
  page.log("счётчик: " + label);
  expect(page, label?.startsWith("2/3") && label.includes("шаг1из2"), "показ начат со второго слайда, шаг 1 из 2");
  const notesInShow = await show.evaluate(`document.body.textContent.includes('Сказать про шаги')`);
  expect(page, !notesInShow, "заметок докладчика в окне показа нет (FR-006)");
  await show.viewport(1280, 720);
  await page.sleep(300);
  await show.shot(shots + "/slides-a2-show.png");
  // Фокус клавиатуры виден: Tab ставит его на кнопку панели, панель не прячется.
  await show.evaluate(`document.querySelector('.ts-show__hud button').focus()`);
  await show.sleep(2500);
  const hudShown = await show.evaluate(`getComputedStyle(document.querySelector('.ts-show__hud')).opacity`);
  expect(page, hudShown === "1", "панель не прячется, пока в ней фокус");
  const ring = await show.evaluate(`getComputedStyle(document.activeElement).outlineStyle`);
  expect(page, ring !== "none", "у сфокусированной кнопки есть контур");
  await show.shot(shots + "/slides-a2-focus.png");
  await show.evaluate(`document.activeElement.blur()`);
  await show.viewport(390, 700);
  await page.sleep(300);
  await show.shot(shots + "/slides-a2-show-narrow.png");
  await show.viewport(1280, 720);

  // SC-002: от нажатия клавиши в окне показа до новой разметки слайда — по часам самого окна.
  await show.evaluate(`(() => {
    window.__lat = [];
    let t0 = 0;
    addEventListener("keydown", () => { t0 = performance.now(); }, true);
    new MutationObserver(() => { if (t0) { window.__lat.push(performance.now() - t0); t0 = 0; } })
      .observe(document.body, { subtree: true, childList: true, characterData: true });
  })()`);
  await show.press("ArrowRight");
  await waitCounter(show, "2/3·шаг2");
  const latency = await show.evaluate(`window.__lat[0]`);
  page.log(`замер: клавиша → новый слайд в окне показа ${Math.round(latency)} мс (SC-002: ≤ 100)`);
  expect(page, latency <= 100, "смена слайда ≤ 100 мс");
  await show.press("End");
  await waitCounter(show, "3/3");
  await show.press("Home");
  await waitCounter(show, "1/3");
  expect(page, true, "End и Home");
  await show.press("ArrowLeft");
  expect(page, (await counter(show)).startsWith("1/3"), "на первом слайде «назад» ничего не меняет");

  // Набор номера (A3).
  await show.press("2");
  await show.waitForElement(".ts-show__goto");
  await show.shot(shots + "/slides-a3-goto.png");
  await show.press("Enter");
  await waitCounter(show, "2/3");
  await show.press("9");
  await show.press("Enter");
  await waitCounter(show, "3/3");
  expect(page, true, "переход по номеру: 2, затем 9 → последний");

  // Выход: каретка на показанном слайде (FR-004).
  await show.press("Escape");
  await page.waitFor(`(document.querySelector('.ts-statusbar span:nth-child(2)')?.textContent ?? '').includes('Ln 15')`, {
    timeout: 4000,
    label: "каретка на третьем слайде (строка 15, заголовок «Третий»)",
  });
  page.log("каретка после выхода: " + (await caretText(page)));
  expect(page, true, "после Esc каретка на тексте третьего слайда");

  // ── US2: вид докладчика ────────────────────────────────────────────────
  // Одно нажатие открывает окно показа; второе окно по тому же жесту браузер не
  // пускает — тост «Открыть ещё раз» открывает вид докладчика отдельным жестом.
  await page.click("[aria-label='Варианты показа']");
  await page.click("text=Сразу с видом докладчика");
  const show2 = await page.popup("role=show");
  await show2.waitFor(`Boolean(document.querySelector('.ts-slide__paint svg'))`, { timeout: 6000, label: "окно показа" });
  let presenter = await page.popup("role=presenter", { timeout: 1500 }).catch(() => null);
  if (presenter === null) {
    page.log("второе окно по тому же жесту не открылось — тост «Открыть ещё раз»");
    await page.waitForElement("text=Открыть ещё раз");
    await page.shot(shots + "/slides-blocked-toast.png");
    await page.click("text=Открыть ещё раз");
    presenter = await page.popup("role=presenter", { timeout: 6000 });
  }
  await presenter.waitFor(`Boolean(document.querySelector('.ts-presenter__notes'))`, { label: "вид докладчика" });
  await presenter.waitFor(`Boolean(document.querySelector('.ts-presenter .ts-slide__paint svg'))`, { label: "слайд в виде докладчика" });
  await page.sleep(300);
  // Окно докладчика в рабочем размере (≥ 900 px — две колонки) и в узком (стопкой).
  await presenter.viewport(1180, 720);
  await page.sleep(400);
  await presenter.shot(shots + "/slides-a7-presenter.png");
  await presenter.viewport(420, 800);
  await page.sleep(400);
  await presenter.shot(shots + "/slides-a7-presenter-narrow.png");
  await presenter.viewport(1180, 720);
  await show2.press("2");
  await show2.press("Enter");
  await waitCounter(show2, "2/3");
  await presenter.waitFor(`Boolean(document.querySelector('.ts-presenter__notes p'))`, { label: "заметка к слайду 2" });
  const startPage = await counter(show2);
  const notes = await presenter.text(".ts-presenter__notes");
  page.log("слайд: " + startPage + "; заметки в виде докладчика: " + JSON.stringify(notes));
  expect(page, notes.includes("Сказать про шаги"), "заметка к слайду видна докладчику");
  await presenter.press("ArrowRight");
  await waitCounter(show2, "2/3·шаг2");
  expect(page, true, "листание в виде докладчика листает и показ");
  await show2.press("ArrowLeft");
  await presenter.waitFor(`Boolean(document.querySelector('.ts-presenter__thumb--current')?.getAttribute('aria-label')?.includes('2'))`, { label: "лента докладчика следует за показом" });
  expect(page, true, "листание в окне показа листает и вид докладчика");

  // Кнопка «Докладчик» в окне показа: окно докладчика закрывается и открывается снова.
  const toggle = `[...document.querySelectorAll('.ts-show__hud button')].find((b) => b.textContent.includes('Докладчик')).click()`;
  await show2.evaluate(toggle);
  await page.sleep(800);
  expect(page, (await page.popup("role=presenter", { timeout: 1200 }).catch(() => null)) === null, "«Докладчик» закрывает открытый вид докладчика");
  await show2.evaluate(toggle);
  presenter = await page.popup("role=presenter", { timeout: 4000 });
  await presenter.waitFor(`Boolean(document.querySelector('.ts-presenter__thumb--current'))`, { label: "вид докладчика открыт кнопкой" });
  expect(page, true, "«Докладчик» открывает вид докладчика из окна показа");
  await show2.press("Escape");
  await page.waitFor(`!document.querySelector('.ts-showmenu__main')?.disabled`, { label: "показ завершён" });
  await page.sleep(500);

  // ── US3: правка при открытом показе ────────────────────────────────────
  await page.click(".cm-content");
  await page.press("Home", { ctrl: true });
  for (let i = 0; i < 6; i += 1) await page.press("ArrowDown");
  await page.press("p", { ctrl: true, alt: true });
  const live = await page.popup("/show");
  await live.waitFor(`Boolean(document.querySelector('.ts-slide__paint svg'))`, { timeout: 6000, label: "слайд после повторного запуска" });
  await waitCounter(live, "2/3");
  const before = await live.evaluate(`document.querySelector('.ts-slide__paint').innerHTML.length`);
  await page.click(".cm-content");
  await page.press("Home", { ctrl: true });
  for (let i = 0; i < 6; i += 1) await page.press("ArrowDown");
  await page.press("End");
  const editedAt = Date.now();
  await page.type(" Правка.");
  await live.waitFor(`document.querySelector('.ts-slide__paint').innerHTML.length !== ${before}`, {
    timeout: 5000,
    label: "слайд обновился после правки",
  });
  page.log(`замер: правка → обновлённый слайд ${Date.now() - editedAt} мс (SC-004: ≤ 2000)`);
  expect(page, (await counter(live)).startsWith("2/3"), "номер слайда после правки тот же");
  await live.shot(shots + "/slides-us3-edited.png");

  // Поломанная сборка: плашка A4.
  await page.press("Home", { ctrl: true });
  await page.type("#set page(");
  await live.waitForElement(".ts-show__banner-action", { timeout: 6000 });
  await live.shot(shots + "/slides-a4-stale.png");
  expect(page, true, "плашка «последняя удачная сборка» (A4)");

  // SC-004: страниц стало меньше текущей — показан последний слайд, окно не закрылось.
  await live.press("End");
  await waitCounter(live, "3/3");
  await page.click(".cm-content");
  await page.press("a", { ctrl: true });
  for (const line of ["#set page(width: 16cm, height: 9cm)", "Один", "#pagebreak()", "Два"]) {
    await page.type(line);
    await page.press("Enter");
  }
  const shrunkAt = Date.now();
  await live.waitFor(`(document.querySelector('.ts-show__counter')?.textContent.replace(/\\s+/g, '') ?? '').startsWith('2/2')`, {
    timeout: 6000,
    label: "последняя из двух страниц",
  });
  page.log(`замер: удаление страниц → обновлённый показ ${Date.now() - shrunkAt} мс`);
  expect(page, true, "страниц стало меньше текущего номера — показан последний слайд");
  await live.shot(shots + "/slides-us3-shrunk.png");
  await live.press("Escape");
  await page.sleep(500);

  // ── Ошибки: заметка, которая ни разу не собралась (A5) ─────────────────
  await page.evaluate(`window.__opfs.write("доклады/сломан.typ", "#set page(\\n")`);
  await page.waitForElement("text=сломан", { timeout: 8000 });
  await page.evaluate(`(() => {
    const item = [...document.querySelectorAll('.ts-tree__file > button:first-child')]
      .find((button) => button.querySelector('.ts-tree__name')?.textContent === 'сломан');
    item.setAttribute('data-pick-broken', '1');
  })()`);
  await page.click("[data-pick-broken='1']");
  await page.sleep(2000);
  await page.press("p", { ctrl: true, alt: true });
  const broken = await page.popup("/show");
  await broken.waitFor(`Boolean(document.querySelector('.ts-show__empty'))`, { timeout: 6000, label: "экран «Показывать нечего»" });
  page.log("A5: " + JSON.stringify(await broken.text(".ts-show__empty")));
  await broken.shot(shots + "/slides-a5-empty.png");
  await broken.evaluate(`document.querySelector(".ts-show__empty-actions button").click()`);
  await page.waitFor(`!document.querySelector('.ts-showmenu__main')?.disabled`, { label: "показ завершён" });
  expect(page, true, "A5: «Вернуться к заметке» завершает показ");
};