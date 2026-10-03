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
import { useI18n, type Phrase, type Translate } from "../i18n/index.js";

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
  const { t } = useI18n();

  const [filter, setFilter] = useState("");
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  // Фраза, а не строка: отказ, уже показанный на экране, после смены языка
  // перерисовывается на новом (010 FR-014).
  const [notice, setNotice] = useState<Phrase | null>(null);

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
      const error = result.error;
      setNotice(() =>
        error.kind === "NoEditor"
          ? (tr: Translate) => tr("templates.gallery.noEditor")
          : (tr: Translate) =>
              tr("templates.gallery.insertFailed", { reason: error.reason(tr) }),
      );
    },
    [insertion, client, onClose],
  );

  const folderState = library.view?.folder_state ?? null;

  return (
    <div className="ts-overlay" role="presentation">
      <section className="ts-tpl" aria-label={t("templates.gallery.title")}>
        <div className="ts-tpl__head">
          <span className="ts-tpl__title">{t("templates.gallery.title")}</span>
          <span className="ts-tpl__path">vault/templates</span>{/* i18n-ignore: путь в vault */}
          <span className="ts-spacer" />
          <div className="ts-tpl__filter">
            <Icon name="search" size={12} />
            <input
              className="ts-tpl__filter-input"
              value={filter}
              placeholder={t("templates.gallery.filter")}
              aria-label={t("templates.gallery.filterLabel")}
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
              {t("templates.gallery.fromCurrent")}
            </button>
          )}
          <button
            type="button"
            className="ts-btn ts-btn--icon"
            aria-label={t("templates.gallery.close")}
            onClick={onClose}
          >
            <Icon name="x" size={14} />
          </button>
        </div>

        {folderState !== null && <FolderNotice state={folderState} />}
        {notice !== null && (
          <p className="ts-tpl__notice" role="alert">
            {notice(t)}
          </p>
        )}

        <div className="ts-tpl__body">
          <div className="ts-tpl__grid" role="listbox" aria-label={t("templates.gallery.list")}>
            {library.isLoading && (
              <p className="ts-tpl__empty">{t("templates.gallery.loading")}</p>
            )}

            {!library.isLoading && templates.length === 0 && (
              <EmptyLibrary state={folderState} />
            )}

            {!library.isLoading && templates.length > 0 && groups.length === 0 && (
              <p className="ts-tpl__empty">{t("templates.gallery.noMatch", { filter })}</p>
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

          <aside className="ts-tpl__side" aria-label={t("templates.gallery.selected")}>
            {selected === null ? (
              <p className="ts-tpl__empty">{t("templates.gallery.pickHint")}</p>
            ) : (
              <>
                <div className="ts-eyebrow">{selected.name}</div>
                {selected.group !== undefined && (
                  <p className="ts-tpl__side-path">{selected.group}</p>
                )}

                <span className="ts-tpl__side-label">{t("templates.gallery.apply")}</span>
                <button
                  type="button"
                  className="ts-btn ts-btn--accent"
                  disabled={onCreateNote === undefined}
                  onClick={() => onCreateNote?.(selected)}
                >
                  {t("templates.gallery.createNote")}
                </button>
                <button
                  type="button"
                  className="ts-btn"
                  disabled={insertion === null}
                  title={
                    insertion === null ? t("templates.gallery.insertDisabled") : undefined
                  }
                  onClick={() => void applyInsert(selected)}
                >
                  {t("templates.gallery.insert")}
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
                  {t("templates.gallery.openSource")}
                </button>

                {/* FR-006: заготовка правится тем же путём, что обычная
                    заметка, — отдельного редактора для неё нет, и кнопка выше
                    открывает её в том же редакторе. */}
                <p className="ts-tpl__side-hint">
                  {t("templates.gallery.sideHintBefore")}
                  <code>templates/</code>
                  {t("templates.gallery.sideHintAfter")}
                </p>
              </>
            )}
          </aside>
        </div>

        <div className="ts-tpl__foot">
          <span>{t("templates.gallery.count", { count: templates.length })}</span>
          <span className="ts-spacer" />
          <span>{t("templates.gallery.doubleClick")}</span>
          <span>{t("templates.gallery.escClose")}</span>
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
  const { t } = useI18n();
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
          {t("templates.folder.unreadableBefore")}
          <code>templates/</code>
          {t("templates.folder.unreadableAfter", {
            reason: describeFolderProblem(t, state.reason),
          })}
        </p>
      );
    case "NotWritable":
      return (
        <p className="ts-tpl__notice" role="note">
          {t("templates.folder.readOnlyBefore")}
          <code>templates/</code>
          {t("templates.folder.readOnlyAfter", {
            reason: describeFolderProblem(t, state.reason),
          })}
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
  const { t } = useI18n();
  if (state?.state === "Unreadable") {
    return <p className="ts-tpl__empty">{t("templates.empty.unreadable")}</p>;
  }
  return (
    <p className="ts-tpl__empty">
      {t("templates.empty.before")}
      <code>templates/</code>
      {t("templates.empty.after")}
    </p>
  );
}

/** Причина недоступности папки словами. */
export function describeFolderProblem(t: Translate, reason: FolderProblem): string {
  switch (reason) {
    case "AccessDenied":
      return t("templates.folderProblem.accessDenied");
    case "NotADirectory":
      return t("templates.folderProblem.notADirectory");
    case "VaultReadOnly":
      return t("templates.folderProblem.vaultReadOnly");
    case "Io":
      return t("templates.folderProblem.io");
    default: {
      const exhaustive: never = reason;
      return exhaustive;
    }
  }
}
