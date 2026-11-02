"use client";

import { useEffect, useRef, useState } from "react";
import type { LessonState, LessonSummary } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { Icon } from "../icons/Icons.js";
import { useI18n, type MessageKey } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";

// Список уроков (фича 051, макет G, H). Список строит ядро по всем файлам `.lesson`
// vault; здесь — порядок «свежие сверху», показ и клавиатура.

export interface LessonsScreenProps {
  client: VaultClient;
  /** Номер изменений vault: список перечитывается. */
  revision: unknown;
  onOpen: (path: string) => void;
  onNew: () => void;
  onClose: () => void;
}

const STATUS: Record<LessonState, MessageKey> = {
  Active: "lessons.status.active",
  Finished: "lessons.status.finished",
  Stopped: "lessons.status.stopped",
};

// Подпись списка (макет G): ждёт ответа, ответ оборван, завершён, остановлен. Признак
// ожидания считает ядро (`LessonSummary.waiting`), здесь он только показывается.
function statusOf(lesson: LessonSummary): { label: MessageKey; tone: string } {
  if (lesson.waiting === "Retry") return { label: "lessons.status.cut", tone: "Retry" };
  if (lesson.waiting === "User") return { label: "lessons.status.waiting", tone: "Active" };
  return { label: STATUS[lesson.state], tone: lesson.state };
}

export function LessonsScreen({ client, revision, onOpen, onNew, onClose }: LessonsScreenProps): React.JSX.Element {
  const { t, formatDate } = useI18n();
  useDismissOnEscape(onClose);
  const [lessons, setLessons] = useState<LessonSummary[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [selected, setSelected] = useState(0);
  const listRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    let alive = true;
    client.lessons
      .list()
      .then((list) => {
        if (!alive) return;
        setFailed(false);
        setLessons([...list].sort((a, b) => b.last_at - a.last_at));
      })
      .catch(() => alive && setFailed(true));
    return () => {
      alive = false;
    };
  }, [client, revision]);

  useEffect(() => {
    listRef.current
      ?.querySelector<HTMLElement>(`[data-lesson-index="${selected}"]`)
      ?.scrollIntoView?.({ block: "nearest" });
  }, [selected]);

  const count = lessons?.length ?? 0;
  const onKeyDown = (event: React.KeyboardEvent): void => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelected((index) => Math.min(count - 1, index + 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelected((index) => Math.max(0, index - 1));
    }
  };

  return (
    <div className="ts-overlay" role="presentation">
      <section className="ts-search ts-lessons" aria-label={t("lessons.list.title")} onKeyDown={onKeyDown}>
        <div className="ts-search__query">
          <span className="ts-history__title">{t("lessons.list.title")}</span>
          <span className="ts-spacer" />
          <button type="button" className="ts-btn ts-btn--accent" onClick={onNew}>
            {t("lessons.list.new")}
          </button>
          <button
            type="button"
            className="ts-btn ts-btn--ghost"
            onClick={onClose}
            aria-label={t("lessons.list.close")}
          >
            <Icon name="x" size={14} />
          </button>
        </div>

        <div className="ts-search__results ts-lessons__list" ref={listRef} role="list">
          {failed && <p className="ts-search__error">{t("lessons.list.failed")}</p>}
          {lessons !== null && lessons.length === 0 && (
            <div className="ts-lessons__empty">
              <p className="ts-search__empty">{t("lessons.list.empty")}</p>
              <button type="button" className="ts-btn ts-btn--accent" onClick={onNew}>
                {t("lessons.list.new")}
              </button>
            </div>
          )}
          {(lessons ?? []).map((lesson, index) => (
            <div key={lesson.path} role="listitem">
              <button
                type="button"
                data-lesson-index={index}
                className={index === selected ? "ts-lessons__row ts-lessons__row--on" : "ts-lessons__row"}
                autoFocus={index === 0}
                onFocus={() => setSelected(index)}
                onClick={() => onOpen(lesson.path)}
              >
                <span className="ts-lessons__text">
                  <span className="ts-lessons__name">{lesson.goal}</span>
                  <span className="ts-lessons__sub">
                    {t("lessons.list.meta", {
                      when: formatDate(new Date(lesson.last_at)),
                      steps: lesson.steps,
                    })}
                  </span>
                </span>
                <span className={`ts-lessons__state ts-lessons__state--${statusOf(lesson).tone}`}>
                  {t(statusOf(lesson).label)}
                </span>
              </button>
            </div>
          ))}
        </div>
        <div className="ts-search__hints ts-replace__foot">
          <span>{t("lessons.list.hints")}</span>
        </div>
      </section>
    </div>
  );
}
