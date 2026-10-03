"use client";

import { useEffect, useRef, useState } from "react";
import type {
  ExcerptReason,
  PreviewState,
  PreviewUnavailable,
  Template,
} from "../wasm-core/typsidian_core.js";
import { Icon } from "../icons/Icons.js";
import { previewKey } from "./useTemplateLibrary.js";
import { useI18n, type Translate } from "../i18n/index.js";

// Карточка заготовки (T021, макет «Typsidian Утилиты» § Шаблоны).
//
// Пять состояний превью показываются по-разному, и причина текстового превью
// названа **словами** (FR-011, FR-012, FR-017).
//
// Главный запрет фичи: `CompilerUnavailable` не схлопывается в
// `DoesNotCompile`. Схлопнув их, мы сказали бы пользователю, что не
// компилируются все его заготовки, — он сделал бы единственный доступный вывод,
// что сломаны они, а не конвейер превью. Это не косметика: это разница между «у
// тебя ошибка в Typst» и «мы пока не умеем показывать отрисованный вид».
//
// Превью строится **только когда карточка видна** (FR-014, SC-004): галерея на
// 200 заготовок иначе прочитала бы 200 файлов ради десятка карточек на экране.
// При этом карточка выбираема с самого начала — `Pending` относится к превью, а
// не к карточке.

export interface TemplateCardProps {
  template: Template;
  isSelected: boolean;
  /**
   * Построить превью. Вызывается, только когда карточка попала в область
   * видимости; результат кэширует ядро.
   */
  buildPreview: (template: Template) => Promise<PreviewState>;
  onSelect: (template: Template) => void;
  /** Двойное нажатие — применить сразу, не тянувшись к панели справа. */
  onApply?: (template: Template) => void;
}

export function TemplateCard({
  template,
  isSelected,
  buildPreview,
  onSelect,
  onApply,
}: TemplateCardProps): React.JSX.Element {
  const [preview, setPreview] = useState<PreviewState>({ state: "Pending" });
  const hostRef = useRef<HTMLDivElement | null>(null);
  const [isVisible, setIsVisible] = useState(false);

  // Видимость — через `IntersectionObserver`, а не через расчёт прокрутки:
  // считать координаты пришлось бы на каждый кадр, а браузер уже знает ответ.
  // Где наблюдателя нет (серверный рендер, старое окружение), карточка
  // считается видимой: лучше лишнее чтение файла, чем карточка, навсегда
  // застрявшая в `Pending`.
  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    if (typeof IntersectionObserver === "undefined") {
      setIsVisible(true);
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) setIsVisible(true);
      },
      // Запас в пол-экрана: превью успевает построиться до того, как карточка
      // доедет до глаз.
      { rootMargin: "50% 0px" },
    );
    observer.observe(host);
    return () => observer.disconnect();
  }, []);

  // Пересчёт привязан к **хэшу содержимого**, а не к ссылке на `template`:
  // ссылка новая на каждый ответ ядра, а хэш меняется ровно тогда, когда
  // изменилось содержимое (research.md §4). Переименование файла хэша не
  // трогает — и превью не пересчитывается, как и требует SC-010.
  const contentKey = previewKey(template);
  useEffect(() => {
    if (!isVisible) return;
    let cancelled = false;
    setPreview({ state: "Pending" });
    void (async () => {
      const built = await buildPreview(template);
      if (!cancelled) setPreview(built);
    })();
    return () => {
      cancelled = true;
    };
    // `template` в замыкании всегда текущий; в зависимостях его нет намеренно —
    // см. комментарий выше.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isVisible, contentKey, buildPreview]);

  return (
    <div
      ref={hostRef}
      className={
        isSelected ? "ts-tpl-card ts-tpl-card--selected" : "ts-tpl-card"
      }
      role="option"
      aria-selected={isSelected}
      tabIndex={0}
      onClick={() => onSelect(template)}
      onDoubleClick={() => onApply?.(template)}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onSelect(template);
        }
      }}
    >
      <div className="ts-tpl-card__head">
        <Icon name="file" size={13} />
        <span className="ts-tpl-card__name">{template.name}</span>
        <span className="ts-spacer" />
        {template.group !== undefined && (
          <span className="ts-tpl-card__group">{template.group}</span>
        )}
      </div>

      <PreviewBody preview={preview} />

      <div className="ts-tpl-card__foot">
        <PreviewNote preview={preview} />
      </div>
    </div>
  );
}

/**
 * Тело карточки. Пять состояний — пять разных видов; исчерпывающий разбор
 * размеченного объединения, забыть вариант нельзя (принцип IV).
 */
function PreviewBody({ preview }: { preview: PreviewState }): React.JSX.Element {
  switch (preview.state) {
    case "Rendered":
      return (
        <div
          className="ts-tpl-card__rendered"
          // Отрисованный вид приходит из ядра как SVG страницы. Сегодня этого
          // состояния не порождает ни одна включённая реализация порта превью.
          dangerouslySetInnerHTML={{ __html: preview.svg }}
        />
      );
    case "SourceExcerpt":
      return <pre className="ts-tpl-card__source">{preview.excerpt}</pre>;
    case "Unavailable":
      return (
        <div className="ts-tpl-card__source ts-tpl-card__source--empty">
          <Icon name="x" size={14} />
        </div>
      );
    case "Pending":
      return (
        <div
          className="ts-tpl-card__source ts-tpl-card__source--empty"
          aria-hidden="true"
        />
      );
    default: {
      const exhaustive: never = preview;
      return exhaustive;
    }
  }
}

/** Подпись под превью: почему карточка выглядит именно так. */
function PreviewNote({
  preview,
}: {
  preview: PreviewState;
}): React.JSX.Element | null {
  const { t } = useI18n();
  switch (preview.state) {
    case "Rendered":
      return <span className="ts-tpl-card__note">{t("templates.card.rendered")}</span>;
    case "SourceExcerpt":
      return (
        <span className="ts-tpl-card__note">
          {describeExcerptReason(t, preview.reason)}
        </span>
      );
    case "Unavailable":
      return (
        <span className="ts-tpl-card__note ts-tpl-card__note--warn" role="note">
          {describeUnavailable(t, preview.reason)}
        </span>
      );
    case "Pending":
      return <span className="ts-tpl-card__note">{t("templates.card.pending")}</span>;
    default: {
      const exhaustive: never = preview;
      return exhaustive;
    }
  }
}

/**
 * Почему вместо отрисованного вида — исходный текст.
 *
 * Две причины, две формулировки. Ни одна не называет заготовку сломанной:
 * фрагмент кода сам по себе обычно и не должен компилироваться — это штатный
 * случай (FR-013, SC-005).
 */
export function describeExcerptReason(t: Translate, reason: ExcerptReason): string {
  switch (reason) {
    case "CompilerUnavailable":
      return t("templates.card.compilerUnavailable");
    case "DoesNotCompile":
      return t("templates.card.doesNotCompile");
    default: {
      const exhaustive: never = reason;
      return exhaustive;
    }
  }
}

/** Почему превью не построено вовсе (FR-017). */
export function describeUnavailable(t: Translate, reason: PreviewUnavailable): string {
  switch (reason) {
    case "NotFound":
      return t("templates.card.notFound");
    case "Unreadable":
      return t("templates.card.unreadable");
    case "Io":
      return t("templates.card.io");
    default: {
      const exhaustive: never = reason;
      return exhaustive;
    }
  }
}
