"use client";

import { useEffect, useRef, useState } from "react";

/**
 * Размеры колонок рабочей области.
 *
 * Макет (handoff § Spacing / radii) задаёт их жёстко: боковая панель 248px,
 * редактор и предпросмотр 1:1. Здесь эти же числа остаются значениями по
 * умолчанию — двигая границу, пользователь отклоняется от макета сам, а
 * приложение открывается таким, каким его нарисовали.
 *
 * Хранится не в настройках (фича 004), а в `localStorage`: ширина колонки —
 * состояние окна, а не пользовательская настройка. Панель настроек пишет через
 * платформенное хранилище, общее для всех окон, и ширина из одного окна
 * приезжала бы в другое; `localStorage` привязан к источнику и живёт ровно там,
 * где нужен. Ни одна из трёх оболочек не обязана его иметь — поэтому все
 * обращения обёрнуты, и отказ хранилища просто оставляет значения по умолчанию.
 */

/** Ширина боковой панели из макета — она же значение по умолчанию. */
export const SIDEBAR_DEFAULT_WIDTH = 248;
/** Уже этого дерево перестаёт читаться: имена файлов уходят в многоточие. */
export const SIDEBAR_MIN_WIDTH = 168;
export const SIDEBAR_MAX_WIDTH = 560;

/** Доля редактора в паре «редактор + предпросмотр». Макет — 1:1. */
export const SPLIT_DEFAULT = 0.5;
/** Границы доли: за ними одна из колонок вырождается в полосу. */
export const SPLIT_MIN = 0.2;
export const SPLIT_MAX = 0.8;

/** Шаг клавиатуры. Отдельный от мыши: стрелками двигают прицельно. */
const SIDEBAR_KEY_STEP = 16;
const SPLIT_KEY_STEP = 0.02;

/** Пауза перед записью: перетаскивание меняет размер каждый кадр. */
const PERSIST_DEBOUNCE_MS = 250;

const STORAGE_KEY = "typsidian.panes";

export interface PaneLayout {
  /** Ширина боковой панели в пикселях. */
  sidebarWidth: number;
  /** Доля редактора в остатке ширины, 0…1. */
  editorShare: number;
  /**
   * Дерево файлов скрыто.
   *
   * Отдельный признак, а не нулевая ширина: скрытая панель не рисуется вовсе,
   * и её ширина обязана дожить до возвращения. Нулём же выражалась бы «панель
   * шириной 0», из которой некуда вернуться.
   */
  sidebarHidden: boolean;
}

export interface PaneLayoutHandle extends PaneLayout {
  /**
   * Контейнер колонок. Перетаскивание считается от его рамки, а не от окна:
   * над `.ts-body` есть шапка и полоса уведомлений переменной высоты, и
   * отсчёт от окна разъехался бы с раскладкой на величину, которую здесь
   * неоткуда узнать.
   */
  bodyRef: React.RefObject<HTMLDivElement | null>;
  /** Переменные раскладки для инлайн-стиля контейнера. */
  style: React.CSSProperties;
  dragSidebar: (clientX: number) => void;
  stepSidebar: (direction: -1 | 1) => void;
  resetSidebar: () => void;
  /** Скрыть дерево файлов или вернуть его прежней ширины. */
  toggleSidebar: () => void;
  dragSplit: (clientX: number) => void;
  stepSplit: (direction: -1 | 1) => void;
  resetSplit: () => void;
}

const DEFAULTS: PaneLayout = {
  sidebarWidth: SIDEBAR_DEFAULT_WIDTH,
  editorShare: SPLIT_DEFAULT,
  sidebarHidden: false,
};

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

/**
 * Прочитать сохранённую раскладку. Чужое и испорченное значение не должно
 * ломать окно, поэтому каждое поле проверяется отдельно и подрезается по тем же
 * границам, что и перетаскивание: файл хранилища правится руками.
 */
function readStored(): PaneLayout {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (raw === null) return DEFAULTS;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) return DEFAULTS;
    const { sidebarWidth, editorShare, sidebarHidden } = parsed as Partial<PaneLayout>;
    return {
      sidebarWidth:
        typeof sidebarWidth === "number" && Number.isFinite(sidebarWidth)
          ? clamp(sidebarWidth, SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH)
          : DEFAULTS.sidebarWidth,
      editorShare:
        typeof editorShare === "number" && Number.isFinite(editorShare)
          ? clamp(editorShare, SPLIT_MIN, SPLIT_MAX)
          : DEFAULTS.editorShare,
      sidebarHidden:
        typeof sidebarHidden === "boolean" ? sidebarHidden : DEFAULTS.sidebarHidden,
    };
  } catch {
    // Хранилища нет или оно закрыто (приватное окно, запрет на данные сайта) —
    // это не отказ: колонки просто останутся такими, как в макете.
    return DEFAULTS;
  }
}

export function usePaneLayout(): PaneLayoutHandle {
  // Первый рендер — всегда значения по умолчанию, даже если в хранилище лежат
  // другие: `Workspace` рендерится и на сервере (apps/web — Next.js), где
  // хранилища нет, и прочитанное здесь разошлось бы с серверной разметкой.
  // Сохранённое приезжает эффектом, то есть уже на клиенте.
  const [layout, setLayout] = useState<PaneLayout>(DEFAULTS);
  const bodyRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    // Сохранённое применяется, только если раскладку ещё не трогали. Эффект
    // выполняется после первого кадра, и без этой проверки он затирал бы
    // границу, которую успели потянуть в этом промежутке, — окно дёргалось бы
    // назад под рукой.
    setLayout((current) =>
      current.sidebarWidth === DEFAULTS.sidebarWidth &&
      current.editorShare === DEFAULTS.editorShare &&
      current.sidebarHidden === DEFAULTS.sidebarHidden
        ? readStored()
        : current,
    );
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      try {
        window.localStorage.setItem(STORAGE_KEY, JSON.stringify(layout));
      } catch {
        // Запись могла не пройти (квота, приватное окно). Границу это не
        // ломает: раскладка уже применена, потеряется только её память.
      }
    }, PERSIST_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [layout]);

  const dragSidebar = (clientX: number): void => {
    const body = bodyRef.current;
    if (body === null) return;
    const { left } = body.getBoundingClientRect();
    setLayout((prev) => ({
      ...prev,
      sidebarWidth: clamp(clientX - left, SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH),
    }));
  };

  const stepSidebar = (direction: -1 | 1): void => {
    setLayout((prev) => ({
      ...prev,
      sidebarWidth: clamp(
        prev.sidebarWidth + direction * SIDEBAR_KEY_STEP,
        SIDEBAR_MIN_WIDTH,
        SIDEBAR_MAX_WIDTH,
      ),
    }));
  };

  const resetSidebar = (): void => {
    setLayout((prev) => ({ ...prev, sidebarWidth: SIDEBAR_DEFAULT_WIDTH }));
  };

  const toggleSidebar = (): void => {
    setLayout((prev) => ({ ...prev, sidebarHidden: !prev.sidebarHidden }));
  };

  const dragSplit = (clientX: number): void => {
    const body = bodyRef.current;
    if (body === null) return;
    const rect = body.getBoundingClientRect();
    setLayout((prev) => {
      // Гибкая часть начинается сразу за боковой панелью: сама ручка ширины не
      // занимает, поэтому её толщину вычитать не из чего. Скрытое дерево места
      // не занимает вовсе — иначе граница редактора и предпросмотра уезжала бы
      // на ширину панели, которой на экране нет.
      const start = rect.left + (prev.sidebarHidden ? 0 : prev.sidebarWidth);
      const width = rect.right - start;
      if (width <= 0) return prev;
      return {
        ...prev,
        editorShare: clamp((clientX - start) / width, SPLIT_MIN, SPLIT_MAX),
      };
    });
  };

  const stepSplit = (direction: -1 | 1): void => {
    setLayout((prev) => ({
      ...prev,
      editorShare: clamp(
        prev.editorShare + direction * SPLIT_KEY_STEP,
        SPLIT_MIN,
        SPLIT_MAX,
      ),
    }));
  };

  const resetSplit = (): void => {
    setLayout((prev) => ({ ...prev, editorShare: SPLIT_DEFAULT }));
  };

  // Размеры уезжают переменными, а не пропами колонок: ширину боковой панели
  // `.ts-sidebar` уже читает из `--ts-sidebar-w`, и вторая дорога к тому же
  // размеру развела бы макет с приложением. Приведение нужно потому, что
  // `CSSProperties` не знает пользовательских свойств.
  const style = {
    "--ts-sidebar-w": `${layout.sidebarWidth}px`,
    "--ts-editor-grow": layout.editorShare,
    "--ts-preview-grow": 1 - layout.editorShare,
  } as React.CSSProperties;

  return {
    ...layout,
    bodyRef,
    style,
    dragSidebar,
    stepSidebar,
    resetSidebar,
    toggleSidebar,
    dragSplit,
    stepSplit,
    resetSplit,
  };
}
