"use client";

import { useEffect, useState } from "react";
import type { AiProvider, AiSettings } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { useI18n } from "../i18n/index.js";

// Готовность обучения с ИИ к новому уроку или практике (фичи 051, 053): включено ли оно,
// выбрана ли модель, есть ли ключ. Общая для диалогов «Новый урок» и «Новая практика».

export type Readiness =
  | { state: "checking" }
  | { state: "ready"; settings: AiSettings }
  | { state: "blocked"; reason: "disabled" | "noModel" | "noKey" | "noEndpoint"; provider: AiProvider };

export const PROVIDER_LABEL = {
  Anthropic: "ai.provider.anthropic",
  OpenAi: "ai.provider.openai",
  Custom: "ai.provider.custom",
} as const;

export function useAiReadiness(client: VaultClient, vaultId: string): Readiness {
  const [readiness, setReadiness] = useState<Readiness>({ state: "checking" });
  useEffect(() => {
    let alive = true;
    void (async () => {
      const settings = await client.ai.settings();
      const provider = settings.provider ?? "Anthropic";
      if (!settings.enabled) return { state: "blocked", reason: "disabled", provider } as const;
      if (provider === "Custom" && (settings.custom_endpoint ?? "") === "") {
        return { state: "blocked", reason: "noEndpoint", provider } as const;
      }
      if ((settings.model ?? "") === "") return { state: "blocked", reason: "noModel", provider } as const;
      if (provider !== "Custom" && !(await client.ai.keyState(vaultId, provider).catch(() => false))) {
        return { state: "blocked", reason: "noKey", provider } as const;
      }
      return { state: "ready", settings } as const;
    })()
      .then((next) => alive && setReadiness(next))
      .catch(() => alive && setReadiness({ state: "blocked", reason: "disabled", provider: "Anthropic" }));
    return () => {
      alive = false;
    };
  }, [client, vaultId]);
  return readiness;
}

/** Почему начать нельзя, и путь в настройки. */
export function BlockedNotice({
  blocked,
  onCancel,
  onOpenSettings,
}: {
  blocked: Extract<Readiness, { state: "blocked" }>;
  onCancel(): void;
  onOpenSettings(): void;
}): React.JSX.Element {
  const { t } = useI18n();
  return (
    <div className="ts-lesson__box ts-lesson__box--warn" role="alert">
      {blocked.reason === "disabled" && t("lessons.cannot.disabled")}
      {blocked.reason === "noKey" && t("lessons.cannot.noKey", { provider: t(PROVIDER_LABEL[blocked.provider]) })}
      {blocked.reason === "noModel" && t("lessons.cannot.noModel")}
      {blocked.reason === "noEndpoint" && t("lessons.cannot.noEndpoint")}
      <div className="ts-lesson__acts">
        <button type="button" className="ts-btn" onClick={onCancel}>
          {t("lessons.new.cancel")}
        </button>
        <button type="button" className="ts-btn ts-btn--accent" autoFocus onClick={onOpenSettings}>
          {t("lessons.cannot.openSettings")}
        </button>
      </div>
    </div>
  );
}

/**
 * Какие из путей закрыты для агента (проверка доступа 050). Проверяются только ещё не
 * проверенные пути: заметок в vault тысячи, поэтому вызывающий отдаёт лишь видимые.
 * Сбой проверки не закрывает путь: решение всё равно принимает ядро при создании.
 */
export function useClosedPaths(client: VaultClient, paths: readonly string[]): ReadonlySet<string> {
  const [closed, setClosed] = useState<ReadonlySet<string>>(() => new Set());
  const [asked] = useState(() => new Set<string>());
  const key = paths.join("\n");
  useEffect(() => {
    const fresh = key === "" ? [] : key.split("\n").filter((path) => !asked.has(path));
    if (fresh.length === 0) return;
    for (const path of fresh) asked.add(path);
    void Promise.all(
      fresh.map(async (path) => [path, await client.ai.checkAccess(path).catch(() => true)] as const),
    ).then((results) => {
      const shut = results.filter(([, open]) => !open).map(([path]) => path);
      if (shut.length > 0) setClosed((prev) => new Set([...prev, ...shut]));
    });
  }, [client, key, asked]);
  return closed;
}
