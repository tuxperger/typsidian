// Единственное место, откуда вызывается экспорт движка (фича 002, T015,
// contracts/board-engine-bridge.md). Обе истории пользуются им: US1 строит
// здесь превью двух версий при конфликте, US2 — снимок для вставки. Второй
// вызов `exportToSvg` где-либо ещё запрещён: поведение при отказе экспорта
// разошлось бы между двумя местами, а FR-016 требует одного и того же —
// отказ становится значением, а не исключением.

import type { Phrase } from "../i18n/index.js";

/**
 * Аргументы экспорта выводятся из сигнатуры самого движка, а не описываются
 * руками. Так обвязка не называет его внутренних типов (принцип VIII: DOM и
 * внутренности движка — не наш API) и при этом остаётся типизированной без
 * `any` (принцип IV).
 */
type ExportToSvg = (typeof import("@excalidraw/excalidraw"))["exportToSvg"];
type ExportArguments = Parameters<ExportToSvg>[0];

/** Сцена доски в терминах движка: элементы, вложения и состояние вида. */
export type BoardScene = Pick<ExportArguments, "elements" | "files" | "appState">;

/**
 * Результат экспорта. Отказ — вариант значения, а не исключение: превью
 * внешней версии может не построиться (повреждённый файл, несовместимая версия
 * движка), и FR-016 требует, чтобы выбор между версиями при этом остался
 * доступен, а конфликт не разрешился сам.
 */
export type SvgExport =
  | { ok: true; svg: string }
  | { ok: false; reason: Phrase };

/**
 * Экспортирует сцену в SVG.
 *
 * Движок загружается динамически по той же причине, что и полотно: он требует
 * `window` и на сервере не выполняется (research.md §1).
 */
export async function exportBoardToSvg(scene: BoardScene): Promise<SvgExport> {
  try {
    const engine = await import("@excalidraw/excalidraw");
    const element = await engine.exportToSvg({
      elements: scene.elements,
      files: scene.files,
      appState: scene.appState,
    });
    return { ok: true, svg: new XMLSerializer().serializeToString(element) };
  } catch (cause) {
    return { ok: false, reason: describeExportFailure(cause) };
  }
}

/**
 * Экспортирует **содержимое файла доски**.
 *
 * Та же обёртка, только вход — байты файла, а не разобранная сцена. Живёт здесь,
 * а не у вызывающих: содержимое доски приходит из ядра непрозрачной строкой
 * (contracts/board-store-trait.md), и разбирать её в двух местах — превью
 * конфликта и снимок — значило бы дважды описать, что считать сценой. Нечитаемый
 * файл даёт тот же вариант отказа, что и падение экспорта: FR-016 требует, чтобы
 * и то и другое оставляло выбор пользователю доступным.
 */
export async function exportBoardContentToSvg(content: string): Promise<SvgExport> {
  const scene = parseScene(content);
  if (scene === null) {
    return { ok: false, reason: (t) => t("boards.export.unparsable") };
  }
  return exportBoardToSvg(scene);
}

/**
 * Содержимое файла → сцена в терминах движка. Поля берутся по одному, а не
 * приведением объекта целиком: экспорт получает ровно то, что описано в
 * [`BoardScene`], и никаких других полей файла.
 */
function parseScene(content: string): BoardScene | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(content);
  } catch {
    return null;
  }
  if (typeof parsed !== "object" || parsed === null) return null;

  const scene = parsed as Partial<BoardScene>;
  if (!Array.isArray(scene.elements)) return null;
  return {
    elements: scene.elements,
    files: scene.files ?? null,
    // Отсутствие `appState` в файле — не отказ: движок в этом случае берёт свои
    // значения по умолчанию, а положение камеры содержимым не считается
    // (spec.md Assumptions).
    appState: scene.appState ?? {},
  };
}

/**
 * Причина отказа словами. Пользователю показывается именно она — «не удалось
 * построить превью» без объяснения неотличимо от пустого рисунка.
 */
function describeExportFailure(cause: unknown): Phrase {
  // Сообщение движка — его собственный текст, переводить его нечем.
  if (cause instanceof Error && cause.message !== "") return () => cause.message;
  return (t) => t("boards.export.engineFailed");
}
