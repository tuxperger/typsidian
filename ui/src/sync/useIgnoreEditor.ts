"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type {
  DiffLine,
  IgnoreCheck,
  IgnoreDocument,
  IgnoreFile,
  IgnoreRefusal,
  SyncError,
} from "../wasm-core/typsidian_core.js";
import { asSyncError } from "./sync-client.js";
import type { SyncControl } from "./useSync.js";

// Состояние редактора правил исключения (фича 012, data-model.md «Переходы
// состояния диалога»).
//
// Правил здесь нет (принцип I): можно ли править файл, чего в нём не хватает,
// разошёлся ли он с диском и нужно ли подтверждение — отвечает ядро. Хук
// держит текст, передаёт выбор пользователя флагами запроса и показывает
// исход.

/** Пауза после остановки набора перед проверкой текста. */
const CHECK_DEBOUNCE_MS = 300;

/** Что диалог показывает поверх поля. */
export type IgnoreEditorStage =
  | { kind: "edit" }
  /** Не хватает правил приложения — нужно отдельное подтверждение (FR-012). */
  | { kind: "confirm"; missing: string[]; overwrite: boolean }
  /** На диске не та версия, с которой начата правка (FR-015). */
  | { kind: "diverged"; disk: IgnoreDocument; diff: DiffLine[] }
  /** Закрытие с несохранённой правкой (FR-006). */
  | { kind: "closing" };

/** Сообщение под полем; сбрасывается следующим действием. */
export type IgnoreEditorNotice = "saved" | "updatedOutside" | "staleOutside";

export interface IgnoreEditorState {
  /** `null` — файл ещё читается. */
  document: IgnoreDocument | null;
  text: string;
  check: IgnoreCheck | null;
  dirty: boolean;
  canSave: boolean;
  saving: boolean;
  stage: IgnoreEditorStage;
  notice: IgnoreEditorNotice | null;
  refusal: IgnoreRefusal | null;
  error: SyncError | null;
  setText: (text: string) => void;
  restoreBlock: () => void;
  save: () => void;
  /** Сохранить без правил приложения — ответ на `confirm`. */
  confirmIncomplete: () => void;
  /** Записать свою версию поверх изменившейся на диске — ответ на `diverged`. */
  overwrite: () => void;
  /** Взять версию с диска, отбросив свою правку. */
  takeDisk: () => void;
  backToEdit: () => void;
  /** Просьба закрыть: `true` — закрывать можно сразу. */
  requestClose: () => boolean;
}

export function useIgnoreEditor(sync: SyncControl, file: IgnoreFile): IgnoreEditorState {
  const { client, revision, refresh } = sync;
  const [document, setDocument] = useState<IgnoreDocument | null>(null);
  const [text, setTextState] = useState("");
  const [check, setCheck] = useState<IgnoreCheck | null>(null);
  const [saving, setSaving] = useState(false);
  const [stage, setStage] = useState<IgnoreEditorStage>({ kind: "edit" });
  const [notice, setNotice] = useState<IgnoreEditorNotice | null>(null);
  const [refusal, setRefusal] = useState<IgnoreRefusal | null>(null);
  const [error, setError] = useState<SyncError | null>(null);

  // Свежие значения для колбэков и эффектов, которые не должны
  // перезапускаться на каждое нажатие клавиши.
  const documentRef = useRef(document);
  documentRef.current = document;
  const textRef = useRef(text);
  textRef.current = text;

  const adopt = useCallback((next: IgnoreDocument) => {
    setDocument(next);
    setTextState(next.text);
    setCheck(next.check);
  }, []);

  // Чтение файла — при открытии и на каждое обновление сводки (такт
  // синхронизации, событие vault): так замечается правка извне (FR-014).
  useEffect(() => {
    let cancelled = false;
    void client
      .ignoreOpen(file)
      .then((next) => {
        if (cancelled) return;
        const current = documentRef.current;
        if (current === null) {
          adopt(next);
          return;
        }
        if (current.base === next.base && current.exists === next.exists) return;
        if (textRef.current === current.text) {
          // Своей правки нет — показываем то, что на диске.
          adopt(next);
          setNotice("updatedOutside");
        } else {
          // Своя правка есть: текст не трогаем, решение — при сохранении.
          setNotice("staleOutside");
        }
      })
      .catch((cause: unknown) => {
        if (!cancelled) setError(asSyncError(cause));
      });
    return () => {
      cancelled = true;
    };
  }, [client, file, revision, adopt]);

  // Проверка обязательных правил по мере набора (FR-010).
  useEffect(() => {
    if (documentRef.current === null) return undefined;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void client
        .ignoreCheck(file, text)
        .then((next) => {
          if (!cancelled) setCheck(next);
        })
        .catch(() => undefined);
    }, CHECK_DEBOUNCE_MS);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [client, file, text]);

  const setText = useCallback((next: string) => {
    setTextState(next);
    setNotice((current) => (current === "staleOutside" ? current : null));
    setRefusal(null);
    setError(null);
  }, []);

  const submit = useCallback(
    async (flags: { confirmIncomplete: boolean; overwrite: boolean }): Promise<void> => {
      const current = documentRef.current;
      if (current === null) return;
      setSaving(true);
      setNotice(null);
      setRefusal(null);
      setError(null);
      try {
        const outcome = await client.ignoreSave({
          file,
          text: textRef.current,
          line_ending: current.line_ending,
          base: current.base,
          confirm_incomplete: flags.confirmIncomplete,
          overwrite: flags.overwrite,
        });
        switch (outcome.kind) {
          case "Saved":
            adopt(outcome.document);
            setStage({ kind: "edit" });
            setNotice("saved");
            void refresh();
            break;
          case "Diverged":
            setStage({ kind: "diverged", disk: outcome.disk, diff: outcome.diff });
            break;
          case "NeedsConfirmation":
            setStage({ kind: "confirm", missing: outcome.missing, overwrite: flags.overwrite });
            break;
          case "Refused":
            setStage({ kind: "edit" });
            setRefusal(outcome.reason);
            break;
          default: {
            const exhaustive: never = outcome;
            return exhaustive;
          }
        }
      } catch (cause) {
        setStage({ kind: "edit" });
        setError(asSyncError(cause));
      } finally {
        setSaving(false);
      }
    },
    [client, file, adopt, refresh],
  );

  const save = useCallback(() => {
    void submit({ confirmIncomplete: false, overwrite: false });
  }, [submit]);

  const confirmIncomplete = useCallback(() => {
    const overwrite = stage.kind === "confirm" && stage.overwrite;
    void submit({ confirmIncomplete: true, overwrite });
  }, [stage, submit]);

  const overwrite = useCallback(() => {
    void submit({ confirmIncomplete: false, overwrite: true });
  }, [submit]);

  const takeDisk = useCallback(() => {
    if (stage.kind !== "diverged") return;
    adopt(stage.disk);
    setStage({ kind: "edit" });
    setNotice(null);
  }, [stage, adopt]);

  const backToEdit = useCallback(() => setStage({ kind: "edit" }), []);

  const restoreBlock = useCallback(() => {
    void client
      .ignoreRestore(file, textRef.current)
      .then(setText)
      .catch((cause: unknown) => setError(asSyncError(cause)));
  }, [client, file, setText]);

  const dirty = document !== null && text !== document.text;

  const requestClose = useCallback((): boolean => {
    const current = documentRef.current;
    if (current === null || textRef.current === current.text) return true;
    setStage({ kind: "closing" });
    return false;
  }, []);

  return {
    document,
    text,
    check,
    dirty,
    // Несуществующий файл сохраняется и без правки: это и есть его создание.
    canSave:
      document !== null &&
      document.editable === "Editable" &&
      !saving &&
      (dirty || !document.exists),
    saving,
    stage,
    notice,
    refusal,
    error,
    setText,
    restoreBlock,
    save,
    confirmIncomplete,
    overwrite,
    takeDisk,
    backToEdit,
    requestClose,
  };
}
