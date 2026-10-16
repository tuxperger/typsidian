"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  Capture,
  CaptureFailure,
  CaptureTarget,
  TextEdit,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import type { CaptureSource } from "./capture-source.js";

/**
 * Лист захвата (фича 022, US1, D1–D3).
 *
 * Правил записи здесь нет (принцип I): вид записи, дописывание в конец, путь
 * назначения и решение «правка буфера или запись файла» — ответы ядра
 * (`packages/core/src/capture`). Хук держит очередь источника, выбор
 * пользователя и порядок действий: записать → `resolve` → вернуться в
 * исходное приложение.
 *
 * Запись остаётся в очереди источника, пока она не записана или пользователь не
 * отказался сам (FR-005, SC-003): отказ записи `resolve` не вызывает.
 */
export interface CaptureHost {
  client: VaultClient | null;
  /** Vault ещё открывается: «нет доступа» показывать рано. */
  vaultOpening: boolean;
  /** Приём «Поделиться»; `undefined` — оболочка его не даёт. */
  source: CaptureSource | undefined;
  /** Текст заметки, загруженной в редактор; `undefined` — заметка не открыта. */
  openTextOf: (path: string) => string | undefined;
  /**
   * Применить правку буфера открытой заметки одной транзакцией; сохранение —
   * обычным путём документа. `false` — текст заметки уже изменился.
   */
  applyEdit: (path: string, edit: TextEdit, expected: string) => Promise<boolean>;
  /** Заметки из недавних (открытые вкладки) — для назначения. */
  recent: readonly string[];
  /** Запись состоялась: дерево перечитывается. */
  onWritten: () => void;
  /** Ярлык «Сегодня» снаружи приложения. */
  onToday: () => void;
}

/** Что показывает лист. */
export interface CaptureSheetModel {
  /** Захват как он будет записан: с выбранным назначением и припиской. */
  capture: Capture;
  /** Своя мысль из команды «Во входящие»: текст набирается в листе. */
  manual: boolean;
  text: string;
  setText: (text: string) => void;
  target: CaptureTarget;
  setTarget: (target: CaptureTarget) => void;
  note: string;
  setNote: (note: string) => void;
  recent: readonly string[];
  /** Заметка назначения по правилам ядра; `null` — пока не известна. */
  destination: string | null;
  /** Предпросмотр и размер картинки захвата. */
  image: { url: string; bytes: number } | null;
  /** Причина, по которой записать нельзя; `null` — лист принимает подтверждение. */
  failure: CaptureFailure | null;
  busy: boolean;
  /** Сколько захватов ждут записи, включая показанный. */
  waiting: number;
  /** Сколько ещё стоит за показанным. */
  more: number;
  save: () => void;
  /** Закрыть. Захват, который нельзя записать, остаётся в очереди. */
  close: () => void;
  /** Явный отказ: убрать захват из очереди. */
  discard: () => void;
}

export interface CaptureState {
  sheet: CaptureSheetModel | null;
  /** «Во входящие» (команда палитры и ярлык): пустой лист для своей мысли. */
  openManual: () => void;
}

const INBOX: CaptureTarget = { kind: "Inbox" };

function textOf(capture: Capture): string {
  return capture.content.kind === "Text" ? capture.content.text : "";
}

function newId(): string {
  return typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : `manual-${Date.now()}`;
}

function momentOf(now: Date): Capture["at"] {
  return {
    year: now.getFullYear(),
    month: now.getMonth() + 1,
    day: now.getDate(),
    hour: now.getHours(),
    minute: now.getMinutes(),
  };
}

export function useCapture(host: CaptureHost): CaptureState {
  const { client, vaultOpening, source, openTextOf, applyEdit, onWritten, onToday } = host;
  const [queue, setQueue] = useState<Capture[]>([]);
  const [manual, setManual] = useState<Capture | null>(null);
  const [open, setOpen] = useState(false);
  const [target, setTarget] = useState<CaptureTarget>(INBOX);
  const [note, setNote] = useState("");
  const [text, setText] = useState("");
  const [failure, setFailure] = useState<CaptureFailure | null>(null);
  const [busy, setBusy] = useState(false);
  const [destination, setDestination] = useState<string | null>(null);
  const [image, setImage] = useState<{ url: string; bytes: number } | null>(null);

  // Свежие значения для обработчиков источника: подписка не должна
  // переоформляться на каждую отрисовку.
  const hostRef = useRef(host);
  hostRef.current = host;
  // Захваты, которые пришли, пока vault был недоступен: записываются сами, как
  // только он откроется (US1 AS5, quickstart §3.4).
  const deferred = useRef(new Set<string>());
  const autoRunning = useRef(false);

  const current = manual ?? queue[0] ?? null;
  const currentId = current?.id ?? null;
  const isManual = manual !== null;

  // ── Очередь источника ──────────────────────────────────────────────
  const reload = useCallback(async () => {
    if (source === undefined) {
      setQueue([]);
      return;
    }
    try {
      const pending = await source.pending();
      setQueue(pending);
      if (pending.length > 0) setOpen(true);
    } catch {
      // Очередь не прочиталась — она на месте, прочтём при следующем событии.
    }
  }, [source]);

  // Разбор очереди при открытии vault и при смене источника.
  useEffect(() => {
    void reload();
  }, [reload, client]);

  useEffect(() => {
    if (source === undefined) return undefined;
    const offShared = source.onShared(() => void reload());
    const offShortcut = source.onShortcut((action) => {
      if (action === "today") hostRef.current.onToday();
      else openManualRef.current();
    });
    return () => {
      offShared();
      offShortcut();
    };
  }, [source, reload]);

  // Другой захват на листе — всё поле формы начинается с его значений.
  useEffect(() => {
    if (current === null) return;
    setTarget(current.target);
    setNote(current.note ?? "");
    setText(textOf(current));
    setFailure(null);
    setBusy(false);
    // Только смена захвата; сам объект меняется вместе с правками формы.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentId]);

  // ── Назначение по правилам ядра ────────────────────────────────────
  useEffect(() => {
    if (client === null || current === null) {
      setDestination(null);
      return undefined;
    }
    let alive = true;
    void (async () => {
      try {
        const settings = await client.capture.settings(await client.vaultSettings());
        const path = await client.capture.targetPath(settings, { ...current, target });
        if (alive) setDestination(path ?? null);
      } catch {
        if (alive) setDestination(null);
      }
    })();
    return () => {
      alive = false;
    };
    // Назначение зависит от выбора, а не от текста набираемой мысли.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [client, currentId, target]);

  // ── Картинка захвата ───────────────────────────────────────────────
  useEffect(() => {
    if (current === null || current.content.kind !== "Image" || source === undefined) {
      setImage(null);
      return undefined;
    }
    let alive = true;
    let url: string | null = null;
    const content = current.content;
    void (async () => {
      try {
        const bytes = await source.image(current);
        if (!alive) return;
        url = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: content.mime }));
        setImage({ url, bytes: bytes.byteLength });
      } catch {
        if (alive) setImage(null);
      }
    })();
    return () => {
      alive = false;
      if (url !== null) URL.revokeObjectURL(url);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [source, currentId]);

  /** Захват в том виде, как он уйдёт в ядро. */
  const built = useCallback(
    (base: Capture): Capture => {
      const trimmed = note.trim();
      const { note: _dropped, ...rest } = base;
      void _dropped;
      return {
        ...rest,
        ...(trimmed === "" ? {} : { note: trimmed }),
        content: isManual && base.id === manual?.id ? { kind: "Text", text } : base.content,
        target: base.id === currentId ? target : base.target,
        at: isManual && base.id === manual?.id ? momentOf(new Date()) : base.at,
      };
    },
    [note, isManual, manual, text, currentId, target],
  );

  /** Записать захват. Возвращает причину отказа либо `null`. */
  const write = useCallback(
    async (capture: Capture): Promise<CaptureFailure | null> => {
      const live = hostRef.current;
      if (live.client === null) return "noVault";
      const vault = live.client;
      try {
        const settings = await vault.capture.settings(await vault.vaultSettings());
        const path = await vault.capture.targetPath(settings, capture);
        // Заметка открыта в редакторе — ядру нужен её текст, а не файл (FR-004).
        const buffer = path === undefined ? undefined : live.openTextOf(path);
        const bytes =
          capture.content.kind === "Image" && live.source !== undefined
            ? await live.source.image(capture)
            : undefined;
        const outcome = await vault.capture.write(capture, buffer, bytes);
        switch (outcome.kind) {
          case "Failed":
            return outcome.reason;
          case "EditBuffer":
            // Текст успел измениться — правка по старым смещениям испортила бы его.
            if (buffer === undefined || !(await live.applyEdit(outcome.path, outcome.edit, buffer))) {
              return "conflict";
            }
            break;
          case "Written":
            break;
          default: {
            const exhaustive: never = outcome;
            return exhaustive;
          }
        }
        live.onWritten();
        return null;
      } catch {
        return "io";
      }
    },
    [],
  );

  /** Захват записан (или от него отказались): убрать из очереди и вернуться. */
  const complete = useCallback(
    async (capture: Capture, goBack: boolean) => {
      const isOwn = manual !== null && capture.id === manual.id;
      if (isOwn) {
        setManual(null);
        if (queue.length === 0) setOpen(false);
        return;
      }
      try {
        await source?.resolve(capture.id);
      } catch {
        // Не убралась — останется в очереди и спросится снова; потерь нет.
        return;
      }
      deferred.current.delete(capture.id);
      const rest = queue.filter((entry) => entry.id !== capture.id);
      setQueue(rest);
      if (rest.length === 0) {
        setOpen(false);
        if (goBack) await source?.returnToSource().catch(() => undefined);
      }
    },
    [manual, queue, source],
  );

  const blocked = client === null && !vaultOpening;
  // Захваты, показанные без vault, дождутся его и запишутся сами.
  useEffect(() => {
    if (blocked) for (const entry of queue) deferred.current.add(entry.id);
  }, [blocked, queue]);

  useEffect(() => {
    if (client === null || autoRunning.current) return;
    const next = queue.find((entry) => deferred.current.has(entry.id));
    if (next === undefined) return;
    autoRunning.current = true;
    void (async () => {
      const reason = await write(next);
      deferred.current.delete(next.id);
      if (reason === null) await complete(next, false);
      else {
        setFailure(reason);
        setOpen(true);
      }
      autoRunning.current = false;
    })();
  }, [client, queue, write, complete]);

  const save = useCallback(() => {
    if (current === null || busy) return;
    const capture = built(current);
    setBusy(true);
    setFailure(null);
    void (async () => {
      const reason = await write(capture);
      if (reason !== null) {
        setFailure(reason);
        setBusy(false);
        return;
      }
      await complete(capture, true);
      setBusy(false);
    })();
  }, [current, busy, built, write, complete]);

  const discard = useCallback(() => {
    if (current === null) return;
    void complete(current, true);
  }, [current, complete]);

  const shownFailure: CaptureFailure | null = blocked ? "noVault" : failure;

  const close = useCallback(() => {
    // Подтверждённый, но не записанный захват остаётся ждать; неподтверждённый
    // закрытие листа — отказ от него.
    if (shownFailure !== null || current === null) setOpen(false);
    else discard();
  }, [shownFailure, current, discard]);

  const openManual = useCallback(() => {
    setManual({
      id: newId(),
      content: { kind: "Text", text: "" },
      at: momentOf(new Date()),
      target: INBOX,
    });
    setOpen(true);
  }, []);
  const openManualRef = useRef(openManual);
  openManualRef.current = openManual;

  const sheet = useMemo<CaptureSheetModel | null>(() => {
    if (!open || current === null || vaultOpening) return null;
    return {
      capture: built(current),
      manual: isManual,
      text,
      setText,
      target,
      setTarget,
      note,
      setNote,
      recent: host.recent,
      destination,
      image,
      failure: shownFailure,
      busy,
      waiting: queue.length + (isManual ? 1 : 0),
      more: Math.max(0, queue.length - (isManual ? 0 : 1)),
      save,
      close,
      discard,
    };
  }, [
    open,
    current,
    vaultOpening,
    built,
    isManual,
    text,
    target,
    note,
    host.recent,
    destination,
    image,
    shownFailure,
    busy,
    queue.length,
    save,
    close,
    discard,
  ]);

  return { sheet, openManual };
}
