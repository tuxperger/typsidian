// Вкладки и разделение редактора (фича 014, quickstart.md §2–§6): vault из
// quickstart §1 в приватном хранилище браузера → вкладки → история → две группы →
// перезапуск → замеры → узкое окно (телефон).
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/tabs.mjs --opfs-vault --shot /tmp/tabs-end.png
//
// Снимки: каталог из переменной TABS_SHOTS (по умолчанию /tmp). Сценарий падает
// на первом несошедшемся условии; замеры печатает строками «замер: …».
const shots = process.env.TABS_SHOTS ?? "/tmp";
// Бюджеты времени (SC-001, SC-002) проверяются только на рабочей сборке
// (`next build` + `next start`): dev-сборка React вдвое-втрое медленнее, и
// там замеры только печатаются (T064).
const budgets = process.env.TABS_BUDGETS === "1";

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
    async read(path) {
      const at = path.lastIndexOf("/");
      const dir = await this.dir(at === -1 ? "" : path.slice(0, at), false);
      return (await (await dir.getFileHandle(path.slice(at + 1))).getFile()).text();
    },
    async remove(path) {
      const at = path.lastIndexOf("/");
      const dir = await this.dir(at === -1 ? "" : path.slice(0, at), false);
      await dir.removeEntry(path.slice(at + 1));
    },
  };
`;

// Счётчики вызовов модели вкладок на границе wasm: набор текста не должен их
// наращивать (FR-027).
const WASM_SPY = `
  window.__calls = { workspaceApply: 0, workspaceView: 0, beginEdit: 0 };
  const wrap = (instantiate) => async (source, imports) => {
    const result = await instantiate.call(WebAssembly, source, imports);
    const instance = result.instance ?? result;
    // Экспорты заморожены — копия с обёрнутыми вызовами вместо Proxy.
    const exports = {};
    for (const [key, value] of Object.entries(instance.exports)) {
      exports[key] = typeof value === "function" && key in window.__calls
        ? (...args) => { window.__calls[key] += 1; return value(...args); }
        : value;
    }
    const patched = { exports };
    return result.instance ? { module: result.module, instance: patched } : patched;
  };
  WebAssembly.instantiateStreaming = wrap(WebAssembly.instantiateStreaming);
  WebAssembly.instantiate = wrap(WebAssembly.instantiate);
`;

const expect = (page, condition, what) => {
  if (!condition) throw new Error("не сошлось: " + what);
  page.log("  ok: " + what);
};

const R = {
  mod: { ctrl: true, alt: true },
};

/** Названия вкладок всех полос в порядке показа. */
const titles = (page, strip = 0) =>
  page.evaluate(`[...document.querySelectorAll('.ts-tabs')][${strip}] ? [...[...document.querySelectorAll('.ts-tabs')][${strip}].querySelectorAll('.ts-tab')].map((tab) => tab.querySelector('.ts-tab__title').textContent) : []`);

const activeTitle = (page, strip = 0) =>
  page.evaluate(`(() => { const tab = [...document.querySelectorAll('.ts-tabs')][${strip}]?.querySelector('.ts-tab--on .ts-tab__title'); return tab ? tab.textContent : null; })()`);

/** Открыть файл из дерева по имени; `nth` — номер среди одноимённых. */
const openTree = async (page, name, nth = 0) => {
  await page.evaluate(`(() => {
    const items = [...document.querySelectorAll('.ts-tree__file > button:first-child')]
      .filter((button) => button.querySelector('.ts-tree__name')?.textContent === ${JSON.stringify(name)});
    items[${nth}].setAttribute('data-pick', '1');
  })()`);
  await page.click("[data-pick='1']");
  await page.evaluate(`document.querySelector('[data-pick]')?.removeAttribute('data-pick')`);
  // Действие уходит в очередь ядра и ждёт идущую сборку предпросмотра: ждём результат, а не паузу.
  await page.waitFor(`document.querySelector('.ts-tabs[data-focused="true"] .ts-tab--on .ts-tab__title')?.textContent === ${JSON.stringify(name)}`, { timeout: 10000, label: "активна вкладка " + name });
  await page.sleep(150);
};

/** Ждёт, пока в файле vault появится текст (запись идёт через очередь ядра и ждёт сборку предпросмотра). */
const fileHas = async (page, path, text, timeout = 8000) => {
  await page.waitFor(`window.__opfs.read(${JSON.stringify(path)}).then((body) => body.includes(${JSON.stringify(text)}))`, { timeout });
  return true;
};

/** Активная вкладка полосы целиком в её видимой части (FR-008). */
const activeVisible = (page, strip = 0) =>
  page.evaluate(`(() => { const strip = [...document.querySelectorAll('.ts-tabs__strip')][${strip}]; const tab = strip?.querySelector('.ts-tab--on'); if (!tab) return false; const s = strip.getBoundingClientRect(); const t = tab.getBoundingClientRect(); return t.left >= s.left - 1 && t.right <= s.right + 1; })()`);

const caret = (page) => page.evaluate(`document.querySelector('.ts-statusbar span:nth-child(2)')?.textContent ?? null`);
const scrollTop = (page) => page.evaluate(`document.querySelector('.ts-egroup[data-focused="true"] .cm-scroller')?.scrollTop ?? null`);
const calls = (page) => page.evaluate(`JSON.stringify(window.__calls)`).then((json) => JSON.parse(json));

export default async (page) => {
  // Перезагрузка нужна, чтобы шпион встал до загрузки wasm.
  await page.addInitScript(WASM_SPY);
  await page.reload();
  await page.evaluate(OPFS + "true");
  const longText = "= Результаты\n\n" + Array.from({ length: 60 }, (_, i) => `Строка ${i + 1} текста результатов.`).join("\n") + "\n";
  await page.evaluate(`(async () => {
    const o = window.__opfs;
    await o.write("Диссертация/Введение.typ", "= Введение\\n\\nПостановка — см. [[Результаты]].\\n");
    await o.write("Диссертация/Результаты.typ", ${JSON.stringify(longText)});
    await o.write("Диссертация/План.typ", "= План диссертации\\n\\n- расчёт\\n");
    await o.write("Заметки/План.typ", "= План заметок\\n");
    await o.write("Схема стенда.excalidraw", JSON.stringify({ type: "excalidraw", version: 2, source: "typsidian", elements: [], appState: {}, files: {} }));
    for (let i = 1; i <= 12; i += 1) await o.write("Пачка/Заметка " + String(i).padStart(2, "0") + ".typ", "= Заметка " + i + "\\n");
    // Замер на 50 вкладках (quickstart.md §6, SC-002).
    for (let i = 1; i <= 50; i += 1) await o.write("Много/Лист " + String(i).padStart(2, "0") + ".typ", "= Лист " + i + "\\n");
  })()`);

  await page.click("text=Открыть vault");
  await page.waitForElement("text=Введение");
  // Первый тик сверки с диском видит только что созданные тестом файлы; правка
  // до него дала бы ложный конфликт — это свойство стенда, а не вкладок.
  await page.sleep(2500);

  // ── Foundational: одна заметка работает как прежде ────────────────────
  await openTree(page, "Введение");
  await page.waitForElement(".ts-tabs .ts-tab");
  expect(page, JSON.stringify(await titles(page)) === JSON.stringify(["Введение"]), "одна вкладка «Введение»");
  await page.click(".cm-content");
  await page.press("End", { ctrl: true });
  await page.type(" Дописано.");
  expect(page, await fileHas(page, "Диссертация/Введение.typ", "Дописано."), "автосохранение записало набранное");
  expect(page, (await page.text(".ts-statusbar")).includes("сохранено"), "статус-бар: сохранено");
  expect(page, (await page.text(".ts-pane--preview")).includes("Дописано"), "предпросмотр следует за буфером");

  // ── US1: вкладки ───────────────────────────────────────────────────────
  await openTree(page, "Результаты");
  await openTree(page, "План", 0);
  await openTree(page, "План", 1);
  await openTree(page, "Схема стенда");
  const names = await titles(page);
  page.log("вкладки: " + JSON.stringify(names));
  expect(page, JSON.stringify(names) === JSON.stringify(["Введение", "Результаты", "План", "План", "Схема стенда"]), "пять вкладок в порядке открытия");
  const hints = await page.evaluate(`[...document.querySelectorAll('.ts-tab__hint')].map((h) => h.textContent)`);
  page.log("подсказки одноимённых: " + JSON.stringify(hints));
  expect(page, hints.length === 2 && hints[0] !== hints[1], "одноимённые «План» различаются подсказкой");
  expect(page, (await page.evaluate(`document.querySelector('.ts-tab[data-kind="Board"] .ts-tab__tag')?.textContent`)) === "доска", "у доски тег «доска»");
  await page.shot(shots + "/1a-tabs.png");

  // Каретка и прокрутка вкладки сохраняются (FR-004).
  await page.click(".ts-tab:nth-child(2)");
  await page.waitFor(`document.querySelector('.cm-content')?.textContent.includes('Строка 1 ')`);
  await page.evaluate(`document.querySelector('.cm-scroller').scrollTop = 400`);
  await page.sleep(300);
  const rect = await page.evaluate(`(() => { const line = [...document.querySelectorAll('.cm-line')].find((l) => l.getBoundingClientRect().top > 200); const r = line.getBoundingClientRect(); return { x: r.left + 40, y: r.top + r.height / 2 }; })()`);
  await page.mouse("mousePressed", rect.x, rect.y);
  await page.mouse("mouseReleased", rect.x, rect.y);
  await page.sleep(200);
  const caretBefore = await caret(page);
  const scrollBefore = await scrollTop(page);
  page.log(`каретка ${caretBefore}, прокрутка ${scrollBefore}`);
  await page.click(".ts-tab:nth-child(1)");
  await page.sleep(400);
  await page.click(".ts-tab:nth-child(2)");
  await page.sleep(500);
  expect(page, (await caret(page)) === caretBefore, "каретка вернулась на место");
  const scrollAfter = await scrollTop(page);
  page.log("прокрутка после возврата: " + scrollAfter);
  expect(page, Math.abs(scrollAfter - scrollBefore) <= 2, "прокрутка вернулась на место");

  // Повторное открытие не плодит вкладок (FR-002).
  await openTree(page, "Введение");
  expect(page, (await titles(page)).length === 5, "открытие открытой заметки не добавило вкладку");
  expect(page, (await activeTitle(page)) === "Введение", "открытая заметка стала активной");

  // Закрытие активной возвращает предыдущую по давности (FR-006).
  await page.click(".ts-tab:nth-child(2)");
  await page.sleep(200);
  await page.click(".ts-tab:nth-child(1)");
  await page.sleep(200);
  await page.click(".ts-tab--on .ts-tab__close");
  await page.sleep(500);
  expect(page, (await activeTitle(page)) === "Результаты", "после закрытия активна предыдущая по давности");
  await openTree(page, "Введение");

  // Набрал и сразу закрыл — текст в файле (SC-004).
  await page.click(".cm-content");
  await page.press("End", { ctrl: true });
  await page.type(" Быстро.");
  await page.click(".ts-tab--on .ts-tab__close");
  expect(page, await fileHas(page, "Диссертация/Введение.typ", "Быстро."), "закрытие дописало набранное");
  await openTree(page, "Введение");

  // Автосохранение неактивной вкладки идёт дальше (SC-004).
  await page.click(".cm-content");
  await page.press("End", { ctrl: true });
  await page.type(" Фоном.");
  await page.click(".ts-tab:nth-child(1)");
  expect(page, await fileHas(page, "Диссертация/Введение.typ", "Фоном."), "неактивная вкладка дописалась сама");

  // Полоса прокручивается, активная видна (FR-008).
  for (let i = 1; i <= 12; i += 1) await openTree(page, "Заметка " + String(i).padStart(2, "0"));
  const visible = await page.evaluate(`(() => { const strip = document.querySelector('.ts-tabs__strip'); const tab = strip.querySelector('.ts-tab--on'); const s = strip.getBoundingClientRect(); const t = tab.getBoundingClientRect(); return t.left >= s.left - 1 && t.right <= s.right + 1 && strip.scrollWidth > strip.clientWidth; })()`);
  expect(page, visible, "полоса прокручена, активная вкладка видна");
  await page.shot(shots + "/1a-scroll.png");

  // ── US2: история переходов группы ────────────────────────────────────────
  const backButton = ".ts-tabs__navbtn[aria-label='Назад']";
  const forwardButton = ".ts-tabs__navbtn[aria-label='Вперёд']";
  const disabled = (selector) => page.evaluate(`document.querySelector(${JSON.stringify(selector)})?.disabled ?? null`);
  // Каретка в «Введении» на третьей строке: место ухода должно вернуться.
  await openTree(page, "Введение");
  await page.click(".cm-line:nth-child(3)");
  await page.sleep(200);
  const introCaret = await caret(page);
  await openTree(page, "Результаты");
  await openTree(page, "План", 0);
  await page.press("[", { ...R.mod, code: "BracketLeft" });
  await page.waitFor(`document.querySelector('.ts-tab--on .ts-tab__title')?.textContent === 'Результаты'`, { timeout: 8000 });
  expect(page, true, "«Назад» (Mod+Alt+[) вернул «Результаты»");
  // Другая раскладка: та же физическая клавиша, другая буква.
  await page.press("х", { ...R.mod, code: "BracketLeft" });
  await page.waitFor(`document.querySelector('.ts-tab--on .ts-tab__title')?.textContent === 'Введение'`, { timeout: 8000 });
  expect(page, true, "«Назад» при русской раскладке вернул «Введение»");
  await page.sleep(400);
  expect(page, (await caret(page)) === introCaret, "каретка вернулась на место ухода (" + introCaret + ")");
  await page.press("]", { ...R.mod, code: "BracketRight" });
  await page.waitFor(`document.querySelector('.ts-tab--on .ts-tab__title')?.textContent === 'Результаты'`, { timeout: 8000 });
  expect(page, true, "«Вперёд» (Mod+Alt+]) вернул «Результаты»");
  expect(page, (await disabled(forwardButton)) === false, "«Вперёд» доступна, пока ветка не отброшена");
  expect(page, await activeVisible(page), "активная вкладка видна и после прокрутки полосы влево");
  await openTree(page, "План", 1);
  await page.sleep(300);
  expect(page, (await disabled(forwardButton)) === true, "после нового перехода «Вперёд» приглушена (ветка отброшена)");
  await page.shot(shots + "/1a-history.png");

  // Удалённая снаружи заметка пропускается при движении по истории.
  await openTree(page, "Заметка 01");
  await page.evaluate(`window.__opfs.remove("Заметки/План.typ")`);
  await page.waitFor(`![...document.querySelectorAll('.ts-tab')].some((t) => t.querySelector('.ts-tab__hint')?.textContent === 'Заметки')`, { timeout: 10000 });
  expect(page, true, "вкладка удалённой заметки закрылась (Deleted)");
  await page.press("[", { ...R.mod, code: "BracketLeft" });
  await page.sleep(600);
  expect(page, (await activeTitle(page)) !== "План" || (await page.text(".ts-tab--on .ts-tab__hint")) !== "Заметки", "«Назад» не открывает удалённую заметку");
  await page.evaluate(`window.__opfs.write("Заметки/План.typ", "= План заметок\\n")`);
  await page.waitFor(`[...document.querySelectorAll('.ts-tree__name')].filter((n) => n.textContent === 'План').length === 2`, { timeout: 10000, label: "«Заметки/План» снова в дереве" });

  // Закрытая вкладка открывается заново (FR-013).
  await openTree(page, "Введение");
  await openTree(page, "Результаты");
  await page.click(".ts-tab--on .ts-tab__close");
  await page.waitFor(`![...document.querySelectorAll('.ts-tab__title')].some((t) => t.textContent === 'Результаты')`, { timeout: 8000, label: "вкладка «Результаты» закрыта" });
  await page.click(backButton);
  await page.waitFor(`document.querySelector('.ts-tab--on .ts-tab__title')?.textContent === 'Результаты'`, { timeout: 8000, label: "«Назад» открыл закрытую" });
  expect(page, true, "«Назад» открыл закрытую «Результаты» заново");

  // ── US3: две группы ──────────────────────────────────────────────────────
  const stripCount = () => page.evaluate(`document.querySelectorAll('.ts-tabs').length`);
  const focusedSlot = () => page.evaluate(`document.querySelector('.ts-tabs[data-focused="true"]')?.dataset.slot ?? null`);
  const groupText = (index) => page.evaluate(`[...document.querySelectorAll('.ts-egroup')][${index}]?.querySelector('.cm-content')?.textContent ?? ''`);
  const cell = (index) => `.ts-groups__cell:nth-child(${index === 0 ? 1 : 3})`;
  const splitButton = ".ts-tabs__nav--end .ts-tabs__navbtn";
  const pressed = () => page.evaluate(`document.querySelector(${JSON.stringify(splitButton)})?.getAttribute('aria-pressed')`);

  await openTree(page, "Введение");
  await page.press("\\", { ...R.mod, code: "Backslash" });
  await page.waitFor(`document.querySelectorAll('.ts-tabs').length === 2`, { timeout: 8000, label: "две полосы вкладок" });
  expect(page, JSON.stringify(await titles(page, 1)) === JSON.stringify(["Введение"]), "Mod+Alt+\\ открыл ту же заметку во второй группе");
  expect(page, (await focusedSlot()) === "Second", "фокус перешёл во вторую группу");
  expect(page, (await pressed()) === "true", "«◫» читается как «убрать разделение»");
  expect(page, (await activeVisible(page, 0)) && (await activeVisible(page, 1)), "активные вкладки обеих групп видны в сузившихся полосах");

  // Одна заметка в двух группах — одно содержимое (FR-017, SC-006).
  await page.click(cell(1) + " .cm-content");
  await page.press("End", { ctrl: true });
  await page.type(" Вдвоём.");
  expect(page, (await groupText(0)).includes("Вдвоём."), "набранное во второй группе сразу видно в первой");
  await page.click(cell(0) + " .cm-content");
  await page.press("Home", { ctrl: true });
  await page.type("Слева. ");
  expect(page, (await groupText(1)).includes("Слева."), "набранное в первой группе сразу видно во второй");
  await page.click(cell(1) + " .cm-content");
  await page.press("z", { ctrl: true });
  await page.sleep(150);
  const afterUndo = await groupText(0);
  expect(page, !afterUndo.includes("Вдвоём.") && afterUndo.includes("Слева."), "Ctrl+Z во второй группе отменил своё, чужое осталось");
  expect(page, await fileHas(page, "Диссертация/Введение.typ", "Слева."), "общий документ записан в файл");

  // Предпросмотр и линия активной вкладки следуют за группой с кареткой (FR-009).
  await openTree(page, "План", 0);
  expect(page, (await titles(page, 1)).includes("План"), "заметка из дерева открылась в группе с фокусом");
  await page.waitFor(`document.querySelector('.ts-pane--preview')?.innerText.includes('План диссертации')`, { timeout: 10000, label: "предпросмотр «План»" });
  await page.click(cell(0) + " .cm-content");
  await page.waitFor(`document.querySelector('.ts-tabs[data-focused="true"]')?.dataset.slot === 'First'`, { timeout: 8000, label: "фокус в первой группе" });
  await page.waitFor(`document.querySelector('.ts-pane--preview')?.innerText.includes('Слева')`, { timeout: 10000, label: "предпросмотр «Введение»" });
  expect(page, true, "предпросмотр следует за группой с кареткой");
  expect(page, (await page.text(cell(0) + " .ts-statusbar")).includes("группа 1 из 2"), "статус-бар: «группа 1 из 2»");
  await page.shot(shots + "/1b-split.png");

  // Активная доска при двух группах: колонка предпросмотра остаётся, пустая (research.md §9).
  await openTree(page, "Схема стенда");
  await page.waitFor(`document.querySelector('.ts-pane--preview')?.innerText.includes('У доски нет предпросмотра')`, { timeout: 10000, label: "пустое состояние предпросмотра у доски" });
  expect(page, true, "у активной доски колонка предпросмотра остаётся с пустым состоянием");

  // Перенос сочетанием (FR-016).
  await page.evaluate(`[...document.querySelectorAll('.ts-tabs[data-slot="First"] .ts-tab')].find((t) => t.querySelector('.ts-tab__title').textContent === 'Результаты').setAttribute('data-pick', '1')`);
  await page.click("[data-pick='1']");
  await page.evaluate(`document.querySelector('[data-pick]')?.removeAttribute('data-pick')`);
  await page.waitFor(`document.querySelector('.ts-tabs[data-slot="First"] .ts-tab--on .ts-tab__title')?.textContent === 'Результаты'`, { timeout: 8000, label: "«Результаты» активна в первой группе" });
  await page.press("|", { ...R.mod, shift: true, code: "Backslash" });
  await page.waitFor(`[...document.querySelectorAll('.ts-tabs[data-slot="Second"] .ts-tab__title')].some((t) => t.textContent === 'Результаты')`, { timeout: 8000, label: "«Результаты» во второй группе" });
  expect(page, !(await titles(page, 0)).includes("Результаты"), "Mod+Alt+Shift+\\ перенёс вкладку, в первой группе её нет");

  // Перенос перетаскиванием на другую полосу.
  const secondBefore = (await titles(page, 1)).length;
  const dragged = await page.evaluate(`(() => {
    const strip = document.querySelector('.ts-tabs[data-slot="First"] .ts-tabs__strip');
    const s = strip.getBoundingClientRect();
    // Заметка, уже открытая во второй группе, не переносится, а активируется там.
    const taken = [...document.querySelectorAll('.ts-tabs[data-slot="Second"] .ts-tab__title')].map((t) => t.textContent);
    const tab = [...strip.querySelectorAll('.ts-tab')].find((t) => !t.classList.contains('ts-tab--on') && !taken.includes(t.querySelector('.ts-tab__title').textContent));
    // Полоса узкая и прокручена к активной: подводим вкладку к левому краю.
    strip.scrollLeft += tab.getBoundingClientRect().left - s.left;
    const r = tab.getBoundingClientRect();
    const target = document.querySelector('.ts-tabs[data-slot="Second"] .ts-tabs__strip').getBoundingClientRect();
    return { title: tab.querySelector('.ts-tab__title').textContent, x: r.left + 8, y: r.top + r.height / 2, tx: target.right - 12, ty: target.top + target.height / 2 };
  })()`);
  await page.mouse("mousePressed", dragged.x, dragged.y);
  await page.mouse("mouseMoved", dragged.x + 12, dragged.y);
  await page.mouse("mouseMoved", dragged.tx, dragged.ty);
  await page.mouse("mouseReleased", dragged.tx, dragged.ty);
  await page.waitFor(`document.querySelectorAll('.ts-tabs[data-slot="Second"] .ts-tab').length === ${secondBefore + 1}`, { timeout: 8000, label: "перетащенная вкладка во второй группе" });
  expect(page, (await titles(page, 1)).at(-1) === dragged.title, "перетаскивание перенесло «" + dragged.title + "» в конец второй полосы");

  // Граница групп сдвигается с клавиатуры (FR-018).
  const grow = () => page.evaluate(`Number(document.querySelector('.ts-groups__cell').style.flexGrow)`);
  const growBefore = await grow();
  await page.evaluate(`document.querySelector('.ts-groups > .ts-resizer').focus()`);
  await page.press("ArrowRight");
  await page.sleep(300);
  expect(page, (await grow()) > growBefore, "стрелка сдвинула границу групп");

  // Узкое окно: одна полоса с вкладками обеих групп, разделение недоступно (FR-020).
  await page.viewport(880, 800);
  await page.waitFor(`document.querySelectorAll('.ts-tabs').length === 1`, { timeout: 8000, label: "одна полоса при 880 px" });
  expect(page, (await titles(page)).filter((t) => t === "Введение").length === 1, "заметка из обеих групп показана один раз");
  expect(page, (await disabled(splitButton)) === true, "«◫» недоступна в узком окне");
  await page.shot(shots + "/1b-narrow.png");
  await page.viewport(1440, 813);
  await page.waitFor(`document.querySelectorAll('.ts-tabs').length === 2`, { timeout: 8000, label: "две группы после расширения" });
  expect(page, true, "после расширения окна две группы на месте");

  // Закрытие последней вкладки группы убирает разделение (FR-019).
  for (let i = 0; i < 20 && (await stripCount()) === 2; i += 1) {
    await page.click('.ts-tabs[data-slot="Second"] .ts-tab__close');
    await page.sleep(400);
  }
  expect(page, (await stripCount()) === 1, "закрыта последняя вкладка второй группы — группа одна");
  expect(page, (await pressed()) === "false", "«◫» снова «разделить»");

  // ── Телефон: «‹ ›» в шапке вместо полосы (макет 1c) ───────────────────────
  await openTree(page, "Введение");
  await openTree(page, "План", 0);
  await page.viewport(300, 700);
  await page.waitFor(`document.querySelectorAll('.ts-tabs').length === 0 && document.querySelectorAll('.ts-phone-nav button').length === 2`, { timeout: 8000, label: "шапка телефона" });
  expect(page, Boolean(await page.evaluate(`Boolean(document.querySelector('.ts-open-notes__count'))`)), "на телефоне полосы нет, в шапке счётчик открытых заметок");
  const phoneNav = () => page.evaluate(`[...document.querySelectorAll('.ts-phone-nav button')].map((b) => b.disabled)`);
  expect(page, JSON.stringify(await phoneNav()) === JSON.stringify([false, true]), "в шапке «Назад» доступна, «Вперёд» приглушена");
  await page.click(".ts-phone-nav button");
  await page.waitFor(`document.querySelector('.ts-phone-nav')?.innerText.includes('Введение')`, { timeout: 8000, label: "«Назад» в шапке телефона" });
  expect(page, (await phoneNav())[1] === false, "«Назад» в шапке вернул «Введение», «Вперёд» стала доступна");
  await page.shot(shots + "/1c-phone.png");
  await page.viewport(1440, 813);
  await page.waitFor(`document.querySelectorAll('.ts-tabs').length >= 1`, { timeout: 8000, label: "полоса вкладок после телефона" });

  // ── Колонка заметки следует за активной вкладкой (013, FR-009, T069) ─────
  // Оглавление показывает заголовки активной заметки — по первому из них
  // видно, за какой заметкой идёт колонка.
  const outlineHead = () => page.evaluate(`document.querySelector('.ts-outline__tree [role=treeitem] .ts-outline__text')?.textContent ?? null`);
  const outlineIs = (head, label) =>
    page.waitFor(`document.querySelector('.ts-outline__tree [role=treeitem] .ts-outline__text')?.textContent === ${JSON.stringify(head)}`, { timeout: 8000, label: "оглавление «" + head + "» " + label });
  const tabByTitle = (title, strip = 0) =>
    `[...[...document.querySelectorAll('.ts-tabs')][${strip}].querySelectorAll('.ts-tab')].find((tab) => tab.querySelector('.ts-tab__title').textContent === ${JSON.stringify(title)})`;

  await openTree(page, "Лист 03");
  await page.click('.ts-egroup[data-focused="true"] .cm-content');
  // Команда — переключатель (скрыта → показать, другой режим → оглавление,
  // уже оглавление → скрыть), поэтому жмём, пока дерево не появится.
  for (let i = 0; i < 3 && !(await page.evaluate(`Boolean(document.querySelector('.ts-outline__tree'))`)); i += 1) {
    await page.press("O", { ctrl: true, shift: true, code: "KeyO" });
    await page.waitForElement(".ts-outline__tree", { timeout: 2000 }).catch(() => {});
  }
  await outlineIs("Лист 3", "у открытой заметки");
  await openTree(page, "Результаты");
  await outlineIs("Результаты", "после открытия вкладки");
  await page.evaluate(`${tabByTitle("Лист 03")}.click()`);
  await outlineIs("Лист 3", "после переключения вкладки");
  expect(page, true, "оглавление следует за переключением вкладки");

  await page.press("\\", { ...R.mod, code: "Backslash" });
  await page.waitFor(`document.querySelectorAll('.ts-tabs').length === 2`, { timeout: 8000, label: "две группы для колонки" });
  await openTree(page, "План", 0);
  await outlineIs("План диссертации", "у группы с фокусом");
  await page.click('.ts-groups__cell:nth-child(1) .cm-content');
  await outlineIs("Лист 3", "после смены группы");
  expect(page, true, "оглавление следует за сменой группы");
  await page.click('.ts-groups__cell:nth-child(3) .cm-content');
  await outlineIs("План диссертации", "при возврате во вторую группу");
  await page.click(`.ts-tabs[data-slot="Second"] ${backButton}`);
  await outlineIs("Лист 3", "после «Назад»");
  expect(page, true, "оглавление следует за «Назад»");
  await openTree(page, "Результаты");
  await outlineIs("Результаты", "перед закрытием");
  await page.click('.ts-tabs[data-slot="Second"] .ts-tab--on .ts-tab__close');
  await page.waitFor(`document.querySelector('.ts-tabs[data-slot="Second"] .ts-tab--on .ts-tab__title')?.textContent !== 'Результаты'`, { timeout: 8000, label: "вкладка закрыта" });
  const afterClose = await activeTitle(page, 1);
  const headOf = { "Лист 03": "Лист 3", "План": "План диссертации", "Результаты": "Результаты" };
  await outlineIs(headOf[afterClose] ?? afterClose, "после закрытия вкладки");
  expect(page, true, "оглавление следует за закрытием вкладки (" + (await outlineHead()) + ")");
  for (let i = 0; i < 20 && (await stripCount()) === 2; i += 1) {
    await page.click('.ts-tabs[data-slot="Second"] .ts-tab__close');
    await page.sleep(400);
  }

  // ── Замеры (quickstart.md §6, T064) ──────────────────────────────────────
  /** От щелчка по вкладке до редактора с её текстом, готового к набору (SC-001). */
  const switchMs = (title, head) =>
    page.evaluate(`new Promise((resolve) => {
      const tab = ${tabByTitle(title)};
      const started = performance.now();
      tab.click();
      const ready = () => {
        const editor = document.querySelector('.ts-egroup[data-focused="true"] .cm-content');
        const on = document.querySelector('.ts-tabs[data-focused="true"] .ts-tab--on .ts-tab__title')?.textContent;
        if (on === ${JSON.stringify(title)} && editor?.getAttribute('contenteditable') === 'true' && editor.textContent.includes(${JSON.stringify(head)})) resolve(performance.now() - started);
        else requestAnimationFrame(ready);
      };
      ready();
    })`);
  /** Задержка ввода: от `beforeinput` до кадра после правки DOM редактора, медиана (SC-002). */
  const typingMs = async (count) => {
    await page.click('.ts-egroup[data-focused="true"] .cm-content');
    await page.press("End", { ctrl: true });
    await page.evaluate(`(() => {
      window.__lat = [];
      let started = null;
      window.__latIn = () => { started = performance.now(); };
      document.addEventListener('beforeinput', window.__latIn, true);
      const content = document.querySelector('.ts-egroup[data-focused="true"] .cm-content');
      window.__latObs = new MutationObserver(() => {
        if (started === null) return;
        const at = started;
        started = null;
        requestAnimationFrame(() => window.__lat.push(performance.now() - at));
      });
      window.__latObs.observe(content, { childList: true, subtree: true, characterData: true });
    })()`);
    await page.sleep(300);
    const before = await calls(page);
    for (let i = 0; i < count; i += 1) {
      await page.type("ж");
      await page.sleep(40);
    }
    await page.sleep(200);
    const samples = await page.evaluate(`(() => {
      document.removeEventListener('beforeinput', window.__latIn, true);
      window.__latObs.disconnect();
      return JSON.stringify(window.__lat);
    })()`).then((json) => JSON.parse(json));
    samples.sort((a, b) => a - b);
    const median = samples.length === 0 ? null : samples[Math.floor(samples.length / 2)];
    const after = await calls(page);
    return { median, apply: after.workspaceApply - before.workspaceApply, view: after.workspaceView - before.workspaceView };
  };

  await openTree(page, "Введение");
  await openTree(page, "Результаты");
  const switches = [];
  for (let i = 0; i < 12; i += 1) {
    const [title, head] = i % 2 === 0 ? ["Введение", "Введение"] : ["Результаты", "Результаты"];
    switches.push(await switchMs(title, head));
    await page.sleep(200);
  }
  // Во время сборки предпросмотра: правка запускает сборку, переключение сразу за ней.
  await page.click('.ts-egroup[data-focused="true"] .cm-content');
  await page.type(" сборка");
  const duringBuild = await switchMs("Введение", "Введение");
  page.log("  замер: переключение вкладки, мс = " + switches.map((ms) => ms.toFixed(1)).join(" / ") + "; во время сборки — " + duringBuild.toFixed(1));
  if (budgets) expect(page, Math.max(...switches, duringBuild) <= 100, "SC-001: переключение на открытую вкладку ≤100 мс");

  // Одна вкладка против пятидесяти: остальные закрываются, затем открываются листы.
  await page.evaluate(`(() => { const tab = ${tabByTitle("Введение")}; tab.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: 10, clientY: 10 })); })()`);
  await page.click("text=Закрыть остальные");
  await page.waitFor(`document.querySelectorAll('.ts-tab').length === 1`, { timeout: 8000, label: "одна вкладка" });
  const one = await typingMs(30);
  for (let i = 1; i <= 49; i += 1) await openTree(page, "Лист " + String(i).padStart(2, "0"));
  expect(page, (await page.evaluate(`document.querySelectorAll('.ts-tab').length`)) === 50, "открыто 50 вкладок");
  const fifty = await typingMs(30);
  page.log(`  замер: задержка ввода, мс (медиана) = ${one.median?.toFixed(1)} при 1 вкладке, ${fifty.median?.toFixed(1)} при 50`);
  page.log(`  замер: вызовы модели вкладок за 30 знаков — workspaceApply ${one.apply}/${fifty.apply}, workspaceView ${one.view}/${fifty.view} (1/50 вкладок)`);
  // Одно обращение за серию допустимо — первая правка помечает вкладку
  // несохранённой; с числом знаков и вкладок оно расти не должно.
  expect(page, [one, fifty].every((run) => run.apply === 0 && run.view <= 1), "FR-027: набор текста не обращается к модели вкладок на каждый знак");
  const switchFifty = await switchMs("Лист 01", "Лист 1");
  page.log("  замер: переключение при 50 вкладках, мс = " + switchFifty.toFixed(1));
  if (budgets) {
    // Допуск 1 мс сверх 10 %: медиана — единицы миллисекунд, и 10 % от неё
    // меньше разрешения таймера и разброса между прогонами.
    expect(page, fifty.median <= one.median * 1.1 + 1, "SC-002: при 50 вкладках ввод не медленнее чем на 10 %");
    expect(page, switchFifty <= 100, "FR-027: переключение при 50 вкладках ≤100 мс");
  }

  // ── US4: перезапуск (quickstart.md §5, T063) ─────────────────────────────
  await page.evaluate(`(() => { const tab = ${tabByTitle("Лист 01")}; tab.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: 10, clientY: 10 })); })()`);
  await page.click("text=Закрыть остальные");
  await page.waitFor(`document.querySelectorAll('.ts-tab').length === 1`, { timeout: 8000, label: "одна вкладка перед набором" });
  await openTree(page, "Введение");
  await page.press("\\", { ...R.mod, code: "Backslash" });
  await page.waitFor(`document.querySelectorAll('.ts-tabs').length === 2`, { timeout: 8000, label: "две группы перед перезапуском" });
  await openTree(page, "План", 0);
  await openTree(page, "Лист 02");
  const snapshot = async () => ({
    first: await titles(page, 0),
    second: await titles(page, 1),
    active: [await activeTitle(page, 0), await activeTitle(page, 1)],
    focused: await focusedSlot(),
  });
  const before = await snapshot();
  expect(page, before.first.length + before.second.length === 5, "пять вкладок в двух группах: " + JSON.stringify(before));
  await page.sleep(1500); // запись набора — после паузы 400 мс
  const reopen = async () => {
    await page.click("text=Открыть vault").catch(() => {});
    await page.waitForElement("text=Введение", { timeout: 30000 });
  };
  await page.reload();
  await reopen();
  await page.waitFor(`document.querySelectorAll('.ts-tabs').length === 2`, { timeout: 15000, label: "две группы после перезагрузки" });
  const after = await snapshot();
  expect(page, JSON.stringify(after) === JSON.stringify(before), "SC-003: вкладки, порядок, группы и активные восстановлены " + JSON.stringify(after));

  // Заметка удалена, пока приложение закрыто: её вкладки нет, сообщений нет (FR-023).
  await page.evaluate(`location.href = '/__typsidian-none'`);
  await page.waitFor(`location.pathname === '/__typsidian-none' && document.readyState === 'complete'`, { timeout: 15000, label: "страница без приложения" });
  await page.evaluate(OPFS + "true");
  await page.evaluate(`window.__opfs.remove("Много/Лист 02.typ")`);
  await page.evaluate(`location.href = '/'`);
  await page.waitFor(`location.pathname === '/' && document.readyState === 'complete'`, { timeout: 15000, label: "приложение снова" });
  await reopen();
  await page.waitFor(`document.querySelectorAll('.ts-tab').length >= 4`, { timeout: 15000, label: "вкладки восстановлены" });
  const pruned = await snapshot();
  expect(page, ![...pruned.first, ...pruned.second].includes("Лист 02") && pruned.first.length + pruned.second.length === 4, "удалённая заметка пропущена, остальные на месте " + JSON.stringify(pruned));
  const alerts = await page.evaluate(`[...document.querySelectorAll('[role=alert], [role=alertdialog], .ts-tab-refused')].map((el) => el.textContent.trim()).filter(Boolean).join(' | ')`);
  expect(page, alerts === "", "сообщений о пропущенной вкладке нет" + (alerts ? ": " + alerts : ""));

  // Испорченный ключ `workspaces` в настройках устройства: vault открывается без вкладок.
  await page.evaluate(`location.href = '/__typsidian-none'`);
  await page.waitFor(`location.pathname === '/__typsidian-none' && document.readyState === 'complete'`, { timeout: 15000, label: "страница без приложения" });
  const spoiled = await page.evaluate(`(async () => {
    const root = await navigator.storage.getDirectory();
    const handle = await root.getFileHandle('settings.json');
    const settings = JSON.parse(await (await handle.getFile()).text());
    if (!('workspaces' in settings)) return false;
    settings.workspaces = 'испорчено';
    const writable = await handle.createWritable();
    await writable.write(JSON.stringify(settings, null, 2));
    await writable.close();
    return true;
  })()`);
  expect(page, spoiled === true, "ключ workspaces найден и испорчен");
  await page.evaluate(`location.href = '/'`);
  await page.waitFor(`location.pathname === '/' && document.readyState === 'complete'`, { timeout: 15000, label: "приложение снова" });
  await reopen();
  await page.sleep(1500);
  expect(page, (await page.evaluate(`document.querySelectorAll('.ts-tab').length`)) === 0, "испорченный набор: vault открыт без вкладок");
};
