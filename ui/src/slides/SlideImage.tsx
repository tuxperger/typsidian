"use client";

import { useLayoutEffect, useRef, useState } from "react";

/** Страница, готовая к показу: картинка и размер в пунктах. */
export interface SlidePage {
  /** Отпечаток страницы; по нему окно кэширует SVG (research.md §10). */
  digest: string;
  /** SVG страницы; `undefined` — картинка ещё не пришла, рисуется бумага. */
  svg: string | undefined;
  widthPt: number;
  heightPt: number;
}

export interface SlideImageProps {
  page: SlidePage;
  /** Подпись для средств чтения с экрана: у картинки текста нет (глифы — контуры). */
  label: string;
  className?: string;
}

/**
 * Страница-слайд: SVG, вписанный в рамку по `min(w/W, h/H)`, без текстового слоя.
 *
 * Рамка — родитель; он обязан иметь размер (flex/grid с заданной высотой).
 * Размер слайда считается из размера рамки, а не задаётся CSS-трюком с
 * `aspect-ratio`: так слайд не вылезает ни по ширине, ни по высоте при любом
 * соотношении сторон окна и страницы.
 *
 * SVG — вывод компилятора ядра, не пользовательский ввод; вставляется разметкой
 * так же, как в предпросмотре (`PreviewPage`). При смене отпечатка разметка
 * подменяется в одном кадре, без промежуточной пустой страницы.
 */
export function SlideImage({ page, label, className }: SlideImageProps): React.JSX.Element {
  const frameRef = useRef<HTMLDivElement | null>(null);
  const [frame, setFrame] = useState<{ width: number; height: number } | null>(null);

  useLayoutEffect(() => {
    const node = frameRef.current;
    if (node === null) return undefined;
    const measure = (): void => {
      const next = { width: node.clientWidth, height: node.clientHeight };
      setFrame((previous) =>
        previous !== null && previous.width === next.width && previous.height === next.height
          ? previous
          : next,
      );
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  const scale =
    frame === null || page.widthPt <= 0 || page.heightPt <= 0
      ? 0
      : Math.min(frame.width / page.widthPt, frame.height / page.heightPt);

  return (
    <div ref={frameRef} className={className ? `ts-slide-frame ${className}` : "ts-slide-frame"}>
      {scale > 0 && (
        <div
          className="ts-slide"
          role="img"
          aria-label={label}
          style={{ width: page.widthPt * scale, height: page.heightPt * scale }}
        >
          {page.svg !== undefined && (
            <div className="ts-slide__paint" dangerouslySetInnerHTML={{ __html: page.svg }} />
          )}
        </div>
      )}
    </div>
  );
}
