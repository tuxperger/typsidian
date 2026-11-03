"use client";

import { useId, useMemo, useState } from "react";
import type { LessonPhase, Note } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { useI18n } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import { BlockedNotice, PROVIDER_LABEL, useAiReadiness, useClosedPaths } from "./aiReadiness.js";
import { failureKind, requestLessonStart } from "./lessonEnv.js";

// Диалог «Новый урок» (фича 051, макет D, E): цель, заметки для опоры и условия, при
// которых начать нельзя. «Начать урок» создаёт файл и открывает вкладку, которая сама
// заказывает первый ход: это и есть явное действие пользователя.

export interface NewLessonDialogProps {
  client: VaultClient;
  vaultId: string;
  /** Заметки vault — для выбора опоры. */
  notes: readonly Note[];
  /** Язык интерфейса — язык учителя по умолчанию. */
  language: "ru" | "en";
  onCreated: (path: string) => void;
  onOpenSettings: () => void;
  onCancel: () => void;
}

const PICK_LIMIT = 8;

export function NewLessonDialog({
  client,
  vaultId,
  notes,
  language,
  onCreated,
  onOpenSettings,
  onCancel,
}: NewLessonDialogProps): React.JSX.Element {
  const { t } = useI18n();
  const id = useId();
  useDismissOnEscape(onCancel);
  const readiness = useAiReadiness(client, vaultId);
  const [goal, setGoal] = useState("");
  const [chosen, setChosen] = useState<string[]>([]);
  const [picking, setPicking] = useState(false);
  const [query, setQuery] = useState("");
  // Замер понимания перед уроком — по умолчанию (052, макет A); его можно пропустить.
  const [start, setStart] = useState<LessonPhase>("Probe");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const matches = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return notes
      .filter((note) => note.kind === "Note" && !chosen.includes(note.path))
      .filter((note) => needle === "" || note.path.toLowerCase().includes(needle))
      .slice(0, PICK_LIMIT);
  }, [notes, query, chosen]);

  const closed = useClosedPaths(client, [...matches.map((note) => note.path), ...chosen]);

  const create = async (): Promise<void> => {
    if (busy || goal.trim() === "" || readiness.state !== "ready") return;
    setBusy(true);
    setError(null);
    try {
      const path = await client.lessons.create(goal.trim(), chosen, language, start);
      requestLessonStart(path);
      onCreated(path);
    } catch (failure) {
      const kind = failureKind(failure);
      setError(kind === null ? t("ai.failure.Unknown") : t(`ai.failure.${kind}`));
      setBusy(false);
    }
  };

  const blocked = readiness.state === "blocked" ? readiness : null;
  const settings = readiness.state === "ready" ? readiness.settings : null;
  const title = t("lessons.new.title");

  return (
    <div className="ts-overlay ts-overlay--center" role="presentation">
      <section
        className="ts-lesson-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby={`${id}-title`}
        onKeyDown={(event) => {
          if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
            event.preventDefault();
            void create();
          }
        }}
      >
        <h2 className="ts-lesson-dialog__title" id={`${id}-title`}>
          {title}
        </h2>

        {readiness.state === "checking" && <p role="status">{t("lessons.new.checking")}</p>}

        {blocked !== null && <BlockedNotice blocked={blocked} onCancel={onCancel} onOpenSettings={onOpenSettings} />}

        {settings !== null && (
          <>
            <label className="ts-lesson__lbl" htmlFor={`${id}-goal`}>
              {t("lessons.new.goal")}
            </label>
            <textarea
              id={`${id}-goal`}
              className="ts-lesson__ta"
              autoFocus
              rows={2}
              maxLength={500}
              placeholder={t("lessons.new.goalPlaceholder")}
              value={goal}
              onChange={(event) => setGoal(event.target.value)}
            />

            <div className="ts-lesson__lbl">
              {t("lessons.new.notes")} <span className="ts-lesson__hint">{t("lessons.q.optional")}</span>
            </div>
            <div className="ts-lesson__chips">
              {chosen.map((path) => (
                <button
                  key={path}
                  type="button"
                  className="ts-lesson__chip ts-lesson__chip--on"
                  aria-label={t("lessons.new.remove", { path })}
                  onClick={() => setChosen((list) => list.filter((item) => item !== path))}
                >
                  {path} ✕
                </button>
              ))}
              <button type="button" className="ts-btn" aria-expanded={picking} onClick={() => setPicking((open) => !open)}>
                {t("lessons.new.add")}
              </button>
            </div>
            {picking && (
              <div className="ts-lesson__pick">
                <input
                  className="ts-field__input"
                  aria-label={t("lessons.new.search")}
                  placeholder={t("lessons.new.search")}
                  value={query}
                  autoFocus
                  onChange={(event) => setQuery(event.target.value)}
                />
                <ul className="ts-lesson__picklist">
                  {matches.length === 0 && <li className="ts-lesson__hint">{t("lessons.new.noMatches")}</li>}
                  {matches.map((note) => (
                    <li key={note.path}>
                      <button
                        type="button"
                        className="ts-lesson__pickrow"
                        disabled={closed.has(note.path)}
                        onClick={() => {
                          setChosen((list) => [...list, note.path]);
                          setQuery("");
                        }}
                      >
                        {note.path}
                        {closed.has(note.path) && (
                          <span className="ts-practice__srcsub ts-practice__srcsub--closed">{t("practice.new.closed")}</span>
                        )}
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            )}

            <div className="ts-lesson__lbl" id={`${id}-start`}>
              {t("lessons.new.startLabel")}
            </div>
            <span className="ts-lesson__seg" role="group" aria-labelledby={`${id}-start`}>
              {(["Probe", "Teach"] as const).map((item) => (
                <button key={item} type="button" aria-pressed={start === item} onClick={() => setStart(item)}>
                  {item === "Probe" ? t("lessons.new.start.probe") : t("lessons.new.start.teach")}
                </button>
              ))}
            </span>
            <p className="ts-lesson__hint ts-lesson__start-hint">
              {start === "Probe" ? t("lessons.new.start.probeHint") : t("lessons.new.start.teachHint")}
            </p>

            <p className="ts-lesson__hint">
              {t("lessons.new.hint", {
                folder: settings.lessons_folder ?? "",
                provider: t(PROVIDER_LABEL[settings.provider ?? "Anthropic"]),
                model: settings.model ?? "",
              })}
            </p>
            {error !== null && (
              <p className="ts-lesson__bad" role="alert">
                {error}
              </p>
            )}
            <div className="ts-lesson__acts">
              <button type="button" className="ts-btn" onClick={onCancel}>
                {t("lessons.new.cancel")}
              </button>
              <button
                type="button"
                className="ts-btn ts-btn--accent"
                disabled={busy || goal.trim() === ""}
                onClick={() => void create()}
              >
                {t("lessons.new.start")}
              </button>
            </div>
          </>
        )}
      </section>
    </div>
  );
}
