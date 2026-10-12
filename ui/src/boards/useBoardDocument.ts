"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { BoardSaveState } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import type { Phrase } from "../i18n/index.js";

// Принцип I: этот хук не решает, что считать содержимым и когда запись
// допустима — он вызывает ядро и исполняет его ответ. Правило «из Failed нет
// автоперехода в Clean» живёт в `packages/core/src/boards/save.rs`; здесь оно
// только отражается.

/**
 * Пауза от последнего штриха до записи (research.md §6).
 *
 * SC-004 даёт 2 секунды от «перестал рисовать» до файла — 800 мс оставляют
 * запас на сериализацию и запись даже на пределе в 2 000 элементов. Писать по
 * каждому изменению нельзя: рисование порождает событие на каждый штрих.
 */
export const BOARD_AUTOSAVE_DEBOUNCE_MS = 800;

/**
 * Состояние сохранения доски для показа. То же, что `BoardSaveState` ядра, но
 * причина отказа — фраза, а не готовая строка: показанный отказ обязан
 * перерисоваться на новом языке (фича 010, FR-014).
 */
export type BoardSaveView =
  | Exclude<BoardSaveState, { state: "Failed" }>
  | { state: "Failed"; reason: Phrase };

export interface BoardDocument {
  /** Содержимое, прочитанное из файла; `null` — доска ещё не загружена. */
  content: string | null;
  /**
   * Номер загруженного содержимого. Растёт при каждой загрузке и при замене
   * версии после конфликта — движок принимает сцену только при монтировании,
   * поэтому по этому номеру он пересоздаётся (contracts/board-engine-bridge.md).
   */
  contentRevision: number;
  saveState: BoardSaveView;
  /** Ошибка чтения доски словами; `null` — читается нормально. */
  readError: Phrase | null;
  /** Движок сообщил об изменении рисунка. */
  changed: (next: string) => void;
  /**
   * Записать немедленно, не дожидаясь дебаунса.
   *
   * Обязателен перед переключением на другую заметку, закрытием окна и
   * построением снимка (research.md §6): иначе снимок построится по
   * состоянию, которого нет в файле.
   *
   * Отвечает состоянием сохранения после записи (фича 014): закрыть вкладку
   * доски можно, только если оно `Clean`.
   */
  flush: () => Promise<BoardSaveView>;
  /**
   * Показать на полотне другое содержимое — версию, выбранную при разрешении
   * конфликта (FR-015). Единственный случай, когда приложение перезаписывает
   * состояние движка; во всех прочих оно только читает.
   */
  replace: (next: string) => void;
  /** Текущее содержимое буфера — то, что уйдёт в файл и в снимок. */
  buffer: () => string | null;
}

/**
 * Открытая доска: загрузка, автосохранение с дебаунсом и видимое состояние
 * сохранения (FR-008, FR-009, FR-010).
 */
export function useBoardDocument(
  client: VaultClient | null,
  path: string | null,
): BoardDocument {
  const [content, setContent] = useState<string | null>(null);
  const [contentRevision, setContentRevision] = useState(0);
  const [saveState, setSaveState] = useState<BoardSaveView>({ state: "Clean" });
  const [readError, setReadError] = useState<Phrase | null>(null);

  // Буфер держится в ref, а не в state: запись читает его из таймера и из
  // `flush`, и обе стороны должны видеть последний штрих, а не тот, что был на
  // момент планирования.
  const bufferRef = useRef<string | null>(null);
  const timerRef = useRef<number | null>(null);
  // Последнее показанное состояние — для ответа `flush`, который не может ждать рендера.
  const saveStateRef = useRef<BoardSaveView>({ state: "Clean" });
  const publishSaveState = useCallback((next: BoardSaveView | ((previous: BoardSaveView) => BoardSaveView)) => {
    saveStateRef.current = typeof next === "function" ? next(saveStateRef.current) : next;
    setSaveState(saveStateRef.current);
  }, []);

  const cancelPendingSave = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const save = useCallback(async (): Promise<BoardSaveView> => {
    const buffer = bufferRef.current;
    if (!client || path === null || buffer === null) return saveStateRef.current;

    publishSaveState({ state: "Saving" });
    try {
      // `expected` не передаётся: обнаружение расхождения с диском — работа
      // ядра, которое держит снимок открытого файла. Придумывать его здесь
      // значило бы завести вторую версию правила о конфликтах.
      await client.writeBoard(path, buffer, undefined);
      publishSaveState({ state: "Clean" });
    } catch (cause) {
      publishSaveState({ state: "Failed", reason: describeWriteFailure(cause) });
    }
    return saveStateRef.current;
  }, [client, path, publishSaveState]);

  // Загрузка доски. Отказ чтения — не исключение, а состояние: повреждённый
  // файл остаётся на диске нетронутым (FR-017).
  useEffect(() => {
    let cancelled = false;
    cancelPendingSave();
    bufferRef.current = null;
    setContent(null);
    setReadError(null);
    publishSaveState({ state: "Clean" });
    if (!client || path === null) return;

    void (async () => {
      try {
        const text = await client.readBoard(path);
        if (cancelled) return;
        bufferRef.current = text;
        setContent(text);
        setContentRevision((revision) => revision + 1);
      } catch (cause) {
        if (!cancelled) setReadError(() => describeReadFailure(cause));
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [client, path, cancelPendingSave, publishSaveState]);

  const changed = useCallback(
    (next: string) => {
      // Движок зовёт `onChange` и там, где рисунок не менялся: на монтировании
      // со сценой из файла и на изменениях вида (прокрутка, курсор). Записывать
      // и поднимать правку по такому вызову значит переписать файл от одного
      // открытия доски — а это ровно то расхождение снимка, которое Сценарий 5
      // шаг 3 считает дефектом.
      if (bufferRef.current === next) return;

      bufferRef.current = next;
      // Функциональная форма, а не новый объект: пока состояние уже `Dirty`,
      // React обязан увидеть ту же ссылку и не перерисовывать дерево. Новый
      // объект на каждый штрих перерисовывал бы весь workspace и съедал бюджет
      // отклика SC-005.
      publishSaveState((previous) =>
        previous.state === "Dirty" ? previous : { state: "Dirty" },
      );
      // Ядро обязано знать о несохранённых правках доски — так же, как о
      // правках заметки (`NoteEditor` → `beginEdit`). Без этого конвейер
      // обнаружения не поднимет конфликт: наличие буфера — это и есть признак,
      // отличающий «тихо подхватить внешнее изменение» от «спросить
      // пользователя» (FR-014, 001 FR-035).
      if (client && path !== null) void client.beginEdit(path, next);
      cancelPendingSave();
      timerRef.current = window.setTimeout(() => {
        timerRef.current = null;
        void save();
      }, BOARD_AUTOSAVE_DEBOUNCE_MS);
    },
    [client, path, cancelPendingSave, save, publishSaveState],
  );

  const flush = useCallback(async (): Promise<BoardSaveView> => {
    cancelPendingSave();
    return save();
  }, [cancelPendingSave, save]);

  const replace = useCallback((next: string) => {
    cancelPendingSave();
    bufferRef.current = next;
    setContent(next);
    setContentRevision((revision) => revision + 1);
    // Выбранная версия уже на диске: `resolveConflict*` записал её сам, и
    // повторная запись отсюда была бы вторым решением о том же файле.
    publishSaveState({ state: "Clean" });
  }, [cancelPendingSave, publishSaveState]);

  const buffer = useCallback(() => bufferRef.current, []);

  // Незавершённый дебаунс при размонтировании — потерянная правка. Сброс
  // синхронный: браузер не ждёт промисов при закрытии вкладки, поэтому
  // предупреждение о несохранённом делает `useWindowCloseGuard`, а здесь —
  // только отмена таймера, чтобы он не выстрелил в отсутствующий компонент.
  useEffect(() => cancelPendingSave, [cancelPendingSave]);

  return {
    content,
    contentRevision,
    saveState,
    readError,
    changed,
    flush,
    replace,
    buffer,
  };
}

/** Причина отказа записи словами (FR-010). */
function describeWriteFailure(cause: unknown): Phrase {
  const kind = kindOf(cause);
  switch (kind) {
    case "Conflicted":
      return (t) => t("boards.save.conflicted");
    case "ReadOnlyProfile":
      return (t) => t("boards.save.readOnlyProfile");
    case "VaultNotWritable":
      return (t) => t("boards.save.vaultNotWritable");
    case "Io":
      return (t) => t("boards.save.io");
    default:
      return (t) => t("boards.save.failed");
  }
}

/** Причина отказа чтения словами (FR-017). */
function describeReadFailure(cause: unknown): Phrase {
  switch (kindOf(cause)) {
    case "NotFound":
      return (t) => t("boards.read.notFound");
    case "Unreadable":
      return (t) => t("boards.read.unreadable");
    default:
      return (t) => t("boards.read.io");
  }
}

/**
 * Ошибки ядра пересекают границу как размеченные объединения, но тип
 * отклонённого промиса на стороне TypeScript не виден. Сужение вместо `as`:
 * принцип IV запрещает глушить это приведением типа.
 */
function kindOf(cause: unknown): string | null {
  if (typeof cause === "object" && cause !== null && "kind" in cause) {
    const { kind } = cause as { kind: unknown };
    return typeof kind === "string" ? kind : null;
  }
  return null;
}
