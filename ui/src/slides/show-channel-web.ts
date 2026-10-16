import {
  newShowSessionId,
  type HostMessage,
  type ShowHost,
  type ShowRole,
  type ShowViewer,
  type ViewerMessage,
} from "./show-channel.js";

// Канал показа для web (фича 019, research.md §8): окно — `window.open` на тот же
// origin, сообщения — `BroadcastChannel`. Окна одного показа делят имя канала с
// идентификатором сессии, поэтому два приложения в соседних вкладках не слышат
// друг друга.

const WINDOW_NAMES: Record<ShowRole, string> = {
  show: "typsidian-show-show",
  presenter: "typsidian-show-presenter",
};

const channelName = (session: string): string => `typsidian-show-${session}`;

/** Размеры окон: показ — под проектор, докладчик — обычное окно. */
const WINDOW_FEATURES: Record<ShowRole, string> = {
  show: "popup=yes,width=1280,height=720",
  presenter: "popup=yes,width=1100,height=700",
};

/** Адрес окна. Маршрут `/show` — `apps/web/app/show/page.tsx`. */
function showUrl(session: string, role: ShowRole): string {
  const url = new URL("/show", window.location.href);
  url.searchParams.set("session", session);
  url.searchParams.set("role", role);
  return url.toString();
}

/** Хозяин в окне редактора web. */
export function createWebShowHost(): ShowHost {
  const session = newShowSessionId();
  const windows = new Map<ShowRole, Window>();
  const handlers = new Set<(message: ViewerMessage) => void>();
  let channel: BroadcastChannel | null = null;

  const ensureChannel = (): BroadcastChannel => {
    if (channel === null) {
      channel = new BroadcastChannel(channelName(session));
      channel.onmessage = (event: MessageEvent<ViewerMessage | HostMessage>) => {
        const message = event.data;
        // Свои же сообщения в тот же канал не возвращаются; но в канале говорит
        // и хозяин, и зрители, и зрителям ничего не нужно из того, что шлёт хозяин.
        if (!isViewerMessage(message)) return;
        for (const handler of [...handlers]) handler(message);
      };
    }
    return channel;
  };

  return {
    supports: { windows: true, presenter: true },
    open: async (role) => {
      ensureChannel();
      // Имя окна постоянное: повторное открытие находит прежнее окно.
      const opened = window.open(showUrl(session, role), WINDOW_NAMES[role], WINDOW_FEATURES[role]);
      if (opened === null) return false;
      windows.set(role, opened);
      return true;
    },
    close: async (role) => {
      const opened = windows.get(role);
      windows.delete(role);
      if (opened !== undefined && !opened.closed) opened.close();
      if (windows.size === 0) {
        channel?.close();
        channel = null;
      }
    },
    send: (message) => {
      ensureChannel().postMessage(message);
    },
    onMessage: (handler) => {
      handlers.add(handler);
      ensureChannel();
      return () => {
        handlers.delete(handler);
      };
    },
  };
}

/**
 * Вид докладчика из окна показа. Открыть окно браузер разрешает только в
 * обработчике жеста, а нажатие «Докладчик» случилось здесь, не в окне
 * редактора, — поэтому окно открывает или закрывает зритель сам. Имя окна то же,
 * что у хозяина: уже открытое окно находится по имени и закрывается, а не
 * открывается заново. Хозяин узнаёт об окне из его `ready` и `closed`.
 * `false` — браузер окно не дал, пусть попробует хозяин (и скажет об отказе).
 */
function togglePresenterWindow(session: string): boolean {
  const opened = window.open("", WINDOW_NAMES.presenter, WINDOW_FEATURES.presenter);
  if (opened === null) return false;
  if (opened.location.href === "about:blank") opened.location.replace(showUrl(session, "presenter"));
  else opened.close();
  return true;
}

/** Зритель в окне показа или докладчика web. */
export function createWebShowViewer(session: string): ShowViewer {
  const channel = new BroadcastChannel(channelName(session));
  return {
    send: (message) => {
      if (message.kind === "togglePresenter" && togglePresenterWindow(session)) return;
      channel.postMessage(message);
    },
    onMessage: (handler) => {
      const listener = (event: MessageEvent<ViewerMessage | HostMessage>): void => {
        if (!isHostMessage(event.data)) return;
        // Показ завершён: окно, открытое скриптом, закрывается само. Окно вида
        // докладчика, открытое из окна показа, хозяину не известно — закрыть его
        // может только оно.
        if (event.data.kind === "end") window.setTimeout(() => window.close(), 0);
        handler(event.data);
      };
      channel.addEventListener("message", listener);
      return () => channel.removeEventListener("message", listener);
    },
  };
}

const VIEWER_KINDS: ReadonlySet<string> = new Set([
  "ready",
  "action",
  "exit",
  "closed",
  "togglePresenter",
  "revealError",
]);

function isViewerMessage(message: ViewerMessage | HostMessage): message is ViewerMessage {
  return VIEWER_KINDS.has(message.kind);
}

function isHostMessage(message: ViewerMessage | HostMessage): message is HostMessage {
  return !VIEWER_KINDS.has(message.kind);
}
