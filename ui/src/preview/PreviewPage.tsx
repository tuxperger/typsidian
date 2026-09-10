"use client";

import type { RenderedPage } from "../wasm-core/typsidian_core.js";
import { TextLayer } from "./TextLayer.js";

/**
 * Одна страница: картинка плюс текстовый слой поверх неё.
 *
 * Оба слоя берутся из одного и того же [`RenderedPage`], и это не мелочь:
 * картинка от одной сборки с текстом от другой дала бы выделение, съехавшее
 * относительно букв. Ядро отдаёт страницу целиком именно поэтому
 * (contracts/text-layer.md § Согласованность с картинкой).
 *
 * SVG картинки вставляется разметкой: это вывод компилятора, а не
 * пользовательский ввод — он порождён ядром из текста заметки и наружу не
 * ходит (принцип IX).
 */
export function PreviewPage({ page }: { page: RenderedPage }): React.JSX.Element {
  return (
    <article
      className="ts-preview__page"
      aria-label={`Страница ${page.number}`}
      style={{ aspectRatio: `${page.width_pt} / ${page.height_pt}` }}
    >
      {/* Картинка декоративна: текста она не несёт (глифы — контуры `<path>`),
          а читаемое содержимое страницы даёт слой ниже. Без пометки программа
          чтения с экрана обошла бы страницу дважды. */}
      <div
        className="ts-preview__paint"
        aria-hidden="true"
        dangerouslySetInnerHTML={{ __html: page.svg }}
      />
      <TextLayer page={page} />
    </article>
  );
}
