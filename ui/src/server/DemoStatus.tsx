"use client";

import { useEffect, useState } from "react";
import { useI18n } from "../i18n/index.js";
import {
  DEMO_QUOTA_EVENT,
  demoStart,
  demoUsage,
  type DemoUsage,
  type ServerDemo,
} from "./server-api.js";

// Гость демо-сервера (фича 039, макет D2–D5): остаток времени и объёма в шапке,
// полоса за пять минут до конца, окно окончания и отказ по квоте. Срок и квоту
// считает сервер; здесь — только показ относительно часов браузера.

const WARN_SECONDS = 5 * 60;
const USAGE_EVERY_MS = 20_000;

function useNow(fast: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), fast ? 1000 : 15_000);
    return () => clearInterval(timer);
  }, [fast]);
  return now;
}

/** Секунд до конца демо. */
function useSecondsLeft(expiresAt: number): number {
  const [fast, setFast] = useState(false);
  const now = useNow(fast);
  const left = Math.max(0, Math.floor(expiresAt - now / 1000));
  useEffect(() => setFast(left <= WARN_SECONDS + 60), [left]);
  return left;
}

function clock(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  return `${minutes}:${String(seconds % 60).padStart(2, "0")}`;
}

function megabytes(bytes: number): string {
  return (bytes / (1024 * 1024)).toFixed(1);
}

/** Пилюля «демо · осталось N мин» в шапке; подсказка — демо-объём. */
export function DemoPill({ expiresAt }: { expiresAt: number }): React.JSX.Element {
  const { t } = useI18n();
  const left = useSecondsLeft(expiresAt);
  const [usage, setUsage] = useState<DemoUsage | null>(null);

  useEffect(() => {
    let alive = true;
    const load = async (): Promise<void> => {
      const result = await demoUsage();
      if (alive && result.ok) setUsage(result.value);
    };
    void load();
    const timer = setInterval(() => void load(), USAGE_EVERY_MS);
    const onQuota = (): void => void load();
    window.addEventListener(DEMO_QUOTA_EVENT, onQuota);
    return () => {
      alive = false;
      clearInterval(timer);
      window.removeEventListener(DEMO_QUOTA_EVENT, onQuota);
    };
  }, []);

  const full = usage !== null && usage.used >= usage.quota;
  const label =
    left === 0
      ? t("server.demo.over")
      : left <= WARN_SECONDS
        ? t("server.demo.leftClock", { clock: clock(left) })
        : t("server.demo.left", { minutes: Math.ceil(left / 60) });
  const usageText =
    usage === null
      ? undefined
      : t("server.demo.usage", {
          used: megabytes(usage.used),
          quota: megabytes(usage.quota).replace(/\.0$/, ""),
        });
  const className = [
    "ts-demo-pill",
    left <= WARN_SECONDS ? "ts-demo-pill--late" : "",
    full ? "ts-demo-pill--full" : "",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <span className={className} role="status" title={usageText}>
      <span className="ts-demo-pill__dot" aria-hidden="true" />
      <span className="ts-demo-pill__time">{label}</span>
      {usage !== null && (
        <span className="ts-demo-pill__meter" aria-label={usageText}>
          <i style={{ width: `${Math.min(100, (usage.used / usage.quota) * 100)}%` }} />
        </span>
      )}
    </span>
  );
}

/**
 * Полоса за пять минут до конца (D3), отказ по квоте (D5) и окно окончания (D4).
 * `ended` — сервер уже не принимает сессию гостя.
 */
export function DemoOverlays({
  demo,
  expiresAt,
  ended,
}: {
  demo: ServerDemo;
  expiresAt: number;
  ended: boolean;
}): React.JSX.Element | null {
  const { t } = useI18n();
  const left = useSecondsLeft(expiresAt);
  const [warned, setWarned] = useState(false);
  const [quota, setQuota] = useState<number | null>(null);
  const [restarting, setRestarting] = useState(false);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const onQuota = (event: Event): void => {
      const detail = (event as CustomEvent<{ quota_mb?: number }>).detail;
      setQuota(detail.quota_mb ?? demo.quotaMb);
      clearTimeout(timer);
      timer = setTimeout(() => setQuota(null), 8000);
    };
    window.addEventListener(DEMO_QUOTA_EVENT, onQuota);
    return () => {
      clearTimeout(timer);
      window.removeEventListener(DEMO_QUOTA_EVENT, onQuota);
    };
  }, [demo.quotaMb]);

  if (ended || left === 0) {
    const again = async (): Promise<void> => {
      setRestarting(true);
      const result = await demoStart();
      // Новый гость — новая сессия: страница открывает его vault с начала.
      if (result.ok) location.reload();
      else setRestarting(false);
    };
    return (
      <div className="ts-server-overlay" role="dialog" aria-modal="true">
        <div className="ts-server-card">
          <h1 className="ts-server-card__brand">{t("server.demo.endedTitle")}</h1>
          <p className="ts-server-card__lead">
            {t("server.demo.ended", { minutes: demo.ttlMinutes })}
          </p>
          <button
            type="button"
            className="ts-btn ts-btn--accent ts-server-card__wide"
            disabled={restarting}
            onClick={() => void again()}
          >
            {restarting ? t("server.demo.starting") : t("server.demo.again")}
          </button>
        </div>
      </div>
    );
  }

  return (
    <>
      {left <= WARN_SECONDS && !warned && (
        <div className="ts-demo-strip" role="status">
          <span>{t("server.demo.soon", { clock: clock(left) })}</span>
          <button type="button" className="ts-btn" onClick={() => setWarned(true)}>
            {t("server.demo.soonOk")}
          </button>
        </div>
      )}
      {quota !== null && (
        <div className="ts-demo-toast" role="alert">
          {t("server.demo.full", { quota })}
        </div>
      )}
    </>
  );
}
