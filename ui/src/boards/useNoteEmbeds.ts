"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { Embed } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { writeBoardSnapshot } from "./exportSnapshot.js";

// Вставки открытой заметки и их состояние (T057-T059, FR-026…FR-032).
//
// Ни одно правило здесь не живёт: какие вызовы `#image` считаются вставками,
// что такое устаревание и когда состояние надо пересчитать — решает ядро
// (`embedsOf`, `embedsChanged`). Этот хук вызывает его и держит результат для
// отрисовки.
//
// Запись — только по явному действию пользователя (FR-030, инвариант 2
// data-model.md): `update` вызывается из обработчика кнопки, и никакой пересчёт
// состояния сам к записи не приводит.

export interface NoteEmbeds {
  embeds: Embed[];
  /** Идёт запись снимка. */
  isUpdating: boolean;
  /** Отказ последнего действия словами; `null` — отказа не было. */
  error: string | null;
  /** Внешнее или своё изменение файла — повод пересчитать состояние вставок. */
  fileChanged: (path: string) => void;
  /**
   * Привести снимок вставки в соответствие с доской (FR-028) либо восстановить
   * отсутствующий снимок (FR-032) — это одно и то же действие над файлом,
   * различаются только слова, которыми оно предложено пользователю.
   */
  update: (embed: Embed) => Promise<void>;
  dismissError: () => void;
}

export function useNoteEmbeds(
  client: VaultClient | null,
  notePath: string | null,
  noteText: string,
): NoteEmbeds {
  const [embeds, setEmbeds] = useState<Embed[]>([]);
  const [isUpdating, setIsUpdating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Растёт при каждом поводе пересчитать: изменилась доска или снимок.
  const [revision, setRevision] = useState(0);

  // Текст держится и в ref: `fileChanged` обязан остаться стабильным между
  // нажатиями клавиш, иначе подписчик пересоздавался бы на каждый символ.
  const noteTextRef = useRef(noteText);
  noteTextRef.current = noteText;

  /**
   * Состав вставок в тексте. Разбор файлов не читает, а состояние вставок —
   * читает, поэтому перечитывать состояние на каждый набранный символ нельзя:
   * пересчёт запускается, только если состав изменился.
   */
  const [signature, setSignature] = useState("");

  useEffect(() => {
    if (!client || notePath === null) {
      setSignature("");
      return;
    }
    let cancelled = false;
    void (async () => {
      const found = await client.embedsInText(notePath, noteText);
      const next = found.map((one) => one.id.value).join("|");
      if (!cancelled) setSignature(next);
    })();
    return () => {
      cancelled = true;
    };
  }, [client, notePath, noteText]);

  useEffect(() => {
    if (!client || notePath === null || signature === "") {
      setEmbeds([]);
      return;
    }
    let cancelled = false;
    void (async () => {
      const found = await client.embedsOf(notePath, noteTextRef.current);
      if (!cancelled) setEmbeds(found);
    })();
    return () => {
      cancelled = true;
    };
  }, [client, notePath, signature, revision]);

  const fileChanged = useCallback(
    (path: string) => {
      if (!client || notePath === null) return;
      void (async () => {
        // Решает ядро: правило «изменение доски устаревает вставку» не должно
        // появиться второй раз здесь.
        const event = await client.embedsChanged(
          notePath,
          noteTextRef.current,
          path,
        );
        if (event !== undefined) setRevision((current) => current + 1);
      })();
    },
    [client, notePath],
  );

  const update = useCallback(
    async (embed: Embed): Promise<void> => {
      const boardPath = embed.board_path;
      if (!client || boardPath === undefined) return;
      setIsUpdating(true);
      setError(null);
      try {
        // Содержимое доски берётся из файла: доска, открытая на полотне, к этому
        // моменту уже дописана — переключение на заметку сбрасывает дебаунс
        // автосохранения (research.md §6).
        const boardContent = await client.readBoard(boardPath);
        const result = await writeBoardSnapshot({
          client,
          plan: { snapshot_path: embed.snapshot_path, board_path: boardPath },
          boardContent,
        });
        if (result.ok) setRevision((current) => current + 1);
        else setError(result.reason);
      } catch (cause) {
        setError(describeBoardReadFailure(cause));
      } finally {
        setIsUpdating(false);
      }
    },
    [client],
  );

  const dismissError = useCallback(() => setError(null), []);

  return { embeds, isUpdating, error, fileChanged, update, dismissError };
}

/** Почему доску-источник не удалось прочитать (FR-031). */
function describeBoardReadFailure(cause: unknown): string {
  const kind =
    typeof cause === "object" && cause !== null && "kind" in cause
      ? (cause as { kind: unknown }).kind
      : null;
  switch (kind) {
    case "NotFound":
      return "доска-источник не найдена — её удалили или переместили";
    case "Unreadable":
      return "файл доски не читается: он повреждён или создан несовместимой версией";
    default:
      return "доску-источник не удалось прочитать";
  }
}
