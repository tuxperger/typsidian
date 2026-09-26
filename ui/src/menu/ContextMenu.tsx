"use client";

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";

/**
 * Собственное контекстное меню приложения — по правому клику в дереве файлов,
 * редакторе, предпросмотре и шапке.
 *
 * Один экземпляр на рабочую область: меню открывается по месту клика, а не
 * внутри строки, из которой его вызвали, — иначе оно обрезалось бы
 * прокруткой колонки и перекрывалось соседними панелями.
 *
 * Меню ничего не решает само: пункты приходят готовыми от зоны, где был
 * клик, и каждый вызывает уже существующее действие (принцип I).
 *
 * **Shift + правый клик** оставляет системное меню браузера: в нём проверка
 * орфографии и «Исследовать элемент», и отнимать их насовсем незачем.
 */

export type ContextMenuItem =
  | {
      kind?: "item";
      label: string;
      /** Подпись сочетания справа — только отображение, клавиши вешаются не здесь. */
      shortcut?: string;
      disabled?: boolean;
      /** Разрушительное действие — красным. */
      danger?: boolean;
      run: () => void | Promise<void>;
    }
  | { kind: "separator" };

/** Пункты с разделителями по краям и подряд убираются — зоны собирают списки условно. */
function tidy(items: ContextMenuItem[]): ContextMenuItem[] {
  const out: ContextMenuItem[] = [];
  for (const item of items) {
    if (item.kind === "separator") {
      if (out.length === 0 || out[out.length - 1]?.kind === "separator") continue;
    }
    out.push(item);
  }
  while (out.length > 0 && out[out.length - 1]?.kind === "separator") out.pop();
  return out;
}

interface OpenMenu {
  x: number;
  y: number;
  items: ContextMenuItem[];
}

interface ContextMenuApi {
  /**
   * Открыть меню по событию `contextmenu`. Возвращает `false`, если событие
   * оставлено браузеру (Shift или пустой список), — тогда вызывающий ничего
   * больше не делает.
   */
  open: (event: React.MouseEvent | MouseEvent, items: ContextMenuItem[]) => boolean;
  close: () => void;
  /**
   * Короткое сообщение внизу окна: итог действия, у которого нет своего места
   * на экране, — «путь скопирован» или причина, по которой буфер недоступен.
   */
  notify: (message: string) => void;
}

const ContextMenuContext = createContext<ContextMenuApi | null>(null);

/**
 * Доступ к меню из любой зоны. Вне провайдера — пустышка: правый клик тогда
 * остаётся браузеру, а не теряется.
 */
export function useContextMenu(): ContextMenuApi {
  return useContext(ContextMenuContext) ?? FALLBACK;
}

const FALLBACK: ContextMenuApi = { open: () => false, close: () => {}, notify: () => {} };

export function ContextMenuProvider({
  children,
}: {
  children: React.ReactNode;
}): React.JSX.Element {
  const [menu, setMenu] = useState<OpenMenu | null>(null);

  const close = useCallback(() => setMenu(null), []);

  const open = useCallback(
    (event: React.MouseEvent | MouseEvent, items: ContextMenuItem[]) => {
      if (event.shiftKey) return false;
      const cleaned = tidy(items);
      if (cleaned.length === 0) return false;
      event.preventDefault();
      event.stopPropagation();
      setMenu({ x: event.clientX, y: event.clientY, items: cleaned });
      return true;
    },
    [],
  );

  const [notice, setNotice] = useState<{ text: string; id: number } | null>(null);
  const notify = useCallback((text: string) => {
    setNotice({ text, id: Date.now() });
  }, []);
  useEffect(() => {
    if (notice === null) return undefined;
    const timer = window.setTimeout(() => setNotice(null), 4000);
    return () => window.clearTimeout(timer);
  }, [notice]);

  const api = useMemo(() => ({ open, close, notify }), [open, close, notify]);

  return (
    <ContextMenuContext.Provider value={api}>
      {children}
      {menu && <MenuLayer menu={menu} onClose={close} />}
      {notice && (
        <div key={notice.id} className="ts-ctxmenu__notice" role="status">
          {notice.text}
        </div>
      )}
    </ContextMenuContext.Provider>
  );
}

function MenuLayer({
  menu,
  onClose,
}: {
  menu: OpenMenu;
  onClose: () => void;
}): React.JSX.Element {
  const ref = useRef<HTMLDivElement | null>(null);
  const [position, setPosition] = useState({ left: menu.x, top: menu.y });
  // Фокус возвращается туда, где был до меню: иначе после «Копировать» в
  // редакторе каретка терялась бы вместе с фокусом.
  const returnFocus = useRef<Element | null>(null);

  // Меню прижимается к краям окна: у правого и нижнего края оно
  // открывается влево/вверх от курсора, как системное.
  useLayoutEffect(() => {
    const node = ref.current;
    if (!node) return;
    const rect = node.getBoundingClientRect();
    const margin = 4;
    let left = menu.x;
    let top = menu.y;
    if (left + rect.width + margin > window.innerWidth) {
      left = Math.max(margin, menu.x - rect.width);
    }
    if (top + rect.height + margin > window.innerHeight) {
      top = Math.max(margin, window.innerHeight - rect.height - margin);
    }
    setPosition({ left, top });
  }, [menu]);

  useEffect(() => {
    returnFocus.current = document.activeElement;
    firstEnabled(ref.current)?.focus();
    const restore = returnFocus.current;
    return () => {
      // Только если фокус никто не забрал: пункт мог открыть поле ввода
      // («Переименовать», «Создать заметку здесь»), и вернуть фокус назад
      // значило бы снять его с поля — а потеря фокуса там означает
      // «подтвердить», и файл создавался бы с именем по умолчанию.
      const free = document.activeElement === null || document.activeElement === document.body;
      if (free && restore instanceof HTMLElement && document.contains(restore)) {
        restore.focus({ preventScroll: true });
      }
    };
  }, [menu]);

  // Закрытие: клик мимо, прокрутка, смена размера, уход из окна.
  useEffect(() => {
    const onPointer = (event: PointerEvent) => {
      if (ref.current && event.target instanceof Node && ref.current.contains(event.target)) {
        return;
      }
      onClose();
    };
    const onScroll = (event: Event) => {
      if (ref.current && event.target instanceof Node && ref.current.contains(event.target)) {
        return;
      }
      onClose();
    };
    window.addEventListener("pointerdown", onPointer, true);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", onClose);
    window.addEventListener("blur", onClose);
    return () => {
      window.removeEventListener("pointerdown", onPointer, true);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", onClose);
      window.removeEventListener("blur", onClose);
    };
  }, [onClose]);

  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    const items = enabledItems(ref.current);
    const index = items.indexOf(document.activeElement as HTMLButtonElement);
    const move = (next: number) => {
      event.preventDefault();
      items[(next + items.length) % items.length]?.focus();
    };
    switch (event.key) {
      case "Escape":
        event.preventDefault();
        event.stopPropagation();
        onClose();
        break;
      case "ArrowDown":
        move(index + 1);
        break;
      case "ArrowUp":
        move(index <= 0 ? items.length - 1 : index - 1);
        break;
      case "Home":
        move(0);
        break;
      case "End":
        move(items.length - 1);
        break;
      case "Tab":
        event.preventDefault();
        onClose();
        break;
    }
  };

  return (
    <div
      ref={ref}
      role="menu"
      className="ts-ctxmenu"
      style={{ left: position.left, top: position.top }}
      onKeyDown={onKeyDown}
      // Своё меню поверх своего меню не открывается, а системное — тем более.
      onContextMenu={(event) => event.preventDefault()}
    >
      {menu.items.map((item, index) =>
        item.kind === "separator" ? (
          <div key={`s${index}`} role="separator" className="ts-ctxmenu__sep" />
        ) : (
          <button
            key={`${index}:${item.label}`}
            type="button"
            role="menuitem"
            className={
              item.danger ? "ts-ctxmenu__item ts-ctxmenu__item--danger" : "ts-ctxmenu__item"
            }
            disabled={item.disabled}
            // Нажатие мышью не должно забирать фокус у редактора до того, как
            // пункт отработает: «Вырезать» и «Вставить» работают с выделением.
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => {
              onClose();
              void item.run();
            }}
          >
            <span className="ts-ctxmenu__label">{item.label}</span>
            {item.shortcut && <span className="ts-ctxmenu__key">{item.shortcut}</span>}
          </button>
        ),
      )}
    </div>
  );
}

function enabledItems(root: HTMLElement | null): HTMLButtonElement[] {
  if (!root) return [];
  return Array.from(
    root.querySelectorAll<HTMLButtonElement>('button[role="menuitem"]:not(:disabled)'),
  );
}

function firstEnabled(root: HTMLElement | null): HTMLButtonElement | undefined {
  return enabledItems(root)[0];
}

/**
 * Запись в буфер обмена. Отказ среды (нет разрешения, небезопасный контекст)
 * возвращается строкой-причиной, а не глотается: пользователь нажал
 * «Копировать» и должен узнать, что ничего не скопировалось.
 */
export async function copyText(text: string): Promise<string | null> {
  try {
    if (!navigator.clipboard?.writeText) throw new Error("unsupported");
    await navigator.clipboard.writeText(text);
    return null;
  } catch {
    return "Скопировать не удалось: среда не дала доступа к буферу обмена. Используйте Ctrl+C.";
  }
}

/** Чтение буфера — то же, но причин отказа больше: браузер спрашивает разрешение. */
export async function readText(): Promise<{ text: string } | { error: string }> {
  try {
    if (!navigator.clipboard?.readText) throw new Error("unsupported");
    return { text: await navigator.clipboard.readText() };
  } catch {
    return {
      error: "Вставить из меню не удалось: среда не дала прочитать буфер обмена. Используйте Ctrl+V.",
    };
  }
}
