"use client";

import { useCallback, useEffect, useState } from "react";
import { useI18n, type I18n } from "../i18n/index.js";
import { useContextMenu } from "../menu/ContextMenu.js";
import {
  createVault,
  deleteVault,
  renameVault,
  vaults as listVaults,
  type ServerFailure,
  type ServerSession,
  type ServerVault,
} from "./server-api.js";
import { agoText } from "./when.js";

// «Мои vault» (фича 037, макет S3, US2): открыть, создать, переименовать, удалить с
// подтверждением, где названо число заметок. Имя проверяет сервер; здесь — только
// показ его отказа.

export interface VaultListProps {
  session: ServerSession;
  onOpen: (name: string) => void;
  onLogout: () => void;
}

function failureText(i18n: I18n, failure: ServerFailure): string {
  if (failure.kind === "Invalid" && failure.reason === "Taken") return i18n.t("server.vaults.taken");
  if (failure.kind === "Invalid") return i18n.t("server.vaults.badName");
  if (failure.kind === "Offline") return i18n.t("server.login.offline");
  return i18n.t("server.vaults.failed");
}

export function VaultList({ session, onOpen, onLogout }: VaultListProps): React.JSX.Element {
  const i18n = useI18n();
  const { t } = i18n;
  const menu = useContextMenu();
  const [list, setList] = useState<ServerVault[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [newName, setNewName] = useState("");
  const [renaming, setRenaming] = useState<{ name: string; value: string } | null>(null);
  const [deleting, setDeleting] = useState<ServerVault | null>(null);
  const [selected, setSelected] = useState<string | null>(null);

  const reload = useCallback(async () => {
    const result = await listVaults();
    if (result.ok) setList(result.value);
    else setError(failureText(i18n, result.failure));
  }, [i18n]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const run = async (action: Promise<{ ok: boolean; failure?: ServerFailure }>): Promise<boolean> => {
    const result = await action;
    if (!result.ok) {
      setError(failureText(i18n, result.failure ?? { kind: "Internal" }));
      return false;
    }
    setError(null);
    await reload();
    return true;
  };

  const initial = (session.displayName || session.login).slice(0, 1).toUpperCase();

  return (
    <main className="ts-server-screen">
      <section className="ts-server-card ts-server-card--list" aria-label={t("server.vaults.title")}>
        <header className="ts-server-list__head">
          <h1>{t("server.vaults.title")}</h1>
          <span className="ts-spacer" />
          <span className="ts-server-user">
            <span className="ts-server-user__avatar" aria-hidden="true">
              {initial}
            </span>
            {session.login}
          </span>
          <button type="button" className="ts-btn ts-btn--quiet" onClick={onLogout}>
            {t("server.vaults.logout")}
          </button>
        </header>

        {list !== null && list.length === 0 && (
          <p className="ts-server-list__empty">{t("server.vaults.empty")}</p>
        )}
        <ul className="ts-server-list">
          {(list ?? []).map((vault) => (
            <li
              key={vault.name}
              className={
                "ts-server-list__row" + (selected === vault.name ? " ts-server-list__row--on" : "")
              }
              onClick={() => setSelected(vault.name)}
              onDoubleClick={() => onOpen(vault.name)}
            >
              {renaming?.name === vault.name ? (
                <form
                  className="ts-server-list__rename"
                  onSubmit={(event) => {
                    event.preventDefault();
                    void run(renameVault(session, vault.name, renaming.value)).then((ok) => {
                      if (ok) setRenaming(null);
                    });
                  }}
                >
                  <input
                    className="ts-field__input"
                    aria-label={t("server.vaults.renameTo")}
                    autoFocus
                    value={renaming.value}
                    onChange={(event) => setRenaming({ name: vault.name, value: event.target.value })}
                  />
                  <button type="submit" className="ts-btn ts-btn--outline">
                    {t("server.vaults.save")}
                  </button>
                  <button type="button" className="ts-btn ts-btn--quiet" onClick={() => setRenaming(null)}>
                    {t("server.vaults.cancel")}
                  </button>
                </form>
              ) : (
                <>
                  <span>
                    <b>{vault.name}</b>
                    <span className="ts-server-list__meta">
                      {t("server.vaults.notes", { count: vault.notes })}
                      {vault.modified !== undefined && ` · ${agoText(i18n, vault.modified)}`}
                    </span>
                  </span>
                  <button type="button" className="ts-btn ts-btn--outline" onClick={() => onOpen(vault.name)}>
                    {t("server.vaults.open")}
                  </button>
                  <button
                    type="button"
                    className="ts-btn ts-btn--quiet"
                    aria-label={t("server.vaults.more")}
                    onClick={(event) =>
                      menu.open(event, [
                        {
                          label: t("server.vaults.rename"),
                          run: () => setRenaming({ name: vault.name, value: vault.name }),
                        },
                        { label: t("server.vaults.delete"), run: () => setDeleting(vault) },
                      ])
                    }
                  >
                    ⋯
                  </button>
                </>
              )}
            </li>
          ))}
        </ul>

        {deleting !== null && (
          <div className="ts-server-confirm" role="alertdialog">
            <b>{t("server.vaults.confirmDelete", { name: deleting.name, count: deleting.notes })}</b>
            <span>{t("server.vaults.confirmDeleteHint")}</span>
            <div className="ts-server-row">
              <button
                type="button"
                className="ts-btn ts-btn--danger"
                onClick={() => {
                  const name = deleting.name;
                  setDeleting(null);
                  void run(deleteVault(session, name));
                }}
              >
                {t("server.vaults.confirmDeleteButton")}
              </button>
              <button type="button" className="ts-btn ts-btn--quiet" onClick={() => setDeleting(null)}>
                {t("server.vaults.cancel")}
              </button>
            </div>
          </div>
        )}

        {error !== null && (
          <div className="ts-server-alert ts-server-list__error" role="alert">
            {error}
          </div>
        )}

        <form
          className="ts-server-list__create"
          onSubmit={(event) => {
            event.preventDefault();
            const name = newName.trim();
            if (name === "") return;
            void run(createVault(session, name)).then((ok) => {
              if (ok) setNewName("");
            });
          }}
        >
          <input
            className="ts-field__input"
            placeholder={t("server.vaults.newName")}
            aria-label={t("server.vaults.newName")}
            value={newName}
            onChange={(event) => setNewName(event.target.value)}
          />
          <button type="submit" className="ts-btn ts-btn--accent" disabled={newName.trim() === ""}>
            {t("server.vaults.create")}
          </button>
        </form>
      </section>
    </main>
  );
}
