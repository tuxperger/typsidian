"use client";

import { useEffect, useState } from "react";
import { ShowWindowRoot } from "@typsidian/ui/slides/ShowWindowRoot.js";
import {
  parseShowLocation,
  type ShowRole,
  type ShowViewer,
} from "@typsidian/ui/slides/show-channel.js";
import { createWebShowViewer } from "@typsidian/ui/slides/show-channel-web.js";

/**
 * Окно показа слайдов и вид докладчика (фича 019, contracts/show-channel.md).
 *
 * Его открывает окно редактора через `window.open('/show?session=…&role=…')`.
 * Здесь нет ни рабочей области, ни ядра: окно рисует страницы, которые ему
 * присылает хозяин по `BroadcastChannel`, и шлёт обратно действия. Адрес
 * читается на клиенте: канал и роль известны только после загрузки страницы.
 */
export default function ShowPage(): React.JSX.Element {
  const [target, setTarget] = useState<{ viewer: ShowViewer; role: ShowRole } | null>(null);

  useEffect(() => {
    const { session, role } = parseShowLocation(window.location.search);
    // Адрес без сессии — окно открыто не из приложения: слушать нечего.
    if (session === null) return;
    setTarget({ viewer: createWebShowViewer(session), role });
  }, []);

  if (target === null) return <div className="ts-show ts-show--loading" />;
  return <ShowWindowRoot viewer={target.viewer} role={target.role} />;
}
