"use client";

import { useEffect, useId, useMemo, useState } from "react";
import type { LessonSummary, Note } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { useI18n } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import { BlockedNotice, PROVIDER_LABEL, useAiReadiness, useClosedPaths } from "./aiReadiness.js";
import { failureKind, requestLessonStart } from "./lessonEnv.js";

// Диалог «Новая практика» (фича 053, макет A): материал — уроки и заметки либо описание
// темы. Какие заметки пустить к модели, решает ядро (проверка доступа 050); здесь только
// выбор. «Начать практику» создаёт файл и открывает вкладку, которая сама заказывает
// первую задачу: это и есть явное действие пользователя.

export interface NewPracticeDialogProps {
  client: VaultClient;
  vaultId: string;
  /** Заметки vault — для выбора материала. */
  notes: readonly Note[];
  /** Язык интерфейса — язык практики по умолчанию. */
  language: "ru" | "en";
  onCreated: (path: string) => void;
  onOpenSettings: () => void;
  onCancel: () => void;
}

type Mode = "sources" | "topic";

const PICK_LIMIT = 8;
/** Название практики по описанию темы: начало первой строки. */
const TITLE_FROM_TOPIC = 60;

/** Имя заметки без папки и расширения — название практики по умолчанию. */
function baseName(path: string): string {
  const name = path.split("/").pop() ?? path;
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(0, dot) : name;
}

export function NewPracticeDialog({
  client,
  vaultId,
  notes,
  language,
  onCreated,
  onOpenSettings,
  onCancel,
}: NewPracticeDialogProps): React.JSX.Element {
  const { t } = useI18n();
  const id = useId();
  useDismissOnEscape(onCancel);
  const readiness = useAiReadiness(client, vaultId);
  const [mode, setMode] = useState<Mode>("sources");
  const [lessons, setLessons] = useState<LessonSummary[] | null>(null);
  const [chosenLessons, setChosenLessons] = useState<string[]>([]);
  const [chosenNotes, setChosenNotes] = useState<string[]>([]);
  const [picking, setPicking] = useState(false);
  const [query, setQuery] = useState("");
  const [name, setName] = useState("");
  const [topic, setTopic] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Уроки для выбора: сами практики материалом не служат.
  useEffect(() => {
    let alive = true;
    client.lessons
      .list()
      .then((list) => alive && setLessons(list.filter((lesson) => lesson.kind !== "Practice").sort((a, b) => b.last_at - a.last_at)))
      .catch(() => alive && setLessons([]));
    return () => {
      alive = false;
    };
  }, [client]);

  const matches = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return notes
      .filter((note) => note.kind === "Note" && !chosenNotes.includes(note.path))
      .filter((note) => needle === "" || note.path.toLowerCase().includes(needle))
      .slice(0, PICK_LIMIT);
  }, [notes, query, chosenNotes]);

  // Закрытые для агента уроки и заметки (видимые и выбранные) помечаются и не выбираются.
  const closed = useClosedPaths(client, [
    ...(lessons ?? []).map((lesson) => lesson.path),
    ...matches.map((note) => note.path),
    ...chosenNotes,
  ]);

  const firstLesson =lessons?.find((lesson) => chosenLessons.includes(lesson.path));
  const fallbackName = firstLesson?.goal ?? (chosenNotes[0] !== undefined ? baseName(chosenNotes[0]) : "");
  const topicName = Array.from(topic.trim().split("\n")[0] ?? "").slice(0, TITLE_FROM_TOPIC).join("");
  const title = name.trim() !== "" ? name.trim() : mode === "sources" ? fallbackName : topicName;
  const ready =
    mode === "sources" ? chosenLessons.length + chosenNotes.length > 0 && title !== "" : topic.trim() !== "" && title !== "";

  const create = async (): Promise<void> => {
    if (busy || !ready || readiness.state !== "ready") return;
    setBusy(true);
    setError(null);
    try {
      const path =
        mode === "sources"
          ? await client.lessons.createPractice(title, chosenLessons, chosenNotes, undefined, language)
          : await client.lessons.createPractice(title, [], [], topic.trim(), language);
      requestLessonStart(path);
      onCreated(path);
    } catch (failure) {
      const kind = failureKind(failure);
      setError(kind === null ? t("ai.failure.Unknown") : t(`ai.failure.${kind}`));
      setBusy(false);
    }
  };

  const toggleLesson = (path: string): void =>
    setChosenLessons((list) => (list.includes(path) ? list.filter((item) => item !== path) : [...list, path]));

  const blocked = readiness.state === "blocked" ? readiness : null;
  const settings = readiness.state === "ready" ? readiness.settings : null;

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
          {t("practice.new.title")}
        </h2>

        {readiness.state === "checking" && <p role="status">{t("lessons.new.checking")}</p>}

        {blocked !== null && <BlockedNotice blocked={blocked} onCancel={onCancel} onOpenSettings={onOpenSettings} />}

        {settings !== null && (
          <>
            <span className="ts-lesson__seg ts-practice__mode" role="group" aria-label={t("practice.new.mode")}>
              {(["sources", "topic"] as const).map((item) => (
                <button key={item} type="button" aria-pressed={mode === item} onClick={() => setMode(item)}>
                  {item === "sources" ? t("practice.new.mode.sources") : t("practice.new.mode.topic")}
                </button>
              ))}
            </span>

            {mode === "sources" ? (
              <>
                <div className="ts-lesson__lbl" id={`${id}-lessons`}>
                  {t("practice.new.lessons")}
                </div>
                <div className="ts-practice__srcs" role="group" aria-labelledby={`${id}-lessons`}>
                  {lessons === null && <p className="ts-lesson__hint">{t("lessons.new.checking")}</p>}
                  {lessons !== null && lessons.length === 0 && (
                    <p className="ts-lesson__hint">{t("practice.new.noLessons")}</p>
                  )}
                  {(lessons ?? []).map((lesson) => {
                    const shut = closed.has(lesson.path);
                    return (
                      <label key={lesson.path} className={`ts-practice__src${shut ? " ts-practice__src--closed" : ""}`}>
                        <input
                          type="checkbox"
                          checked={!shut && chosenLessons.includes(lesson.path)}
                          disabled={shut}
                          onChange={() => toggleLesson(lesson.path)}
                        />
                        <span className="ts-practice__srctext">
                          <span className="ts-practice__srcname">{lesson.goal}</span>
                          <span className={`ts-practice__srcsub${shut ? " ts-practice__srcsub--closed" : ""}`}>
                            {shut ? t("practice.new.closed") : t("practice.new.lessonMeta", { steps: lesson.steps })}
                          </span>
                        </span>
                      </label>
                    );
                  })}
                </div>

                <div className="ts-lesson__lbl">
                  {t("practice.new.notes")} <span className="ts-lesson__hint">{t("lessons.q.optional")}</span>
                </div>
                <div className="ts-lesson__chips">
                  {chosenNotes.map((path) => (
                    <button
                      key={path}
                      type="button"
                      className="ts-lesson__chip ts-lesson__chip--on"
                      aria-label={t("lessons.new.remove", { path })}
                      onClick={() => setChosenNotes((list) => list.filter((item) => item !== path))}
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
                              setChosenNotes((list) => [...list, note.path]);
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
              </>
            ) : (
              <>
                <label className="ts-lesson__lbl" htmlFor={`${id}-topic`}>
                  {t("practice.new.topic")}
                </label>
                <textarea
                  id={`${id}-topic`}
                  className="ts-lesson__ta"
                  autoFocus
                  rows={3}
                  maxLength={2000}
                  placeholder={t("practice.new.topicPlaceholder")}
                  value={topic}
                  onChange={(event) => setTopic(event.target.value)}
                />
              </>
            )}

            <label className="ts-lesson__lbl" htmlFor={`${id}-name`}>
              {t("practice.new.nameLabel")}
            </label>
            <input
              id={`${id}-name`}
              className="ts-field__input"
              maxLength={200}
              placeholder={mode === "sources" ? t("practice.new.namePlaceholder") : t("practice.new.nameTopicPlaceholder")}
              value={name}
              onChange={(event) => setName(event.target.value)}
            />

            <p className="ts-lesson__hint ts-practice__newhint">
              {t("practice.new.hint", {
                folder: settings.lessons_folder ?? "",
                provider: t(PROVIDER_LABEL[settings.provider ?? "Anthropic"]),
                model: settings.model ?? "",
              })}
            </p>
            {!ready && (
              <p className="ts-lesson__hint" role="status">
                {mode === "sources" ? t("practice.new.needSource") : t("practice.new.needTopic")}
              </p>
            )}
            {error !== null && (
              <p className="ts-lesson__bad" role="alert">
                {error}
              </p>
            )}
            <div className="ts-lesson__acts">
              <button type="button" className="ts-btn" onClick={onCancel}>
                {t("lessons.new.cancel")}
              </button>
              <button type="button" className="ts-btn ts-btn--accent" disabled={busy || !ready} onClick={() => void create()}>
                {t("practice.new.start")}
              </button>
            </div>
          </>
        )}
      </section>
    </div>
  );
}
