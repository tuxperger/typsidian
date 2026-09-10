import type { Metadata } from "next";
import type { ReactNode } from "react";
import { IBM_Plex_Mono, IBM_Plex_Sans, Source_Serif_4 } from "next/font/google";
import "@typsidian/ui/styles/app.css";

/**
 * IBM Plex Sans/Mono — шрифты дизайн-системы «Спецформа» (handoff §
 * Typography). `next/font` кладёт файлы в статику приложения и раздаёт с его
 * же origin: обращений к Google Fonts во время работы не происходит, чего
 * требует FR-042 фичи 004 («ни одного сетевого обращения»).
 *
 * Кириллица в subsets не для полноты, а по необходимости: весь интерфейс
 * русскоязычный.
 */
const plexSans = IBM_Plex_Sans({
  subsets: ["latin", "cyrillic"],
  weight: ["400", "500", "600", "700"],
  variable: "--font-ibm-plex-sans",
  display: "swap",
});

const plexMono = IBM_Plex_Mono({
  subsets: ["latin", "cyrillic"],
  weight: ["400", "500", "600"],
  variable: "--font-ibm-plex-mono",
  display: "swap",
});

/**
 * Source Serif 4 — только для текста «бумаги» в предпросмотре (handoff §
 * Typography: «Preview pane body copy»). Тем же путём, что и Plex: макет тянет
 * его ссылкой на Google Fonts, но FR-042 это запрещает.
 *
 * Начертание не указано намеренно: это переменный шрифт, и документация Next
 * (`01-getting-started/13-fonts.md`) рекомендует не фиксировать `weight` для
 * таких — иначе вместо одного файла со всей осью придёт набор статических.
 * Курсив нужен для подписей к рисункам и формул, поэтому он в `style`.
 */
const sourceSerif = Source_Serif_4({
  subsets: ["latin", "cyrillic"],
  style: ["normal", "italic"],
  variable: "--font-source-serif",
  display: "swap",
});

export const metadata: Metadata = {
  title: "Typsidian",
  description: "Локальный редактор заметок на Typst",
};

export default function RootLayout({
  children,
}: {
  children: ReactNode;
}): React.JSX.Element {
  return (
    <html
      lang="ru"
      className={`${plexSans.variable} ${plexMono.variable} ${sourceSerif.variable}`}
    >
      <body>{children}</body>
    </html>
  );
}
