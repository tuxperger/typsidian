// HTTP API сервера Typsidian (фича 037, contracts/server-api.md): вход, аккаунт,
// сессии, свои vault. Тот же origin, что у страницы: cookie сессии `HttpOnly`, и
// скрипт её не видит; изменяющие запросы несут CSRF-токен из `GET /api/me`.
//
// Отказ сервера — значение `{ kind: … }`, как у ядра; сетевой сбой — `Offline`.

import { withoutNullFields } from "../tauri-invoke.js";

/** Сведения для экрана входа. */
export interface ServerInfo {
  name: string;
  password: boolean;
  /** Подпись кнопки провайдера входа; нет — провайдер не настроен. */
  oidc?: string;
  /** Демо-сервер (фича 039): входа нет, только гости. */
  demo?: ServerDemo;
}

/** Условия демо-сервера. */
export interface ServerDemo {
  ttlMinutes: number;
  quotaMb: number;
}

/** Событие окна: транспорт получил отказ по демо-квоте (фича 039). */
export const DEMO_QUOTA_EVENT = "typsidian-demo-quota";

/** Объём vault гостя, байты. */
export interface DemoUsage {
  used: number;
  quota: number;
}

/** Текущий аккаунт и CSRF-токен его сессии. */
export interface ServerSession {
  login: string;
  displayName: string;
  hasPassword: boolean;
  csrf: string;
  /** Гость демо-сервера: когда vault будет удалён (секунды Unix). */
  expiresAt?: number;
}

export interface ServerVault {
  name: string;
  notes: number;
  /** Секунды Unix. */
  modified?: number;
}

export interface ServerSessionEntry {
  id: string;
  createdAt: number;
  lastSeen: number;
  userAgent: string;
  ip: string;
  current: boolean;
}

export type ServerFailure =
  | { kind: "Unauthenticated" }
  | { kind: "BadCredentials" }
  | { kind: "Forbidden" }
  | { kind: "NoVault" }
  | { kind: "NotFound" }
  | { kind: "TooManyAttempts"; retry_after_s: number }
  | { kind: "Invalid"; field: string; reason: string }
  | { kind: "UnknownAccount" }
  | { kind: "DemoExpired" }
  | { kind: "TooManyDemos"; retry_after_s: number }
  | { kind: "DemoFull" }
  | { kind: "DemoQuota"; quota_mb: number }
  | { kind: "Internal" }
  /** Сервер недоступен (сеть, прокси, сервер остановлен). */
  | { kind: "Offline" };

export type ServerResult<T> = { ok: true; value: T } | { ok: false; failure: ServerFailure };

const JSON_HEADERS = { "Content-Type": "application/json" };

/** Ответы прокси, когда сам сервер за ним не отвечает. */
const GATEWAY = new Set([502, 503, 504]);

/**
 * Ответ пришёл не от сервера Typsidian, а от прокси перед ним, который до сервера
 * не достучался: шлюзовой код или ошибка без JSON (сервер свои отказы всегда
 * отдаёт JSON; прокси `next dev` на отказ соединения отвечает 500 с текстом).
 */
export function unreachable(response: Response): boolean {
  if (GATEWAY.has(response.status)) return true;
  const json = (response.headers.get("content-type") ?? "").includes("application/json");
  return response.status >= 500 && !json;
}

function failureOf(value: unknown): ServerFailure {
  if (typeof value === "object" && value !== null && "kind" in value) {
    return value as ServerFailure;
  }
  return { kind: "Internal" };
}

/** Запрос к API; тело ответа — JSON. */
export async function request<T>(
  method: string,
  path: string,
  options: { csrf?: string; body?: unknown } = {},
): Promise<ServerResult<T>> {
  let response: Response;
  try {
    response = await fetch(path, {
      method,
      credentials: "same-origin",
      headers: {
        ...(options.body === undefined ? {} : JSON_HEADERS),
        ...(options.csrf === undefined ? {} : { "X-Typsidian-CSRF": options.csrf }),
      },
      ...(options.body === undefined ? {} : { body: JSON.stringify(options.body) }),
    });
  } catch {
    return { ok: false, failure: { kind: "Offline" } };
  }
  if (unreachable(response)) return { ok: false, failure: { kind: "Offline" } };
  let value: unknown = null;
  try {
    value = await response.json();
  } catch {
    // Пустое или не-JSON тело (прокси, статика): отказ без подробностей.
  }
  // `None` сервера — `null`; общий UI сверяется с `undefined` (как `tauri-invoke.ts`).
  value = withoutNullFields(value);
  if (!response.ok) return { ok: false, failure: failureOf(value) };
  return { ok: true, value: value as T };
}

export async function serverInfo(): Promise<ServerInfo | null> {
  const result = await request<ServerInfo>("GET", "/api/info");
  return result.ok ? result.value : null;
}

/** Текущая сессия; `null` — входа нет (или он истёк). */
export async function currentSession(): Promise<ServerResult<ServerSession | null>> {
  const result = await request<ServerSession>("GET", "/api/me");
  if (
    !result.ok &&
    (result.failure.kind === "Unauthenticated" || result.failure.kind === "DemoExpired")
  ) {
    return { ok: true, value: null };
  }
  return result;
}

export function login(loginName: string, password: string): Promise<ServerResult<ServerSession>> {
  return request<ServerSession>("POST", "/api/login", {
    body: { login: loginName, password },
  });
}

/** Гость демо-сервера (фича 039): временный аккаунт, его vault и сессия. */
export function demoStart(): Promise<ServerResult<ServerSession & { vault: string }>> {
  return request<ServerSession & { vault: string }>("POST", "/api/demo/start", { body: {} });
}

export function demoUsage(): Promise<ServerResult<DemoUsage>> {
  return request<DemoUsage>("GET", "/api/demo/usage");
}

/** Адрес начала входа через провайдера: переход страницы, а не запрос. */
export function oidcStartUrl(): string {
  return "/api/oidc/start";
}

export function logout(session: ServerSession): Promise<ServerResult<null>> {
  return request<null>("POST", "/api/logout", { csrf: session.csrf });
}

export function logoutAll(session: ServerSession): Promise<ServerResult<null>> {
  return request<null>("POST", "/api/logout-all", { csrf: session.csrf });
}

export function changePassword(
  session: ServerSession,
  current: string,
  next: string,
): Promise<ServerResult<null>> {
  return request<null>("POST", "/api/password", {
    csrf: session.csrf,
    body: { current, next },
  });
}

export function sessions(): Promise<ServerResult<ServerSessionEntry[]>> {
  return request<ServerSessionEntry[]>("GET", "/api/sessions");
}

export function endSession(session: ServerSession, id: string): Promise<ServerResult<null>> {
  return request<null>("DELETE", `/api/sessions/${encodeURIComponent(id)}`, {
    csrf: session.csrf,
  });
}

export function vaults(): Promise<ServerResult<ServerVault[]>> {
  return request<ServerVault[]>("GET", "/api/vaults");
}

export function createVault(session: ServerSession, name: string): Promise<ServerResult<ServerVault>> {
  return request<ServerVault>("POST", "/api/vaults", { csrf: session.csrf, body: { name } });
}

export function renameVault(
  session: ServerSession,
  name: string,
  next: string,
): Promise<ServerResult<ServerVault>> {
  return request<ServerVault>("PATCH", vaultPath(name), {
    csrf: session.csrf,
    body: { name: next },
  });
}

export function deleteVault(session: ServerSession, name: string): Promise<ServerResult<null>> {
  return request<null>("DELETE", vaultPath(name), { csrf: session.csrf });
}

export function vaultPath(name: string): string {
  return `/api/vaults/${encodeURIComponent(name)}`;
}

/** Адрес скачивания файла vault (экспорт): путь внутри vault, по сегментам. */
export function vaultFileUrl(vault: string, path: string): string {
  return `${vaultPath(vault)}/file/${path.split("/").map(encodeURIComponent).join("/")}`;
}
