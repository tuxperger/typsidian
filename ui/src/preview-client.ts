import type {
  BatchState,
  BatchStep,
  BatchView,
  CompiledDocument,
  ExportCandidate,
  ExportReport,
  InstalledTemplate,
  InstallReport,
  ScrollAnchor,
  VaultOverrides,
} from "./wasm-core/typsidian_core.js";

// Поверхность предпросмотра, которой пользуется общий UI (фича 005).
//
// Отдельно от `VaultClient` по той же причине, по которой в ядре это отдельный
// объект: предпросмотр не читает vault и не владеет им. На web это компилятор в
// том же процессе, на desktop — команда Tauri; общий UI не знает, какой из них
// используется (принцип II).

export interface PreviewClient {
  /**
   * Собрать заметку из текста редактора.
   *
   * `revision` — монотонная версия текста; ядро помечает ею результат, чтобы
   * более старая сборка не вытеснила более новую (FR-009).
   */
  compile(
    notePath: string,
    text: string,
    revision: number,
    anchor?: ScrollAnchor,
  ): Promise<CompiledDocument>;

  /**
   * Собрать заметку, доустанавливая недостающие внешние шаблоны (фича 007,
   * FR-001).
   *
   * Отдельный метод, а не флаг у `compile`: этот путь может обращаться в сеть и
   * писать в vault, и разница обязана быть видна на границе, а не прятаться в
   * параметре. Заметка без подключений проходит здесь так же, как через
   * `compile`, и в сеть не заходит (FR-008).
   *
   * `overrides` — настройки уровня vault: включена ли автозагрузка и какие
   * адреса указаны для конкретных шаблонов. Решение «идти ли в сеть» принимает
   * ядро по этим значениям, а не интерфейс (принцип I).
   *
   * `undefined` в реализации означает, что этот клиент загрузку не
   * поддерживает; вызывающая сторона тогда пользуется `compile`.
   */
  compileWithTemplates?(
    notePath: string,
    text: string,
    revision: number,
    overrides: VaultOverrides,
    anchor?: ScrollAnchor,
  ): Promise<CompiledDocument>;

  /**
   * Установить шаблон по явному запросу пользователя (фича 007, FR-010).
   *
   * Отдельно от `compileWithTemplates`, потому что это другое действие: сборка
   * идёт по правилам настроек и может в сеть не пойти вовсе, а здесь
   * пользователь попросил прямо. Выключенная автозагрузка запрещает приложению
   * ходить в сеть **самому**, но не отнимает у человека возможность попросить.
   *
   * `undefined` — клиент этого не умеет; тогда кнопка не показывается.
   */
  installTemplate?(spec: string, overrides: VaultOverrides): Promise<InstallReport>;

  /**
   * Прекратить идущую сборку: заметку закрыли или перешли к другой (FR-011).
   * `undefined` — клиент этого не умеет; тогда сборка дорабатывает, а её
   * результат отбрасывается.
   */
  cancel?(): void;

  /**
   * Отметка состояния связанных файлов (FR-020): изменилась — пора
   * пересобрать. Что в неё входит, решает ядро; интерфейс только сравнивает.
   */
  dependencyMark?(paths: string[]): Promise<string>;

  /**
   * Установленные шаблоны: имя, версия, источник и отпечаток (фича 007, US5).
   *
   * Перечень читается из служебного места vault; сети не требует.
   */
  installedTemplates?(): Promise<InstalledTemplate[]>;

  /**
   * Экспортировать заметку в PDF (фича 009, contracts/frontend-export.md).
   *
   * `text` — текст редактора, если заметка открыта: в PDF попадает то, что
   * пользователь видит сейчас (FR-006). `null` — ядро читает заметку с диска.
   * Отказ — вариант `ExportReport.outcome`, а не исключение.
   */
  exportPdf(
    notePath: string,
    text: string | null,
    overrides: VaultOverrides,
  ): Promise<ExportReport>;

  /** Строки Центра экспорта: те же записи, что у дерева (FR-010, FR-011). */
  exportCandidates(overrides: VaultOverrides): Promise<ExportCandidate[]>;

  /**
   * Переходы пакета — чистые функции ядра (research.md §5). Значение пакета
   * держит UI, правила очереди считает ядро.
   */
  exportBatch: ExportBatchOps;
}

export interface ExportBatchOps {
  start(
    paths: string[],
    stopOnError: boolean,
    overrides: VaultOverrides,
  ): Promise<BatchState>;
  next(state: BatchState): Promise<BatchStep>;
  record(
    state: BatchState,
    report: ExportReport,
    elapsedMs: number,
  ): Promise<BatchState>;
  skip(state: BatchState, notePath: string): Promise<BatchState>;
  cancel(state: BatchState): Promise<BatchState>;
  view(state: BatchState): Promise<BatchView>;
}
