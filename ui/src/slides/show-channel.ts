import type {
  Diagnostic,
  PageSlideInfo,
  ShowAction,
  ShowKeymap,
  ShowState,
  SlideLabel,
} from "../wasm-core/typsidian_core.js";

// Канал между окном редактора и окнами показа (фича 019, contracts/show-channel.md).
//
// Окно редактора — хозяин: оно держит `ShowState`, вызывает ядро и рассылает
// результат. Окна показа и докладчика — зрители: рисуют присланное и шлют
// действия обратно. Правил показа (что значит «дальше», где граница колоды)
// здесь нет — они в ядре (принцип I); канал только возит значения.
//
// Реализации по оболочкам: `show-channel-web.ts` (`window.open` +
// `BroadcastChannel`), `show-channel-tauri.ts` (`WebviewWindow` + события Tauri)
// и «в памяти» ниже — для Android, где показ слоем в том же окне.

/** Роль окна: показ на проектор или вид докладчика. */
export type ShowRole = "show" | "presenter";

/** Размер страницы в пунктах — для вписывания в рамку. */
export interface ShowPageSize {
  w: number;
  h: number;
}

/** Хозяин → зритель. */
export type HostMessage =
  | { kind: "hello"; keymap: ShowKeymap; locale: string }
  | {
      kind: "deck";
      /** Адресат: `svgs` у каждого окна свои (их кэши разные). Без адресата — всем. */
      to?: ShowRole;
      /** Отпечатки страниц по порядку. */
      digests: string[];
      /** SVG только тех страниц, которых у адресата ещё нет (research.md §10). */
      svgs: Record<string, string>;
      sizes: ShowPageSize[];
      slides: PageSlideInfo[];
    }
  | {
      kind: "state";
      state: ShowState;
      label: SlideLabel;
      /** Сборка текущего текста не удалась — показана прошлая (A4). */
      stale: boolean;
      /** Первая ошибка: пояснение к `stale` (A4) или причина пустого экрана (A5). */
      failure?: Diagnostic;
    }
  | { kind: "end" };

/** Зритель → хозяин. */
export type ViewerMessage =
  | { kind: "ready"; role: ShowRole; have: string[] }
  | { kind: "action"; action: ShowAction }
  | { kind: "exit" }
  | { kind: "closed"; role: ShowRole }
  | { kind: "togglePresenter" }
  | { kind: "revealError" };

/** Окно редактора. */
export interface ShowHost {
  /** Открывает окно роли; `false` — окно не открылось (блокировщик, отказ ОС). */
  open(role: ShowRole): Promise<boolean>;
  close(role: ShowRole): Promise<void>;
  send(message: HostMessage): void;
  onMessage(handler: (message: ViewerMessage) => void): () => void;
  /** Что умеет оболочка: есть ли отдельные окна и вид докладчика (FR-009). */
  readonly supports: { windows: boolean; presenter: boolean };
}

/** Окно показа или докладчика. */
export interface ShowViewer {
  send(message: ViewerMessage): void;
  onMessage(handler: (message: HostMessage) => void): () => void;
}

/** Случайный идентификатор показа: имя канала и окон. */
export function newShowSessionId(): string {
  const bytes = new Uint8Array(6);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

/** Разбор адреса окна показа: сессия и роль. Роль по умолчанию — показ. */
export function parseShowLocation(search: string): { session: string | null; role: ShowRole } {
  const params = new URLSearchParams(search);
  return {
    session: params.get("session"),
    role: params.get("role") === "presenter" ? "presenter" : "show",
  };
}

/** Пара «хозяин — зритель» в одном окне (Android, `ShowOverlay`). */
export interface InMemoryShow {
  host: ShowHost;
  viewer: ShowViewer;
  /** Слой показа открыт. */
  isOpen(): boolean;
  /** Подписка на открытие и закрытие слоя; возвращает отписку. */
  subscribe(listener: () => void): () => void;
}

/**
 * Хозяин и зритель в одном окне. Сообщения доходят в следующем микротакте:
 * ответ не должен прийти внутрь вызова, который его вызвал (повторный вход в
 * обработчик). Вид докладчика не поддерживается — `supports.presenter = false`.
 */
export function createInMemoryShowHost(): InMemoryShow {
  const hostHandlers = new Set<(message: ViewerMessage) => void>();
  const viewerHandlers = new Set<(message: HostMessage) => void>();
  const listeners = new Set<() => void>();
  let open = false;

  const setOpen = (next: boolean): void => {
    if (open === next) return;
    open = next;
    for (const listener of [...listeners]) listener();
  };

  const host: ShowHost = {
    supports: { windows: false, presenter: false },
    open: async (role) => {
      if (role !== "show") return false;
      setOpen(true);
      return true;
    },
    close: async (role) => {
      if (role === "show") setOpen(false);
    },
    send: (message) => {
      queueMicrotask(() => {
        for (const handler of [...viewerHandlers]) handler(message);
      });
    },
    onMessage: (handler) => {
      hostHandlers.add(handler);
      return () => {
        hostHandlers.delete(handler);
      };
    },
  };

  const viewer: ShowViewer = {
    send: (message) => {
      queueMicrotask(() => {
        for (const handler of [...hostHandlers]) handler(message);
      });
    },
    onMessage: (handler) => {
      viewerHandlers.add(handler);
      return () => {
        viewerHandlers.delete(handler);
      };
    },
  };

  return {
    host,
    viewer,
    isOpen: () => open,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}
