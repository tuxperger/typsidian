"use client";

import { useCallback, useEffect, useId, useRef, useState } from "react";
import { Icon, type IconName } from "../icons/Icons.js";
import { useI18n, type MessageKey } from "../i18n/index.js";
import type { CommandId, Template } from "../wasm-core/typsidian_core.js";
import { NewNoteFromTemplate } from "../templates/NewNoteFromTemplate.js";
import { hintWith, type ShortcutsState } from "../workspace/useShortcuts.js";

// Создание файла по макету «Typsidian Доски» → экран «Новый файл».
//
// Форма именно такая, а не две кнопки в тулбаре: в макете это квадратная «+»
// рядом с поиском, открывающая меню выбора типа **над деревом**. Сам макет
// объясняет почему — «тип выбирается в момент создания, файл появляется в
// текущей папке сразу, без модалки и формы».
//
// Диаграммы draw.io в меню нет: в макете это третий тип, но в приложении
// такого файла не существует, а мёртвая строка хуже её отсутствия — то же
// правило, по которому в палитре ⌘K нет экспорта BibTeX.

/** Что можно создать. Совпадает с видами, которые понимает ядро. */
export type CreateKind = "note" | "board";

interface Choice {
  kind: CreateKind;
  title: MessageKey;
  hint: MessageKey;
  icon: IconName;
  /**
   * Команда, чьё сочетание создаёт этот вид. Подпись берётся у неё же — раньше
   * здесь стоял литерал «⌘N», и после переназначения он начал бы врать.
   */
  command: CommandId;
  className: string;
}

const CHOICES: Choice[] = [
  {
    kind: "note",
    title: "tree.create.note",
    hint: "tree.create.noteHint",
    icon: "file",
    command: "new-file",
    className: "ts-create__icon ts-create__icon--note",
  },
  {
    kind: "board",
    title: "tree.create.board",
    hint: "tree.create.boardHint",
    icon: "board",
    command: "new-board",
    className: "ts-create__icon ts-create__icon--board",
  },
];

export interface CreateFileMenuProps {
  /** Куда создавать — подпись в шапке меню; пустая строка означает корень. */
  folderName: string;
  disabled?: boolean;
  /**
   * Заготовки для второго уровня под «Заметка Typst» (фича 003, FR-020).
   *
   * Пустой список — второго уровня нет вовсе: строка «Пустая заметка» под
   * пустым списком объясняла бы выбор, которого не существует.
   */
  templates?: Template[];
  /** Открыть галерею целиком — когда заготовок больше, чем помещается. */
  onOpenTemplates?: () => void;
  onCreate: (kind: CreateKind, template?: Template | null) => void;
  /**
   * Действующие сочетания. Приходят пропом, а не читаются здесь: один разбор на
   * приложение, и меню обязано показывать те же клавиши, что и работают.
   */
  shortcuts: ShortcutsState;
}

export function CreateFileMenu({
  folderName,
  disabled = false,
  templates = [],
  onOpenTemplates,
  onCreate,
  shortcuts,
}: CreateFileMenuProps): React.JSX.Element {
  const { t } = useI18n();
  const [isOpen, setIsOpen] = useState(false);
  const [active, setActive] = useState(0);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const menuId = useId();

  const choose = useCallback(
    (kind: CreateKind, template: Template | null = null) => {
      setIsOpen(false);
      onCreate(kind, template);
    },
    [onCreate],
  );

  // Сочетания из макета работают, а не только нарисованы: «шорткат создаёт
  // нужный тип, минуя меню». Слушатель глобальный по той же причине, что и у
  // палитры, — создать файл нужно и когда фокус в редакторе.
  //
  // Заметка и доска — две команды, а не одна с модификатором: иначе
  // переназначить их по отдельности было бы нечем.
  useEffect(() => {
    if (disabled) return;
    const onKeyDown = (event: KeyboardEvent): void => {
      if (shortcuts.matches("new-board", event)) {
        event.preventDefault();
        choose("board");
      } else if (shortcuts.matches("new-file", event)) {
        event.preventDefault();
        choose("note");
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [disabled, choose, shortcuts]);

  // Клик мимо закрывает меню. Слушатель ставится только пока оно открыто.
  useEffect(() => {
    if (!isOpen) return;
    const onPointerDown = (event: PointerEvent): void => {
      if (!rootRef.current?.contains(event.target as Node)) setIsOpen(false);
    };
    window.addEventListener("pointerdown", onPointerDown);
    return () => window.removeEventListener("pointerdown", onPointerDown);
  }, [isOpen]);

  return (
    <div className="ts-create" ref={rootRef}>
      <button
        type="button"
        className="ts-create__plus"
        title={hintWith(t("tree.create.button"), shortcuts.displayOf("new-file"))}
        aria-label={t("tree.create.button")}
        aria-haspopup="menu"
        aria-expanded={isOpen}
        aria-controls={isOpen ? menuId : undefined}
        disabled={disabled}
        onClick={() => {
          setActive(0);
          setIsOpen((open) => !open);
        }}
      >
        <Icon name="plus" size={15} />
      </button>

      {isOpen && (
        <div
          className="ts-create__menu"
          id={menuId}
          role="menu"
          aria-label={t("tree.create.menuLabel")}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              event.preventDefault();
              setIsOpen(false);
            } else if (event.key === "ArrowDown") {
              event.preventDefault();
              setActive((current) => (current + 1) % CHOICES.length);
            } else if (event.key === "ArrowUp") {
              event.preventDefault();
              setActive((current) => (current - 1 + CHOICES.length) % CHOICES.length);
            } else if (event.key === "Enter") {
              event.preventDefault();
              const choice = CHOICES[active];
              if (choice) choose(choice.kind);
            }
          }}
        >
          <div className="ts-create__head">
            <span className="ts-eyebrow">
              {folderName === ""
                ? t("tree.create.inVault")
                : t("tree.create.inFolder", { folder: folderName })}
            </span>
          </div>

          <div className="ts-create__list">
            {CHOICES.map((choice, index) => (
              <div key={choice.kind}>
                <button
                  type="button"
                  role="menuitem"
                  className="ts-create__row"
                  data-active={index === active}
                  // Фокус ведёт первый пункт: меню открывается ради выбора, и
                  // стрелки должны работать сразу, без лишнего Tab.
                  autoFocus={index === 0}
                  onMouseEnter={() => setActive(index)}
                  onClick={() => choose(choice.kind)}
                >
                  <Icon name={choice.icon} size={15} className={choice.className} />
                  <span className="ts-create__text">
                    <span className="ts-create__title">{t(choice.title)}</span>
                    <span className="ts-create__hint">{t(choice.hint)}</span>
                  </span>
                  <span className="ts-create__key">
                    {shortcuts.displayOf(choice.command)}
                  </span>
                </button>

                {/* Второй уровень — сразу под «Заметка Typst», а не отдельным
                    шагом после создания (макет «Утилиты» § Шаблоны). */}
                {choice.kind === "note" && templates.length > 0 && (
                  <NewNoteFromTemplate
                    templates={templates}
                    onChoose={(template) => choose("note", template)}
                    {...(onOpenTemplates
                      ? {
                          onOpenGallery: () => {
                            setIsOpen(false);
                            onOpenTemplates();
                          },
                        }
                      : {})}
                  />
                )}
              </div>
            ))}
          </div>

          <div className="ts-create__foot">
            <span>
              <Icon name="enter" size={10} />
              {t("tree.create.footCreate")}
            </span>
            <span>{t("tree.create.footSelect")}</span>
            <span className="ts-create__esc">Esc</span>{/* i18n-ignore: имя клавиши */}
          </div>
        </div>
      )}
    </div>
  );
}
