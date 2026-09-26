"use client";

import { useCallback, useMemo, useState } from "react";
import type {
  FolderProblem,
  PreviewState,
  Template,
  TemplateFolderState,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { Icon } from "../icons/Icons.js";
import { TemplateCard } from "./TemplateCard.js";
import {
  groupTemplates,
  templateKey,
  type TemplateLibraryState,
} from "./useTemplateLibrary.js";
import { insertTemplate, type InsertionTarget } from "./insertion/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";

// Галерея заготовок (T022, T023) — экран «Шаблоны и сниппеты» макета
// «Typsidian Утилиты».
//
// # Чего здесь нет по сравнению с макетом
//
// Панели «переменные» (`номер`, `заголовок`, `автор`, `дата`) справа. Это
// параметризованные шаблоны с плейсхолдерами, и спека выносит их **за скоуп**
// прямым текстом: «параметризованные шаблоны с переменными/плейсхолдерами — это
// отдельная более сложная фича на будущее». Содержимое вставляется дословно
// (FR-032, инвариант 4), поэтому поля ввода над ним показывали бы подстановку,
// которой не происходит, — это хуже их отсутствия.
//
// Счётчиков «исп. 23 раза» тоже нет: приложение не ведёт статистику применения
// и завести её значило бы новое персистентное состояние ради подписи.
//
// # Момент построения превью
//
// Галерея открывается по списку файлов, без единого превью; превью строятся
// только для карточек, попавших в область видимости (research.md §4). Отсюда
// SC-003: список пригоден для выбора сразу, не дожидаясь ни одного чтения.

export interface TemplateGalleryProps {
  client: VaultClient;
  /**
   * Состав библиотеки — пропом, а не своим вызовом хука.
   *
   * Тот же состав нужен второму уровню меню «+», и он уже читается выше по
   * дереву. Свой `useTemplateLibrary` здесь означал бы второй запрос к ядру на
   * каждое событие файла и две копии состояния, расходящиеся между собой.
   */
  library: TemplateLibraryState;
  /**
   * Цель вставки — порт `InsertionTarget`. `null`, когда заметка не открыта:
   * вставлять некуда, и кнопка честно недоступна.
   */
  insertion?: InsertionTarget | null;
  /** Создать заметку из заготовки (FR-020, FR-021). */
  onCreateNote?: (template: Template) => void;
  /** Открыть файл заготовки в редакторе — обычной заметкой (FR-006). */
  onOpenSource?: (path: string) => void;
  /** Сохранить открытую заметку как заготовку (US2). */
  onSaveCurrentNote?: () => void;
  onClose: () => void;
}

export function TemplateGallery({
  client,
  library,
  insertion = null,
  onCreateNote,
  onOpenSource,
  onSaveCurrentNote,
  onClose,
}: TemplateGalleryProps): React.JSX.Element {
  useDismissOnEscape(onClose);

  const [filter, setFilter] = useState("");
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const templates = useMemo(
    () => library.view?.templates ?? [],
    [library.view],
  );
  const groups = useMemo(
    () => groupTemplates(templates, filter),
    [templates, filter],
  );
  const selected = useMemo(
    () => templates.find((template) => template.path === selectedPath) ?? null,
    [templates, selectedPath],
  );

  const buildPreview = useCallback(
    (template: Template): Promise<PreviewState> =>
      client.templatePreview(template),
    [client],
  );

  const applyInsert = useCallback(
    async (template: Template) => {
      if (!insertion) return;
      // Вызов идёт **через порт**, даже при единственной реализации: это
      // граница гейта принципа V (contracts/insertion-target.md).
      const result = await insertTemplate(insertion, client, template);
      if (result.ok) {
        setNotice(null);
        onClose();
        return;
      }
      setNotice(
        result.error.kind === "NoEditor"
          ? "Вставлять некуда: заметка не открыта."
          : `Вставить не удалось: ${result.error.reason}.`,
      );
    },
    [insertion, client, onClose],
  );

  const folderState = library.view?.folder_state ?? null;

  return (
    <div className="ts-overlay" role="presentation">
      <section className="ts-tpl" aria-label="Шаблоны и сниппеты">
        <div className="ts-tpl__head">
          <span className="ts-tpl__title">Шаблоны и сниппеты</span>
          <span className="ts-tpl__path">vault/templates</span>
          <span className="ts-spacer" />
          <div className="ts-tpl__filter">
            <Icon name="search" size={12} />
            <input
              className="ts-tpl__filter-input"
              value={filter}
              placeholder="фильтр…"
              aria-label="Фильтр по имени заготовки"
              onChange={(event) => setFilter(event.target.value)}
            />
          </div>
          {onSaveCurrentNote && (
            <button
              type="button"
              className="ts-btn"
              disabled={folderState !== null && !acceptsWrites(folderState)}
              onClick={onSaveCurrentNote}
            >
              <Icon name="plus" size={12} />
              Из текущей заметки
            </button>
          )}
          <button
            type="button"
            className="ts-btn ts-btn--icon"
            aria-label="Закрыть галерею"
            onClick={onClose}
          >
            <Icon name="x" size={14} />
          </button>
        </div>

        {folderState !== null && <FolderNotice state={folderState} />}
        {notice !== null && (
          <p className="ts-tpl__notice" role="alert">
            {notice}
          </p>
        )}

        <div className="ts-tpl__body">
          <div className="ts-tpl__grid" role="listbox" aria-label="Заготовки">
            {library.isLoading && (
              <p className="ts-tpl__empty">Читаем библиотеку…</p>
            )}

            {!library.isLoading && templates.length === 0 && (
              <EmptyLibrary state={folderState} />
            )}

            {!library.isLoading && templates.length > 0 && groups.length === 0 && (
              <p className="ts-tpl__empty">
                По запросу «{filter}» заготовок нет.
              </p>
            )}

            {groups.map((group) => (
              <div key={group.group ?? "/"} className="ts-tpl__group">
                <div className="ts-tpl__group-head">
                  <Icon name="folder" size={12} />
                  {group.group ?? "templates"}
                  <span className="ts-tpl__group-meta">
                    {group.templates.length}
                  </span>
                </div>
                <div className="ts-tpl__cards">
                  {group.templates.map((template) => (
                    <TemplateCard
                      key={templateKey(template)}
                      template={template}
                      isSelected={template.path === selectedPath}
                      buildPreview={buildPreview}
                      onSelect={(picked) => {
                        setNotice(null);
                        setSelectedPath(picked.path);
                      }}
                      onApply={(picked) => {
                        if (insertion) void applyInsert(picked);
                        else onCreateNote?.(picked);
                      }}
                    />
                  ))}
                </div>
              </div>
            ))}
          </div>

          <aside className="ts-tpl__side" aria-label="Выбранная заготовка">
            {selected === null ? (
              <p className="ts-tpl__empty">
                Выберите заготовку — её можно развернуть в новую заметку или
                вставить в текущую позицию курсора.
              </p>
            ) : (
              <>
                <div className="ts-eyebrow">{selected.name}</div>
                {selected.group !== undefined && (
                  <p className="ts-tpl__side-path">{selected.group}</p>
                )}

                <span className="ts-tpl__side-label">применить</span>
                <button
                  type="button"
                  className="ts-btn ts-btn--accent"
                  disabled={onCreateNote === undefined}
                  onClick={() => onCreateNote?.(selected)}
                >
                  Создать заметку по шаблону
                </button>
                <button
                  type="button"
                  className="ts-btn"
                  disabled={insertion === null}
                  title={
                    insertion === null
                      ? "Откройте заметку, чтобы вставить заготовку в курсор"
                      : undefined
                  }
                  onClick={() => void applyInsert(selected)}
                >
                  Вставить в курсор
                </button>
                <button
                  type="button"
                  className="ts-btn"
                  disabled={onOpenSource === undefined}
                  onClick={() => {
                    onOpenSource?.(selected.path);
                    onClose();
                  }}
                >
                  Открыть исходник шаблона
                </button>

                {/* FR-006: заготовка правится тем же путём, что обычная
                    заметка, — отдельного редактора для неё нет, и кнопка выше
                    открывает её в том же редакторе. */}
                <p className="ts-tpl__side-hint">
                  Заготовка — обычная заметка в папке <code>templates/</code>:
                  она видна в дереве, участвует в поиске и в графе ссылок.
                </p>
              </>
            )}
          </aside>
        </div>

        <div className="ts-tpl__foot">
          <span>{templates.length} заготовок</span>
          <span className="ts-spacer" />
          <span>двойное нажатие — применить</span>
          <span>Esc закрыть</span>
        </div>
      </section>
    </div>
  );
}

/** Принимает ли папка запись — то же правило, что в ядре. */
function acceptsWrites(state: TemplateFolderState): boolean {
  return state.state === "Present" || state.state === "Absent";
}

/**
 * Состояние папки словами (FR-004, FR-009, FR-016).
 *
 * Четыре состояния **не схлопываются**: «папки ещё нет» — штатное состояние
 * нового vault, которое чинится первым же сохранением, а «папка не читается» —
 * проблема. Показать их одинаково значило бы превратить штатный путь в ошибку.
 */
function FolderNotice({
  state,
}: {
  state: TemplateFolderState;
}): React.JSX.Element | null {
  switch (state.state) {
    case "Present":
      return null;
    case "Absent":
      // Не сообщение об ошибке и даже не предупреждение: пустая галерея всё
      // объясняет сама (см. `EmptyLibrary`).
      return null;
    case "Unreadable":
      return (
        <p className="ts-tpl__notice ts-tpl__notice--warn" role="alert">
          Папку <code>templates/</code> не удалось прочитать:{" "}
          {describeFolderProblem(state.reason)}. Остальное приложение работает
          как обычно.
        </p>
      );
    case "NotWritable":
      return (
        <p className="ts-tpl__notice" role="note">
          Папка <code>templates/</code> доступна только на чтение:{" "}
          {describeFolderProblem(state.reason)}. Заготовки можно применять, но
          сохранить новую нельзя.
        </p>
      );
    default: {
      const exhaustive: never = state;
      return exhaustive;
    }
  }
}

/** Пустая библиотека — штатное состояние, а не ошибка (FR-009, FR-016). */
function EmptyLibrary({
  state,
}: {
  state: TemplateFolderState | null;
}): React.JSX.Element {
  if (state?.state === "Unreadable") {
    return (
      <p className="ts-tpl__empty">
        Пока папка не читается, показать заготовки нечем.
      </p>
    );
  }
  return (
    <p className="ts-tpl__empty">
      Заготовок пока нет. Папка <code>templates/</code> — обычная папка vault:
      положите в неё заметку любым способом, и она появится здесь. Папка
      создастся сама при первом сохранении заготовки из приложения.
    </p>
  );
}

/** Причина недоступности папки словами. */
export function describeFolderProblem(reason: FolderProblem): string {
  switch (reason) {
    case "AccessDenied":
      return "нет прав доступа";
    case "NotADirectory":
      return "по этому пути лежит файл, а не папка";
    case "VaultReadOnly":
      return "весь vault открыт только на чтение";
    case "Io":
      return "отказ чтения с диска";
    default: {
      const exhaustive: never = reason;
      return exhaustive;
    }
  }
}
