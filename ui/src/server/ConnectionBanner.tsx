"use client";

import { useEffect, useState } from "react";
import { useI18n } from "../i18n/index.js";
import type { ConnectionState, ServerConnection } from "./connection.js";

// «Нет связи с сервером» (фича 037, макет S5, FR-010): заметно, с отсчётом до
// повтора и кнопкой «Повторить». Правки при этом ждут в редакторе.

export function useConnectionState(connection: ServerConnection): ConnectionState {
  const [state, setState] = useState(connection.state());
  useEffect(() => connection.subscribe(setState), [connection]);
  return state;
}

export function ConnectionBanner({
  connection,
}: {
  connection: ServerConnection;
}): React.JSX.Element | null {
  const { t } = useI18n();
  const state = useConnectionState(connection);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (state.kind !== "offline") return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [state]);

  if (state.kind !== "offline") return null;
  const seconds = Math.max(0, Math.ceil((state.retryAt - now) / 1000));
  return (
    <div className="ts-server-banner" role="alert">
      <span>{t("server.connection.offline")}</span>
      <span className="ts-spacer" />
      <span className="ts-server-banner__retry">{t("server.connection.retryIn", { seconds })}</span>
      <button type="button" className="ts-btn ts-btn--outline" onClick={() => connection.retryNow()}>
        {t("server.connection.retry")}
      </button>
    </div>
  );
}
