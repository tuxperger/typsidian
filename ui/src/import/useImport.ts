"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type {
  ImportJob,
  ImportState,
  ImportSurvey,
  OnConflict,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { useI18n } from "../i18n/index.js";
import type { ImportSourceChoice, ImportSourcePicker } from "./import-source.js";
import { asImportError, describeImportFailure, reportTexts } from "./describe.js";

// Импорт из Obsidian со стороны интерфейса (фича 021, contracts/core-import-api.md).
//
// Живёт в `Workspace`, а не в диалоге: закрытие диалога после запуска не
// прерывает пакет. Правил здесь нет — перевод, имена, ссылки, журнал и отчёт
// считает ядро (принцип I). Хук ведёт цикл: план → шаг на файл с уступкой
// UI-потоку → отчёт; отмена — выйти из цикла и всё равно записать отчёт.

/** Не чаще раза в столько мс карточка хода перерисовывается: шагов тысячи. */
const PROGRESS_REDRAW_MS = 80;

/** Задержка пересчёта обзора после правки папки назначения. */
const SURVEY_DEBOUNCE_MS = 250;

export interface ImportRun {
  /** `running` — идут шаги; `finishing` — пишется отчёт; `done` — окончен; `failed` — сорван. */
  phase: "running" | "finishing" | "done" | "failed";
  state: ImportState | null;
  /** Отмена нажата, текущая заметка дописывается. */
  cancelling: boolean;
  /** Пакет окончен раньше плана. */
  cancelled: boolean;
  reportPath: string | null;
  /** Причина срыва или недописанного отчёта, словами. */
  error: string | null;
  /** Карточка свёрнута в строку шапки. */
  minimized: boolean;
}

export interface ImportSetup {
  source: ImportSourceChoice | null;
  survey: ImportSurvey | null;
  surveying: boolean;
  destination: string;
  onConflict: OnConflict;
  copyOther: boolean;
  /** Что сорвалось в диалоге (открытие папки, обзор), словами. */
  error: string | null;
}

export interface ImportControl {
  /** Оболочка даёт выбор папки (иначе команды нет). */
  offered: boolean;
  /** Платформа умеет выбирать папку; нет — диалог показывает C5. */
  available: boolean;
  dialogOpen: boolean;
  openDialog: () => void;
  closeDialog: () => void;
  setup: ImportSetup;
  chooseSource: () => void;
  setDestination: (destination: string) => void;
  setOnConflict: (onConflict: OnConflict) => void;
  setCopyOther: (copy: boolean) => void;
  start: () => void;
  run: ImportRun | null;
  running: boolean;
  cancel: () => void;
  toggleMinimized: () => void;
  dismissRun: () => void;
}

const EMPTY_SETUP: ImportSetup = {
  source: null,
  survey: null,
  surveying: false,
  destination: "",
  onConflict: "Skip",
  copyOther: false,
  error: null,
};

/** Папка назначения без краевых косых черт: пусто — корень vault. */
function normalizeDestination(value: string): string {
  return value.trim().replace(/\\/g, "/").replace(/^\/+|\/+$/g, "");
}

const yieldToUi = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

export function useImport(
  client: VaultClient | null,
  picker: ImportSourcePicker | undefined,
): ImportControl {
  const i18n = useI18n();
  const i18nRef = useRef(i18n);
  i18nRef.current = i18n;

  const [dialogOpen, setDialogOpen] = useState(false);
  const [setup, setSetup] = useState<ImportSetup>(EMPTY_SETUP);
  const [run, setRun] = useState<ImportRun | null>(null);

  const cancelRef = useRef(false);
  const runningRef = useRef(false);
  // Поколение: смена vault или размонтирование обрывает цикл прежнего.
  const generationRef = useRef(0);
  const surveyToken = useRef(0);
  const sourceOpen = useRef(false);

  const running = run !== null && (run.phase === "running" || run.phase === "finishing");

  const closeSource = useCallback(() => {
    if (!sourceOpen.current) return;
    sourceOpen.current = false;
    void client?.import.close().catch(() => undefined);
  }, [client]);

  // Обзор источника (C1, C4): сразу после выбора папки и при смене назначения.
  const sourceName = setup.source?.name ?? null;
  const destination = setup.destination;
  useEffect(() => {
    if (!dialogOpen || sourceName === null || client === null || runningRef.current) {
      return undefined;
    }
    const token = ++surveyToken.current;
    setSetup((current) => ({ ...current, surveying: true, error: null }));
    const timer = setTimeout(() => {
      const job: ImportJob = {
        destination: normalizeDestination(destination),
        on_conflict: "Skip",
        copy_other_files: false,
      };
      client.import.survey(job).then(
        (survey) => {
          if (token !== surveyToken.current) return;
          setSetup((current) => ({ ...current, survey, surveying: false }));
        },
        (cause: unknown) => {
          if (token !== surveyToken.current) return;
          setSetup((current) => ({
            ...current,
            survey: null,
            surveying: false,
            error: describeImportFailure(i18nRef.current.t, cause, false),
          }));
        },
      );
    }, SURVEY_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [dialogOpen, sourceName, destination, client]);

  const openDialog = useCallback(() => {
    if (running) return;
    // Папка по умолчанию — слово языка интерфейса (макет C1).
    setSetup({ ...EMPTY_SETUP, destination: i18nRef.current.t("import.dest.default") });
    setDialogOpen(true);
  }, [running]);

  const closeDialog = useCallback(() => {
    surveyToken.current += 1;
    setDialogOpen(false);
    if (!runningRef.current) closeSource();
  }, [closeSource]);

  const chooseSource = useCallback(() => {
    if (picker === undefined || client === null) return;
    void (async () => {
      try {
        const choice = await picker.pick();
        if (choice === null) return;
        await client.import.open(choice.target, choice.name);
        sourceOpen.current = true;
        setSetup((current) => ({ ...current, source: choice, survey: null, error: null }));
      } catch (cause) {
        setSetup((current) => ({
          ...current,
          error: describeImportFailure(i18nRef.current.t, cause, true),
        }));
      }
    })();
  }, [client, picker]);

  const setDestination = useCallback(
    (value: string) => setSetup((current) => ({ ...current, destination: value })),
    [],
  );
  const setOnConflict = useCallback(
    (onConflict: OnConflict) => setSetup((current) => ({ ...current, onConflict })),
    [],
  );
  const setCopyOther = useCallback(
    (copyOther: boolean) => setSetup((current) => ({ ...current, copyOther })),
    [],
  );

  const start = useCallback(() => {
    if (client === null || runningRef.current || setup.source === null) return;
    const generation = generationRef.current;
    const ops = client.import;
    const now = new Date();
    const job: ImportJob = {
      destination: normalizeDestination(setup.destination),
      on_conflict: setup.onConflict,
      copy_other_files: setup.copyOther,
      started_ms: now.getTime(),
      texts: reportTexts(i18nRef.current, now),
    };
    const current = (): boolean => generationRef.current === generation;

    runningRef.current = true;
    cancelRef.current = false;
    surveyToken.current += 1;
    setDialogOpen(false);
    setRun({
      phase: "running",
      state: null,
      cancelling: false,
      cancelled: false,
      reportPath: null,
      error: null,
      minimized: false,
    });

    void (async () => {
      let state: ImportState | null = null;
      let failure: string | null = null;
      try {
        state = await ops.begin(job);
        let redrawn = performance.now();
        let sessionRetries = 0;
        if (current()) setRun((previous) => previous && { ...previous, state });

        while (state.next < state.total && !cancelRef.current && current()) {
          try {
            state = await ops.step(state);
            sessionRetries = 0;
          } catch (cause) {
            // План потерян — начать заново: журнал не даст дублей. Дважды
            // подряд — уже не случайность.
            if (asImportError(cause)?.kind === "SessionLost" && sessionRetries < 1) {
              sessionRetries += 1;
              state = await ops.begin(job);
              continue;
            }
            throw cause;
          }
          const shown = state;
          const now = performance.now();
          if (current() && (now - redrawn >= PROGRESS_REDRAW_MS || shown.next >= shown.total)) {
            redrawn = now;
            setRun((previous) => previous && { ...previous, state: shown });
          }
          // Уступить цикл событий: на web шаг идёт в главном потоке, и между
          // заметками ввод, перерисовка и «Отменить» обязаны успеть (принцип X).
          await yieldToUi();
        }
      } catch (cause) {
        failure = describeImportFailure(i18nRef.current.t, cause, false);
      }

      // Отчёт пишется и после конца, и после отмены, и после срыва, если план есть.
      const reached = state;
      if (current()) setRun((previous) => previous && { ...previous, phase: "finishing", state: reached });
      let reportPath: string | null = null;
      if (reached !== null) {
        try {
          reportPath = await ops.finish(reached);
        } catch (cause) {
          failure ??= describeImportFailure(i18nRef.current.t, cause, false);
        }
      }
      void ops.close().catch(() => undefined);
      sourceOpen.current = false;
      runningRef.current = false;
      if (!current()) return;
      setRun((previous) =>
        previous && {
          ...previous,
          phase: failure !== null && reportPath === null ? "failed" : "done",
          state: reached,
          cancelled: reached !== null && reached.next < reached.total,
          reportPath,
          error: failure,
        },
      );
    })();
  }, [client, setup]);

  const cancel = useCallback(() => {
    if (!runningRef.current) return;
    cancelRef.current = true;
    setRun((previous) => previous && { ...previous, cancelling: true });
  }, []);

  const toggleMinimized = useCallback(
    () => setRun((previous) => previous && { ...previous, minimized: !previous.minimized }),
    [],
  );

  const dismissRun = useCallback(() => {
    if (runningRef.current) return;
    setRun(null);
  }, []);

  // Смена vault (новый клиент) или закрытие рабочей области обрывает цикл
  // прежнего: текущая заметка дописывается атомарно или не пишется вовсе —
  // это гарантирует ядро, а журнал сохранит `finish` в хвосте цикла.
  useEffect(() => {
    return () => {
      generationRef.current += 1;
      cancelRef.current = true;
      surveyToken.current += 1;
      sourceOpen.current = false;
      setDialogOpen(false);
      setSetup(EMPTY_SETUP);
      setRun(null);
    };
  }, [client]);

  return {
    offered: picker !== undefined,
    available: picker?.available ?? false,
    dialogOpen,
    openDialog,
    closeDialog,
    setup,
    chooseSource,
    setDestination,
    setOnConflict,
    setCopyOther,
    start,
    run,
    running,
    cancel,
    toggleMinimized,
    dismissRun,
  };
}
