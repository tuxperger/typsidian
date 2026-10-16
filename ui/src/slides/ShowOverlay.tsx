"use client";

import { useEffect, useSyncExternalStore } from "react";
import type { InMemoryShow } from "./show-channel.js";
import { ShowWindowRoot } from "./ShowWindowRoot.js";

export interface ShowOverlayProps {
  show: InMemoryShow;
}

/**
 * Показ слоем на весь экран поверх рабочей области (Android, A8). Хозяин и
 * зритель — в этом же окне (`createInMemoryShowHost`), вида докладчика нет.
 */
export function ShowOverlay({ show }: ShowOverlayProps): React.JSX.Element | null {
  const open = useSyncExternalStore(show.subscribe, show.isOpen, () => false);
  if (!open) return null;
  return <OverlayLayer show={show} />;
}

function OverlayLayer({ show }: { show: InMemoryShow }): React.JSX.Element {
  // Системная «Назад» — выход из показа, а не из приложения: при старте в
  // историю кладётся запись, `popstate` её снимает (research.md §9).
  useEffect(() => {
    let popped = false;
    window.history.pushState({ typsidianShow: true }, "");
    const onPopState = (): void => {
      popped = true;
      show.viewer.send({ kind: "exit" });
    };
    window.addEventListener("popstate", onPopState);
    return () => {
      window.removeEventListener("popstate", onPopState);
      // Выход кнопкой оставил запись в истории — снимаем её, чтобы следующая
      // «Назад» не оказалась лишней.
      if (!popped && window.history.state?.typsidianShow === true) window.history.back();
    };
  }, [show]);

  // На весь экран и в альбомную ориентацию — по возможности: отказ показу не мешает.
  useEffect(() => {
    void document.documentElement.requestFullscreen?.().catch(() => undefined);
    const orientation = screen.orientation as ScreenOrientation & {
      lock?: (orientation: "landscape") => Promise<void>;
    };
    void orientation.lock?.("landscape").catch(() => undefined);
    return () => {
      if (document.fullscreenElement !== null) void document.exitFullscreen().catch(() => undefined);
      orientation.unlock?.();
    };
  }, []);

  return (
    <div className="ts-show-overlay">
      <ShowWindowRoot viewer={show.viewer} role="show" touch />
    </div>
  );
}
