#!/usr/bin/env node
// Управление headless-браузером для проверки web-версии: открыть страницу,
// пройти сценарий, собрать консоль и исключения, снять экран.
//
// Без зависимостей: Chrome DevTools Protocol поверх встроенного в Node 22
// WebSocket и chromium из dev-оболочки (`CHROME_PATH`). Playwright и
// Puppeteer здесь не годятся как зависимость по умолчанию: их собственные
// браузеры на NixOS не стартуют, а новая запись в `pnpm-lock.yaml` меняет хэш
// зависимостей nix-сборки.
//
//   nix develop -c node scripts/dev/browser.mjs URL [сценарий.mjs] [--shot файл.png]
//       [--opfs-vault] [--headed] [--timeout мс]
//
// Сценарий — модуль с `export default async (page) => { … }`; без сценария
// страница просто открывается и ждёт простоя сети. Методы `page` — ниже, в
// `createPage`. Код возврата 1 — в консоли были ошибки или сценарий упал.
//
// `--opfs-vault` подменяет системный выбор папки (`showDirectoryPicker`) на
// приватное хранилище браузера: диалог ОС в headless не открыть, а OPFS даёт
// тот же `FileSystemDirectoryHandle`. Каждый запуск — чистый профиль.

import { spawn } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const args = process.argv.slice(2);
const flag = (name) => {
  const at = args.indexOf(name);
  if (at === -1) return false;
  args.splice(at, 1);
  return true;
};
const option = (name) => {
  const at = args.indexOf(name);
  if (at === -1) return undefined;
  const [, value] = args.splice(at, 2);
  return value;
};

const shotPath = option("--shot");
const timeoutMs = Number(option("--timeout") ?? 15000);
const opfsVault = flag("--opfs-vault");
const headed = flag("--headed");
const [url, scenarioPath] = args;

if (!url) {
  console.error("browser.mjs: нужен URL. Справка — в начале файла.");
  process.exit(2);
}

const chrome = process.env.CHROME_PATH ?? "chromium";
const profile = mkdtempSync(join(tmpdir(), "typsidian-browser-"));

const OPFS_VAULT = `
  window.showDirectoryPicker = async () => {
    const root = await navigator.storage.getDirectory();
    return root.getDirectoryHandle("vault", { create: true });
  };
`;

/** Запускает chromium и отдаёт адрес его отладочного сокета. */
function launch() {
  const child = spawn(
    chrome,
    [
      ...(headed ? [] : ["--headless=new"]),
      "--remote-debugging-port=0",
      `--user-data-dir=${profile}`,
      "--no-first-run",
      "--no-default-browser-check",
      "--disable-gpu",
      "--window-size=1440,900",
      "about:blank",
    ],
    { stdio: ["ignore", "ignore", "pipe"] },
  );
  const endpoint = new Promise((resolveEndpoint, reject) => {
    let buffer = "";
    child.stderr.on("data", (chunk) => {
      buffer += chunk;
      const match = buffer.match(/DevTools listening on (ws:\/\/\S+)/);
      if (match) resolveEndpoint(match[1]);
    });
    child.on("exit", (code) => reject(new Error(`chromium завершился (${code}): ${buffer}`)));
    setTimeout(() => reject(new Error("chromium не открыл отладочный порт")), 20000);
  });
  return { child, endpoint };
}

/** Соединение CDP: `send` — команда, `on` — подписка на событие. */
async function connect(endpoint) {
  const socket = new WebSocket(endpoint);
  await new Promise((done, fail) => {
    socket.addEventListener("open", done, { once: true });
    socket.addEventListener("error", fail, { once: true });
  });
  let nextId = 0;
  const pending = new Map();
  const listeners = new Map();
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    if (message.id !== undefined) {
      const waiter = pending.get(message.id);
      pending.delete(message.id);
      if (message.error) waiter?.reject(new Error(message.error.message));
      else waiter?.resolve(message.result);
      return;
    }
    for (const listener of listeners.get(message.method) ?? []) listener(message.params);
  });
  return {
    send(method, params = {}, sessionId) {
      const id = ++nextId;
      socket.send(JSON.stringify({ id, method, params, sessionId }));
      return new Promise((resolveSend, reject) => pending.set(id, { resolve: resolveSend, reject }));
    },
    on(method, listener) {
      listeners.set(method, [...(listeners.get(method) ?? []), listener]);
    },
    close: () => socket.close(),
  };
}

const sleep = (ms) => new Promise((done) => setTimeout(done, ms));

function createPage(cdp, sessionId, log) {
  const send = (method, params) => cdp.send(method, params, sessionId);

  /** Выполняет выражение на странице и возвращает его значение (JSON). */
  async function evaluate(expression) {
    const { result, exceptionDetails } = await send("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      returnByValue: true,
      userGesture: true,
    });
    if (exceptionDetails) {
      throw new Error(exceptionDetails.exception?.description ?? exceptionDetails.text);
    }
    return result.value;
  }

  /** Ждёт, пока выражение не станет истинным; возвращает его значение. */
  async function waitFor(expression, { timeout = timeoutMs, label = expression } = {}) {
    const deadline = Date.now() + timeout;
    for (;;) {
      const value = await evaluate(expression).catch(() => undefined);
      if (value) return value;
      if (Date.now() > deadline) throw new Error(`не дождался: ${label}`);
      await sleep(100);
    }
  }

  // Поиск элемента: CSS-селектор либо `text=…` — кнопка, ссылка, пункт или
  // вкладка с таким видимым текстом или подписью (первое точное совпадение,
  // иначе первое вхождение).
  const locate = (target) => {
    if (!target.startsWith("text=")) return `document.querySelector(${JSON.stringify(target)})`;
    const text = JSON.stringify(target.slice(5));
    return `(() => {
      const wanted = ${text};
      const label = (el) => (el.getAttribute("aria-label") ?? el.textContent ?? "").trim();
      const all = [...document.querySelectorAll(
        "button, a, [role=button], [role=menuitem], [role=tab], [role=option], summary, label"
      )].filter((el) => el.getClientRects().length > 0);
      return all.find((el) => label(el) === wanted) ?? all.find((el) => label(el).includes(wanted));
    })()`;
  };

  return {
    evaluate,
    waitFor,
    sleep,
    /** Ждёт появления элемента. */
    waitForElement: (target, options) =>
      waitFor(`Boolean(${locate(target)})`, { label: target, ...options }),
    /** Клик настоящими событиями мыши по центру элемента. */
    async click(target) {
      await waitFor(`Boolean(${locate(target)})`, { label: target });
      const box = await evaluate(`(() => {
        const el = ${locate(target)};
        el.scrollIntoView({ block: "center", inline: "center" });
        const r = el.getBoundingClientRect();
        return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
      })()`);
      for (const type of ["mousePressed", "mouseReleased"]) {
        await send("Input.dispatchMouseEvent", { type, ...box, button: "left", clickCount: 1 });
      }
    },
    /** Ввод текста в элемент с фокусом — как с клавиатуры. */
    type: (text) => send("Input.insertText", { text }),
    /**
     * Клавиша: "Escape", "Enter"; модификаторы — { ctrl, alt, shift, meta }.
     * `code` — имя физической клавиши (`"Period"`, `"BracketLeft"`): для знаков и
     * для проверки другой раскладки (`key: "ю", code: "Period"`).
     */
    async press(key, { ctrl, alt, shift, meta, code: physical } = {}) {
      const modifiers = (alt ? 1 : 0) | (ctrl ? 2 : 0) | (meta ? 4 : 0) | (shift ? 8 : 0);
      const code = physical ?? (key.length === 1 ? `Key${key.toUpperCase()}` : key);
      const named = { Enter: 13, Escape: 27, Tab: 9, Backspace: 8, Delete: 46, ArrowLeft: 37, ArrowRight: 39, ArrowUp: 38, ArrowDown: 40, End: 35, Home: 36, PageDown: 34, PageUp: 33 };
      const byCode = { Period: 190, Comma: 188, Backslash: 220, BracketLeft: 219, BracketRight: 221 };
      const windowsVirtualKeyCode = byCode[code] ?? named[key] ?? key.toUpperCase().charCodeAt(0);
      for (const type of ["rawKeyDown", "keyUp"]) {
        await send("Input.dispatchKeyEvent", { type, key, code, modifiers, windowsVirtualKeyCode });
      }
    },
    /** Скрипт, выполняемый в каждом новом документе (до кода страницы); действует после `reload`. */
    addInitScript: (source) => send("Page.addScriptToEvaluateOnNewDocument", { source }),
    /** Перезагрузка страницы и ожидание её готовности. */
    async reload() {
      await send("Page.reload");
      await sleep(300);
      await waitFor(`document.readyState === "complete"`, { label: "перезагрузка страницы" });
    },
    /**
     * Мышь: `type` — "mouseMoved" | "mousePressed" | "mouseReleased"; перетаскивание из шагов.
     * Движение идёт с нажатой левой кнопкой: с `button: "none"` chromium снимает
     * захват указателя (`setPointerCapture`) на первом же шаге.
     */
    mouse: (type, x, y) =>
      send("Input.dispatchMouseEvent", { type, x, y, button: "left", buttons: type === "mouseReleased" ? 0 : 1, clickCount: type === "mouseMoved" ? 0 : 1 }),
    /** Центр элемента в координатах окна — для `mouse`. */
    center: (target) =>
      evaluate(`(() => {
        const el = ${locate(target)};
        const r = el.getBoundingClientRect();
        return { x: r.x + r.width / 2, y: r.y + r.height / 2, left: r.left, right: r.right, top: r.top, bottom: r.bottom };
      })()`),
    /** Видимый текст страницы или элемента. */
    text: (selector = "body") =>
      evaluate(`document.querySelector(${JSON.stringify(selector)})?.innerText ?? null`),
    /** Размер окна страницы (эмуляция метрик): проверка узкого вида. */
    viewport: (width, height) =>
      send("Emulation.setDeviceMetricsOverride", {
        width,
        height,
        deviceScaleFactor: 1,
        mobile: false,
      }),
    /** Снимок экрана в PNG. */
    async shot(path) {
      const { data } = await send("Page.captureScreenshot", { format: "png" });
      writeFileSync(path, Buffer.from(data, "base64"));
      log(`снимок: ${path}`);
    },
    log,
  };
}

const problems = [];
const log = (line) => console.log(line);
// Страница нужна и после падения сценария: снимок места, где он остановился,
// говорит о причине больше, чем текст ошибки.
let openPage = null;
const { child, endpoint } = launch();
let failed = false;

try {
  const cdp = await connect(await endpoint);
  const { targetId } = await cdp.send("Target.createTarget", { url: "about:blank" });
  const { sessionId } = await cdp.send("Target.attachToTarget", { targetId, flatten: true });
  const send = (method, params) => cdp.send(method, params, sessionId);

  cdp.on("Runtime.consoleAPICalled", ({ type, args: values }) => {
    const text = values.map((value) => value.value ?? value.description ?? "").join(" ");
    if (type === "error" || type === "warning") problems.push(`console.${type}: ${text}`);
    log(`[${type}] ${text}`);
  });
  cdp.on("Runtime.exceptionThrown", ({ exceptionDetails }) => {
    const text = exceptionDetails.exception?.description ?? exceptionDetails.text;
    problems.push(`исключение: ${text}`);
    log(`[исключение] ${text}`);
  });
  cdp.on("Network.loadingFailed", ({ errorText, canceled }) => {
    if (!canceled) log(`[сеть] ${errorText}`);
  });

  await send("Runtime.enable");
  await send("Page.enable");
  // Диалог «покинуть страницу» (beforeunload) в headless не закрыть руками, а он
  // замораживает страницу: сценарий с перезагрузкой принимает его сам.
  cdp.on("Page.javascriptDialogOpening", () => {
    void send("Page.handleJavaScriptDialog", { accept: true });
  });
  await send("Network.enable");
  if (opfsVault) await send("Page.addScriptToEvaluateOnNewDocument", { source: OPFS_VAULT });

  const page = createPage(cdp, sessionId, log);
  openPage = page;
  await send("Page.navigate", { url });
  await page.waitFor(`document.readyState === "complete"`, { label: "загрузка страницы" });

  if (scenarioPath) {
    const scenario = await import(pathToFileURL(resolve(scenarioPath)).href);
    await scenario.default(page);
  } else {
    await sleep(1500);
  }
  if (shotPath) await page.shot(shotPath);
  cdp.close();
} catch (cause) {
  failed = true;
  console.error(`browser.mjs: ${cause instanceof Error ? (cause.stack ?? cause.message) : cause}`);
  if (shotPath && openPage) await openPage.shot(shotPath).catch(() => undefined);
} finally {
  child.kill();
  await sleep(200);
  rmSync(profile, { recursive: true, force: true });
}

const errors = problems.filter((line) => !line.startsWith("console.warning"));
if (errors.length > 0) {
  console.error(`\nОшибок на странице: ${errors.length}`);
  for (const line of errors) console.error(`  ${line}`);
}
process.exit(failed || errors.length > 0 ? 1 : 0);
