// Граф связей (фича 025, quickstart.md §2–§3).
//
//   §2 (по умолчанию): 50 связанных заметок → «Граф» → раскладка движется и замирает ≤10 с;
//      наведение подсвечивает соседей (≤50 мс); перетаскивание узла; щелчок открывает заметку;
//      «Вокруг заметки» глубина 1 → 2; группа, фильтр, теги, силы; перезапуск; колонка «Граф»;
//      пустые состояния; телефон.
//   §3 (GRAPH_SCALE=1): 5 000 заметок и ~15 000 связей — время до первого движения, кадры при
//      перетаскивании, покой.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/graph.mjs --opfs-vault --timeout 60000
//   GRAPH_SCALE=1 … --timeout 400000
//
// Снимки: каталог из GRAPH_SHOTS (по умолчанию /tmp), файлы graph-*.png.
const shots = process.env.GRAPH_SHOTS ?? "/tmp";
const scale = process.env.GRAPH_SCALE === "1";

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
  };
`;

const results = {};
const expect = (page, condition, what) => {
  if (!condition) throw new Error("не сошлось: " + what);
  page.log("  ok: " + what);
};
const note = (page, label, value) => {
  results[label] = value;
  page.log(`  замер: ${label} = ${value}`);
};

const GRAPH_BUTTON = "button[title='Граф связей']";
const CANVAS = ".ts-graph__surface";

const pick = async (page, predicate) => {
  await page.evaluate(`(() => {
    const el = [...document.querySelectorAll('button, [role=button]')].find((el) => (${predicate})(el));
    if (!el) throw new Error("нет элемента: " + ${JSON.stringify(String(predicate))});
    el.setAttribute('data-pick', '1');
  })()`);
  await page.click("[data-pick='1']");
  await page.evaluate(`document.querySelector('[data-pick]')?.removeAttribute('data-pick')`);
};

const probe = (page, body) =>
  page.evaluate(`(() => { const c = document.querySelector('${CANVAS}'); const r = c.getBoundingClientRect();
    const p = c.tsProbe; ${body} })()`);

/** Окно-координаты узла по id. */
const nodeXY = (page, id) =>
  probe(page, `const i = p.indexOf(${JSON.stringify(id)}); const [x, y] = p.nodeAt(i); return { i, x: r.left + x, y: r.top + y };`);

const countText = (page) => page.evaluate(`document.querySelector('.ts-graph__count')?.innerText ?? ''`);

const setRange = (page, label, value) =>
  page.evaluate(`(() => {
    const input = [...document.querySelectorAll('.ts-graph__slider')].find((l) => l.innerText.includes(${JSON.stringify(label)})).querySelector('input');
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(input, ${value});
    input.dispatchEvent(new Event('input', { bubbles: true }));
  })()`);

const setText = (page, selector, value) =>
  page.evaluate(`(() => {
    const input = document.querySelector(${JSON.stringify(selector)});
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(input, ${JSON.stringify(value)});
    input.dispatchEvent(new Event('input', { bubbles: true }));
  })()`);

/** Ждёт покоя раскладки и возвращает секунды от `since`. */
const waitSettled = async (page, since, timeout) => {
  await page.waitFor(`document.querySelector('${CANVAS}')?.dataset.moving === 'false'`, { timeout, label: "покой раскладки" });
  return (await page.evaluate(`performance.now()`)) - since;
};

const smallVault = async (page) => {
  const folders = ["проекты", "люди", "встречи", "идеи", "архив"];
  const names = [];
  for (let i = 0; i < 50; i += 1) names.push(`${folders[i % 5]}/заметка-${i}`);
  for (let i = 0; i < 50; i += 1) {
    const links = [(i + 1) % 50, (i * 7 + 3) % 50, (i * 3 + 11) % 50].filter((j) => j !== i && (i % 9 !== 0 || j === (i + 1) % 50));
    // Заметка 49 — без исходящих; «идеи/одинокая» — без связей вовсе.
    const body = links.map((j) => `- [[/${names[j]}]]`).join("\n");
    const tag = i % 4 === 0 ? `#metadata((tags: ("проект",))) <properties>\n` : "";
    await page.evaluate(`window.__opfs.write(${JSON.stringify(names[i] + ".typ")}, ${JSON.stringify(`${tag}= Заметка ${i}\n${body}\n`)})`);
  }
  await page.evaluate(`window.__opfs.write("идеи/одинокая.typ", "= Одинокая\\nНикаких ссылок.\\n")`);
  return names;
};

const bigVault = async (page, n) => {
  // n заметок, ~3 исходящие ссылки на заметку.
  await page.evaluate(`(async () => {
    const n = ${n};
    const name = (i) => 'пап' + (i % 50) + '/н' + i;
    let seed = 12345;
    const rnd = () => (seed = (seed * 1664525 + 1013904223) % 4294967296) / 4294967296;
    for (let i = 0; i < n; i += 1) {
      const links = [];
      for (let k = 0; k < 3; k += 1) {
        // Половина ссылок — на близкие номера (кластеры), половина — случайные.
        const j = k === 0 ? (i + 1) % n : Math.floor(rnd() * n);
        if (j !== i) links.push("- [[/" + name(j) + "]]");
      }
      await window.__opfs.write(name(i) + '.typ', '= Н' + i + '\\n' + links.join('\\n') + '\\n');
    }
  })()`);
};


/** Кадры при перетаскивании узла: rAF страницы и перерисовки холста в секунду. */
const measureDrag = async (page, id, label) => {
  const at = id === "pan" ? await probe(page, "return { x: r.left + 6, y: r.bottom - 6 };") : await nodeXY(page, id);
  await page.evaluate(`(() => { window.__frames = 0; window.__run = true;
    const f = () => { window.__frames += 1; if (window.__run) requestAnimationFrame(f); }; requestAnimationFrame(f);
    window.__draws0 = Number(document.querySelector('${CANVAS}').dataset.draws); window.__ts = performance.now(); })()`);
  await page.mouse("mouseMoved", at.x, at.y);
  await page.mouse("mousePressed", at.x, at.y);
  const start = Date.now();
  let step = 0;
  while (Date.now() - start < 2000) {
    step += 1;
    await page.mouse("mouseMoved", at.x + Math.sin(step / 5) * 120, at.y + Math.cos(step / 5) * 80);
    await page.sleep(8);
  }
  const fps = await page.evaluate(`(() => { const s = (performance.now() - window.__ts) / 1000; window.__run = false;
    return { raf: window.__frames / s, draws: (Number(document.querySelector('${CANVAS}').dataset.draws) - window.__draws0) / s }; })()`);
  await page.mouse("mouseReleased", at.x, at.y);
  note(page, "кадров rAF/с при перетаскивании " + label, fps.raf.toFixed(1));
  note(page, "перерисовок холста/с при перетаскивании " + label, fps.draws.toFixed(1));
};

export default async (page) => {
  // Диагностический зонд холста (`tsProbe`, `data-moving`, `data-draws`, `data-hover-ms`)
  // продукт выставляет только при этом флаге; init-скрипт сохраняет его после перезагрузки.
  const PROBE = "window.__TYPSIDIAN_GRAPH_PROBE = true";
  await page.addInitScript(PROBE);
  await page.evaluate(PROBE);
  await page.evaluate(OPFS + "true");
  let names = [];
  const t0 = Date.now();
  if (scale) await bigVault(page, 5000);
  else names = await smallVault(page);
  page.log(`  vault записан за ${((Date.now() - t0) / 1000).toFixed(1)} с`);

  await page.click("text=Открыть vault");
  await page.waitForElement(scale ? "text=пап0" : "text=проекты", { timeout: 60000 });
  // Пауза после открытия: в режиме масштаба — сколько дать ядру доиндексировать vault
  // (GRAPH_SETTLE_MS), иначе замер «до первого движения» ловит очередь вызовов wasm.
  await page.sleep(scale ? Number(process.env.GRAPH_SETTLE_MS ?? 1000) : 2500);
  // Ждём, пока индекс догонит файлы.
  if (scale) {
    await page.waitFor(`/5\\s?000/.test(document.body.innerText)`, { timeout: 300000, label: "индекс 5 000 заметок" }).catch(() => {});
  }

  // Открытая заметка — центр локального графа.
  if (!scale) {
    await page.evaluate(`document.querySelectorAll('.ts-tree__file > button:first-child').length`);
    await pick(page, (el) => el.querySelector?.('.ts-tree__name')?.textContent === 'заметка-0' || false).catch(async () => {
      await page.click("text=проекты");
      await page.sleep(300);
      await pick(page, (el) => el.querySelector?.('.ts-tree__name')?.textContent === 'заметка-0');
    });
    await page.sleep(600);
  }

  // ── §2/§3: открыть граф, первое движение, покой ─────────────────────────
  await page.evaluate(`(() => {
    window.__t0 = performance.now(); window.__first = null; window.__prev = null;
    const poll = () => {
      const c = document.querySelector('${CANVAS}');
      if (c && window.__canvasAt === undefined) window.__canvasAt = performance.now() - window.__t0;
      if (c && c.tsProbe && window.__first === null) {
        const [x, y] = c.tsProbe.nodeAt(1);
        const key = x + ',' + y;
        if (window.__prev !== null && window.__prev !== key && (x !== 0 || y !== 0)) window.__first = performance.now() - window.__t0;
        window.__prev = key;
      }
      if (window.__first === null) requestAnimationFrame(poll);
    };
    requestAnimationFrame(poll);
  })()`);
  const opened = Date.now();
  await page.click(GRAPH_BUTTON);
  await page.waitForElement(CANVAS, { timeout: 120000 });
  await page.waitFor(`window.__first !== null`, { timeout: 120000, label: "первое движение" });
  note(page, "до первого движения, мс", Math.round(await page.evaluate(`window.__first`)));
  note(page, "до появления холста (граф получен), мс", Math.round(await page.evaluate(`window.__canvasAt`)));
  const settleMs = await waitSettled(page, await page.evaluate(`window.__t0`), 120000);
  note(page, "до покоя, с", (settleMs / 1000).toFixed(1));
  note(page, "счётчик", await countText(page));
  note(page, "rAF/с в покое (холст виден)", await page.evaluate(`new Promise((done) => { let n = 0; const t = performance.now(); const f = () => { n += 1; if (performance.now() - t < 1000) requestAnimationFrame(f); else done((n / ((performance.now() - t) / 1000)).toFixed(1)); }; requestAnimationFrame(f); })`));
  await page.sleep(500);
  await page.shot(`${shots}/graph-${scale ? "scale" : "g1"}.png`);

  if (scale) {
    await measureDrag(page, "пап0/н0.typ", "(5 000)");
    await measureDrag(page, "pan", "сдвиг (5 000)");
    await page.shot(`${shots}/graph-scale-drag.png`);
    // Подсветка.
    const lat = [];
    for (const i of [3, 10, 50, 200, 700]) {
      const p = await nodeXY(page, `пап${i % 50}/н${i}.typ`);
      await page.mouse("mouseMoved", p.x, p.y);
      await page.sleep(120);
      lat.push(Number(await probe(page, `return c.dataset.hoverMs;`)));
    }
    note(page, "задержка подсветки, мс (по узлам)", lat.join(" / "));
    await page.sleep(100);
    console.log("RESULTS " + JSON.stringify(results));
    return;
  }

  // ── Наведение, перетаскивание, щелчок ───────────────────────────────────
  const hub = names[1];
  const target = await nodeXY(page, hub + ".typ");
  expect(page, target.i >= 0, "узел найден в графе");
  const lat = [];
  for (const id of [names[1], names[7], names[20], names[33]]) {
    const p = await nodeXY(page, id + ".typ");
    await page.mouse("mouseMoved", p.x + 40, p.y + 40);
    await page.sleep(80);
    await page.mouse("mouseMoved", p.x, p.y);
    await page.sleep(120);
    lat.push(Number(await probe(page, `return c.dataset.hoverMs;`)));
  }
  note(page, "задержка подсветки, мс", lat.join(" / "));
  expect(page, lat.every((v) => v <= 50), "подсветка ≤50 мс");
  await page.shot(`${shots}/graph-g1-hover.png`);

  // Перетаскивание: узел следует за указателем.
  const before = await nodeXY(page, names[20] + ".typ");
  await page.mouse("mouseMoved", before.x, before.y);
  await page.mouse("mousePressed", before.x, before.y);
  for (let s = 1; s <= 12; s += 1) {
    await page.mouse("mouseMoved", before.x + s * 8, before.y + s * 4);
    await page.sleep(16);
  }
  await page.sleep(150);
  const during = await nodeXY(page, names[20] + ".typ");
  await page.mouse("mouseReleased", before.x + 96, before.y + 48);
  expect(page, Math.hypot(during.x - before.x, during.y - before.y) > 40, "узел сдвинулся вслед за указателем");

  await measureDrag(page, names[20] + ".typ", "(50)");

  // Масштаб кнопками.
  await page.click("[aria-label='Приблизить']");
  await page.sleep(200);
  await page.click("[aria-label='Вписать в экран']");
  await page.sleep(500);

  // Клавиатура: список узлов.
  const listed = await page.evaluate(`document.querySelectorAll('.ts-graph__sr button').length`);
  expect(page, listed >= 50, `список узлов для чтения с экрана (${listed})`);
  await page.evaluate(`document.querySelector('.ts-graph__sr button').focus()`);
  await page.sleep(150);
  await page.shot(`${shots}/graph-focus.png`);

  // ── Группы, фильтр, показ ────────────────────────────────────────────────
  await page.click("text=+ группа");
  await page.sleep(200);
  await setText(page, ".ts-graph__group .ts-graph__input", "папка:проекты");
  await page.sleep(900);
  await page.shot(`${shots}/graph-g1-group.png`);
  const allCount = await countText(page);
  await setText(page, ".ts-graph__panel > .ts-graph__sec .ts-graph__input", "тег:проект");
  await page.sleep(1200);
  const filtered = await countText(page);
  note(page, "счётчик под фильтром тег:проект", `${allCount} -> ${filtered}`);
  expect(page, filtered !== allCount, "фильтр меняет граф");
  await setText(page, ".ts-graph__panel > .ts-graph__sec .ts-graph__input", "");
  await page.sleep(900);
  await pick(page, (el) => el.closest?.('.ts-graph__toggle') === null && false).catch(() => {});
  await page.evaluate(`[...document.querySelectorAll('.ts-graph__toggle')].find((l) => l.innerText.includes('Теги')).querySelector('input').click()`);
  await page.sleep(1000);
  const withTags = await countText(page);
  note(page, "счётчик с тегами", withTags);
  expect(page, withTags !== allCount, "переключатель «Теги» добавляет узлы");
  await page.evaluate(`[...document.querySelectorAll('.ts-graph__toggle')].find((l) => l.innerText.includes('Стрелки')).querySelector('input').click()`);
  await setRange(page, "Отталкивание", 90);
  await setRange(page, "Размер узлов", 80);
  await page.sleep(2500);
  await page.shot(`${shots}/graph-g1-arrows.png`);

  // Сброс — подтверждение внутри интерфейса.
  await page.click("text=Сбросить");
  await page.waitForElement("[role='alertdialog']");
  await page.shot(`${shots}/graph-reset-confirm.png`);
  await pick(page, (el) => el.textContent === 'Отмена');
  await page.sleep(200);

  // Запись в vault.json и перезапуск.
  await page.sleep(1200);
  const json = await page.evaluate(`window.__opfs.read(".typsidian/vault.json")`);
  expect(page, json.includes("graph") && json.includes("show_tags"), "настройки графа записаны в vault.json");
  await page.reload();
  await page.click("text=Открыть vault").catch(() => {});
  await page.waitForElement("text=проекты", { timeout: 30000 });
  await page.sleep(2000);
  await page.click(GRAPH_BUTTON);
  await page.waitForElement(CANVAS);
  const restored = await page.evaluate(`[...document.querySelectorAll('.ts-graph__toggle')].find((l) => l.innerText.includes('Теги')).querySelector('input').checked`);
  expect(page, restored === true, "после перезапуска «Теги» остались включены");

  // Вернуть умолчания: подтверждённый сброс.
  await page.click("text=Сбросить");
  await pick(page, (el) => el.textContent === 'Сбросить' && el.closest('[role=alertdialog]') !== null);
  await page.sleep(500);
  const afterReset = await page.evaluate(`[...document.querySelectorAll('.ts-graph__toggle')].find((l) => l.innerText.includes('Теги')).querySelector('input').checked`);
  expect(page, afterReset === false, "сброс вернул умолчания");

  // ── Локальный граф (G2) ──────────────────────────────────────────────────
  await pick(page, (el) => el.textContent === 'Вокруг заметки');
  await page.sleep(1500);
  // Открытая заметка после перезапуска может отсутствовать: открываем её из дерева.
  const hasDepth = await page.evaluate(`Boolean(document.querySelector('.ts-graph__depth'))`);
  expect(page, hasDepth, "выбор глубины виден в режиме «Вокруг заметки»");
  await pick(page, (el) => el.getAttribute('aria-label') === 'Глубина 1');
  await page.sleep(1200);
  const d1 = await countText(page);
  await pick(page, (el) => el.getAttribute('aria-label') === 'Глубина 2');
  await page.sleep(1500);
  const d2 = await countText(page);
  note(page, "локальный граф глубина 1 -> 2", `${d1} -> ${d2}`);
  await page.shot(`${shots}/graph-g2.png`);

  // ── Колонка заметки (G3) и щелчок по узлу ───────────────────────────────
  await page.click("text=Workspace");
  await page.sleep(600);
  await page.click("text=Панель заметки");
  await page.waitForElement(".ts-notepanel");
  await pick(page, (el) => el.textContent === 'Граф' && el.closest('.ts-notepanel') !== null);
  await page.sleep(1500);
  await page.waitForElement(".ts-graph-mode canvas");
  await page.sleep(2500);
  await page.shot(`${shots}/graph-g3.png`);
  await pick(page, (el) => el.textContent === 'Открыть весь граф');
  await page.waitForElement(`${CANVAS}`);
  const localKept = await page.evaluate(`[...document.querySelectorAll('.ts-graph__seg button')].find((b) => b.getAttribute('aria-pressed') === 'true').textContent`);
  expect(page, localKept === "Вокруг заметки", "«Открыть весь граф» ведёт на экран в режиме «Вокруг заметки»");
  await page.sleep(1200);
  await pick(page, (el) => el.textContent === 'Весь vault');
  await page.sleep(2500);
  const click = await nodeXY(page, names[5] + ".typ");
  await page.mouse("mouseMoved", click.x, click.y);
  await page.mouse("mousePressed", click.x, click.y);
  await page.mouse("mouseReleased", click.x, click.y);
  await page.waitFor(`!document.querySelector('${CANVAS}')`, { label: "щелчок по узлу открыл заметку и закрыл граф" });
  expect(page, true, "щелчок по узлу открыл заметку");

  // ── Пустое состояние (G4) ────────────────────────────────────────────────
  await pick(page, (el) => el.querySelector?.('.ts-tree__name')?.textContent === 'одинокая').catch(async () => {
    await page.click("text=идеи");
    await page.sleep(300);
    await pick(page, (el) => el.querySelector?.('.ts-tree__name')?.textContent === 'одинокая');
  });
  await page.sleep(600);
  await page.click(GRAPH_BUTTON);
  await page.waitForElement(CANVAS);
  await pick(page, (el) => el.textContent === 'Вокруг заметки');
  await page.waitFor(`document.querySelector('.ts-graph__empty')?.innerText.includes('пока нет связей')`, { label: "пустое состояние G4" });
  await page.shot(`${shots}/graph-g4.png`);

  // ── Телефон (G5) ─────────────────────────────────────────────────────────
  await pick(page, (el) => el.textContent === 'Весь vault');
  await page.sleep(2500);
  await page.viewport(390, 800);
  await page.sleep(800);
  await page.click(".ts-graph__sheet-toggle");
  await page.sleep(600);
  await page.shot(`${shots}/graph-g5.png`);
  const overflow = await page.evaluate(`document.documentElement.scrollWidth > window.innerWidth`);
  expect(page, overflow === false, "на 390 px нет горизонтальной прокрутки");

  console.log("RESULTS " + JSON.stringify(results));
};
