import type {
  ConfigDiskState,
  EditorMode,
  OutageReason,
} from "../wasm-core/typsidian_core.js";

export interface KeybindingsSectionProps {
  mode: EditorMode | null;
  configDiskState: ConfigDiskState | null;
  /** Путь к указанному конфигу, если он выбран (FR-026). */
  configPath: string | null;
  /** Доступен ли полноценный режим на этом устройстве (FR-034). */
  fullBackendUnavailableReason: OutageReason | null;
  onChooseFullBackend: () => void;
  onChooseSimplifiedEmulation: () => void;
  onPickConfig: () => void;
  onApplyConfig: () => void;
  onLeavePlainInput: () => void;
}

/**
 * Раздел «Кейбиндинги» (T082, FR-025…FR-035).
 *
 * `EditorMode` — трёхвариантный размеченный union, а не булев флаг: пропуск
 * `PlainInput` в разборе обязан быть ошибкой компиляции (FR-025, принцип IV),
 * потому что именно это состояние легче всего забыть — оно наступает само.
 *
 * Полноценный режим предлагается только когда он доступен. Если недоступен —
 * на месте кнопки стоит причина: упрощённая эмуляция не выдаётся за
 * полноценный режим (FR-034).
 */
export function KeybindingsSection({
  mode,
  configDiskState,
  configPath,
  fullBackendUnavailableReason,
  onChooseFullBackend,
  onChooseSimplifiedEmulation,
  onPickConfig,
  onApplyConfig,
  onLeavePlainInput,
}: KeybindingsSectionProps): React.JSX.Element {
  return (
    <section aria-label="Кейбиндинги">
      <h2>Кейбиндинги</h2>

      <p role="status">{describeMode(mode)}</p>

      {/* FR-035: переход в обычный ввод — заметное сообщение с причиной и
          действием возврата, а не тихая смена поведения редактора. */}
      {mode?.state === "PlainInput" && (
        <div role="alert">
          <p>{describeOutage(mode.reason)}</p>
          <button type="button" onClick={onLeavePlainInput}>
            Вернуться к режиму редактирования
          </button>
        </div>
      )}

      <div role="group" aria-label="Режим редактирования">
        {fullBackendUnavailableReason ? (
          <p role="note">
            Полноценный режим недоступен: {describeOutage(fullBackendUnavailableReason)}
          </p>
        ) : (
          <button type="button" onClick={onChooseFullBackend}>
            Полноценный режим
          </button>
        )}
        <button type="button" onClick={onChooseSimplifiedEmulation}>
          Упрощённая эмуляция
        </button>
      </div>

      <div role="group" aria-label="Конфиг">
        <p>
          Конфиг:{" "}
          {configPath ? <code>{configPath}</code> : <em>не указан</em>}{" "}
          <button type="button" onClick={onPickConfig}>
            {configPath ? "Выбрать другой" : "Указать конфиг"}
          </button>
        </p>
        {configDiskState && (
          <ConfigStatus
            diskState={configDiskState}
            hasPath={configPath !== null}
            onApplyConfig={onApplyConfig}
          />
        )}
      </div>
    </section>
  );
}

/**
 * Признак расхождения конфига с применённым (FR-026). Применение — только по
 * явному действию: автоматически применять нельзя, поэтому «Применить» — это
 * кнопка, а не побочный эффект отрисовки.
 */
function ConfigStatus({
  diskState,
  hasPath,
  onApplyConfig,
}: {
  diskState: ConfigDiskState;
  hasPath: boolean;
  onApplyConfig: () => void;
}): React.JSX.Element | null {
  switch (diskState) {
    case "inSync":
      return <span role="note">Применён.</span>;
    case "changed":
      return (
        <>
          <span role="note">Изменён, не применён.</span>
          <button type="button" onClick={onApplyConfig}>
            Применить
          </button>
        </>
      );
    case "missing":
      if (!hasPath) return null;
      return (
        <span role="note">
          Файл не найден. Ранее применённый конфиг продолжает действовать.
        </span>
      );
    case "unreadable":
      return (
        <span role="note">
          Файл не читается. Ранее применённый конфиг продолжает действовать.
        </span>
      );
    default: {
      const exhaustive: never = diskState;
      return exhaustive;
    }
  }
}

function describeMode(mode: EditorMode | null): string {
  if (!mode) return "Состояние режима редактирования ещё не определено.";
  switch (mode.state) {
    case "FullBackend":
      return "Активен полноценный режим редактирования.";
    case "SimplifiedEmulation":
      return "Активна упрощённая эмуляция.";
    case "PlainInput":
      // Формулировка намеренно не смешивается с эмуляцией: отсутствие
      // vim-раскладки должно быть однозначно отличимо от штатного режима.
      return "Обычный текстовый ввод: полноценный режим отказал, vim-раскладка сейчас не действует.";
    default: {
      const exhaustive: never = mode;
      return exhaustive;
    }
  }
}

function describeOutage(reason: OutageReason): string {
  switch (reason.kind) {
    case "ExternalProcessesUnavailable":
      return "эта платформа не может запускать внешние процессы.";
    case "BackendNotInstalled":
      return "подсистема не найдена на этом устройстве.";
    case "BackendStopped":
      return "внешний процесс завершился или перестал отвечать.";
    default: {
      const exhaustive: never = reason;
      return exhaustive;
    }
  }
}
