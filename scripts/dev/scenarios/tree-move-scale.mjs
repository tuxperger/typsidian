// Перенос в большом дереве (фича 023, SC-006): при 5 000 заметок подсветка
// цели успевает за указателем — от движения до смены подсвеченной папки
// проходит не больше одного кадра. Затем — прерывание переноса большой папки.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/tree-move-scale.mjs --opfs-vault --timeout 120000
//
// `MOVE_SCALE_NOTES=1000` — другой размер vault.
const FRAME_MS = 1000 / 60;
const FOLDERS = 50;
const NEAR = 10;
const row = (path) => `[data-drag-path=${JSON.stringify(path)}] > [data-drag-handle]`;

export default async (page) => {
  const notes = Number(process.env.MOVE_SCALE_NOTES ?? 5000);
  const perFolder = Math.ceil(notes / FOLDERS);

  await page.evaluate(`(async () => {
    const root = await navigator.storage.getDirectory();
    const vault = await root.getDirectoryHandle("vault", { create: true });
    const write = async (dir, name, text) => {
      const file = await dir.getFileHandle(name, { create: true });
      const stream = await file.createWritable();
      await stream.write(text);
      await stream.close();
    };
    // Малые папки идут в дереве первыми и видны без прокрутки — по ним ходит указатель.
    for (let f = 1; f <= ${NEAR}; f += 1) {
      const dir = await vault.getDirectoryHandle("А-" + String(f).padStart(2, "0"), { create: true });
      await write(dir, "заметка.typ", "= Заметка\\n");
    }
    for (let f = 1; f <= ${FOLDERS}; f += 1) {
      const dir = await vault.getDirectoryHandle("Папка-" + String(f).padStart(2, "0"), { create: true });
      for (let n = 1; n <= ${perFolder}; n += 1) {
        await write(dir, "заметка-" + String(n).padStart(3, "0") + ".typ", "= Заметка " + n + "\\n");
      }
    }
  })()`);

  await page.click("text=Открыть vault");
  const source = "А-01/заметка.typ";
  await page.waitForElement(row(source), { timeout: 120000 });
  const rows = await page.evaluate(`document.querySelectorAll("[data-drag-path]").length`);
  page.log(`строк в дереве: ${rows}`);

  // Замер внутри страницы: время от `pointermove` до правки DOM, которой
  // подсветка переходит на другую папку. Наблюдатель срабатывает сразу после
  // обработчика, так что в замер входит вся работа жеста на одно движение.
  await page.evaluate(`(() => {
    const samples = [];
    let movedAt = null;
    window.addEventListener("pointermove", () => { movedAt = performance.now(); }, true);
    new MutationObserver((records) => {
      const lit = records.some((r) => r.target.classList?.contains("ts-tree__folder--drop"));
      if (lit && movedAt !== null) {
        samples.push(performance.now() - movedAt);
        movedAt = null;
      }
    }).observe(document.querySelector(".ts-tree"), { attributes: true, attributeFilter: ["class"], subtree: true });
    window.__moveSamples = samples;
  })()`);

  const from = await page.center(row(source));
  await page.mouse("mousePressed", from.x, from.y);
  await page.mouse("mouseMoved", from.x + 12, from.y + 12);
  await page.waitForElement(".ts-tree-drag");
  await page.sleep(500);

  const points = await page.evaluate(`(() => {
    const limit = window.innerHeight;
    return [...document.querySelectorAll('[data-drag-kind="folder"] > [data-drag-handle]')]
      .filter((el) => el.parentElement.dataset.dragPath !== "А-01")
      .map((el) => el.getBoundingClientRect())
      .filter((r) => r.top > 0 && r.bottom < limit)
      .map((r) => ({ x: r.x + r.width / 2, y: r.y + r.height / 2 }));
  })()`);
  if (points.length < 2) throw new Error(`видимых папок для замера: ${points.length} — нужно хотя бы две`);
  for (let pass = 0; pass < 5; pass += 1) {
    for (const point of points) {
      await page.mouse("mouseMoved", point.x, point.y);
      await page.sleep(40);
    }
  }
  await page.press("Escape");
  await page.mouse("mouseReleased", from.x, from.y);

  const samples = await page.evaluate(`window.__moveSamples`);
  if (samples.length === 0) throw new Error("подсветка цели ни разу не сменилась");
  const sorted = [...samples].sort((a, b) => a - b);
  const median = sorted[Math.floor(sorted.length / 2)];
  const worst = sorted.at(-1);
  page.log(
    `смен цели: ${samples.length}; медиана ${median.toFixed(2)} мс, худшая ${worst.toFixed(2)} мс (кадр — ${FRAME_MS.toFixed(1)} мс)`,
  );
  if (worst > FRAME_MS) throw new Error(`SC-006: смена подсветки заняла ${worst.toFixed(2)} мс — дольше кадра`);

  // Перенос большой папки прерывается на середине: vault остаётся прежним
  // (принцип X, research.md §6). Кнопку ловит сама страница — копирование
  // сотни файлов короче паузы между командами сценария.
  await page.evaluate(`(() => {
    window.__moveCancelled = false;
    const tick = () => {
      const button = document.querySelector(".ts-tree-progress button");
      const done = Number(document.querySelector(".ts-tree-progress progress")?.value ?? 0);
      if (button && done >= 10) { button.click(); window.__moveCancelled = true; return; }
      requestAnimationFrame(tick);
    };
    tick();
  })()`);
  await page.click('[data-drag-path="Папка-01"] > button[aria-haspopup="menu"]');
  await page.click("text=Переместить в…");
  await page.waitForElement("#ts-move-list");
  await page.type("А-02");
  await page.waitFor(
    `document.querySelector("#ts-move-list [role=option]")?.textContent.startsWith("А-02") === true`,
    { label: "папка А-02 в списке" },
  );
  await page.press("Enter");
  await page.waitFor("window.__moveCancelled === true", { label: "кнопка прерывания нажата", timeout: 60000 });
  await page.waitFor('!document.querySelector(".ts-tree-progress")', { label: "перенос остановлен", timeout: 60000 });
  const left = await page.evaluate(`(async () => {
    const root = await navigator.storage.getDirectory();
    const vault = await root.getDirectoryHandle("vault");
    const count = async (dir) => { let n = 0; for await (const _ of dir.keys()) n += 1; return n; };
    const target = await vault.getDirectoryHandle("А-02");
    let copied = true;
    try { await target.getDirectoryHandle("Папка-01"); } catch { copied = false; }
    return { source: await count(await vault.getDirectoryHandle("Папка-01")), copied, notice: document.querySelector(".ts-tree-notice")?.textContent ?? null };
  })()`);
  page.log(`после прерывания: в источнике ${left.source} файлов, копия в цели: ${left.copied}, сообщение: ${left.notice}`);
  if (left.source !== perFolder || left.copied) throw new Error("прерванный перенос изменил vault");
};
