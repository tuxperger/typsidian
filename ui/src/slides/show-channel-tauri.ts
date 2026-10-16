import { emitTo, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow, WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { availableMonitors, currentMonitor, type Monitor } from "@tauri-apps/api/window";
import {
  newShowSessionId,
  type HostMessage,
  type ShowHost,
  type ShowRole,
  type ShowViewer,
  type ViewerMessage,
} from "./show-channel.js";

// Канал показа для рабочего стола (фича 019, research.md §8): окна — `WebviewWindow`
// с точкой входа `show.html`, сообщения — события Tauri `emitTo`/`listen`.
// `BroadcastChannel` между вебвью Tauri не гарантирован, поэтому он здесь не
// используется. Права на создание окон, события и полный экран — в
// `apps/desktop/src-tauri/capabilities/default.json`.

/** Метка окна редактора — единственного окна рабочей области. */
const HOST_LABEL = "main";

const hostEvent = (session: string): string => `typsidian-show:${session}:host`;
const viewerEvent = (session: string): string => `typsidian-show:${session}:viewer`;

/** Метка окна совпадает с ролью: `show` и `presenter` объявлены в capabilities. */
const labelOf = (role: ShowRole): string => role;

function sameMonitor(a: Monitor, b: Monitor): boolean {
  return a.position.x === b.position.x && a.position.y === b.position.y;
}

/**
 * Куда поставить окно роли. Показ уходит на монитор, отличный от текущего (на
 * проектор), если он есть; вид докладчика остаётся на текущем. Монитор один —
 * позицию решает ОС. Возвращает логические координаты верхнего левого угла.
 */
async function placementFor(role: ShowRole): Promise<{ x: number; y: number } | null> {
  const [monitors, current] = await Promise.all([availableMonitors(), currentMonitor()]);
  if (monitors.length < 2 || current === null) return null;
  const target =
    role === "show" ? (monitors.find((monitor) => !sameMonitor(monitor, current)) ?? current) : current;
  const origin = target.position.toLogical(target.scaleFactor);
  // Небольшой сдвиг от угла: окно вида докладчика не прячет заголовок за краем.
  const inset = role === "show" ? 0 : 80;
  return { x: origin.x + inset, y: origin.y + inset };
}

/** Хозяин в окне редактора рабочего стола. */
export function createTauriShowHost(): ShowHost {
  const session = newShowSessionId();
  const windows = new Map<ShowRole, WebviewWindow>();
  const subscriptions = new Set<Promise<UnlistenFn>>();

  return {
    supports: { windows: true, presenter: true },
    open: async (role) => {
      const existing = await WebviewWindow.getByLabel(labelOf(role));
      if (existing !== null) {
        windows.set(role, existing);
        await existing.setFocus().catch(() => undefined);
        return true;
      }
      const place = await placementFor(role).catch(() => null);
      const url = `show.html?session=${session}&role=${role}`;
      const created = new WebviewWindow(labelOf(role), {
        url,
        title: "Typsidian",
        ...(place === null ? {} : { x: place.x, y: place.y }),
      });
      return await new Promise<boolean>((resolve) => {
        void created.once("tauri://created", () => {
          windows.set(role, created);
          // Показ — на весь экран монитора, на котором он оказался.
          if (role === "show") void created.setFullscreen(true).catch(() => undefined);
          resolve(true);
        });
        void created.once("tauri://error", () => resolve(false));
        void created.once("tauri://destroyed", () => {
          if (windows.get(role) === created) windows.delete(role);
        });
      });
    },
    close: async (role) => {
      const opened = windows.get(role) ?? (await WebviewWindow.getByLabel(labelOf(role)));
      windows.delete(role);
      await opened?.close().catch(() => undefined);
      // Фокус возвращается редактору: закрытое окно показа его не отдаёт само.
      if (role === "show") await getCurrentWebviewWindow().setFocus().catch(() => undefined);
    },
    send: (message) => {
      const targets: ShowRole[] =
        message.kind === "deck" && message.to !== undefined ? [message.to] : [...windows.keys()];
      for (const role of targets) {
        void emitTo(labelOf(role), hostEvent(session), message).catch(() => undefined);
      }
    },
    onMessage: (handler) => {
      const subscription = getCurrentWebviewWindow().listen<ViewerMessage>(
        viewerEvent(session),
        (event) => handler(event.payload),
      );
      subscriptions.add(subscription);
      return () => {
        subscriptions.delete(subscription);
        void subscription.then((unlisten) => unlisten());
      };
    },
  };
}

/**
 * Зритель в окне показа или докладчика рабочего стола. Роль берётся из метки
 * окна. Закрытие окна ОС (крестик, Alt+F4) сообщается хозяину как `closed`.
 */
export function createTauriShowViewer(session: string): ShowViewer {
  const current = getCurrentWebviewWindow();
  const role: ShowRole = current.label === labelOf("presenter") ? "presenter" : "show";
  // Отправка идёт за регистрацией слушателей: `ready` до подписки хозяина на
  // ответы потерялся бы вместе с ответом `hello`.
  let settled: Promise<unknown> = Promise.resolve();

  const send = (message: ViewerMessage): Promise<void> => {
    const sent = settled.then(() => emitTo(HOST_LABEL, viewerEvent(session), message));
    settled = sent.catch(() => undefined);
    return sent.catch(() => undefined);
  };

  // Обработчик закрытия окна успевает отправить `closed`: окно уничтожается
  // после того, как он завершится.
  void current.onCloseRequested(async () => {
    await send({ kind: "closed", role });
  });

  return {
    send: (message) => {
      void send(message);
    },
    onMessage: (handler) => {
      const subscription = current.listen<HostMessage>(hostEvent(session), (event) =>
        handler(event.payload),
      );
      settled = settled.then(() => subscription).catch(() => undefined);
      return () => {
        void subscription.then((unlisten) => unlisten());
      };
    },
  };
}
