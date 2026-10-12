import type {
  LinkOutcome,
  MentionsAvailability,
  SourceProblem,
} from "../../wasm-core/typsidian_core.js";
import type { Phrase } from "../../i18n/index.js";

// Подписи режима «Связи» словами (фича 013). Только оформление: исход и
// причину решило ядро, здесь они превращаются в фразы, которые собираются при
// показе — после смены языка текст перерисовывается на новом (010 FR-014).
// Разбор исчерпывающий, с веткой `never`: новый вид в ядре без подписи — ошибка
// `tsc`, а не пустое место на экране.

/**
 * Что сказать о неудачной попытке «Связать». `null` — говорить нечего: запись
 * удалась, и строка просто исчезнет из списка упоминаний.
 */
export function outcomePhrase(outcome: LinkOutcome): Phrase | null {
  switch (outcome.kind) {
    case "Written":
      return null;
    case "EditorEdit":
      // Недостижим, пока нет порта `OpenNoteEditors` (013 T046–T047): текст
      // редактора ядру не передаётся, поэтому правки для редактора оно не
      // возвращает. Исход разобран честным отказом, файл при этом не тронут.
      return (t) => t("links.mentions.noEditor");
    case "Stale":
      return (t) => t("links.mentions.staleNote");
    case "ReadOnly":
      return (t) => t("links.mentions.readOnly");
    case "UnsavedElsewhere":
      return (t) => t("links.mentions.unsaved");
    case "Unexpressible":
      return (t) => t("links.mentions.unexpressible");
    case "Failed":
      return (t) => t("links.mentions.writeFailed");
    default: {
      const exhaustive: never = outcome;
      return exhaustive;
    }
  }
}

/** Почему упоминания не ищутся. `null` — ищутся. */
export function unavailablePhrase(reason: MentionsAvailability): Phrase | null {
  switch (reason.kind) {
    case "Available":
      return null;
    case "TitleTooShort": {
      const title = reason.title;
      return (t) => t("links.mentions.tooShort", { title });
    }
    default: {
      const exhaustive: never = reason;
      return exhaustive;
    }
  }
}

/** Почему у источника нет фрагментов. */
export function problemPhrase(problem: SourceProblem): Phrase {
  switch (problem) {
    case "TooLarge":
      return (t) => t("links.problem.tooLarge");
    case "Unreadable":
      return (t) => t("links.problem.unreadable");
    default: {
      const exhaustive: never = problem;
      return exhaustive;
    }
  }
}
