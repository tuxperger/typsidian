"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  AiFailure,
  Confidence,
  Language,
  LessonEvent,
  LessonView,
  UserAnswer,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { useI18n } from "../i18n/index.js";
import {
  failureKind,
  newRequestId,
  takeLessonStart,
  useLessonEnv,
} from "./lessonEnv.js";
import {
  canAnswer,
  EMPTY_DRAFT,
  LessonQuestion,
  PastQuestion,
  type AnswerDraft,
} from "./LessonQuestion.js";

// Вкладка урока (фича 051, макет A–C, F, I). Формат, ход и продолжение решает ядро;
// здесь — показ переписки, форма ответа и опрос потока: текст идущего ответа
// забирается раз в 150 мс, промежуточный файл на диске обновляется раз в секунду.

const PROGRESS_MS = 150;
const CHECKPOINT_MS = 1000;

export interface LessonTabProps {
  client: VaultClient;
  path: string;
}

/** Идущий ход: что уже накопил поток. */
interface ActiveTurn {
  requestId: string;
  text: string;
  reading: string | undefined;
  /** Ответ, ушедший в файл в начале хода: показывается, пока вид не перечитан. */
  pending: UserAnswer | undefined;
}

type Failure = AiFailure["kind"] | "Unknown";

export function LessonTab({ client, path }: LessonTabProps): React.JSX.Element {
  const { t, lang, formatNumber } = useI18n();
  const env = useLessonEnv();
  const [view, setView] = useState<LessonView | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [turn, setTurn] = useState<ActiveTurn | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const [changed, setChanged] = useState(false);
  const [draft, setDraft] = useState<AnswerDraft>(EMPTY_DRAFT);
  const feedEnd = useRef<HTMLDivElement | null>(null);
  const stopRef = useRef<HTMLButtonElement | null>(null);
  const running = useRef(false);

  useEffect(() => {
    let alive = true;
    setView(null);
    setLoadFailed(false);
    client.lessons
      .open(path)
      .then((opened) => alive && setView(opened))
      .catch(() => alive && setLoadFailed(true));
    return () => {
      alive = false;
    };
  }, [client, path]);

  const reload = useCallback(async (): Promise<void> => {
    try {
      setView(await client.lessons.open(path));
    } catch {
      setLoadFailed(true);
    }
  }, [client, path]);

  const run = useCallback(
    async (answer: UserAnswer | undefined): Promise<void> => {
      if (view === null || running.current) return;
      running.current = true;
      const requestId = newRequestId();
      setTurn({ requestId, text: "", reading: undefined, pending: answer });
      setFailure(null);
      setChanged(false);
      let keepDraft = false;
      try {
        setView(await client.lessons.answer(env.vaultId, path, requestId, answer, view.events_len));
      } catch (error) {
        const kind = failureKind(error);
        if (kind === "LessonChanged") {
          setChanged(true);
          keepDraft = true;
        } else {
          setFailure(kind ?? "Unknown");
        }
        // Ответ и оборванная реплика уже в файле: показываем то, что записано.
        await reload();
      } finally {
        running.current = false;
        setTurn(null);
        if (!keepDraft && answer !== undefined) setDraft(EMPTY_DRAFT);
      }
    },
    [client, env.vaultId, path, reload, view],
  );

  // Первый ход урока, заказанный кнопкой «Начать урок» в диалоге: один раз.
  const started = useRef(false);
  useEffect(() => {
    if (view === null || started.current || view.waiting !== "Start" || view.read_only) return;
    if (!takeLessonStart(path)) return;
    started.current = true;
    void run(undefined);
  }, [view, path, run]);

  // Опрос потока и промежуточного файла, пока идёт ход.
  const requestId = turn?.requestId;
  useEffect(() => {
    if (requestId === undefined) return;
    let alive = true;
    let polling = false;
    let checkpointing = false;
    const poll = window.setInterval(() => {
      if (polling) return;
      polling = true;
      client.lessons
        .progress(env.vaultId, requestId)
        .then((progress) => {
          if (!alive || progress === undefined) return;
          setTurn((current) =>
            current !== null && current.requestId === requestId
              ? { ...current, text: progress.text, reading: progress.reading ?? undefined }
              : current,
          );
        })
        .catch(() => undefined)
        .finally(() => {
          polling = false;
        });
    }, PROGRESS_MS);
    const checkpoint = window.setInterval(() => {
      if (checkpointing) return;
      checkpointing = true;
      client.lessons
        .checkpoint(env.vaultId, path, requestId)
        .catch(() => undefined)
        .finally(() => {
          checkpointing = false;
        });
    }, CHECKPOINT_MS);
    return () => {
      alive = false;
      window.clearInterval(poll);
      window.clearInterval(checkpoint);
    };
  }, [client, env.vaultId, path, requestId]);

  // Форма ответа пропала вместе с фокусом: Esc должен остаться под рукой.
  useEffect(() => {
    if (requestId !== undefined) stopRef.current?.focus();
  }, [requestId]);

  // Лента прокручена к последнему.
  const eventCount = view?.events.length ?? 0;
  const streamed = turn?.text.length ?? 0;
  useEffect(() => {
    feedEnd.current?.scrollIntoView?.({ block: "end" });
  }, [eventCount, streamed, turn === null]);

  const stop = useCallback((): void => {
    if (turn !== null) client.ai.cancel(env.vaultId, turn.requestId);
  }, [client, env.vaultId, turn]);

  const submit = useCallback((): void => {
    if (view === null) return;
    const choices = view.last_question?.choices ?? [];
    let text = draft.text.trim();
    if (choices.length > 0) {
      if (draft.choice === null) return;
      text = t("lessons.q.chosen", { number: draft.choice + 1, text: choices[draft.choice] ?? "" });
    }
    const thinking = draft.thinking.trim();
    void run({
      text,
      ...(choices.length > 0 && draft.choice !== null ? { choice: draft.choice } : {}),
      ...(draft.confidence !== null ? { confidence: draft.confidence } : {}),
      ...(thinking !== "" ? { thinking } : {}),
    });
  }, [draft, run, t, view]);

  const setState = useCallback(
    async (state: "Active" | "Finished"): Promise<void> => {
      try {
        setView(await client.lessons.setState(path, state));
      } catch {
        await reload();
      }
    },
    [client, path, reload],
  );

  const setLanguage = useCallback(
    async (language: Language): Promise<void> => {
      try {
        setView(await client.lessons.setLanguage(path, language));
      } catch {
        await reload();
      }
    },
    [client, path, reload],
  );

  const model = useMemo(() => {
    if (view === null) return "";
    for (let i = view.events.length - 1; i >= 0; i -= 1) {
      const event = view.events[i];
      if (event?.kind === "Teacher" && event.model !== "") return event.model;
    }
    return "";
  }, [view]);

  if (view === null) {
    return (
      <section className="ts-pane ts-lesson" aria-label={t("lessons.command.list")}>
        <div className="ts-placeholder">
          <p role="status">{loadFailed ? t("lessons.loadFailed") : t("lessons.loading")}</p>
        </div>
      </section>
    );
  }

  const volume = view.input_tokens + view.output_tokens;
  const costText =
    view.cost_usd !== undefined
      ? t("lessons.meter.cost", {
          cost: new Intl.NumberFormat(lang, {
            minimumFractionDigits: view.cost_usd >= 0.01 ? 2 : 4,
            maximumFractionDigits: view.cost_usd >= 0.01 ? 2 : 4,
          }).format(view.cost_usd),
        })
      : volume === 0
        ? t("lessons.meter.cost", { cost: formatNumber(0) })
        : t("lessons.meter.costUnknown");
  const volumeText =
    volume >= 1000
      ? t("lessons.meter.volumeK", { count: formatNumber(Math.round(volume / 1000)) })
      : t("lessons.meter.volume", { count: formatNumber(volume) });

  const lastIndex = view.events.length - 1;
  const answering = view.waiting === "User" && turn === null && !view.read_only;
  const lastTeacher = view.events[lastIndex];
  // Вопрос последней реплики учителя переходит в форму ответа, даже если после неё в ленте
  // уже есть строки чтения заметок или смена языка.
  let lastTeacherIndex = -1;
  for (let i = lastIndex; i >= 0; i -= 1) {
    if (view.events[i]?.kind === "Teacher") {
      lastTeacherIndex = i;
      break;
    }
  }
  const cutFailure: Failure | null =
    failure ?? (lastTeacher?.kind === "Teacher" && lastTeacher.interrupted ? (lastTeacher.failure?.kind ?? null) : null);
  const retryShown = view.waiting === "Retry" && turn === null && !view.read_only;
  const language = view.language;

  const failureText = (kind: Failure): string => t(`ai.failure.${kind}`);

  return (
    <section
      className="ts-pane ts-lesson"
      aria-label={view.header.goal}
      onKeyDown={(event) => {
        // Esc останавливает идущий ответ учителя (макет B); без хода его слушает кто-то другой.
        if (event.key === "Escape" && turn !== null) {
          event.preventDefault();
          event.stopPropagation();
          stop();
        }
      }}
    >
      {changed && (
        <div className="ts-lesson__ro" role="status">
          {t("lessons.changed")}
        </div>
      )}
      {view.read_only && (
        <div className="ts-lesson__ro" role="status">
          {t("lessons.readOnly")}
        </div>
      )}

      <header className="ts-lesson__head">
        <div className="ts-lesson__goal">
          <b>{view.header.goal}</b>
          <small>{model === "" ? view.path : `${view.path} · ${model}`}</small>
        </div>
        <div className="ts-lesson__meter" role="group" aria-label={t("lessons.meter.label")}>
          <b>{costText}</b>
          <br />
          {volumeText}
        </div>
      </header>

      <div className="ts-lesson__tools">
        <span className="ts-lesson__hint">{t("lessons.lang.label")}</span>
        <span className="ts-lesson__seg" role="group" aria-label={t("lessons.lang.label")}>
          {(["ru", "en"] as const).map((item) => (
            <button
              key={item}
              type="button"
              aria-pressed={language === item}
              disabled={turn !== null || view.read_only}
              onClick={() => language !== item && void setLanguage(item)}
            >
              {item === "ru" ? t("lessons.lang.ru") : t("lessons.lang.en")}
            </button>
          ))}
        </span>
        <span className="ts-spacer" />
        {view.state === "Active" ? (
          <button
            type="button"
            className="ts-btn"
            disabled={turn !== null || view.read_only}
            onClick={() => void setState("Finished")}
          >
            {t("lessons.finish")}
          </button>
        ) : (
          <button
            type="button"
            className="ts-btn"
            disabled={view.read_only}
            onClick={() => void setState("Active")}
          >
            {t("lessons.resume")}
          </button>
        )}
      </div>

      <div className="ts-lesson__feed" role="log" aria-label={t("lessons.feed")}>
        <Bubble who={t("lessons.who.you")}>{view.header.goal}</Bubble>

        {view.events.map((event, index) => (
          <EventRow
            key={`${index}:${event.kind}:${event.at}`}
            event={event}
            hideQuestion={answering && index === lastTeacherIndex}
            onOpenNote={env.openNote}
          />
        ))}

        {turn?.pending !== undefined && (
          <UserBubble answer={turn.pending} />
        )}
        {turn !== null && turn.text !== "" && (
          <div className="ts-lesson__t">
            <div className="ts-lesson__who">{t("lessons.who.teacher")}</div>
            <div className="ts-lesson__body ts-lesson__stream">{turn.text}</div>
          </div>
        )}

        {view.waiting === "Start" && turn === null && !view.read_only && (
          <div className="ts-lesson__box">
            {t("lessons.start.hint")}
            <div className="ts-lesson__acts">
              <button type="button" className="ts-btn ts-btn--accent" onClick={() => void run(undefined)}>
                {t("lessons.start.button")}
              </button>
            </div>
          </div>
        )}

        {answering && (
          <LessonQuestion
            question={view.last_question ?? undefined}
            draft={draft}
            onDraft={setDraft}
            onSubmit={() => canAnswer(view.last_question ?? undefined, draft) && submit()}
          />
        )}

        {(retryShown || (failure !== null && turn === null)) && (
          <div className="ts-lesson__box ts-lesson__box--err" role="alert">
            {cutFailure !== null && <span className="ts-lesson__bad">{failureText(cutFailure)} </span>}
            {retryShown && lastTeacher?.kind === "Teacher" && lastTeacher.interrupted ? (
              t("lessons.cutSaved")
            ) : retryShown && cutFailure === null ? (
              t("lessons.failedPrefix")
            ) : null}
            {retryShown && (
              <div className="ts-lesson__acts">
                <button
                  type="button"
                  className="ts-btn ts-btn--accent"
                  onClick={() => void run(undefined)}
                >
                  {t("lessons.retry")}
                </button>
              </div>
            )}
          </div>
        )}

        {view.waiting === "Closed" && (
          <p className="ts-lesson__hint" role="status">
            {view.state === "Finished" ? t("lessons.closed.finished") : t("lessons.closed.stopped")}
          </p>
        )}
        <div ref={feedEnd} />
      </div>

      {turn !== null && (
        <div className="ts-lesson__status" role="status">
          <span>
            ◌{" "}
            {turn.reading !== undefined
              ? t("lessons.status.reading", { path: turn.reading })
              : t("lessons.status.answering")}
          </span>
          <button ref={stopRef} type="button" className="ts-btn ts-lesson__stop" onClick={stop}>
            {t("lessons.stop")} <kbd className="ts-lesson__kbd">{t("lessons.stopKey")}</kbd>
          </button>
        </div>
      )}
    </section>
  );
}

function Bubble({ who, children }: { who: string; children: React.ReactNode }): React.JSX.Element {
  return (
    <div className="ts-lesson__t ts-lesson__t--user">
      <div className="ts-lesson__who">{who}</div>
      <div className="ts-lesson__body">{children}</div>
    </div>
  );
}

function ConfidenceChip({ level }: { level: Confidence }): React.JSX.Element {
  const { t } = useI18n();
  return (
    <span className="ts-lesson__chip">
      {t("lessons.chip.confidence", { level: t(`lessons.q.confidence.${level}`).toLowerCase() })}
    </span>
  );
}

function UserBubble({ answer }: { answer: UserAnswer }): React.JSX.Element {
  const { t } = useI18n();
  return (
    <div className="ts-lesson__t ts-lesson__t--user">
      <div className="ts-lesson__who">{t("lessons.who.you")}</div>
      <div className="ts-lesson__body">{answer.text}</div>
      {answer.confidence != null && (
        <div className="ts-lesson__meta">
          <ConfidenceChip level={answer.confidence} />
        </div>
      )}
      {answer.thinking != null && answer.thinking !== "" && (
        <div className="ts-lesson__think">{answer.thinking}</div>
      )}
    </div>
  );
}

function NoteLink({ path, onOpen }: { path: string; onOpen(path: string): void }): React.JSX.Element {
  const { t } = useI18n();
  return (
    <button
      type="button"
      className="ts-lesson__link"
      aria-label={t("lessons.read.open", { path })}
      onClick={() => onOpen(path)}
    >
      {path}
    </button>
  );
}

function EventRow({
  event,
  hideQuestion,
  onOpenNote,
}: {
  event: LessonEvent;
  hideQuestion: boolean;
  onOpenNote(path: string): void;
}): React.JSX.Element | null {
  const { t } = useI18n();
  switch (event.kind) {
    case "User":
      return (
        <UserBubble
          answer={{
            text: event.text,
            ...(event.confidence !== undefined ? { confidence: event.confidence } : {}),
            ...(event.thinking !== undefined ? { thinking: event.thinking } : {}),
          }}
        />
      );
    case "Teacher": {
      const cut = event.interrupted === true;
      return (
        <>
          <div className="ts-lesson__t">
            <div className="ts-lesson__who">{cut ? t("lessons.who.teacherCut") : t("lessons.who.teacher")}</div>
            {event.text !== "" && <div className="ts-lesson__body">{event.text}</div>}
          </div>
          {event.question !== undefined && !hideQuestion && <PastQuestion question={event.question} />}
        </>
      );
    }
    case "NoteRead": {
      const denied = event.outcome.kind !== "Sent";
      const byUser = event.by_user;
      return (
        <div className={denied ? "ts-lesson__read ts-lesson__read--deny" : "ts-lesson__read"}>
          <span>
            {denied
              ? t("lessons.read.askedBefore")
              : byUser
                ? t("lessons.read.byUser")
                : t("lessons.read.before")}
          </span>
          <NoteLink path={event.path} onOpen={onOpenNote} />
          <span>
            {event.outcome.kind === "Denied"
              ? t("lessons.read.denied")
              : event.outcome.kind === "Missing"
                ? t("lessons.read.missing")
                : event.outcome.truncated
                  ? t("lessons.read.truncated")
                  : t("lessons.read.after")}
          </span>
        </div>
      );
    }
    case "Summary":
      return <div className="ts-lesson__sep">{t("lessons.sep.summary", { count: event.covers })}</div>;
    case "Language":
      return (
        <div className="ts-lesson__sep">
          {t("lessons.sep.language", {
            language: event.language === "ru" ? t("lessons.lang.ru") : t("lessons.lang.en"),
          })}
        </div>
      );
    case "State":
      return (
        <div className="ts-lesson__sep">
          {event.state === "Finished"
            ? t("lessons.sep.finished")
            : event.state === "Stopped"
              ? t("lessons.sep.stopped")
              : t("lessons.sep.resumed")}
        </div>
      );
    default: {
      const exhaustive: never = event;
      return exhaustive;
    }
  }
}
