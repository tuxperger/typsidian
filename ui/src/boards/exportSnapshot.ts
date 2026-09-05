"use client";

import type { EmbedPlan } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { exportBoardContentToSvg } from "./exportSvg.js";

// Снимок вставки (T044, FR-019, FR-028).
//
// Порядок шагов здесь — не деталь реализации, а требование research.md §6:
// сначала принудительный сброс дебаунса автосохранения, только потом экспорт.
// Иначе снимок построится по состоянию полотна, которого ещё нет в файле, и
// вставка окажется устаревшей в момент создания.
//
// Экспорт идёт через общую обёртку `exportSvg.ts` (T015) — второго вызова
// `exportToSvg` в проекте нет. Метаданные в SVG вкладывает ядро
// (`writeSnapshot`): формат метаданных и хэш содержимого — его правила, не наши.

export type SnapshotResult = { ok: true } | { ok: false; reason: string };

export interface SnapshotRequest {
  client: VaultClient;
  /** Куда писать и какую доску снимок отражает. */
  plan: Pick<EmbedPlan, "snapshot_path" | "board_path">;
  /** Содержимое доски на момент снимка — то же, что уже записано в файл. */
  boardContent: string;
  /**
   * Сброс дебаунса автосохранения открытой доски. Обязателен, если снимок
   * строится по доске, открытой на полотне.
   */
  flush?: () => Promise<void>;
}

/**
 * Строит снимок доски и записывает его файл.
 *
 * Отказ — значение, а не исключение: движок может не построить SVG (FR-016), а
 * vault может быть недоступен на запись. Вызывающая сторона обязана сказать
 * пользователю причину, а не факт неудачи.
 */
export async function writeBoardSnapshot({
  client,
  plan,
  boardContent,
  flush,
}: SnapshotRequest): Promise<SnapshotResult> {
  if (flush) await flush();

  const exported = await exportBoardContentToSvg(boardContent);
  if (!exported.ok) {
    return { ok: false, reason: `не удалось построить изображение: ${exported.reason}` };
  }

  try {
    await client.writeSnapshot(
      plan.snapshot_path,
      exported.svg,
      plan.board_path,
      boardContent,
    );
    return { ok: true };
  } catch (cause) {
    return { ok: false, reason: describeSnapshotFailure(cause) };
  }
}

/** Причина отказа записи словами. */
function describeSnapshotFailure(cause: unknown): string {
  const kind =
    typeof cause === "object" && cause !== null && "kind" in cause
      ? (cause as { kind: unknown }).kind
      : null;
  switch (kind) {
    case "NotASnapshotPath":
      return "путь снимка ведёт не в папку вставок заметки";
    case "NotAnSvg":
      return "движок вернул не SVG";
    case "VaultNotWritable":
      return "vault недоступен на запись";
    case "Io":
      return "не удалось записать файл снимка";
    default:
      return "записать снимок не удалось";
  }
}
