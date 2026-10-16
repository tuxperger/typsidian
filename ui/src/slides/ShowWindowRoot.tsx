"use client";

import { useEffect } from "react";
import { IconSprite } from "../icons/Icons.js";
import type { Diagnostic } from "../wasm-core/typsidian_core.js";
import { I18nProvider, useI18n, type Translate } from "../i18n/index.js";
import { diagnosticText } from "../i18n/messages.js";
import { PresenterView } from "./PresenterView.js";
import { ShowView } from "./ShowView.js";
import type { ShowRole, ShowViewer } from "./show-channel.js";
import { useShowWindow, type ShowWindowModel } from "./useShowWindow.js";

export interface ShowWindowRootProps {
  viewer: ShowViewer;
  role: ShowRole;
  /**
   * Показ слоем в окне редактора (Android): касания вместо панели, вида
   * докладчика нет.
   */
  touch?: boolean;
}

/**
 * Корень окна показа или докладчика: канал, язык окна редактора и нужный вид по
 * роли. Без рабочей области и без ядра — точки входа оболочек
 * (`apps/web/app/show/page.tsx`, `apps/desktop/src/show.tsx`) монтируют только его.
 */
export function ShowWindowRoot({ viewer, role, touch = false }: ShowWindowRootProps): React.JSX.Element {
  const model = useShowWindow(viewer, role);
  // Слой в окне редактора живёт под его провайдером языка и спрайтом иконок.
  if (touch) return <ShowWindowBody model={model} role={role} touch />;
  return (
    // Язык — тот, что у окна редактора (приходит в `hello`); до него — язык системы.
    <I18nProvider choice={model.locale}>
      <IconSprite />
      <ShowWindowBody model={model} role={role} touch={false} />
    </I18nProvider>
  );
}

function ShowWindowBody({
  model,
  role,
  touch,
}: {
  model: ShowWindowModel;
  role: ShowRole;
  touch: boolean;
}): React.JSX.Element {
  const { t } = useI18n();

  useEffect(() => {
    if (!touch) document.title = t("slides.windowTitle");
  }, [t, touch]);

  // Окно показа и докладчика — само по себе экран, а не часть страницы. Показ
  // не прокручивается; вид докладчика в узком окне — прокручивается.
  useEffect(() => {
    if (touch) return undefined;
    const name = role === "presenter" ? "ts-presenter-document" : "ts-show-document";
    document.documentElement.classList.add(name);
    return () => document.documentElement.classList.remove(name);
  }, [touch, role]);

  const { position } = model;
  if (!model.ready || position === null) {
    // До `hello`, `deck` и `state` — только фон показа (contracts/show-channel.md).
    return (
      <div className="ts-show ts-show--loading" role="status" aria-label={t("slides.view.loading")} />
    );
  }

  const { state, label, stale, failure } = position;
  const errorLine = failure?.location?.line ?? null;

  if (role === "presenter") {
    return (
      <PresenterView
        state={state}
        label={label}
        pages={model.pages}
        slides={model.slides}
        stale={stale}
        failure={failure}
        onAction={model.act}
        onExit={model.exit}
        onRevealError={model.revealError}
      />
    );
  }

  const page = state.total === 0 ? null : (model.pages[state.page] ?? null);
  return (
    <ShowView
      page={page}
      label={page === null ? null : label}
      stale={stale}
      errorLine={errorLine}
      emptyReason={failure === undefined ? null : failureText(t, failure)}
      gotoBuffer={model.gotoBuffer}
      canPresent={!touch}
      touch={touch}
      onPrevious={() => model.act({ kind: "previous" })}
      onNext={() => model.act({ kind: "next" })}
      onPresenter={model.togglePresenter}
      onExit={model.exit}
      onRevealError={model.revealError}
      onClose={model.close}
    />
  );
}

/** Первая ошибка для экрана «Показывать нечего»: место и текст, как в панели ошибок. */
function failureText(t: Translate, failure: Diagnostic): string {
  const place = failure.location;
  const text = diagnosticText(t, failure);
  return place === undefined ? text : `${place.path}:${place.line}:${place.column}\n${text}`;
}
