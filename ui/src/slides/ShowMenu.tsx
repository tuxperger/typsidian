"use client";

import { useEffect, useId, useRef, useState } from "react";
import { Icon, type IconName } from "../icons/Icons.js";
import { useI18n, type MessageKey } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";

export interface ShowMenuProps {
  /** Есть ли в оболочке вид докладчика: без него пункта нет (FR-009). */
  canPresent: boolean;
  /** Действующее сочетание команды «с текущего слайда» (подпись). */
  shortcut: string;
  /** Идёт показ: кнопка не запускает второй. */
  active: boolean;
  onStart: (from: "caret" | "first", withPresenter: boolean) => void;
  /** «Экспорт презентации…» (фича 035); не передан — пункта нет. */
  onExport?: (() => void) | undefined;
}

interface Item {
  id: string;
  icon: IconName;
  title: MessageKey;
  hint?: MessageKey;
  shortcut?: string;
  run: () => void;
}

/**
 * Кнопка «Показ» с меню вариантов запуска (A1). Основное действие — «с текущего
 * слайда»; стрелка раскрывает «с начала» и «сразу с видом докладчика».
 */
export function ShowMenu({
  canPresent,
  shortcut,
  active,
  onStart,
  onExport,
}: ShowMenuProps): React.JSX.Element {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const menuId = useId();

  useDismissOnEscape(open ? () => setOpen(false) : null);

  // Нажатие мимо меню закрывает его.
  useEffect(() => {
    if (!open) return undefined;
    const onPointerDown = (event: PointerEvent): void => {
      if (event.target instanceof Node && rootRef.current?.contains(event.target) === true) return;
      setOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [open]);

  // Фокус — на первый пункт при открытии.
  useEffect(() => {
    if (open) rootRef.current?.querySelector<HTMLButtonElement>('[role="menuitem"]')?.focus();
  }, [open]);

  const items: Item[] = [
    {
      id: "current",
      icon: "caret",
      title: "slides.menu.fromCurrent",
      hint: "slides.menu.fromCurrentHint",
      shortcut,
      run: () => onStart("caret", false),
    },
    { id: "first", icon: "start", title: "slides.menu.fromStart", run: () => onStart("first", false) },
    ...(canPresent
      ? [
          {
            id: "presenter",
            icon: "presenter" as const,
            title: "slides.menu.withPresenter" as const,
            hint: "slides.menu.withPresenterHint" as const,
            run: () => onStart("caret", true),
          },
        ]
      : []),
    ...(onExport !== undefined
      ? [
          {
            id: "export",
            icon: "download" as const,
            title: "slides.export.menu" as const,
            run: onExport,
          },
        ]
      : []),
  ];

  const onMenuKeyDown = (event: React.KeyboardEvent): void => {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    const buttons = Array.from(
      rootRef.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]') ?? [],
    );
    const at = buttons.findIndex((button) => button === document.activeElement);
    const step = event.key === "ArrowDown" ? 1 : -1;
    buttons[(at + step + buttons.length) % buttons.length]?.focus();
    event.preventDefault();
  };

  return (
    <div className="ts-showmenu" ref={rootRef}>
      <div className="ts-showmenu__split">
        <button
          type="button"
          className="ts-btn ts-showmenu__main"
          title={shortcut === "" ? undefined : `${t("slides.menu.fromCurrent")} · ${shortcut}`}
          disabled={active}
          onClick={() => onStart("caret", false)}
        >
          <Icon name="play" size={14} />
          {t("slides.button")}
        </button>
        <button
          type="button"
          className="ts-btn ts-showmenu__more"
          aria-label={t("slides.menu.label")}
          aria-haspopup="menu"
          aria-expanded={open}
          aria-controls={open ? menuId : undefined}
          disabled={active}
          onClick={() => setOpen((shown) => !shown)}
        >
          <Icon name="down" size={14} />
        </button>
      </div>
      {open && (
        <div
          className="ts-showmenu__menu"
          id={menuId}
          role="menu"
          aria-label={t("slides.menu.label")}
          onKeyDown={onMenuKeyDown}
        >
          {items.map((item) => (
            <button
              key={item.id}
              type="button"
              role="menuitem"
              className="ts-showmenu__item"
              onClick={() => {
                setOpen(false);
                item.run();
              }}
            >
              <Icon name={item.icon} size={14} />
              <span className="ts-showmenu__label">
                {t(item.title)}
                {item.hint !== undefined && <small>{t(item.hint)}</small>}
              </span>
              {item.shortcut !== undefined && item.shortcut !== "" && (
                <kbd className="ts-showmenu__key">{item.shortcut}</kbd>
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
