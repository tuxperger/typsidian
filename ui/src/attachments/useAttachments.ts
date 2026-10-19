"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { EditorView } from "@codemirror/view";
import type { VaultClient } from "../vault-client.js";
import type {
  AttachmentBatchState,
  AttachmentBatchView,
  IncomingItem,
  InsertMoment,
  VaultOverrides,
} from "../wasm-core/typsidian_core.js";
import { plainInputTarget } from "../templates/insertion/plainInputTarget.js";
import { savedPath } from "./describe.js";
import { trackSelection, type AttachmentDrop } from "./editorExtension.js";

// Пачка вложений в рабочей области (фича 016, contracts/frontend-attachments.md
// → useAttachments). Правила пачки — в ядре; здесь цикл: шаг, чтение байтов
// одного файла, запись, уступка циклу событий. Текст меняется один раз, в
// конце, через порт вставки фичи 003 (FR-005).

/** Прогресс показывается, если копирование дольше этого (одиночный файл). */
const PROGRESS_DELAY_MS = 400;

/** Что сейчас показать в ряду уведомлений (макет 1b). */
export type AttachmentNotice =
  | { kind: "progress"; view: AttachmentBatchView; current: string | null }
  | { kind: "summary"; view: AttachmentBatchView; notInserted: boolean }
  | { kind: "refused"; view: AttachmentBatchView | null; reason: "busy" | null }
  | { kind: "link" };

/** Ожидающий ответа большой файл (FR-013). */
export interface LargeImageRequest {
  name: string;
  size: number;
  folder: string;
}

export interface AttachmentsApi {
  /** Файлы из буфера, броска или выбора — в заметку `notePath`. */
  insert(files: File[], drop: AttachmentDrop, notePath: string): void;
  /** Вставка выбранных файлов к каретке редактора (команда, меню, шторка). */
  insertAtCaret(files: File[], origin: "Camera" | "File"): void;
  linkDropped(): void;
  cancel(): void;
  notice: AttachmentNotice | null;
  dismiss(): void;
  large: LargeImageRequest | null;
  answerLarge(accept: boolean): void;
  /** Имя последнего одиночного вставленного файла — для статус-бара (макет 1a). */
  lastInserted: string | null;
}

/** Местное время устройства: часов в ядре нет (research.md §5). */
function now(): InsertMoment {
  const date = new Date();
  return {
    year: date.getFullYear(),
    month: date.getMonth() + 1,
    day: date.getDate(),
    hour: date.getHours(),
    minute: date.getMinutes(),
    second: date.getSeconds(),
  };
}

function nextTask(): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, 0));
}

export function useAttachments(
  client: VaultClient | null,
  overrides: VaultOverrides | null,
  /** Путь активной заметки Typst и её редактор — для вставки командой. */
  activeNotePath: string | null,
  getView: (() => EditorView | null) | null,
): AttachmentsApi {
  const [notice, setNotice] = useState<AttachmentNotice | null>(null);
  const [large, setLarge] = useState<LargeImageRequest | null>(null);
  const [lastInserted, setLastInserted] = useState<string | null>(null);
  const running = useRef<{ state: AttachmentBatchState; cancelled: boolean } | null>(null);
  const answer = useRef<((accept: boolean) => void) | null>(null);
  const latest = useRef({ client, overrides, activeNotePath, getView });
  latest.current = { client, overrides, activeNotePath, getView };

  // Смена vault — пачка отменяется: её состояние принадлежит прежнему клиенту.
  useEffect(
    () => () => {
      if (running.current !== null) running.current.cancelled = true;
      answer.current?.(false);
    },
    [client],
  );

  const run = useCallback(async (files: File[], drop: AttachmentDrop, notePath: string) => {
    const { client: current, overrides: settings } = latest.current;
    if (current === null || settings === null) return;
    if (running.current !== null) {
      drop.place.release();
      setNotice({ kind: "refused", view: null, reason: "busy" });
      return;
    }
    const ops = current.attachments;
    const items: IncomingItem[] = files.map((file) => ({
      ...(drop.origin === "File" ? { name: file.name } : {}),
      size: file.size,
      origin: drop.origin,
    }));
    const started = performance.now();
    let state = await ops.start(settings, notePath, items, now());
    if (state.phase.kind === "Refused") {
      drop.place.release();
      setNotice({ kind: "refused", view: await ops.view(state), reason: null });
      return;
    }
    const job = { state, cancelled: false };
    running.current = job;
    setLastInserted(null);
    try {
      for (;;) {
        // Заметку закрыли или сменился vault — дальше не копируем.
        if ((job.cancelled || drop.place.view() === null) && state.phase.kind === "Running") {
          state = await ops.cancel(state);
        }
        const step = await ops.next(state);
        state = step.state;
        job.state = state;
        const next = step.next;
        if (next === undefined) break;
        if (next.kind === "Confirm") {
          const item = state.items[next.index];
          const accept = await new Promise<boolean>((resolve) => {
            answer.current = resolve;
            setLarge({
              name: item?.incoming.name ?? "",
              size: item?.incoming.size ?? 0,
              folder: state.target_folder,
            });
          });
          answer.current = null;
          setLarge(null);
          state = await ops.confirm(state, next.index, accept);
          continue;
        }
        const file = files[next.index];
        if (files.length > 1 || performance.now() - started > PROGRESS_DELAY_MS) {
          setNotice({
            kind: "progress",
            view: await ops.view(state),
            current: file?.name ?? null,
          });
        }
        let bytes: Uint8Array | null = null;
        try {
          if (file !== undefined) bytes = new Uint8Array(await file.arrayBuffer());
        } catch {
          bytes = null;
        }
        state =
          bytes === null
            ? await ops.sourceFailed(state, next.index)
            : await ops.store(state, next.index, bytes);
        job.state = state;
        // Редактор не блокируется между файлами (SC-005).
        await nextTask();
      }

      const view = await ops.view(state);
      const place = drop.place.current();
      let inserted = false;
      if (view.markup_paths.length > 0 && place !== null) {
        const edit = await ops.planMarkup(
          place.textLength,
          place.point,
          place.before,
          place.after,
          view.markup_paths,
          drop.block,
        );
        // Порт вставки 003: одна транзакция — одна единица отмены (FR-005).
        if (edit !== undefined) {
          inserted = plainInputTarget(() => drop.place.view()).apply(edit).ok;
        }
      }
      report(view, inserted);
    } finally {
      drop.place.release();
      running.current = null;
    }

    function report(view: AttachmentBatchView, inserted: boolean): void {
      const notInserted = view.markup_paths.length > 0 && !inserted;
      const only = view.state.items.length === 1 ? view.state.items[0] : undefined;
      const saved = only === undefined ? null : savedPath(only.status);
      if (only !== undefined && saved === null && !notInserted) {
        // Единственный файл не вставлен — отказ с причиной (макет 1b).
        setNotice({ kind: "refused", view, reason: null });
        return;
      }
      const quiet =
        only !== undefined &&
        inserted &&
        view.state.excluded_by.length === 0 &&
        !(only.status.kind === "Stored" && only.status.saved_as !== undefined);
      if (quiet) {
        // Одиночная удачная вставка — строкой статус-бара (макет 1a).
        setLastInserted(saved === null ? null : saved.slice(saved.lastIndexOf("/") + 1));
        setNotice(null);
        return;
      }
      setNotice({ kind: "summary", view, notInserted });
    }
  }, []);

  const insert = useCallback(
    (files: File[], drop: AttachmentDrop, notePath: string) => void run(files, drop, notePath),
    [run],
  );

  const insertAtCaret = useCallback(
    (files: File[], origin: "Camera" | "File") => {
      const { activeNotePath: notePath, getView: viewOf } = latest.current;
      const view = viewOf?.() ?? null;
      if (files.length === 0 || notePath === null || view === null) return;
      void run(
        files,
        {
          origin,
          block: files.length > 1,
          place: trackSelection(view),
        },
        notePath,
      );
    },
    [run],
  );

  return {
    insert,
    insertAtCaret,
    linkDropped: useCallback(() => setNotice({ kind: "link" }), []),
    cancel: useCallback(() => {
      if (running.current !== null) running.current.cancelled = true;
    }, []),
    notice,
    dismiss: useCallback(() => setNotice(null), []),
    large,
    answerLarge: useCallback((accept: boolean) => answer.current?.(accept), []),
    lastInserted,
  };
}
