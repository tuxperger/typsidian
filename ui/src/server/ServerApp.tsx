"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { ContextMenuProvider } from "../menu/ContextMenu.js";
import type { BoardWriteProfile } from "../wasm-core/typsidian_core.js";
import { tauriPreviewClient, tauriVaultClient } from "../vault-client-tauri.js";
import type { LastVault, VaultOpenResult } from "../vault-tree/useVault.js";
import type { SyncClient } from "../sync/sync-client.js";
import { openServerSync } from "../sync/sync-client-tauri.js";
import { AccountSection } from "./AccountSection.js";
import { DemoOverlays, DemoPill } from "./DemoStatus.js";
import { ConnectionBanner, useConnectionState } from "./ConnectionBanner.js";
import { createServerConnection, type ServerConnection } from "./connection.js";
import { remoteTransport } from "./remote-invoke.js";
import {
  currentSession,
  logout,
  serverInfo,
  vaultFileUrl,
  type ServerInfo,
  type ServerSession,
} from "./server-api.js";
import { ServerLogin, type LoginNotice } from "./ServerLogin.js";
import { VaultList } from "./VaultList.js";

// Серверный режим web (фича 037): вход → «Мои vault» → рабочая область над vault на
// сервере. Рабочая область та же, что с папкой на устройстве; её собирает оболочка
// (`apps/web`) — сюда приходит только способ открыть vault и раздел «Аккаунт».

/** Открыть vault на сервере: клиент Tauri поверх HTTP (research.md §6). */
export async function openServerVault(
  vault: string,
  connection: ServerConnection,
): Promise<VaultOpenResult> {
  const transport = remoteTransport(vault, connection);
  let profile: BoardWriteProfile;
  try {
    profile = await transport.invoke<BoardWriteProfile>("board_write_profile");
  } catch {
    return { ok: false, error: { kind: "FolderNotFound" } };
  }
  // История и удалённый репозиторий vault ведёт сервер (фича 038). Без сессии
  // синхронизации vault всё равно открывается — панель просто недоступна.
  let sync: SyncClient | undefined;
  try {
    sync = await openServerSync(transport);
  } catch {
    sync = undefined;
  }
  return {
    ok: true,
    client: {
      ...tauriVaultClient(profile, vault, transport),
      fileUrl: (path) => vaultFileUrl(vault, path),
    },
    preview: tauriPreviewClient(transport),
    ...(sync === undefined ? {} : { sync }),
  };
}

export interface ServerWorkspaceArgs {
  vault: string;
  /** `lastVault` рабочей области: открывает выбранный vault сразу. */
  lastVault: () => Promise<LastVault | null>;
  /** «Открыть vault…» рабочей области — назад к списку. */
  openVaultFolder: () => Promise<VaultOpenResult>;
  /** Раздел «Аккаунт» настроек; на демо-сервере его нет (фича 039). */
  account?: React.ReactNode;
  /** Состояние в шапке рабочей области: остаток демо (фича 039). */
  headerStatus?: React.ReactNode;
}

export interface ServerAppProps {
  /** «← Папка на устройстве». */
  onDeviceMode: () => void;
  renderWorkspace: (args: ServerWorkspaceArgs) => React.ReactNode;
}

/** Vault гостя демо-сервера — имя задаёт сервер (`apps/server/src/demo.rs`). */
const DEMO_VAULT = "Демо"; // i18n-ignore: имя папки на сервере, а не текст

/** Последний открытый vault этого браузера: перезагрузка возвращает в него (US1 AS5). */
const LAST_VAULT_KEY = "typsidian.serverVault";

function lastVaultName(): string | null {
  try {
    return localStorage.getItem(LAST_VAULT_KEY);
  } catch {
    return null;
  }
}

function rememberVault(name: string | null): void {
  try {
    if (name === null) localStorage.removeItem(LAST_VAULT_KEY);
    else localStorage.setItem(LAST_VAULT_KEY, name);
  } catch {
    // Хранилище недоступно — после перезагрузки откроется список.
  }
}

type Screen =
  | { kind: "loading" }
  | { kind: "login"; info: ServerInfo; notice?: LoginNotice }
  | { kind: "vaults"; info: ServerInfo; session: ServerSession }
  | { kind: "workspace"; info: ServerInfo; session: ServerSession; vault: string };

/** Итог возврата от провайдера входа (`?login=…`), один раз. */
function takeLoginNotice(): LoginNotice | undefined {
  if (typeof location === "undefined") return undefined;
  const params = new URLSearchParams(location.search);
  const value = params.get("login");
  if (value === null) return undefined;
  params.delete("login");
  const query = params.toString();
  history.replaceState(null, "", location.pathname + (query === "" ? "" : `?${query}`));
  if (value === "unknown") return "unknownAccount";
  return "oidcFailed";
}

export function ServerApp({ onDeviceMode, renderWorkspace }: ServerAppProps): React.JSX.Element {
  const [screen, setScreen] = useState<Screen>({ kind: "loading" });
  const [connection, setConnection] = useState<ServerConnection | null>(null);

  const signedIn = useCallback((info: ServerInfo, session: ServerSession) => {
    setConnection(createServerConnection(session));
    // Гость демо-сервера сразу в своём единственном vault (фича 039).
    if (info.demo !== undefined) {
      setScreen({ kind: "workspace", info, session, vault: DEMO_VAULT });
      return;
    }
    const last = lastVaultName();
    setScreen(
      last === null
        ? { kind: "vaults", info, session }
        : { kind: "workspace", info, session, vault: last },
    );
  }, []);

  useEffect(() => {
    void (async () => {
      const info = (await serverInfo()) ?? { name: "", password: true };
      const notice = takeLoginNotice();
      const me = await currentSession();
      if (me.ok && me.value !== null) signedIn(info, me.value);
      else setScreen({ kind: "login", info, ...(notice === undefined ? {} : { notice }) });
    })();
  }, [signedIn]);

  const toLogin = useCallback((info: ServerInfo) => {
    setConnection(null);
    setScreen({ kind: "login", info });
  }, []);

  if (screen.kind === "loading") return <main className="ts-server-screen" />;

  if (screen.kind === "login") {
    return (
      <main className="ts-server-screen">
        <ServerLogin
          info={screen.info}
          {...(screen.notice === undefined ? {} : { notice: screen.notice })}
          onSignedIn={(session) => signedIn(screen.info, session)}
          onBack={onDeviceMode}
        />
      </main>
    );
  }

  if (screen.kind === "vaults" || connection === null) {
    const session = screen.session;
    return (
      <ContextMenuProvider>
        <VaultList
          session={session}
          onOpen={(vault) => {
            rememberVault(vault);
            setScreen({ kind: "workspace", info: screen.info, session, vault });
          }}
          onLogout={() => void logout(session).then(() => toLogin(screen.info))}
        />
      </ContextMenuProvider>
    );
  }

  return (
    <ServerWorkspace
      key={screen.vault}
      screen={screen}
      connection={connection}
      renderWorkspace={renderWorkspace}
      onVaults={() => {
        rememberVault(null);
        setScreen({ kind: "vaults", info: screen.info, session: screen.session });
      }}
      onSignedOut={() => toLogin(screen.info)}
    />
  );
}

function ServerWorkspace({
  screen,
  connection,
  renderWorkspace,
  onVaults,
  onSignedOut,
}: {
  screen: Extract<Screen, { kind: "workspace" }>;
  connection: ServerConnection;
  renderWorkspace: ServerAppProps["renderWorkspace"];
  onVaults: () => void;
  onSignedOut: () => void;
}): React.JSX.Element {
  const state = useConnectionState(connection);
  const { vault, info, session } = screen;
  // Гость демо-сервера (фича 039): ни списка vault, ни раздела «Аккаунт», ни
  // повторного входа — вместо них остаток времени и окно окончания.
  const demo = info.demo !== undefined && session.expiresAt !== undefined;
  const args = useMemo<ServerWorkspaceArgs>(
    () => ({
      vault,
      lastVault: async () => ({
        label: vault,
        readyToOpen: true,
        // Vault пропал (удалён в другой вкладке) — назад к списку.
        open: async () => {
          const opened = await openServerVault(vault, connection);
          if (!opened.ok && !demo) onVaults();
          return opened;
        },
      }),
      openVaultFolder: async () => {
        if (!demo) onVaults();
        return { ok: false, error: { kind: "PickerCancelled" } };
      },
      ...(demo
        ? { headerStatus: <DemoPill expiresAt={session.expiresAt ?? 0} /> }
        : { account: <AccountSection connection={connection} onSignedOut={onSignedOut} /> }),
    }),
    [vault, connection, onVaults, onSignedOut, demo, session.expiresAt],
  );
  return (
    <>
      <ConnectionBanner connection={connection} />
      {renderWorkspace(args)}
      {demo && info.demo !== undefined && (
        <DemoOverlays
          demo={info.demo}
          expiresAt={session.expiresAt ?? 0}
          ended={state.kind === "signedOut"}
        />
      )}
      {!demo && state.kind === "signedOut" && (
        // Вход потерян посреди работы: рабочая область остаётся под экраном входа,
        // и ждущие вызовы (с несохранённым текстом) продолжатся после него.
        <div className="ts-server-overlay">
          <ServerLogin
            info={info}
            notice="expired"
            initialLogin={connection.session().login}
            onSignedIn={(session) => {
              if (session.login !== connection.session().login) {
                location.reload();
                return;
              }
              connection.resume(session);
            }}
          />
        </div>
      )}
    </>
  );
}
