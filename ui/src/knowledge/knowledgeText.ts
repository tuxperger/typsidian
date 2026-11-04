import type { AiFailure } from "../wasm-core/typsidian_core.js";
import type { MessageKey } from "../i18n/index.js";
import { failureKind } from "../lessons/lessonEnv.js";

// Тексты отказов в контексте графа знаний (фича 054, research §11). Новых `AiFailure` ядро не
// вводит: четыре прежних вида в графе значат другое, чем в уроке, поэтому читаются по
// `knowledge.failure.*`, остальные — как везде, по `ai.failure.*`.

const IN_GRAPH = {
  JournalFailed: "knowledge.failure.JournalFailed",
  Forbidden: "knowledge.failure.Forbidden",
  LessonUnreadable: "knowledge.failure.LessonUnreadable",
  Cancelled: "knowledge.failure.Cancelled",
} as const satisfies Partial<Record<AiFailure["kind"], MessageKey>>;

/** Ключи текстов отказов: без параметров, поэтому годятся для `t(ключ)`. */
export type FailureKey = Extract<MessageKey, `ai.failure.${string}` | `knowledge.failure.${string}`>;

type InGraph = keyof typeof IN_GRAPH;

function isInGraph(kind: AiFailure["kind"]): kind is InGraph {
  return kind in IN_GRAPH;
}

/** Ключ текста для вида отказа. */
export function failureKeyOfKind(kind: AiFailure["kind"] | null): FailureKey {
  if (kind === null) return "ai.failure.Unknown";
  return isInGraph(kind) ? IN_GRAPH[kind] : `ai.failure.${kind}`;
}

/** Ключ текста для исключения ядра; не от ядра — «что-то пошло не так». */
export function failureKeyOf(error: unknown): FailureKey {
  return failureKeyOfKind(failureKind(error));
}

/** Имя источника для показа: файл без папки и расширения. */
export function sourceName(path: string): string {
  const name = path.split("/").pop() ?? path;
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(0, dot) : name;
}

/** Событие, к которому прокручивается урок, а также источник-урок ли это. */
export function isLessonPath(path: string): boolean {
  return path.endsWith(".lesson");
}

/** Открывает источник утверждения: путь и номер события урока, если он известен. */
export type OpenSource = (path: string, event?: number) => void;

/** Предел утверждений, выбираемых как опора урока или практики (ядро отклоняет больше). */
export const PICK_MAX = 50;

/** Предел путей в одном выборе импорта (фича 055): больше ядро отклоняет как `Forbidden`. */
export const IMPORT_MAX_PATHS = 200;

/**
 * Ключ текста отказа импорта. Три прежних вида в импорте значат другое, чем в уроке (research
 * 055 §6): `Forbidden` — корень, путь вне vault или слишком длинный выбор, `Cancelled` — в vault
 * уже идёт импорт, `LessonChanged` — оценка устарела. Остальные читаются как везде.
 */
export function importFailureKey(error: unknown): FailureKey {
  switch (failureKind(error)) {
    case "Forbidden":
      return "knowledge.failure.ImportForbidden";
    case "Cancelled":
      return "knowledge.failure.ImportBusy";
    case "LessonChanged":
      return "knowledge.failure.ImportChanged";
    default:
      return failureKeyOf(error);
  }
}
