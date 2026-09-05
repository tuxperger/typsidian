import { useState } from "react";
import type {
  Diagnostics,
  OutageReason,
  StoreCapabilities,
} from "../wasm-core/typsidian_core.js";

export interface AdvancedSectionProps {
  capabilities: StoreCapabilities | null;
  /** Диагностика из ядра (FR-039); `null`, пока хранилище не открыто. */
  diagnostics: Diagnostics | null;
  onReveal: () => void;
  onResetToDefaults: () => void;
  /** Итог последнего сброса словами — отказ объясняется, а не замалчивается. */
  resetMessage?: string | null;
}

/**
 * Раздел «Дополнительно» (T041, T095, FR-036, FR-037, FR-039).
 *
 * Расположение хранилища показывается всегда, а действие раскрытия — **только**
 * при `can_reveal`. Ветвление идёт по значению, пришедшему из ядра, а не по
 * таргету: в вебе раскрывать нечего, и там же, где на native кнопка, стоит
 * пояснение, почему её нет (research.md §13).
 *
 * Сброс к умолчаниям выполняется в два шага: FR-037 требует явного
 * подтверждения, и оно сделано отдельным состоянием, а не `window.confirm` —
 * диалог браузера невозможно проверить сценарием и он выглядит чужеродно.
 */
export function AdvancedSection({
  capabilities,
  diagnostics,
  onReveal,
  onResetToDefaults,
  resetMessage = null,
}: AdvancedSectionProps): React.JSX.Element {
  const [isConfirmingReset, setIsConfirmingReset] = useState(false);

  return (
    <section aria-label="Дополнительно">
      <h2>Дополнительно</h2>

      {capabilities && (
        <p>
          Хранилище настроек: <code>{capabilities.location}</code>{" "}
          {capabilities.can_reveal ? (
            <button type="button" onClick={onReveal}>
              Показать в файловом менеджере
            </button>
          ) : (
            <span role="note">
              Открыть это хранилище средствами системы нельзя — оно доступно
              только самому приложению.
            </span>
          )}
        </p>
      )}

      {capabilities && !capabilities.writable && (
        <p role="status">
          Хранилище сейчас недоступно на запись: изменения действуют до конца
          сессии и не переживут перезапуск.
        </p>
      )}

      <h3>Сброс к умолчаниям</h3>
      {isConfirmingReset ? (
        <div role="group" aria-label="Подтверждение сброса">
          <p role="alert">
            Значения вернутся к умолчаниям. Список известных vault и файлы в них
            не изменятся.
          </p>
          <button
            type="button"
            onClick={() => {
              setIsConfirmingReset(false);
              onResetToDefaults();
            }}
          >
            Сбросить
          </button>
          <button type="button" onClick={() => setIsConfirmingReset(false)}>
            Отмена
          </button>
        </div>
      ) : (
        <button type="button" onClick={() => setIsConfirmingReset(true)}>
          Сбросить к умолчаниям
        </button>
      )}
      {resetMessage && <p role="status">{resetMessage}</p>}

      <h3>Диагностика</h3>
      {diagnostics ? (
        <DiagnosticsView diagnostics={diagnostics} />
      ) : (
        <p role="status">Хранилище ещё не открыто.</p>
      )}
    </section>
  );
}

/**
 * FR-039 и FR-042: показывается ровно то, что собрало ядро, и никуда не
 * отправляется. Кнопки «отправить в поддержку» здесь нет и быть не может —
 * передать сведения может только пользователь, скопировав их руками.
 */
function DiagnosticsView({
  diagnostics,
}: {
  diagnostics: Diagnostics;
}): React.JSX.Element {
  const { app_version, editor_mode, editor_mode_availability, environment, storage } =
    diagnostics;

  return (
    <dl>
      <dt>Версия</dt>
      <dd>{app_version}</dd>

      <dt>Режим редактирования</dt>
      <dd>{describeMode(editor_mode)}</dd>

      <dt>Полноценный режим</dt>
      <dd>
        {editor_mode_availability.kind === "Available"
          ? "доступен"
          : `недоступен — ${describeOutage(editor_mode_availability.reason)}`}
      </dd>

      <dt>Окружение</dt>
      <dd>
        {environment.target} · {environment.os_family} ·{" "}
        {environment.can_spawn_processes
          ? "внешние процессы доступны"
          : "внешние процессы недоступны"}
      </dd>

      <dt>Хранилище</dt>
      <dd>
        {storage.capabilities.location} ·{" "}
        {storage.capabilities.writable ? "доступно на запись" : "только чтение"}
      </dd>
    </dl>
  );
}

function describeMode(mode: Diagnostics["editor_mode"]): string {
  switch (mode.state) {
    case "FullBackend":
      return "полноценный";
    case "SimplifiedEmulation":
      return "упрощённая эмуляция";
    case "PlainInput":
      return `обычный ввод — ${describeOutage(mode.reason)}`;
    default: {
      const exhaustive: never = mode;
      return exhaustive;
    }
  }
}

/** Причина отказа называется словами (FR-034), а не кодом варианта. */
function describeOutage(reason: OutageReason): string {
  switch (reason.kind) {
    case "ExternalProcessesUnavailable":
      return "платформа не запускает внешние процессы";
    case "BackendNotInstalled":
      return "подсистема не установлена на этом устройстве";
    case "BackendStopped":
      return "процесс перестал отвечать";
    default: {
      const exhaustive: never = reason;
      return exhaustive;
    }
  }
}
