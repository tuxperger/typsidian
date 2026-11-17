"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  AiFailure,
  AiSettings,
  Confidence,
  Language,
  LessonEvent,
  LessonView,
  ResearchView,
  UserAnswer,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { useI18n } from "../i18n/index.js";
import { KnowledgeJobs } from "../knowledge/KnowledgeJobs.js";
import { KnowledgeLessonBlock } from "../knowledge/KnowledgeLessonBlock.js";
import { useKnowledgeExtract } from "../knowledge/useKnowledgeExtract.js";
import { useKnowledgeJobs } from "../knowledge/useKnowledgeJobs.js";
import {
  failureKind,
  newRequestId,
  onLessonJump,
  takeLessonJump,
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
import { LessonPlan, planProgressOf } from "./LessonPlan.js";
import { ProbeResultCard } from "./ProbeResult.js";
import { awaitingNext, hintNumbers, PracticeForm, PracticeRow } from "./PracticeFeed.js";
import { PracticeSummary } from "./PracticeSummary.js";
import { ResearchCard } from "./ResearchCard.js";
import { ResearchInput, type WebState } from "./ResearchInput.js";
import { ResearchPdfDialog } from "./ResearchPdfDialog.js";
import { useResearch } from "./useResearch.js";

// Вкладка урока (фича 051, макет A–C, F, I). Формат, ход и продолжение решает ядро;
// здесь — показ переписки, форма ответа и опрос потока: текст идущего ответа
// забирается раз в 150 мс, промежуточный файл на диске обновляется раз в секунду.

const PROGRESS_MS = 150;
const CHECKPOINT_MS = 1000;
/** Как часто вкладка перечитывает настройки ИИ (доступность поиска в вебе). */
const SETTINGS_MS = 2000;
/** Уже этой ширины вкладки план — лист со списком, а не колонка (052, макет E). */
const NARROW_PX = 640;
/** Ориентир замера в подписи стадии (инструкции учителя, 052 research §6). */
const PROBE_TARGET = 10;

/** Ширина вкладки, а не окна: рядом могут стоять другие колонки. */
function useNarrow(target: React.RefObject<HTMLElement | null>, active: boolean): boolean {
  const [narrow, setNarrow] = useState(false);
  useEffect(() => {
    const element = target.current;
    if (!active || element === null || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => setNarrow(element.clientWidth < NARROW_PX));
    observer.observe(element);
    setNarrow(element.clientWidth < NARROW_PX);
    return () => observer.disconnect();
  }, [target, active]);
  return narrow;
}

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

const NO_RESEARCH: readonly ResearchView[] = [];

export function LessonTab({ client, path }: LessonTabProps): React.JSX.Element {
  const { t, lang, formatNumber } = useI18n();
  const env = useLessonEnv();
  const [view, setView] = useState<LessonView | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [turn, setTurn] = useState<ActiveTurn | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const [changed, setChanged] = useState(false);
  const [draft, setDraft] = useState<AnswerDraft>(EMPTY_DRAFT);
  // `null` — как решит ширина: колонка открыта, лист закрыт.
  const [planOpen, setPlanOpen] = useState<boolean | null>(null);
  // Сводка практики (053, макет E): закрыта, пока не открыта кнопкой.
  const [summaryOpen, setSummaryOpen] = useState(false);
  const feedEnd = useRef<HTMLDivElement | null>(null);
  const feedRef = useRef<HTMLDivElement | null>(null);
  const sectionRef = useRef<HTMLElement | null>(null);
  const stopRef = useRef<HTMLButtonElement | null>(null);
  const running = useRef(false);
  const narrow = useNarrow(sectionRef, view !== null);

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

  // Граф знаний (фича 054): ядро говорит «пора» в ответе на ход, запрос делает вкладка и
  // урока не ждёт. Закончив, перечитывает вид, если в этот момент не идёт ход.
  const { jobs: knowledgeJobs } = useKnowledgeJobs(client, env.vaultId);
  const extract = useKnowledgeExtract(client, env.vaultId, path, async () => {
    if (!running.current) await reload();
  });
  const startExtract = extract.start;

  // Исследования (фича 056): вид перечитывается, пока что-то идёт. Во время хода урока
  // обновляется только список исследований: события хода вид получит из ответа.
  const lastResearch = useRef("");
  const refreshResearch = useCallback(async (): Promise<void> => {
    try {
      const fresh = await client.lessons.open(path);
      const stamp = `${fresh.events_len}:${JSON.stringify(fresh.research)}`;
      if (stamp === lastResearch.current) return;
      lastResearch.current = stamp;
      if (running.current) {
        setView((current) => (current === null ? current : { ...current, research: fresh.research }));
      } else {
        setView(fresh);
      }
    } catch {
      // Следующий такт попробует снова.
    }
  }, [client, path]);
  const research = useResearch(client, env.vaultId, path, view?.research ?? NO_RESEARCH, refreshResearch);
  const [pdfDialog, setPdfDialog] = useState<string | null>(null);

  // Настройки ИИ: доступность поиска в вебе для кнопки «В вебе».
  const [aiSettings, setAiSettings] = useState<AiSettings | null>(null);
  const reloadSettings = useRef<() => void>(() => undefined);
  // Увеличивается, когда PDF запущен: поле «Исследовать…» очищается.
  const [sent, setSent] = useState(0);
  useEffect(() => {
    let alive = true;
    const load = (): void => {
      client.ai
        .settings()
        .then((found) => alive && setAiSettings(found))
        .catch(() => undefined);
    };
    load();
    // Настройки правятся в другом разделе, а вкладка урока остаётся открытой: обновляем по
    // таймеру, при возврате в окно и при касании поля «Исследовать…» (`onTouch`).
    const timer = window.setInterval(load, SETTINGS_MS);
    window.addEventListener("focus", load);
    reloadSettings.current = load;
    return () => {
      alive = false;
      window.clearInterval(timer);
      window.removeEventListener("focus", load);
    };
  }, [client]);

  const run = useCallback(
    async (answer: UserAnswer | undefined, base: LessonView | null = view): Promise<void> => {
      if (base === null || running.current) return;
      running.current = true;
      const requestId = newRequestId();
      setTurn({ requestId, text: "", reading: undefined, pending: answer });
      setFailure(null);
      setChanged(false);
      let keepDraft = false;
      try {
        // Исследование дописывает события в файл урока само: ответ с устаревшим числом событий
        // отклоняется ядром (`LessonChanged`). Если после вида появились только события
        // исследования, повторяем с новым числом — иначе это чужая правка, и решает ядро.
        let expected = base.events_len;
        let next: LessonView;
        for (let attempt = 0; ; attempt += 1) {
          try {
            next = await client.lessons.answer(env.vaultId, path, requestId, answer, expected);
            break;
          } catch (error) {
            if (failureKind(error) !== "LessonChanged" || attempt >= 3) throw error;
            const fresh = await client.lessons.open(path);
            const added = fresh.events.slice(base.events_len);
            const onlyResearch =
              fresh.events.length >= base.events_len &&
              added.length > 0 &&
              added.every((event) => event.kind.startsWith("Research"));
            if (!onlyResearch) throw error;
            expected = fresh.events_len;
          }
        }
        setView(next);
        // Один раз на ответ хода, а не на каждое открытие вида (research 054 §8).
        if (next.knowledge_auto_due) startExtract();
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
        // Подсказка и «Сдаюсь» не отправляют написанное: черновик остаётся.
        if (!keepDraft && answer !== undefined && (answer.action ?? "Answer") === "Answer") setDraft(EMPTY_DRAFT);
      }
    },
    [client, env.vaultId, path, reload, view, startExtract],
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

  // Практика (053): ответ на задачу и действия вместо ответа. Ядро само проверяет, что
  // действие применимо; устаревший вид кончается «Урок изменился».
  const sendProblem = useCallback((): void => {
    const text = draft.text.trim();
    if (text !== "") void run({ text });
  }, [draft.text, run]);
  const requestHint = useCallback((): void => void run({ text: "", action: "HintRequest" }), [run]);
  const giveUp = useCallback((): void => void run({ text: "", action: "GiveUp" }), [run]);
  const nextProblem = useCallback((): void => void run({ text: "", action: "NextProblem" }), [run]);

  // «Не знаю» в замере: ответ-сведение без текста (052 FR-002, сценарий 4).
  const dontKnow = useCallback((): void => {
    void run({ text: "", dont_know: true });
  }, [run]);

  // «Пропустить замер»: стадия меняется, и учитель сразу продолжает с цели.
  const skipProbe = useCallback(async (): Promise<void> => {
    if (running.current) return;
    try {
      const next = await client.lessons.skipProbe(path);
      setView(next);
      await run(undefined, next);
    } catch {
      await reload();
    }
  }, [client, path, reload, run]);

  // Нажатие на шаг плана: лента прокручивается к реплике, с которой шаг стал текущим.
  const jumpTo = useCallback((anchor: number): void => {
    const holder = feedRef.current?.querySelector(`[data-event="${anchor}"]`);
    const target = holder?.firstElementChild ?? holder;
    if (!(target instanceof HTMLElement)) return;
    target.scrollIntoView?.({ block: "center", behavior: "smooth" });
    target.setAttribute("data-flash", "on");
    window.setTimeout(() => target.removeAttribute("data-flash"), 1600);
    setPlanOpen((open) => (narrow ? false : open));
    if (narrow) setSummaryOpen(false);
  }, [narrow]);

  // Переход из графа знаний к событию урока (054, FR-008): сразу или когда вид загрузится.
  useEffect(() => {
    if (view === null) return;
    const go = (): void => {
      const event = takeLessonJump(path);
      if (event !== undefined) jumpTo(event);
    };
    go();
    return onLessonJump(path, go);
  }, [view, path, jumpTo]);

  const hintNumberOf = useMemo(() => hintNumbers(view?.events ?? []), [view]);

  const loadVersion = useCallback(
    (version: number) => client.lessons.planVersion(path, version),
    [client, path],
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
  const lastTeacher = view.events[lastIndex];
  // Вопрос последней реплики учителя переходит в форму ответа, даже если после неё в ленте
  // уже есть строки чтения заметок или смена языка.
  const lastOf = (kind: LessonEvent["kind"]): number => {
    for (let i = lastIndex; i >= 0; i -= 1) {
      if (view.events[i]?.kind === kind) return i;
    }
    return -1;
  };
  const lastTeacherIndex = lastOf("Teacher");
  // Итог замера (артборд C): после него идёт ожидание без вопроса, пока пользователь не нажмёт
  // «Начать обучение». Ядро отдаёт `User` (позже — `Retry`), поэтому смотрим на события.
  const lastProbeIndex = lastOf("ProbeResult");
  const probeResult = view.probe_result ?? undefined;
  const probePending =
    probeResult !== undefined &&
    !view.read_only &&
    turn === null &&
    (view.waiting === "User" || view.waiting === "Retry") &&
    view.last_question == null &&
    lastProbeIndex > lastTeacherIndex &&
    lastProbeIndex > lastOf("User");
  const isPractice = view.header.kind === "Practice";
  const practice = view.practice ?? undefined;
  const waitingUser = view.waiting === "User" && turn === null && !view.read_only;
  // Практика ждёт ответа на открытую задачу, «Следующую задачу» после итога либо — если
  // агент не поставил задачу — повтора хода.
  const practiceOpen = isPractice && waitingUser && practice?.open_problem != null;
  const practiceNext = isPractice && waitingUser && practice?.open_problem == null && awaitingNext(view.events);
  const practiceStuck = isPractice && waitingUser && !practiceOpen && !practiceNext;
  const lastVerdictIndex = lastOf("Verdict");
  const answering = waitingUser && !probePending && !isPractice;
  const plan = view.plan ?? undefined;
  const planShown = plan !== undefined && (planOpen ?? !narrow);
  const planProgress = plan === undefined ? undefined : planProgressOf(plan);
  const probing = view.phase === "Probe";
  const question = view.last_question ?? undefined;
  const probeQuestion = answering && probing && question?.part != null;
  const phaseText =
    view.probe_count === 0
      ? t("lessons.phase.probe")
      : view.probe_count <= PROBE_TARGET
        ? t("lessons.phase.probeCount", { count: view.probe_count })
        : t("lessons.phase.probeMore", { count: view.probe_count });
  const cutFailure: Failure | null =
    failure ?? (lastTeacher?.kind === "Teacher" && lastTeacher.interrupted ? (lastTeacher.failure?.kind ?? null) : null);
  // Итог замера ждёт «Начать обучение» — это не оборванный ответ.
  const retryShown = view.waiting === "Retry" && turn === null && !view.read_only && !probePending;
  const language = view.language;
  const failureText = (kind: Failure): string => t(`ai.failure.${kind}`);
  // Совпадает с проверкой ядра (`web_search_available`): свой адрес поиска не получает.
  const webState: WebState =
    aiSettings?.web_research !== true ? "off" : aiSettings.provider === "Custom" ? "unavailable" : "ok";
  const researchError =
    research.error === null
      ? null
      : research.error === "SearchDisabled"
        ? t("research.box.off")
        : research.error === "SearchUnavailable"
          ? t("research.box.unavailable")
          : failureText(research.error);
  const shownResearch = new Set(
    view.events.flatMap((event) => (event.kind === "ResearchStarted" ? [event.id] : [])),
  );
  const renderResearch = (id: string): React.JSX.Element | null => {
    const item = view.research.find((entry) => entry.id === id);
    return item === undefined ? null : renderCard(item);
  };
  const renderCard = (item: ResearchView): React.JSX.Element => (
    <ResearchCard
      item={item}
      client={client}
      declined={research.isDeclined(item.id)}
      onCancel={research.cancel}
      onRetry={research.retry}
      onOpenNote={env.openNote}
      onConfirm={research.confirm}
      onDecline={research.decline}
    />
  );


  return (
    <section
      ref={sectionRef}
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
          {plan !== undefined && planProgress !== undefined && (
            <small>
              {plan.changes > 0
                ? t("lessons.plan.summaryChanged", {
                    current: planProgress.current,
                    total: planProgress.total,
                    changes: plan.changes,
                  })
                : t("lessons.plan.summary", { current: planProgress.current, total: planProgress.total })}
            </small>
          )}
        </div>
        {probing && (
          <span className="ts-lesson__phase" role="status">
            {phaseText}
          </span>
        )}
        {isPractice && (
          <span className="ts-lesson__kind">{t("practice.tag")}</span>
        )}
        {isPractice && practice !== undefined && (
          <button
            type="button"
            className="ts-btn ts-btn--sm"
            aria-pressed={summaryOpen}
            onClick={() => setSummaryOpen((open) => !open)}
          >
            {t("practice.summary.toggle")}
          </button>
        )}
        {plan !== undefined && (
          <button
            type="button"
            className="ts-btn ts-btn--sm"
            aria-pressed={planShown}
            onClick={() => setPlanOpen(!planShown)}
          >
            {t("lessons.plan.toggle")}
          </button>
        )}
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
            {isPractice ? t("practice.finish") : t("lessons.finish")}
          </button>
        ) : (
          <button
            type="button"
            className="ts-btn"
            disabled={view.read_only}
            onClick={() => void setState("Active")}
          >
            {isPractice ? t("practice.resume") : t("lessons.resume")}
          </button>
        )}
      </div>

      <div className="ts-lesson__split">
      <div className="ts-lesson__main">
      <div ref={feedRef} className="ts-lesson__feed" role="log" aria-label={t("lessons.feed")}>
        {!isPractice && <Bubble who={t("lessons.who.you")}>{view.header.goal}</Bubble>}

        {view.events.map((event, index) => (
          <div key={`${index}:${event.kind}:${event.at}`} className="ts-lesson__ev" data-event={index}>
            <EventRow
              event={event}
              hideQuestion={answering && index === lastTeacherIndex}
              view={view}
              hintNumber={hintNumberOf.get(index)}
              onNext={practiceNext && index === lastVerdictIndex ? nextProblem : undefined}
              onOpenNote={env.openNote}
              renderResearch={renderResearch}
              {...(probePending && index === lastProbeIndex
                ? {
                    probeActions: {
                      onStart: () => void run(undefined),
                      ...(plan !== undefined ? { onShowPlan: () => setPlanOpen(true) } : {}),
                    },
                  }
                : {})}
            />
          </div>
        ))}

        {view.research
          .filter((item) => !shownResearch.has(item.id))
          .map((item) => (
            <div key={`research:${item.id}`} className="ts-lesson__ev">
              {renderCard(item)}
            </div>
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
            {t(view.header.kind === "Practice" ? "practice.start.hint" : "lessons.start.hint")}
            <div className="ts-lesson__acts">
              <button type="button" className="ts-btn ts-btn--accent" onClick={() => void run(undefined)}>
                {t(view.header.kind === "Practice" ? "practice.start.button" : "lessons.start.button")}
              </button>
            </div>
          </div>
        )}

        {answering && (
          <LessonQuestion
            question={question}
            draft={draft}
            onDraft={setDraft}
            onSubmit={() => canAnswer(question, draft) && submit()}
            {...(probeQuestion ? { onDontKnow: dontKnow } : {})}
          />
        )}
        {answering && probing && (
          <div className="ts-lesson__acts">
            <button type="button" className="ts-btn ts-btn--sm" onClick={() => void skipProbe()}>
              {t("lessons.probe.skip")}
            </button>
          </div>
        )}

        {practiceOpen && (
          <PracticeForm
            text={draft.text}
            onText={(text) => setDraft({ ...draft, text })}
            onSend={sendProblem}
            onHint={requestHint}
            onGiveUp={giveUp}
          />
        )}
        {practiceStuck && (
          <div className="ts-lesson__box" role="status">
            {t("practice.stuck")}
            <div className="ts-lesson__acts">
              <button type="button" className="ts-btn ts-btn--accent" onClick={() => void run(undefined)}>
                {t("lessons.retryRepeat")}
              </button>
            </div>
          </div>
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
            {view.state === "Finished"
              ? t(isPractice ? "practice.closed.finished" : "lessons.closed.finished")
              : t(isPractice ? "practice.closed.stopped" : "lessons.closed.stopped")}
          </p>
        )}
        {!isPractice && (
          <KnowledgeLessonBlock
            path={path}
            pendingChars={view.knowledge_pending_chars}
            jobs={knowledgeJobs}
            extract={extract}
            onOpenKnowledge={env.openKnowledge}
          />
        )}
        <div ref={feedEnd} />
      </div>

      <KnowledgeJobs client={client} vaultId={env.vaultId} jobs={knowledgeJobs} source={path} />

      {!view.read_only && (
        <ResearchInput
          web={webState}
          error={researchError}
          onWeb={(question) => research.start("Web", question)}
          onPdf={(question) => setPdfDialog(question)}
          onOpenSettings={env.openSettings}
          onTouch={() => reloadSettings.current()}
          clearToken={sent}
        />
      )}
      {pdfDialog !== null && (
        <ResearchPdfDialog
          client={client}
          question={pdfDialog}
          onCancel={() => setPdfDialog(null)}
          onStart={(pdf, question) => {
            setPdfDialog(null);
            setSent((count) => count + 1);
            research.start("Pdf", question, pdf);
          }}
        />
      )}

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
      </div>

      {plan !== undefined && planShown && (
        <LessonPlan
          plan={plan}
          history={view.plan_history}
          sheet={narrow}
          onClose={() => setPlanOpen(false)}
          onJump={jumpTo}
          loadVersion={loadVersion}
        />
      )}
      {isPractice && practice !== undefined && summaryOpen && (
        <PracticeSummary
          practice={practice}
          sheet={narrow}
          onClose={() => setSummaryOpen(false)}
          onJump={jumpTo}
        />
      )}
      </div>
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
  if (answer.action === "NextProblem") return <></>;
  const said =
    answer.action === "HintRequest"
      ? t("practice.said.hint")
      : answer.action === "GiveUp"
        ? t("practice.said.giveUp")
        : undefined;
  return (
    <div className="ts-lesson__t ts-lesson__t--user">
      <div className="ts-lesson__who">{t("lessons.who.you")}</div>
      {said !== undefined ? (
        <div className="ts-lesson__body ts-lesson__body--soft">{said}</div>
      ) : answer.dont_know === true && answer.text === "" ? (
        <div className="ts-lesson__body ts-lesson__body--soft">{t("lessons.q.dontKnowSaid")}</div>
      ) : (
        <div className="ts-lesson__body">{answer.text}</div>
      )}
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
  view,
  hintNumber,
  onNext,
  onOpenNote,
  renderResearch,
  probeActions,
}: {
  event: LessonEvent;
  hideQuestion: boolean;
  /** Вид урока: практике нужны её задачи и решения. */
  view: LessonView;
  /** Номер подсказки внутри задачи (053). */
  hintNumber: number | undefined;
  /** «Следующая задача» под последним итогом практики. */
  onNext: (() => void) | undefined;
  onOpenNote(path: string): void;
  /** Карточка исследования по id (фича 056). */
  renderResearch(id: string): React.JSX.Element | null;
  /** Кнопки итога замера: только у последнего итога, после которого урок ждёт. */
  probeActions?: { onStart(): void; onShowPlan?: () => void };
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
            ...(event.dont_know === true ? { dont_know: true } : {}),
          }}
        />
      );
    case "Teacher": {
      const cut = event.interrupted === true;
      // Ход практики из одних инструментов не оставляет текста: пустая подпись не нужна.
      if (event.text === "" && event.question === undefined && !cut) return null;
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
    // События фичи 052: стадия, итог замера, план. Отметки шагов (`PlanProgress`) видны в
    // колонке плана, в ленте им места нет.
    case "Phase":
      return event.skipped === true ? <div className="ts-lesson__sep">{t("lessons.probe.skipped")}</div> : null;
    case "ProbeResult":
      return <ProbeResultCard parts={event.parts} start={event.start} {...(probeActions !== undefined ? { actions: probeActions } : {})} />;
    case "Plan":
      return event.version <= 1 ? (
        <div className="ts-lesson__sep">{t("lessons.plan.created", { count: event.steps.length })}</div>
      ) : (
        <div className="ts-lesson__change" role="status">
          <b>{t("lessons.plan.changedSep", { version: event.version })}</b>
          {event.reason != null && event.reason !== "" && <span> {event.reason}</span>}
        </div>
      );
    case "PlanProgress":
      return null;
    // Чтение графа знаний агентом (фича 054, FR-009): видно в уроке так же, как чтение заметки.
    case "KnowledgeRead":
      return (
        <div className={event.outcome.kind === "Denied" ? "ts-lesson__read ts-lesson__read--deny" : "ts-lesson__read"}>
          <span>
            {event.outcome.kind === "Denied"
              ? t("knowledge.read.denied", { query: event.query })
              : t("knowledge.read.sent", { query: event.query, count: event.outcome.count })}
          </span>
        </div>
      );
    // События фичи 053 (практика): задачи, подсказки и итоги рисует лента практики.
    case "Material":
    case "Problem":
    case "Hint":
    case "Verdict":
    case "NextProblem":
      return <PracticeRow event={event} view={view} hintNumber={hintNumber} onNext={onNext} />;
    case "HintRequest":
    case "GiveUp":
      return <UserBubble answer={{ text: "", action: event.kind }} />;
    // События фичи 056 (исследование): карточки рисует лента по `view.research`, не события.
    case "ResearchStarted":
      return renderResearch(event.id);
    case "ResearchDone":
    case "ResearchFailed":
      return null;
    case "PlanRejected":
      if (
        event.tool === "pose_problem" ||
        event.tool === "give_hint" ||
        event.tool === "conclude" ||
        event.tool === "research" ||
        event.tool === "study_pdf"
      ) {
        return null;
      }
      return (
        <div className="ts-lesson__box ts-lesson__box--warn" role="status">
          {t(event.tool === "plan_progress" ? "lessons.plan.rejectedProgress" : "lessons.plan.rejected", {
            reason: sentence(event.reason),
          })}
        </div>
      );
    default: {
      const exhaustive: never = event;
      return exhaustive;
    }
  }
}

/** Причина отказа как законченное предложение: дальше в тексте идёт ещё одно. */
function sentence(reason: string): string {
  const text = reason.trim();
  return /[.!?…]$/.test(text) ? text : `${text}.`;
}
