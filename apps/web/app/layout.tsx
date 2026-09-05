import type { Metadata } from "next";
import type { ReactNode } from "react";
import { IBM_Plex_Mono, IBM_Plex_Sans } from "next/font/google";
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
    <html lang="ru" className={`${plexSans.variable} ${plexMono.variable}`}>
      <body>{children}</body>
    </html>
  );
}
