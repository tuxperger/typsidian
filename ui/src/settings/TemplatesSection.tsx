"use client";

import { useCallback, useEffect, useState } from "react";
import type { PreviewClient } from "../preview-client.js";
import type { InstalledTemplate } from "../wasm-core/typsidian_core.js";
import type { VaultSettingsState } from "./useVaultSettings.js";

/**
 * Раздел «Шаблоны» (фича 007; макет «Typsidian Шаблоны из сети», карточка 1d
 * «Настройки · Шаблоны» и список установленного из 1b).
 *
 * Один компонент на обе панели настроек — web (`SettingsPanel`) и папки
 * (`VaultSettingsPanel`, desktop и Android): возможность загрузки объявлена
 * одинаково на всех клиентах, различается только исход попытки (FR-016). Всё,
 * что здесь показывается, живёт внутри папки заметок и доступно всем трём
 * оболочкам через `VaultBackend`.
 *
 * Решений здесь нет (принцип I): годность адреса проверяет ядро при записи,
 * перечень установленного и отпечатки считает ядро. Раздел читает, пишет и
 * показывает.
 *
 * Чего из макета здесь нет намеренно: тумблера «Проверять отпечаток при
 * каждой сборке» и поля «Ограничение по времени». Сверка в ядре идёт всегда, а
 * предел ожидания — постоянная (`FETCH_TIMEOUT`); контрол, который ничего не
 * меняет, хуже его отсутствия. Кнопки «Загрузить сейчас» тоже нет: в
 * настройках ей нечего загружать, явная загрузка живёт у замечания в
 * предпросмотре (FR-010).
 */
export interface TemplatesSectionProps {
  vaultSettings: VaultSettingsState;
  /** `null` — сборка на этом клиенте ещё не готова: перечня установленного нет. */
  preview: PreviewClient | null;
  /**
   * Показывать ли здесь причину отказа записи. Панель, где её уже показывает
   * соседний раздел, выключает, чтобы одна причина не звучала дважды.
   */
  showError?: boolean;
}

export function TemplatesSection({
  vaultSettings,
  preview,
  showError = true,
}: TemplatesSectionProps): React.JSX.Element {
  const overrides = vaultSettings.overrides;
  // Пустой раздел без объяснения выглядел как отсутствие фичи: настройки
  // шаблонов принадлежат папке заметок, и без прочитанной папки их нет.
  if (overrides === null) {
    return vaultSettings.error !== null ? (
      <p role="alert">{vaultSettings.error}</p>
    ) : (
      <p role="status">
        Откройте папку с заметками — настройки внешних шаблонов хранятся в ней.
      </p>
    );
  }

  const autoDownload = overrides.auto_download_templates ?? true;

  return (
    <div className="ts-srows">
      {showError && vaultSettings.error !== null && (
        <p role="alert">{vaultSettings.error}</p>
      )}

      <label className="ts-srow">
        <span className="ts-srow__text">
          <span className="ts-srow__label">Загружать недостающие шаблоны</span>
          <span className="ts-srow__hint">
            Из официального реестра Typst и своих адресов. Заметка без
            подключений в сеть не ходит никогда. Шаблон — код, исполняемый при
            сборке: имя, версия и адрес каждой загрузки называются в
            предпросмотре.
          </span>
        </span>
        <input
          type="checkbox"
          role="switch"
          className="ts-switch"
          checked={autoDownload}
          onChange={(event) =>
            void vaultSettings.setAutoDownloadTemplates(event.target.checked)
          }
        />
      </label>

      <SourcesEditor
        sources={overrides.template_sources.map((entry) => ({
          spec: entry.spec,
          url: entry.url,
        }))}
        onChange={(next) => vaultSettings.setTemplateSources(next)}
      />

      <InstalledList preview={preview} />

      <div className="ts-srow ts-srow--last">
        <span className="ts-srow__text">
          <span className="ts-srow__label">При выключенной автозагрузке</span>
          <span className="ts-srow__hint">
            Недостающий шаблон показывается в предпросмотре с кнопкой «Загрузить
            шаблон» — выбор остаётся за пользователем.
          </span>
        </span>
      </div>
    </div>
  );
}

interface SourceRow {
  spec: string;
  url: string;
}

/** Адреса архивов для конкретных шаблонов (US4, FR-004). */
function SourcesEditor({
  sources,
  onChange,
}: {
  sources: SourceRow[];
  onChange: (next: SourceRow[]) => Promise<boolean>;
}): React.JSX.Element {
  const [spec, setSpec] = useState("");
  const [url, setUrl] = useState("");

  const add = async () => {
    const entry = { spec: spec.trim(), url: url.trim() };
    if (entry.spec === "" || entry.url === "") return;
    // Тот же шаблон с новым адресом заменяет прежнюю запись: у шаблона один
    // источник, и двух ответов на «откуда брать» быть не может.
    const saved = await onChange([
      ...sources.filter((known) => known.spec !== entry.spec),
      entry,
    ]);
    // Отклонённое ядром остаётся в полях: причину покажет раздел, а набранное
    // пользователь поправит, а не наберёт заново.
    if (!saved) return;
    setSpec("");
    setUrl("");
  };

  return (
    <>
      <div className="ts-srow ts-srow--flat">
        <span className="ts-srow__text">
          <span className="ts-srow__label">Свои адреса архивов</span>
          <span className="ts-srow__hint">
            Архив для конкретной версии шаблона берётся по указанному адресу, а
            не из реестра. Прочие пространства имён остаются локальными.
          </span>
        </span>
      </div>

      <div className="ts-sources">
        {sources.length > 0 && (
          <ul className="ts-sources__list" aria-label="Свои адреса шаблонов">
            {sources.map((entry) => (
              <li key={entry.spec} className="ts-sources__row">
                <span className="ts-sources__spec" title={entry.spec}>
                  {entry.spec}
                </span>
                <span className="ts-sources__url" title={entry.url}>
                  {entry.url}
                </span>
                <button
                  type="button"
                  className="ts-btn ts-sources__button"
                  aria-label={`Убрать адрес для ${entry.spec}`}
                  title="Убрать"
                  onClick={() =>
                    void onChange(sources.filter((known) => known.spec !== entry.spec))
                  }
                >
                  ✕
                </button>
              </li>
            ))}
          </ul>
        )}

        <form
          className="ts-sources__row"
          onSubmit={(event) => {
            event.preventDefault();
            void add();
          }}
        >
          <input
            type="text"
            className="ts-sources__spec"
            aria-label="Шаблон"
            placeholder="@preview/имя:1.0.0"
            value={spec}
            onChange={(event) => setSpec(event.target.value)}
          />
          <input
            type="url"
            className="ts-sources__url"
            aria-label="Адрес архива"
            placeholder="https://…"
            value={url}
            onChange={(event) => setUrl(event.target.value)}
          />
          <button
            type="submit"
            className="ts-btn ts-sources__button"
            aria-label="Добавить адрес"
            title="Добавить"
            disabled={spec.trim() === "" || url.trim() === ""}
          >
            +
          </button>
        </form>
      </div>
    </>
  );
}

/**
 * Отпечаток в строке — сокращённо, как в макете: `9f2c…00e1`. Нажатие
 * показывает его целиком: сверять два устройства глазами (FR-024) по
 * сокращению нельзя, а подсказка по наведению на Android не появится.
 */
function Fingerprint({ digest }: { digest: string }): React.JSX.Element {
  const [full, setFull] = useState(false);
  const short =
    digest.length > 12 ? `${digest.slice(0, 4)}…${digest.slice(-4)}` : digest;
  return (
    <button
      type="button"
      className="ts-tag ts-tag--button"
      title={full ? "Свернуть" : digest}
      aria-label={full ? `Отпечаток ${digest}` : "Показать отпечаток целиком"}
      onClick={() => setFull((value) => !value)}
    >
      {full ? digest : `отпечаток ${short}`}
    </button>
  );
}

/** Установленные шаблоны: имя, версия, источник, отпечаток (US5, FR-022). */
function InstalledList({
  preview,
}: {
  preview: PreviewClient | null;
}): React.JSX.Element | null {
  const [installed, setInstalled] = useState<InstalledTemplate[] | null>(null);
  const [failed, setFailed] = useState(false);

  const load = useCallback(async () => {
    if (!preview?.installedTemplates) return;
    try {
      setInstalled(await preview.installedTemplates());
      setFailed(false);
    } catch {
      setFailed(true);
    }
  }, [preview]);

  useEffect(() => {
    void load();
  }, [load]);

  if (!preview?.installedTemplates) return null;

  return (
    <>
      <div className="ts-srow ts-srow--flat">
        <span className="ts-srow__text">
          <span className="ts-srow__label">
            Установленные
            {installed !== null && installed.length > 0 && (
              <span className="ts-tag">{installed.length} установлено</span>
            )}
          </span>
          <span className="ts-srow__hint">
            Одинаковый отпечаток на двух устройствах значит одинаковое
            содержимое. Изменённый после установки шаблон предпросмотр назовёт,
            но не перезапишет.
          </span>
        </span>
        <button type="button" className="ts-btn" onClick={() => void load()}>
          Обновить
        </button>
      </div>

      <div className="ts-installed">
        {failed ? (
          <p role="alert">Перечень установленных шаблонов прочитать не удалось.</p>
        ) : installed === null ? (
          <p role="status" className="ts-srow__hint">Читается…</p>
        ) : installed.length === 0 ? (
          <p role="status" className="ts-srow__hint">
            Приложение пока не устанавливало ни одного шаблона.
          </p>
        ) : (
          <ul className="ts-installed__list" aria-label="Установленные шаблоны">
            {installed.map((entry) => (
              <li key={entry.spec} className="ts-installed__row">
                <span className="ts-installed__dot" aria-hidden="true" />
                <span className="ts-installed__spec">{entry.spec}</span>
                <span className="ts-installed__source" title={entry.url || undefined}>
                  {sourceLabel(entry.url)} · {entry.file_count} файлов
                </span>
                <span className="ts-spacer" />
                <Fingerprint digest={entry.fingerprint} />
              </li>
            ))}
          </ul>
        )}
      </div>
    </>
  );
}

/** «реестр» / «свой адрес» — как в строках списка макета 1b. */
function sourceLabel(url: string): string {
  if (url === "") return "источник неизвестен";
  return url.startsWith("https://packages.typst.org/") ? "реестр" : "свой адрес";
}
