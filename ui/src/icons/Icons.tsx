"use client";

// Набор иконок из макета Claude Design «Typsidian: Typst Editor Design»
// (handoff § Assets). Спрайт `<symbol>` — ровно тот, что в прототипе: контуры
// перенесены дословно, чтобы расхождение с макетом читалось как diff, а не как
// «кажется, скругл--ение другое».
//
// Почему свой спрайт, а не пакет Lucide: handoff предлагает взять Lucide
// напрямую, но это ещё одна зависимость ради двенадцати путей, которые уже
// нарисованы. Штрих у них тот же — 2px, `currentColor`, — поэтому замена на
// Lucide позже не потребует переписывать вызовы, только этот файл.

export type IconName =
  | "file"
  | "folder"
  | "search"
  | "network"
  | "refresh"
  | "cmd"
  | "enter"
  | "x"
  | "chevron"
  | "updown"
  | "image"
  | "terminal"
  | "plus"
  | "board"
  | "heading"
  | "list"
  | "listnum"
  | "sigma"
  | "table"
  | "code"
  | "quote"
  | "link"
  | "tag"
  | "bookmark"
  | "text"
  | "page"
  | "layout"
  | "shape"
  | "braces"
  | "data"
  | "tab"
  | "download"
  | "check"
  | "play"
  | "start"
  | "caret"
  | "down"
  | "presenter"
  | "fullscreen";

/**
 * Спрайт монтируется один раз в корне приложения; иконки ссылаются на него
 * через `<use href="#i-…">`.
 *
 * `aria-hidden` и нулевой размер: это склад определений, а не изображение.
 */
export function IconSprite(): React.JSX.Element {
  return (
    <svg
      style={{ width: 0, height: 0, position: "absolute" }}
      aria-hidden="true"
      focusable="false"
    >
      <defs>
        <symbol id="i-file" viewBox="0 0 24 24">
          <path
            d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <polyline
            points="14 2 14 8 20 8"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <line x1="9" y1="13" x2="15" y2="13" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          <line x1="9" y1="17" x2="13" y2="17" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
        </symbol>

        <symbol id="i-folder" viewBox="0 0 24 24">
          <path
            d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13c0 1.1.9 2 2 2Z"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </symbol>

        <symbol id="i-search" viewBox="0 0 24 24">
          <circle cx="11" cy="11" r="8" fill="none" stroke="currentColor" strokeWidth="2" />
          <line x1="21" y1="21" x2="16.65" y2="16.65" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
        </symbol>

        <symbol id="i-network" viewBox="0 0 24 24">
          <circle cx="18" cy="5" r="2.5" fill="none" stroke="currentColor" strokeWidth="2" />
          <circle cx="6" cy="12" r="2.5" fill="none" stroke="currentColor" strokeWidth="2" />
          <circle cx="18" cy="19" r="2.5" fill="none" stroke="currentColor" strokeWidth="2" />
          <line x1="8.2" y1="10.8" x2="15.8" y2="6.2" stroke="currentColor" strokeWidth="2" />
          <line x1="8.2" y1="13.2" x2="15.8" y2="17.8" stroke="currentColor" strokeWidth="2" />
        </symbol>

        <symbol id="i-refresh" viewBox="0 0 24 24">
          <path d="M21 12a9 9 0 1 1-2.64-6.36" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          <polyline
            points="21 3 21 9 15 9"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </symbol>

        <symbol id="i-cmd" viewBox="0 0 24 24">
          <path
            d="M9 3a3 3 0 0 0-3 3v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3v12a3 3 0 1 0 3-3H9"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </symbol>

        <symbol id="i-enter" viewBox="0 0 24 24">
          <polyline
            points="9 10 4 15 9 20"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <path
            d="M20 4v7a4 4 0 0 1-4 4H4"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </symbol>

        <symbol id="i-x" viewBox="0 0 24 24">
          <line x1="18" y1="6" x2="6" y2="18" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          <line x1="6" y1="6" x2="18" y2="18" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
        </symbol>

        {/* Экспорт (фича 009): Lucide `download` и `check`. */}
        <symbol id="i-download" viewBox="0 0 24 24">
          <path
            d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <polyline
            points="7 10 12 15 17 10"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <line x1="12" y1="15" x2="12" y2="3" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
        </symbol>

        <symbol id="i-check" viewBox="0 0 24 24">
          <polyline
            points="20 6 9 17 4 12"
            fill="none"
            stroke="currentColor"
            strokeWidth="3"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </symbol>

        <symbol id="i-chevron" viewBox="0 0 24 24">
          <polyline
            points="9 18 15 12 9 6"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </symbol>

        <symbol id="i-updown" viewBox="0 0 24 24">
          <polyline
            points="7 15 12 20 17 15"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <polyline
            points="7 9 12 4 17 9"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </symbol>

        <symbol id="i-image" viewBox="0 0 24 24">
          <rect x="3" y="3" width="18" height="18" rx="2" fill="none" stroke="currentColor" strokeWidth="2" />
          <circle cx="8.5" cy="8.5" r="1.5" fill="none" stroke="currentColor" strokeWidth="2" />
          <polyline
            points="21 15 16 10 5 21"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </symbol>

        {/* Из макета «Typsidian Доски» (`#d-plus`, `#d-board`): создание файла
            и признак доски. У доски свой глиф, а не «картинка», — в макете
            досок он отличается и от файла, и от диаграммы. */}
        <symbol id="i-plus" viewBox="0 0 24 24">
          <line x1="12" y1="5" x2="12" y2="19" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          <line x1="5" y1="12" x2="19" y2="12" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
        </symbol>

        <symbol id="i-board" viewBox="0 0 24 24">
          <rect x="2.5" y="6.5" width="11" height="11" fill="none" stroke="currentColor" strokeWidth="2" />
          <circle cx="16.5" cy="9.5" r="5" fill="none" stroke="currentColor" strokeWidth="2" />
        </symbol>

        <symbol id="i-terminal" viewBox="0 0 24 24">
          <polyline
            points="4 17 10 11 4 5"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <line x1="12" y1="19" x2="20" y2="19" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
        </symbol>

        {/* Из макета «Typsidian Вставка» (`#e-*`): разделы меню вставки. Штрих
            тот же 2px, что и у остальных, — набор один, а не два похожих. */}
        <symbol id="i-heading" viewBox="0 0 24 24">
          <path
            d="M6 4v16M18 4v16M6 12h12"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
          />
        </symbol>

        <symbol id="i-list" viewBox="0 0 24 24">
          <line x1="10" y1="6" x2="20" y2="6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          <line x1="10" y1="12" x2="20" y2="12" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          <line x1="10" y1="18" x2="20" y2="18" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          <circle cx="5" cy="6" r="1.5" fill="currentColor" />
          <circle cx="5" cy="12" r="1.5" fill="currentColor" />
          <circle cx="5" cy="18" r="1.5" fill="currentColor" />
        </symbol>

        <symbol id="i-listnum" viewBox="0 0 24 24">
          <line x1="10" y1="6" x2="20" y2="6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          <line x1="10" y1="12" x2="20" y2="12" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          <line x1="10" y1="18" x2="20" y2="18" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          <path
            d="M4 4.5h1.5V8M4 11h2.4L4 14.5h2.4M4 17h2.4v1.6H4.6v1.5h1.8"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.4"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </symbol>

        <symbol id="i-sigma" viewBox="0 0 24 24">
          <polyline
            points="18 5 6 5 12 12 6 19 18 19"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinejoin="round"
          />
        </symbol>

        <symbol id="i-table" viewBox="0 0 24 24">
          <rect x="3" y="4" width="18" height="16" rx="1" fill="none" stroke="currentColor" strokeWidth="2" />
          <line x1="3" y1="9.5" x2="21" y2="9.5" stroke="currentColor" strokeWidth="2" />
          <line x1="9.5" y1="4" x2="9.5" y2="20" stroke="currentColor" strokeWidth="2" />
        </symbol>

        <symbol id="i-code" viewBox="0 0 24 24">
          <polyline
            points="8 7 3 12 8 17"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <polyline
            points="16 7 21 12 16 17"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </symbol>

        <symbol id="i-quote" viewBox="0 0 24 24">
          <line x1="5" y1="5" x2="5" y2="19" stroke="currentColor" strokeWidth="3" strokeLinecap="round" />
          <line x1="11" y1="8" x2="20" y2="8" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          <line x1="11" y1="13" x2="18" y2="13" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
        </symbol>

        <symbol id="i-link" viewBox="0 0 24 24">
          <path
            d="M10 14a4 4 0 0 0 5.66 0l3-3a4 4 0 0 0-5.66-5.66l-1.5 1.5"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
          />
          <path
            d="M14 10a4 4 0 0 0-5.66 0l-3 3a4 4 0 0 0 5.66 5.66l1.5-1.5"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
          />
        </symbol>

        {/* Закладка (фича 032): флажок того же штриха, что у соседних. */}
        <symbol id="i-bookmark" viewBox="0 0 24 24">
          <path
            d="M6 3h12v18l-6-4-6 4z"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinejoin="round"
          />
        </symbol>

        <symbol id="i-tag" viewBox="0 0 24 24">
          <path
            d="M3 12V4a1 1 0 0 1 1-1h8l9 9-9 9-9-9z"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinejoin="round"
          />
          <circle cx="7.5" cy="7.5" r="1.4" fill="currentColor" />
        </symbol>

        <symbol id="i-text" viewBox="0 0 24 24">
          <path
            d="M5 6h14M12 6v13"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
          />
        </symbol>

        <symbol id="i-page" viewBox="0 0 24 24">
          <rect x="5" y="3" width="14" height="7" rx="1" fill="none" stroke="currentColor" strokeWidth="2" />
          <line
            x1="2"
            y1="13.5"
            x2="22"
            y2="13.5"
            stroke="currentColor"
            strokeWidth="2"
            strokeDasharray="3 3"
            strokeLinecap="round"
          />
          <rect x="5" y="17" width="14" height="4" rx="1" fill="none" stroke="currentColor" strokeWidth="2" />
        </symbol>

        <symbol id="i-layout" viewBox="0 0 24 24">
          <rect x="3" y="4" width="18" height="16" rx="1" fill="none" stroke="currentColor" strokeWidth="2" />
          <line x1="12" y1="4" x2="12" y2="20" stroke="currentColor" strokeWidth="2" />
        </symbol>

        <symbol id="i-shape" viewBox="0 0 24 24">
          <rect x="3" y="3" width="12" height="12" rx="1" fill="none" stroke="currentColor" strokeWidth="2" />
          <circle cx="15" cy="15" r="6" fill="none" stroke="currentColor" strokeWidth="2" />
        </symbol>

        <symbol id="i-braces" viewBox="0 0 24 24">
          <path
            d="M9 4H8a2 2 0 0 0-2 2v3a2 2 0 0 1-2 2 2 2 0 0 1 2 2v3a2 2 0 0 0 2 2h1"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <path
            d="M15 4h1a2 2 0 0 1 2 2v3a2 2 0 0 0 2 2 2 2 0 0 0-2 2v3a2 2 0 0 1-2 2h-1"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </symbol>

        <symbol id="i-data" viewBox="0 0 24 24">
          <ellipse cx="12" cy="6" rx="8" ry="3" fill="none" stroke="currentColor" strokeWidth="2" />
          <path
            d="M4 6v12c0 1.66 3.58 3 8 3s8-1.34 8-3V6"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          />
          <path d="M4 12c0 1.66 3.58 3 8 3s8-1.34 8-3" fill="none" stroke="currentColor" strokeWidth="2" />
        </symbol>

        <symbol id="i-tab" viewBox="0 0 24 24">
          <line x1="3" y1="12" x2="15" y2="12" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          <polyline
            points="11 8 15 12 11 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <line x1="20" y1="5" x2="20" y2="19" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
        </symbol>

        {/* Показ слайдов (фича 019, макет A1–A8): контуры макета в сетке 24. */}
        <symbol id="i-play" viewBox="0 0 24 24">
          <path d="M7.5 5.25v13.5l10.5-6.75z" fill="currentColor" />
        </symbol>

        <symbol id="i-start" viewBox="0 0 24 24">
          <path d="M6 5.25v13.5" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          <path d="M9.75 5.25v13.5l9.75-6.75z" fill="currentColor" />
        </symbol>

        <symbol id="i-caret" viewBox="0 0 24 24">
          <path
            d="M12 3.75v16.5M8.25 3.75h7.5M8.25 20.25h7.5"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
          />
        </symbol>

        <symbol id="i-down" viewBox="0 0 24 24">
          <polyline
            points="6 9 12 15 18 9"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </symbol>

        <symbol id="i-presenter" viewBox="0 0 24 24">
          <rect x="2.25" y="4.5" width="12" height="9" fill="none" stroke="currentColor" strokeWidth="2" />
          <rect x="16.5" y="4.5" width="5.25" height="4.5" fill="none" stroke="currentColor" strokeWidth="2" />
          <path
            d="M2.25 18h19.5M16.5 12.75h5.25"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
          />
        </symbol>

        <symbol id="i-fullscreen" viewBox="0 0 24 24">
          <path
            d="M3.75 9V3.75H9M15 3.75h5.25V9M20.25 15v5.25H15M9 20.25H3.75V15"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
          />
        </symbol>
      </defs>
    </svg>
  );
}

export interface IconProps {
  name: IconName;
  /** Сторона квадрата в пикселях; в макете 12–14px по месту. */
  size?: number;
  className?: string;
}

/**
 * Иконка из спрайта. Всегда декоративная: смысл несёт соседний текст или
 * `aria-label` кнопки, поэтому здесь `aria-hidden` без исключений — иначе
 * скринридер прочитал бы одно и то же дважды.
 */
export function Icon({ name, size = 14, className }: IconProps): React.JSX.Element {
  return (
    <svg
      className={className}
      style={{ width: size, height: size, flex: "0 0 auto" }}
      aria-hidden="true"
      focusable="false"
    >
      <use href={`#i-${name}`} />
    </svg>
  );
}
