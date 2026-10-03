/**
 * Язык из настроек устройства (фича 010, FR-012, FR-015, FR-029).
 *
 * Один провайдер на все три оболочки: каждая передаёт сюда своего клиента
 * хранилища устройства, а откуда взять выбор и как узнать о его смене, решается
 * здесь один раз — иначе web, desktop и Android разошлись бы в мелочах
 * (принцип II, SC-007).
 *
 * О смене выбора провайдер узнаёт от панели настроек: `useSettings` объявляет
 * действующее значение через [`announceLanguageChoice`] после каждого
 * применения, сброса и внешней правки файла. Отдельного опроса хранилища здесь
 * нет — панель его уже ведёт.
 */

import { createElement, useEffect, useState, type ReactNode } from "react";
import type { DeviceSettingsClient } from "../settings/device-settings-client.js";
import { I18nProvider } from "./index.js";

type Listener = (choice: string | undefined) => void;

const listeners = new Set<Listener>();

/** Сообщить действующий выбор языка из настроек устройства. */
export function announceLanguageChoice(choice: string | undefined): void {
  for (const listener of listeners) listener(choice);
}

export function DeviceI18nProvider(props: {
  device: DeviceSettingsClient;
  children: ReactNode;
}): React.JSX.Element {
  // До ответа хранилища — «как в системе»: язык нужен уже первому кадру.
  const [choice, setChoice] = useState<string | undefined>(undefined);

  useEffect(() => {
    let cancelled = false;
    listeners.add(setChoice);
    props.device.snapshot().then(
      (snapshot) => {
        if (!cancelled) setChoice(snapshot.settings.language);
      },
      // Хранилище недоступно — остаётся язык системы (FR-029). Сообщение о
      // недоступности покажет панель настроек, уже на этом языке.
      () => {},
    );
    return () => {
      cancelled = true;
      listeners.delete(setChoice);
    };
  }, [props.device]);

  return createElement(I18nProvider, { choice, children: props.children });
}
