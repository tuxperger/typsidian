"use client";

import { memo, useEffect, useState } from "react";
import type { RenderedPage } from "../wasm-core/typsidian_core.js";
import { TextLayer } from "./TextLayer.js";
import { useI18n } from "../i18n/index.js";

/**
 * Одна страница: картинка плюс текстовый слой поверх неё.
 *
 * Оба слоя берутся из одного и того же [`RenderedPage`], и это не мелочь:
 * картинка от одной сборки с текстом от другой дала бы выделение, съехавшее
 * относительно букв. Ядро отдаёт страницу целиком именно поэтому
 * (contracts/text-layer.md § Согласованность с картинкой).
 *
 * # Почему картинка — `<img>`, а не SVG в разметке
 *
 * В SVG страницы каждый глиф — отдельный контур, на заметке в полтора десятка
 * страниц это десятки тысяч узлов. Встроенные в документ, они проходят стили и
 * раскладку заново на каждом изменении ширины листа: перетаскивание границы
 * колонок давало кадр под секунду (005 FR-007, FR-026). Тот же SVG в `<img>`
 * браузер рисует как изображение — без узлов в документе, — и кадр дешевле на
 * порядок. Ссылок из картинки это не отнимает: на них и прежде нельзя было
 * нажать — поверх лежит текстовый слой.
 *
 * Адрес — `blob:`, а не `data:`: строка в мегабайты не копируется в атрибут, а
 * отпускается вместе со страницей.
 */
function PreviewPageView({ page }: { page: RenderedPage }): React.JSX.Element {
  const { t } = useI18n();
  const [paint, setPaint] = useState<string | null>(null);
  useEffect(() => {
    const url = URL.createObjectURL(new Blob([page.svg], { type: "image/svg+xml" }));
    setPaint(url);
    return () => URL.revokeObjectURL(url);
  }, [page.svg]);

  return (
    <article
      className="ts-preview__page"
      aria-label={t("preview.page", { number: page.number })}
      style={{ aspectRatio: `${page.width_pt} / ${page.height_pt}` }}
    >
      {/* Картинка декоративна: текста она не несёт (глифы — контуры `<path>`),
          а читаемое содержимое страницы даёт слой ниже. Без пометки программа
          чтения с экрана обошла бы страницу дважды. */}
      <div className="ts-preview__paint" aria-hidden="true">
        {paint !== null && <img src={paint} alt="" draggable={false} />}
      </div>
      <TextLayer page={page} />
    </article>
  );
}

/**
 * Страница перерисовывается, только когда изменился её отпечаток (FR-010,
 * T069): ядро отдаёт все страницы документа, а какие из них остались прежними,
 * говорит `digest`. Он покрывает и картинку, и текстовый слой — они из одной
 * сборки. Без этого правка последней страницы заново вставляла бы разметку
 * каждой предыдущей.
 */
export const PreviewPage = memo(
  PreviewPageView,
  (previous, next) =>
    previous.page.digest === next.page.digest && previous.page.index === next.page.index,
);
