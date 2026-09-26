"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { VaultClient } from "../vault-client.js";

// Принцип I: этот хук не решает, когда запись допустима. Признак «по этой
// заметке есть неразрешённый конфликт» он спрашивает у ядра
// (`pendingConflict`), а не выводит из своего состояния, и запись отдаёт тому же
// `write`, что и раньше отдавала кнопка. Своё здесь только одно — ритм: пауза
// после последнего нажатия.

/**
 * Пауза от последнего нажатия клавиши до записи файла.
 *
 * Тот же порядок, что у автосохранения досок (`BOARD_AUTOSAVE_DEBOUNCE_MS`), но
 * короче: заметка — это текст, его сериализовать не нужно, а «сохранено» в
 * статус-баре должно появляться в темпе набора, а не через секунду после него.
 * Писать по каждому нажатию нельзя — это файл на диск на каждую букву.
 */
export const NOTE_AUTOSAVE_DEBOUNCE_MS = 600;

/**
 * Что показывает статус-бар редактора.
 *
 * Форма — как у `BoardSaveState` из ядра, но тип свой: у заметки есть
 * состояние, которого у доски нет. `Held` — «ядро держит конфликт по этому
 * файлу, и до решения пользователя автосохранение молчит»; у досок тот же гейт
 * стоит внутри ядра (`boards::store::write_board`) и наружу приходит отказом.
 */
export type NoteSaveState =
  | { state: "Clean" }
  | { state: "Dirty" }
  | { state: "Saving" }
  | { state: "Held" }
  | { state: "Failed"; reason: string };

export interface NoteAutoSave {
  state: NoteSaveState;
  /** Объявить новую версию буфера — она уйдёт в файл после паузы. */
  changed: (next: string) => void;
  /**
   * Записать немедленно, не дожидаясь паузы.
   *
   * Обязателен перед переключением на другую заметку и перед размонтированием:
   * иначе последние нажатия остались бы в буфере дебаунса, а на диске — текст
   * до них.
   */
  flush: () => Promise<void>;
  /**
   * Принять текст как уже лежащий на диске — чтение файла и принятую внешнюю
   * версию. Не запись: версия уже на диске, и писать её второй раз значило бы
   * принять за пользователя решение, которого он не принимал.
   */
  accept: (text: string) => void;
}

/**
 * Автосохранение открытой заметки (замена кнопки «Сохранить»).
 *
 * Почему без `expected`: расхождение с диском обнаруживает ядро, которое держит
 * снимок открытого файла. Придумывать ожидаемый отпечаток здесь значило бы
 * завести вторую версию правила о конфликтах — та же причина, по которой его не
 * передаёт автосохранение досок.
 */
export function useNoteAutoSave(
  client: VaultClient | null,
  path: string | null,
): NoteAutoSave {
  const [state, setState] = useState<NoteSaveState>({ state: "Clean" });

  // Буфер в ref, а не в state: его читают таймер и `flush`, и оба обязаны
  // видеть последнее нажатие, а не то, что было на момент планирования записи.
  const bufferRef = useRef<string | null>(null);
  // Что уже лежит на диске. По нему видно, есть ли что писать: повторная запись
  // того же текста трогала бы файл от одного открытия заметки.
  const savedRef = useRef<string | null>(null);
  const timerRef = useRef<number | null>(null);

  // Какая заметка открыта **сейчас**. Запись, начатая перед переключением,
  // дописывает прежний файл — это правильно, — но её состояние в статус-баре
  // относилось бы уже к другой заметке: «сохранение…» и тем более отказ
  // показались бы про ту, которую только что открыли.
  const pathRef = useRef(path);
  pathRef.current = path;
  const report = useCallback((next: NoteSaveState, forPath: string): void => {
    if (pathRef.current === forPath) setState(next);
  }, []);

  const cancelPendingSave = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const save = useCallback(async (): Promise<void> => {
    const buffer = bufferRef.current;
    const forPath = path;
    if (!client || forPath === null || buffer === null) return;
    if (buffer === savedRef.current) return;

    // Пока ядро держит неразрешённый конфликт по этой заметке, запись молчит:
    // выбор версии принадлежит пользователю (001 FR-021), и автосохранение «на
    // всякий случай» стёрло бы внешнюю правку прежде, чем он её увидел. Тот же
    // гейт, что у досок, только у них он внутри ядра.
    try {
      if ((await client.pendingConflict(forPath)) !== undefined) {
        report({ state: "Held" }, forPath);
        return;
      }
    } catch {
      // Спросить не удалось — считаем, что конфликт возможен, и не пишем:
      // ошибиться в эту сторону значит отложить запись, в другую — потерять
      // чужой текст.
      report({ state: "Held" }, forPath);
      return;
    }

    report({ state: "Saving" }, forPath);
    try {
      await client.write(forPath, buffer, undefined);
      savedRef.current = buffer;
      // Пока шла запись, могли успеть набрать ещё: тогда состояние остаётся
      // «есть несохранённое», а не становится «сохранено».
      report(
        bufferRef.current === buffer ? { state: "Clean" } : { state: "Dirty" },
        forPath,
      );
    } catch (cause) {
      report({ state: "Failed", reason: describeWriteFailure(cause) }, forPath);
    }
  }, [client, path, report]);

  const changed = useCallback(
    (next: string) => {
      if (bufferRef.current === next) return;
      bufferRef.current = next;
      if (next === savedRef.current) return;
      setState((previous) =>
        previous.state === "Dirty" ? previous : { state: "Dirty" },
      );
      cancelPendingSave();
      timerRef.current = window.setTimeout(() => {
        timerRef.current = null;
        void save();
      }, NOTE_AUTOSAVE_DEBOUNCE_MS);
    },
    [cancelPendingSave, save],
  );

  const flush = useCallback(async (): Promise<void> => {
    cancelPendingSave();
    await save();
  }, [cancelPendingSave, save]);

  const accept = useCallback(
    (text: string) => {
      cancelPendingSave();
      bufferRef.current = text;
      savedRef.current = text;
      setState({ state: "Clean" });
    },
    [cancelPendingSave],
  );

  // Смена заметки: незаписанное принадлежало прежнему файлу, и писать его после
  // переключения было бы уже не в тот буфер. Дописывает `NoteEditor` перед
  // сменой пути — здесь только отмена таймера, чтобы он не выстрелил по новому
  // пути старым текстом.
  useEffect(() => {
    cancelPendingSave();
    bufferRef.current = null;
    savedRef.current = null;
    setState({ state: "Clean" });
  }, [client, path, cancelPendingSave]);

  // Незавершённый дебаунс при размонтировании — таймер в отсутствующем
  // компоненте. Предупреждение о несохранённом делает `useWindowCloseGuard`:
  // браузер не ждёт промисов при закрытии вкладки, и дописать отсюда нечем.
  useEffect(() => cancelPendingSave, [cancelPendingSave]);

  return { state, changed, flush, accept };
}

/** Причина отказа записи словами ядра (001 FR-009). */
function describeWriteFailure(cause: unknown): string {
  if (typeof cause === "object" && cause !== null && "kind" in cause) {
    switch ((cause as { kind: unknown }).kind) {
      case "Conflict":
        return "Заметка изменилась на диске — выберите, какую версию оставить.";
      case "AccessDenied":
        return "Vault недоступен на запись — набранное не потеряно.";
      case "Io":
        return "Не удалось записать заметку на диск — набранное не потеряно.";
      default:
        break;
    }
  }
  return "Сохранить не удалось — набранное не потеряно.";
}
