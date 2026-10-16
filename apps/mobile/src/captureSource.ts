import { addPluginListener, invoke } from "@tauri-apps/api/core";
import type { Capture } from "@typsidian/ui/wasm-core/typsidian_core.js";
import type { CaptureSource } from "@typsidian/ui/capture/capture-source.js";

/**
 * Источник захватов Android: плагин `capture` (фича 022).
 *
 * Очередь «Поделиться» и ярлыки живут в Kotlin-части плагина
 * (`src-tauri/plugins/capture/android`); здесь только вызовы его команд.
 * Запись в vault делает ядро, окно зовёт её само, а потом `resolve`.
 */
const PLUGIN = "capture";

/** Байты из base64, которым плагин отдаёт картинку (одной строкой). */
function fromBase64(data: string): Uint8Array {
  const binary = atob(data);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

/**
 * Подписка, которая оформляется асинхронно: `addPluginListener` возвращает
 * обещание, а отписка нужна синхронно. Отписка до завершения подписки снимает
 * её сразу после.
 */
function subscribe(event: string, handler: (payload: unknown) => void): () => void {
  let off: (() => Promise<void>) | undefined;
  let cancelled = false;
  void addPluginListener(PLUGIN, event, handler).then((listener) => {
    if (cancelled) void listener.unregister();
    else off = () => listener.unregister();
  });
  return () => {
    cancelled = true;
    if (off) void off();
    off = undefined;
  };
}

export function createCaptureSource(): CaptureSource {
  return {
    pending: () => invoke<Capture[]>(`plugin:${PLUGIN}|pending`),
    resolve: (id) => invoke<void>(`plugin:${PLUGIN}|resolve`, { id }),
    returnToSource: () => invoke<void>(`plugin:${PLUGIN}|return_to_source`),
    image: async (capture) => {
      if (capture.content.kind !== "Image") return new Uint8Array();
      const data = await invoke<string>(`plugin:${PLUGIN}|image`, {
        file: capture.content.file,
      });
      return fromBase64(data);
    },
    onShared: (handler) => subscribe("shared", () => handler()),
    onShortcut: (handler) =>
      subscribe("shortcut", (payload) => {
        const action = (payload as { action?: unknown } | null)?.action;
        if (action === "today" || action === "capture") handler(action);
      }),
  };
}
