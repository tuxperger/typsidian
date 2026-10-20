"use client";

import { Fragment, useRef } from "react";
import { useI18n } from "../i18n/index.js";
import { useMenuPosition } from "../editor/useMenuPosition.js";
import type { CompletionSession } from "./useCompletion.js";
import { groupTitle, itemFacts } from "./describe.js";

export interface CompletionPopupProps {
  session: CompletionSession;
  /** Узкий экран: список над строкой набора на ширину редактора, без подвала. */
  narrow: boolean;
  /** Прямоугольник редактора — для узкой подачи. */
  editorBox: () => DOMRect | null;
  onPick: (index: number) => void;
  onHover: (index: number) => void;
}

/** Идентификатор строки — для `aria-activedescendant` поля редактора. */
export function completionOptionId(index: number): string {
  return `ts-complete-${index}`;
}

/**
 * Список меток и заметок у каретки (фича 018, макет 1a и 1b). Фокус не
 * забирает: набор продолжается в редакторе, ↑↓ ↵ ⇥ esc перехватывает
 * расширение `completionKeys`. Строки гасят `pointerdown`, чтобы редактор не
 * терял фокус и экранную клавиатуру.
 */
export function CompletionPopup({
  session,
  narrow,
  editorBox,
  onPick,
  onHover,
}: CompletionPopupProps): React.JSX.Element {
  const { t } = useI18n();
  const ref = useRef<HTMLDivElement | null>(null);
  const position = useMenuPosition(ref, session.anchor);
  const box = narrow ? editorBox() : null;
  const style: React.CSSProperties =
    narrow && box !== null && session.anchor !== null
      ? {
          left: box.left,
          width: box.width,
          bottom: window.innerHeight - session.anchor.top + 4,
        }
      : {
          left: position?.left ?? 0,
          top: position?.top ?? 0,
          visibility: position === null ? "hidden" : "visible",
        };
  const selected = session.items[session.active];

  let index = 0;
  return (
    <div
      ref={ref}
      className={narrow ? "ts-complete ts-complete--narrow" : "ts-complete"}
      style={style}
    >
      <ul className="ts-complete__list" role="listbox" aria-label={t("completion.label")}>
        {session.list.groups.map((group) => (
          <Fragment key={group.kind}>
            <li className="ts-complete__head" role="presentation">
              {t("completion.group.head", {
                title: groupTitle(t, group.kind),
                count: group.total,
              })}
            </li>
            {group.items.map((item) => {
              const at = index++;
              const facts = itemFacts(t, item);
              return (
                <li
                  key={`${group.kind}\u0000${item.label}\u0000${item.insert}`}
                  id={completionOptionId(at)}
                  role="option"
                  aria-selected={at === session.active}
                  className={
                    at === session.active ? "ts-complete__row ts-complete__row--on" : "ts-complete__row"
                  }
                  onPointerMove={() => onHover(at)}
                  onPointerDown={(event) => {
                    event.preventDefault();
                    onPick(at);
                  }}
                >
                  <span className="ts-complete__name">
                    <Highlighted text={item.label} spans={item.matched} />
                  </span>
                  <span className="ts-tag ts-complete__tag">{facts.tag}</span>
                  {facts.duplicate && (
                    <span className="ts-tag ts-complete__tag--warn">{t("completion.duplicate")}</span>
                  )}
                  <span className="ts-complete__text">{facts.text}</span>
                  {facts.side !== "" && <span className="ts-complete__side">{facts.side}</span>}
                </li>
              );
            })}
            {group.total > group.items.length && (
              <li className="ts-complete__more" role="presentation">
                {t("completion.group.more", { shown: group.items.length, total: group.total })}
              </li>
            )}
          </Fragment>
        ))}
      </ul>
      {!narrow && (
        <div className="ts-complete__foot">
          <span>{t("completion.footer")}</span>
          {selected !== undefined && (
            <span className="ts-complete__will">
              {t("completion.willBe", { text: selected.preview })}
            </span>
          )}
        </div>
      )}
    </div>
  );
}

/** Имя с подсвеченными отрезками совпадения (UTF-16 — как индексы строки JS). */
function Highlighted({
  text,
  spans,
}: {
  text: string;
  spans: readonly [number, number][];
}): React.JSX.Element {
  const parts: React.ReactNode[] = [];
  let at = 0;
  for (const [from, to] of spans) {
    if (from > at) parts.push(text.slice(at, from));
    parts.push(<mark key={from}>{text.slice(from, to)}</mark>);
    at = to;
  }
  if (at < text.length) parts.push(text.slice(at));
  return <>{parts}</>;
}
