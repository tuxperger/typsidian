"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { AiFailure, AiProvider, AiSettings, ProbeResult } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { useI18n, type MessageKey } from "../i18n/index.js";
import { AiJournal } from "./AiJournal.js";

/**
 * Раздел «Обучение с ИИ» (фича 050, макет A–F, I). Все правила — в ядре (`ai`):
 * здесь только показ, поэтому незашифрованный адрес распознаётся по отказу ядра,
 * а не проверяется заново.
 */
export interface AiSectionProps {
  client: VaultClient | null;
  /** Id записи vault на устройстве; на сервере его подставляет сервер. */
  vaultId: string;
}

const PROVIDERS: { id: AiProvider; label: "ai.provider.anthropic" | "ai.provider.openai" | "ai.provider.custom" }[] = [
  { id: "Anthropic", label: "ai.provider.anthropic" },
  { id: "OpenAi", label: "ai.provider.openai" },
  { id: "Custom", label: "ai.provider.custom" },
];

/** Переключатели графа знаний (фича 054); `fallback` — значение, пока ядро поля не прислало. */
const KNOWLEDGE_SWITCHES = [
  {
    field: "knowledge_auto",
    label: "knowledge.settings.auto",
    hint: "knowledge.settings.autoHint",
    fallback: true,
  },
  {
    field: "knowledge_agent_reads",
    label: "knowledge.settings.agentReads",
    hint: "knowledge.settings.agentReadsHint",
    fallback: false,
  },
  {
    field: "knowledge_map",
    label: "knowledge.settings.map",
    hint: "knowledge.settings.mapHint",
    fallback: true,
  },
] as const satisfies readonly {
  field: "knowledge_auto" | "knowledge_agent_reads" | "knowledge_map";
  label: MessageKey;
  hint: MessageKey;
  fallback: boolean;
}[];

type Check =
  | { state: "idle" }
  | { state: "running"; requestId: string }
  | { state: "ok"; result: ProbeResult }
  | { state: "failed"; kind: AiFailure["kind"] | null };

function failureKind(error: unknown): AiFailure["kind"] | null {
  if (typeof error === "object" && error !== null && "kind" in error) {
    return (error as AiFailure).kind;
  }
  return null;
}

function newRequestId(): string {
  // `randomUUID` есть только в защищённом контексте, а сервер может идти по http.
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
}

/**
 * Ядро всегда отдаёт все поля, но `#[serde(default)]` делает их необязательными в
 * типах; здесь они заполнены, чтобы разметка не проверяла их на каждом шаге.
 */
type FilledSettings = AiSettings & { provider: AiProvider; denied_folders: string[] };

function filled(settings: AiSettings): FilledSettings {
  return {
    ...settings,
    provider: settings.provider ?? "Anthropic",
    denied_folders: settings.denied_folders ?? [],
  };
}

/** Изменение настроек: готовые поля или поля, вычисленные из текущих. */
type Patch = Partial<AiSettings> | ((current: FilledSettings) => Partial<AiSettings>);

export function AiSection({ client, vaultId }: AiSectionProps): React.JSX.Element {
  const { t } = useI18n();
  const [settings, setSettings] = useState<FilledSettings | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [keyPresent, setKeyPresent] = useState(false);
  const [keyDraft, setKeyDraft] = useState("");
  const [endpointDraft, setEndpointDraft] = useState("");
  const [folderDraft, setFolderDraft] = useState("");
  const [models, setModels] = useState<string[]>([]);
  const [check, setCheck] = useState<Check>({ state: "idle" });
  const [saveFailure, setSaveFailure] = useState<AiFailure["kind"] | null | undefined>(undefined);
  const [journalOpen, setJournalOpen] = useState(false);
  const running = useRef<string | null>(null);

  useEffect(() => {
    if (client === null) return;
    let cancelled = false;
    void client.ai.settings().then((loaded) => {
      if (cancelled) return;
      setSettings(filled(loaded));
      setEndpointDraft(loaded.custom_endpoint ?? "");
    });
    return () => {
      cancelled = true;
    };
  }, [client]);

  const provider = settings?.provider;
  useEffect(() => {
    if (client === null || provider === undefined) return;
    let cancelled = false;
    void client.ai
      .keyState(vaultId, provider)
      .then((present) => {
        if (!cancelled) setKeyPresent(present);
      })
      .catch(() => {
        if (!cancelled) setKeyPresent(false);
      });
    return () => {
      cancelled = true;
    };
  }, [client, vaultId, provider]);

  // Сохранения идут по очереди, и каждое накладывает своё изменение на то, что
  // ядро прочитает сейчас: иначе потеря фокуса двух полей подряд записала бы
  // второе поверх настроек без первого. После записи показывается прочитанное
  // ядром, а не черновик: адрес и модель могут уточниться при нормализации.
  const saving = useRef<Promise<unknown>>(Promise.resolve());
  const save = useCallback(
    (patch: Patch): Promise<boolean> => {
      if (client === null) return Promise.resolve(false);
      const run = saving.current.then(async () => {
        try {
          const current = filled(await client.ai.settings());
          await client.ai.setSettings({ ...current, ...(typeof patch === "function" ? patch(current) : patch) });
          setSettings(filled(await client.ai.settings()));
          setSaveFailure(undefined);
          return true;
        } catch (error) {
          setSaveFailure(failureKind(error));
          return false;
        }
      });
      saving.current = run;
      return run;
    },
    [client],
  );

  if (client === null || settings === null) {
    return <p role="status">{t("settings.vaultNotOpen")}</p>;
  }

  const failureText = (kind: AiFailure["kind"] | null): string =>
    kind === null ? t("ai.failure.Unknown") : t(`ai.failure.${kind}`);

  if (!settings.enabled) {
    return (
      <div className="ts-srows">
        <label className="ts-srow ts-srow--last">
          <span className="ts-srow__text">
            <span className="ts-srow__label">{t("ai.enable")}</span>
            <span className="ts-srow__hint">{t("ai.enableHintOff")}</span>
          </span>
          <input
            type="checkbox"
            className="ts-switch"
            role="switch"
            checked={confirming}
            onChange={(event) => setConfirming(event.target.checked)}
          />
        </label>
        {confirming && (
          <div className="ts-replace__panel ts-replace__panel--warn" role="alertdialog">
            <span className="ts-replace__title">{t("ai.confirm.title", { name: client.rootName })}</span>
            <span className="ts-replace__meta">{t("ai.confirm.intro")}</span>
            <ul className="ts-ai__list">
              <li>{t("ai.confirm.what")}</li>
              <li>{t("ai.confirm.notBackground")}</li>
              <li>{t("ai.confirm.journal")}</li>
              <li>{t("ai.confirm.folders")}</li>
            </ul>
            <div className="ts-replace__row">
              <button
                type="button"
                className="ts-btn ts-btn--accent"
                onClick={() => {
                  setConfirming(false);
                  void save({ enabled: true });
                }}
              >
                {t("ai.confirm.enable")}
              </button>
              <button type="button" className="ts-btn ts-btn--quiet" onClick={() => setConfirming(false)}>
                {t("ai.cancel")}
              </button>
            </div>
          </div>
        )}
        {saveFailure !== undefined && <p role="alert">{failureText(saveFailure)}</p>}
      </div>
    );
  }

  const runProbe = async (): Promise<void> => {
    const requestId = newRequestId();
    running.current = requestId;
    setCheck({ state: "running", requestId });
    // Кнопку жмут прямо из поля: сначала должна записаться потеря его фокуса.
    await saving.current;
    if (running.current !== requestId) return;
    try {
      const result = await client.ai.probe(vaultId, requestId);
      if (running.current === requestId) setCheck({ state: "ok", result });
      try {
        setModels(await client.ai.models(vaultId, newRequestId()));
      } catch {
        // Сервер не отдаёт список: имя модели вводится вручную (AS 1.4).
        setModels([]);
      }
    } catch (error) {
      if (running.current === requestId) setCheck({ state: "failed", kind: failureKind(error) });
    }
  };

  const loadModels = async (): Promise<void> => {
    const requestId = newRequestId();
    running.current = requestId;
    setCheck({ state: "running", requestId });
    await saving.current;
    if (running.current !== requestId) return;
    try {
      const listed = await client.ai.models(vaultId, requestId);
      setModels(listed);
      if (running.current === requestId) setCheck({ state: "idle" });
    } catch (error) {
      if (running.current === requestId) setCheck({ state: "failed", kind: failureKind(error) });
    }
  };

  const cancelCheck = (): void => {
    if (check.state !== "running") return;
    client.ai.cancel(vaultId, check.requestId);
    running.current = null;
    setCheck({ state: "idle" });
  };

  const storeKey = async (): Promise<void> => {
    try {
      await client.ai.setKey(vaultId, settings.provider, keyDraft);
      setKeyDraft("");
      setKeyPresent(true);
      setSaveFailure(undefined);
    } catch (error) {
      setSaveFailure(failureKind(error));
    }
  };

  const deleteKey = async (): Promise<void> => {
    try {
      await client.ai.deleteKey(vaultId, settings.provider);
      setKeyPresent(false);
    } catch (error) {
      setSaveFailure(failureKind(error));
    }
  };

  /**
   * Одобрение адреса на устройстве. Незашифрованный адрес ядро без подтверждения
   * не одобряет — тогда показывается предупреждение о нём.
   */
  const approve = async (insecureOk: boolean): Promise<void> => {
    await saving.current;
    try {
      await client.ai.approveEndpoint(vaultId, insecureOk);
      setCheck({ state: "idle" });
    } catch (error) {
      const kind = failureKind(error);
      if (kind === "InsecureNotConfirmed") setCheck({ state: "failed", kind });
      else setSaveFailure(kind);
    }
  };

  const insecure = check.state === "failed" && check.kind === "InsecureNotConfirmed";
  const unapproved = check.state === "failed" && check.kind === "EndpointNotApproved";

  return (
    <div className="ts-srows">
      <label className="ts-srow">
        <span className="ts-srow__text">
          <span className="ts-srow__label">{t("ai.enable")}</span>
        </span>
        <input
          type="checkbox"
          className="ts-switch"
          role="switch"
          checked
          onChange={() => void save({ enabled: false })}
        />
      </label>

      <div className="ts-srow">
        <span className="ts-srow__text">
          <span className="ts-srow__label">{t("ai.connection")}</span>
        </span>
        <span className="ts-ai__seg" role="group" aria-label={t("ai.connection")}>
          {PROVIDERS.map((item) => (
            <button
              key={item.id}
              type="button"
              className="ts-btn"
              aria-pressed={settings.provider === item.id}
              onClick={() => {
                setModels([]);
                setCheck({ state: "idle" });
                void save({ provider: item.id, model: undefined });
              }}
            >
              {t(item.label)}
            </button>
          ))}
        </span>
      </div>

      {settings.provider === "Custom" && (
        <label className="ts-srow">
          <span className="ts-srow__text">
            <span className="ts-srow__label">{t("ai.endpoint")}</span>
          </span>
          <input
            className="ts-field__input ts-ai__input"
            value={endpointDraft}
            placeholder="http://127.0.0.1:11434/v1"
            onChange={(event) => setEndpointDraft(event.target.value)}
            onBlur={(event) => {
              // Значение поля, а не черновик: потеря фокуса может прийти раньше
              // отрисовки с последним вводом.
              const endpoint = event.target.value.trim();
              if (endpoint === (settings.custom_endpoint ?? "")) return;
              // Адрес, введённый здесь, одобряется на этом устройстве; пришедший с
              // синхронизацией — только по «Доверять адресу» (ревью 050, M1).
              void save({ custom_endpoint: endpoint === "" ? undefined : endpoint }).then((saved) => {
                if (saved && endpoint !== "") void approve(false);
              });
            }}
          />
        </label>
      )}

      <div className="ts-srow">
        <span className="ts-srow__text">
          <span className="ts-srow__label">{t("ai.key")}</span>
          <span className="ts-srow__hint">
            {settings.provider === "Custom" ? t("ai.keyOptional") : t("ai.keyHint")}
          </span>
        </span>
        {keyPresent ? (
          <>
            <span className="ts-srow__hint">{t("ai.keySaved")}</span>
            <button type="button" className="ts-btn" onClick={() => setKeyPresent(false)}>
              {t("ai.keyReplace")}
            </button>
            <button type="button" className="ts-btn" onClick={() => void deleteKey()}>
              {t("ai.keyDelete")}
            </button>
          </>
        ) : (
          <>
            <input
              className="ts-field__input ts-ai__input"
              type="password"
              autoComplete="off"
              aria-label={t("ai.key")}
              value={keyDraft}
              onChange={(event) => setKeyDraft(event.target.value)}
            />
            <button type="button" className="ts-btn" disabled={keyDraft === ""} onClick={() => void storeKey()}>
              {t("ai.keySave")}
            </button>
          </>
        )}
      </div>

      <label className="ts-srow">
        <span className="ts-srow__text">
          <span className="ts-srow__label">{t("ai.model")}</span>
          {models.length === 0 && <span className="ts-srow__hint">{t("ai.modelHint")}</span>}
        </span>
        <input
          key={`${settings.provider}:${settings.model ?? ""}`}
          className="ts-field__input ts-ai__input"
          list="ts-ai-models"
          defaultValue={settings.model ?? ""}
          onBlur={(event) => {
            const model = event.target.value.trim();
            if (model !== (settings.model ?? "")) {
              void save({ model: model === "" ? undefined : model });
            }
          }}
        />
        <datalist id="ts-ai-models">
          {models.map((model) => (
            <option key={model} value={model} />
          ))}
        </datalist>
      </label>

      <label className="ts-srow">
        <span className="ts-srow__text">
          <span className="ts-srow__label">{t("ai.lessonsFolder")}</span>
          <span className="ts-srow__hint">{t("ai.lessonsFolderHint")}</span>
        </span>
        <input
          key={settings.lessons_folder ?? ""}
          className="ts-field__input ts-ai__input"
          defaultValue={settings.lessons_folder ?? ""}
          onBlur={(event) => {
            // Пустая папка — не значение: урок негде создать, остаётся прежняя.
            const folder = event.target.value.trim().replace(/^\/+|\/+$/g, "");
            if (folder === "") {
              event.target.value = settings.lessons_folder ?? "";
              return;
            }
            if (folder !== (settings.lessons_folder ?? "")) void save({ lessons_folder: folder });
          }}
        />
      </label>

      {saveFailure !== undefined && <p role="alert">{failureText(saveFailure)}</p>}

      {check.state === "running" && (
        <div className="ts-srow" role="status">
          <span className="ts-srow__hint">{t("ai.checking")}</span>
          <button type="button" className="ts-btn" onClick={cancelCheck}>
            {t("ai.cancel")}
          </button>
        </div>
      )}
      {check.state === "ok" && (
        <p className="ts-ai__ok" role="status">
          {t("ai.checkOk", { tokens: (check.result.reply.input_tokens ?? 0) + (check.result.reply.output_tokens ?? 0) })}{" "}
          {check.result.cost_usd != null
            ? t("ai.costAbout", { cost: check.result.cost_usd.toFixed(4) })
            : t("ai.costUnknown")}
        </p>
      )}
      {check.state === "failed" && !insecure && !unapproved && (
        <p className="ts-ai__error" role="alert">
          {failureText(check.kind)}
        </p>
      )}
      {insecure && (
        <div className="ts-replace__panel ts-replace__panel--warn" role="alertdialog">
          <span className="ts-replace__title">{t("ai.insecure.title")}</span>
          <span className="ts-replace__meta">{t("ai.insecure.hint")}</span>
          <div className="ts-replace__row">
            <button
              type="button"
              className="ts-btn ts-btn--danger"
              onClick={() => {
                setCheck({ state: "idle" });
                void save((current) => ({ insecure_confirmed_for: current.custom_endpoint })).then((saved) => {
                  if (saved) void approve(true);
                });
              }}
            >
              {t("ai.insecure.connect")}
            </button>
            <button type="button" className="ts-btn ts-btn--quiet" onClick={() => setCheck({ state: "idle" })}>
              {t("ai.cancel")}
            </button>
          </div>
        </div>
      )}
      {unapproved && (
        <div className="ts-replace__panel ts-replace__panel--warn" role="alertdialog">
          <span className="ts-replace__title">{t("ai.approve.title")}</span>
          <span className="ts-replace__meta">
            {t("ai.approve.hint", { endpoint: settings.custom_endpoint ?? "" })}
          </span>
          <div className="ts-replace__row">
            <button type="button" className="ts-btn ts-btn--danger" onClick={() => void approve(false)}>
              {t("ai.approve.trust")}
            </button>
            <button type="button" className="ts-btn ts-btn--quiet" onClick={() => setCheck({ state: "idle" })}>
              {t("ai.cancel")}
            </button>
          </div>
        </div>
      )}

      <div className="ts-replace__row">
        <button type="button" className="ts-btn" onClick={() => setJournalOpen((open) => !open)}>
          {t("ai.journal.open")}
        </button>
        <button type="button" className="ts-btn" disabled={check.state === "running"} onClick={() => void loadModels()}>
          {t("ai.loadModels")}
        </button>
        <button
          type="button"
          className="ts-btn ts-btn--accent"
          disabled={check.state === "running"}
          onClick={() => void runProbe()}
        >
          {t("ai.check")}
        </button>
      </div>

      <div className="ts-srow ts-srow--flat">
        <span className="ts-srow__text">
          <span className="ts-srow__label">{t("ai.folders")}</span>
          <span className="ts-srow__hint">{t("ai.foldersHint")}</span>
        </span>
        <input
          className="ts-field__input ts-ai__input"
          aria-label={t("ai.folders")}
          placeholder={t("ai.folderPlaceholder")}
          value={folderDraft}
          onChange={(event) => setFolderDraft(event.target.value)}
        />
        <button
          type="button"
          className="ts-btn"
          disabled={folderDraft.trim() === ""}
          onClick={() => {
            const folder = folderDraft.trim();
            void save((current) => ({ denied_folders: [...current.denied_folders, folder] }));
            setFolderDraft("");
          }}
        >
          {t("ai.folderAdd")}
        </button>
      </div>
      <div className="ts-ai__tags">
        {settings.denied_folders.map((folder) => (
          <button
            key={folder}
            type="button"
            className="ts-btn ts-btn--quiet"
            aria-label={t("ai.folderRemove", { folder })}
            onClick={() =>
              void save((current) => ({
                denied_folders: current.denied_folders.filter((item) => item !== folder),
              }))
            }
          >
            {folder}/ ✕
          </button>
        ))}
      </div>

      {/* Граф знаний (фича 054, макет D): три переключателя; значения по умолчанию задаёт ядро. */}
      <div className="ts-eyebrow">{t("knowledge.settings.title")}</div>
      {KNOWLEDGE_SWITCHES.map((item) => (
        <label key={item.field} className="ts-srow">
          <span className="ts-srow__text">
            <span className="ts-srow__label">{t(item.label)}</span>
            <span className="ts-srow__hint">{t(item.hint)}</span>
          </span>
          <input
            type="checkbox"
            className="ts-switch"
            role="switch"
            checked={settings[item.field] ?? item.fallback}
            onChange={(event) => void save({ [item.field]: event.target.checked })}
          />
        </label>
      ))}

      {/* Исследования в уроке (фича 056, макет E): поиск в вебе, очередь, PDF по инициативе учителя. */}
      <div className="ts-eyebrow">{t("research.settings.title")}</div>
      <label className="ts-srow">
        <span className="ts-srow__text">
          <span className="ts-srow__label">{t("research.settings.web")}</span>
          <span className="ts-srow__hint">{t("research.settings.webHint")}</span>
        </span>
        <input
          type="checkbox"
          className="ts-switch"
          role="switch"
          checked={settings.web_research ?? false}
          onChange={(event) => void save({ web_research: event.target.checked })}
        />
      </label>
      <label className="ts-srow">
        <span className="ts-srow__text">
          <span className="ts-srow__label">{t("research.settings.parallel")}</span>
          <span className="ts-srow__hint">{t("research.settings.parallelHint")}</span>
        </span>
        <input
          type="number"
          className="ts-field__input ts-ai__number"
          min={1}
          max={4}
          step={1}
          value={settings.research_parallel ?? 2}
          onChange={(event) => {
            const value = Math.round(Number(event.target.value));
            if (Number.isFinite(value) && value >= 1 && value <= 4) void save({ research_parallel: value });
          }}
        />
      </label>
      <label className="ts-srow">
        <span className="ts-srow__text">
          <span className="ts-srow__label">{t("research.settings.agentPdf")}</span>
          <span className="ts-srow__hint">{t("research.settings.agentPdfHint")}</span>
        </span>
        <input
          type="checkbox"
          className="ts-switch"
          role="switch"
          checked={settings.research_agent_pdf ?? false}
          onChange={(event) => void save({ research_agent_pdf: event.target.checked })}
        />
      </label>

      {journalOpen && <AiJournal client={client} />}
    </div>
  );
}
