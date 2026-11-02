"use client";

import { createContext, useContext } from "react";
import type { AiFailure } from "../wasm-core/typsidian_core.js";

// Что вкладке урока нужно от рабочей области (фича 051): id vault для ключа
// устройства, переход к заметке и к настройкам. Контекстом, а не пропсами: вкладка
// стоит за тремя слоями групп вкладок, и ни один из них об уроках не знает.

export interface LessonEnv {
  /** Id записи vault на устройстве; на сервере его подставляет сервер. */
  vaultId: string;
  openNote(path: string): void;
  openSettings(): void;
}

const NO_ENV: LessonEnv = { vaultId: "", openNote: () => undefined, openSettings: () => undefined };

const LessonEnvContext = createContext<LessonEnv>(NO_ENV);

export const LessonEnvProvider = LessonEnvContext.Provider;

export function useLessonEnv(): LessonEnv {
  return useContext(LessonEnvContext);
}

/**
 * Уроки, чей первый ход пользователь уже заказал кнопкой «Начать урок» в диалоге:
 * вкладка, открывшись, начинает его сама. Без этой отметки урок в состоянии «не
 * начат» сам ничего не отправляет — фоновой отправки нет (принцип IX).
 */
const startRequests = new Set<string>();

export function requestLessonStart(path: string): void {
  startRequests.add(path);
}

export function takeLessonStart(path: string): boolean {
  return startRequests.delete(path);
}

/** Вид отказа из исключения ядра; `null` — исключение не от ядра. */
export function failureKind(error: unknown): AiFailure["kind"] | null {
  if (typeof error === "object" && error !== null && "kind" in error) {
    return (error as AiFailure).kind;
  }
  return null;
}

/** Идентификатор запроса: `randomUUID` есть только в защищённом контексте. */
export function newRequestId(): string {
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
}

/** Подпись сочетания «Mod+Enter» на этой платформе. */
export function modEnterLabel(): string {
  const apple = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);
  return apple ? "⌘+Enter" : "Ctrl+Enter";
}
