// Перенос в дереве файлов (фича 023): жест, отмена, открытая заметка с
// несохранённым текстом, недопустимая цель, занятое имя, предупреждение о
// ссылках, «Переместить в…» с клавиатуры, Escape во время жеста.
//
//   nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
//       scripts/dev/scenarios/tree-move.mjs --opfs-vault --shot /tmp/tree-move.png
//
// `MOVE_SHOTS=/каталог` — снимки шагов для сверки с макетом.
const row = (path) => `[data-drag-path=${JSON.stringify(path)}] > [data-drag-handle]`;

export default async (page) => {
  const shots = process.env.MOVE_SHOTS;
  const shot = async (name) => {
    if (shots) await page.shot(`${shots}/${name}.png`);
  };
  const failures = [];
  const check = (label, ok) => {
    page.log(`${ok ? "✓" : "✗"} ${label}`);
    if (!ok) failures.push(label);
  };

  await page.evaluate(`(async () => {
    const root = await navigator.storage.getDirectory();
    const vault = await root.getDirectoryHandle("vault", { create: true });
    const write = async (path, text) => {
      const parts = path.split("/");
      let dir = vault;
      for (const part of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(part, { create: true });
      const file = await dir.getFileHandle(parts.at(-1), { create: true });
      const stream = await file.createWritable();
      await stream.write(text);
      await stream.close();
    };
    await write("Курсовая/Главы/01-введение.typ", "= Введение\\n");
    await write("Курсовая/Главы/02-обзор.typ", "= Обзор\\nСм. [[/Лекции/Матанализ/лекция-01]].\\n");
    await write("Курсовая/план.typ", "= План курсовой\\n");
    await write("Лекции/Матанализ/лекция-01.typ", "= Лекция 1\\n");
    await write("Лекции/Матанализ/схема.png", "не картинка, но файл");
    await write("Лекции/Физика/лекция-01.typ", "= Физика 1\\n");
    await write("Входящие/идея.typ", "= Идея\\n");
    await write("Входящие/черновик.typ", "= Черновик\\n");
    await write("читать.typ", "= Читать\\n");
    await write("план.typ", "= План\\n");
  })()`);

  /** Есть ли файл в vault на диске. */
  const exists = (path) =>
    page.evaluate(`(async () => {
      const root = await navigator.storage.getDirectory();
      let dir = await root.getDirectoryHandle("vault");
      const parts = ${JSON.stringify(path)}.split("/");
      try {
        for (const part of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(part);
        await dir.getFileHandle(parts.at(-1));
        return true;
      } catch { return false; }
    })()`);
  const read = (path) =>
    page.evaluate(`(async () => {
      const root = await navigator.storage.getDirectory();
      let dir = await root.getDirectoryHandle("vault");
      const parts = ${JSON.stringify(path)}.split("/");
      for (const part of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(part);
      return (await (await dir.getFileHandle(parts.at(-1))).getFile()).text();
    })()`);

  /** Жест мышью: взять строку, провести к цели, по желанию отпустить. */
  const drag = async (from, to, { release = true, hold = 0 } = {}) => {
    const a = await page.center(from);
    const b = typeof to === "string" ? await page.center(to) : to;
    await page.mouse("mousePressed", a.x, a.y);
    const steps = 8;
    for (let i = 1; i <= steps; i += 1) {
      await page.mouse("mouseMoved", a.x + ((b.x - a.x) * i) / steps, a.y + ((b.y - a.y) * i) / steps);
      await page.sleep(20);
    }
    await page.sleep(150 + hold);
    if (release) {
      await page.mouse("mouseReleased", b.x, b.y);
      await page.sleep(700);
    }
    return b;
  };
  const noticeText = () => page.text(".ts-tree-notice");

  await page.click("text=Открыть vault");
  await page.waitForElement(row("Входящие/идея.typ"), { timeout: 30000 });

  // 1. Заметка → папка: подсветка цели, ярлык, итог и сообщение.
  const over = await drag(row("Входящие/идея.typ"), row("Курсовая/Главы"), { release: false });
  check(
    "1: цель подсвечена, ярлык показан",
    await page.evaluate(
      `Boolean(document.querySelector('[data-drag-path="Курсовая/Главы"].ts-tree__folder--drop')) && Boolean(document.querySelector(".ts-tree-drag"))`,
    ),
  );
  await shot("1-drag-over-folder");
  await page.mouse("mouseReleased", over.x, over.y);
  await page.waitForElement(row("Курсовая/Главы/идея.typ"));
  check("1: файл на диске в новом месте", await exists("Курсовая/Главы/идея.typ"));
  check("1: на прежнем месте файла нет", !(await exists("Входящие/идея.typ")));
  check("1: сообщение об итоге", ((await noticeText()) ?? "").includes("Перенесено в Курсовая/Главы"));
  await shot("1-done");

  // 2. Отмена возвращает на место.
  await page.click("text=Отменить");
  await page.waitForElement(row("Входящие/идея.typ"));
  check("2: отмена вернула файл", (await exists("Входящие/идея.typ")) && !(await exists("Курсовая/Главы/идея.typ")));

  // 3. Открытая заметка с набранным текстом остаётся открытой, текст цел.
  await page.click(row("Входящие/черновик.typ"));
  await page.waitForElement(".cm-content");
  await page.sleep(500);
  await page.click(".cm-content");
  await page.press("End", { ctrl: true });
  await page.type("несохранённая строка");
  await drag(row("Входящие/черновик.typ"), row("Лекции/Физика"));
  await page.waitForElement(row("Лекции/Физика/черновик.typ"));
  check(
    "3: редактор показывает набранный текст",
    ((await page.text(".cm-content")) ?? "").includes("несохранённая строка"),
  );
  check(
    "3: строка под новым путём выделена как открытая",
    await page.evaluate(
      `document.querySelector('[data-drag-path="Лекции/Физика/черновик.typ"] > [data-drag-handle]')?.getAttribute("aria-current") === "true"`,
    ),
  );
  await page.sleep(1500);
  check(
    "3: текст дошёл до файла на новом месте",
    (await read("Лекции/Физика/черновик.typ")).includes("несохранённая строка"),
  );
  check("3: на старом месте файл не воссоздан", !(await exists("Входящие/черновик.typ")));

  // 4. Папка над собственной вложенной: подсветки нет, бросок ничего не меняет.
  const inner = await drag(row("Лекции"), row("Лекции/Матанализ"), { release: false });
  check(
    "4: недопустимая цель не подсвечена, ярлык приглушён",
    await page.evaluate(
      `!document.querySelector(".ts-tree__folder--drop, .ts-tree--drop-root") && Boolean(document.querySelector(".ts-tree-drag--denied"))`,
    ),
  );
  await shot("4-denied");
  await page.mouse("mouseReleased", inner.x, inner.y);
  await page.sleep(500);
  check("4: папка на месте", await exists("Лекции/Матанализ/лекция-01.typ"));

  // 5. Занятое имя.
  await drag(row("план.typ"), row("Курсовая"));
  check("5: сообщение о занятом имени", ((await noticeText()) ?? "").includes("уже есть"));
  check("5: оба файла на месте", (await exists("план.typ")) && (await exists("Курсовая/план.typ")));
  await shot("5-name-conflict");
  await page.click(".ts-tree-notice button");

  // 6. Перенос, разрывающий ссылку: диалог, «Отмена» ничего не меняет.
  await drag(row("Лекции/Матанализ/лекция-01.typ"), row("Входящие"));
  await page.waitForElement(".ts-dialog");
  check("6: в диалоге названа ссылающаяся заметка", ((await page.text(".ts-dialog")) ?? "").includes("02-обзор.typ"));
  await shot("6-links-dialog");
  await page.click("text=Отмена");
  await page.sleep(400);
  check("6: после отказа заметка на месте", await exists("Лекции/Матанализ/лекция-01.typ"));

  // 7. Папка целиком, с невидимым файлом, — затем «Переместить в…» с клавиатуры.
  await drag(row("Лекции/Матанализ"), row("Курсовая"));
  await page.waitForElement(".ts-dialog");
  await page.click("text=Перенести");
  await page.waitForElement(row("Курсовая/Матанализ/лекция-01.typ"));
  check(
    "7: папка переехала вместе с невидимым файлом",
    (await exists("Курсовая/Матанализ/схема.png")) && !(await exists("Лекции/Матанализ/схема.png")),
  );

  await page.click('[data-drag-path="читать.typ"] > button[aria-haspopup="menu"]');
  await page.click("text=Переместить в…");
  await page.waitForElement("#ts-move-list");
  await page.type("гл");
  await page.sleep(500);
  page.log("— «Переместить в…»:\n" + (await page.text(".ts-palette")));
  await shot("7-move-to");
  await page.press("Enter");
  await page.waitForElement(row("Курсовая/Главы/читать.typ"));
  check("7: «Переместить в…» перенесло заметку", await exists("Курсовая/Главы/читать.typ"));

  // 8. Escape во время жеста — ничего не перенесено.
  await drag(row("Входящие/идея.typ"), row("Курсовая"), { release: false });
  await page.press("Escape");
  await page.sleep(300);
  check("8: Escape убрал ярлык", !(await page.evaluate(`Boolean(document.querySelector(".ts-tree-drag"))`)));
  const c = await page.center(row("Курсовая"));
  await page.mouse("mouseReleased", c.x, c.y);
  await page.sleep(400);
  check("8: заметка на месте", await exists("Входящие/идея.typ"));

  page.log("— дерево в конце:\n" + (await page.text(".ts-tree")));
  if (failures.length > 0) throw new Error(`не прошли проверки: ${failures.join("; ")}`);
};
