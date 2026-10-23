"use client";

import { useState } from "react";
import type {
  BookmarkPlace,
  BookmarkRow,
  BookmarkTarget,
  ViewEntry,
} from "../wasm-core/typsidian_core.js";
import { Icon, type IconName } from "../icons/Icons.js";
import { useI18n, type Translate } from "../i18n/index.js";
import { useContextMenu, type ContextMenuItem } from "../menu/ContextMenu.js";
import type { BookmarksState } from "./useBookmarks.js";

// Раздел «Закладки» над деревом (фича 032, макет B1). Что где лежит и
// доступна ли цель — ответ ядра; здесь показ, перетаскивание, Alt+↑/↓ и меню.

export interface BookmarksPanelProps {
  bookmarks: BookmarksState;
  readOnly: boolean;
  activePath: string | null;
  onOpen: (row: BookmarkRow) => void;
}

/** Где лежит закладка: группа (`null` — верхний уровень) и позиция в ней. */
interface Slot {
  group: string | null;
  index: number;
}

const DRAG_TYPE = "application/x-typsidian-bookmark";

function iconOf(target: BookmarkTarget): IconName {
  switch (target.kind) {
    case "Note":
      return "file";
    case "Section":
      return "heading";
    case "Search":
      return "search";
    case "Folder":
      return "folder";
  }
}

/** Пояснение под именем: заметка раздела, условия поиска, «папка». */
function subtitle(t: Translate, target: BookmarkTarget): string {
  switch (target.kind) {
    case "Note":
      return "";
    case "Section":
      return t("bookmarks.sub.section", {
        note: target.path.replace(/^.*\//, "").replace(/\.typ$/, ""),
      });
    case "Search": {
      const parts = [t("bookmarks.sub.search")];
      if (target.query.case_sensitive) parts.push("Aa");
      if (target.query.regex) parts.push("regex");
      if (target.query.links_to) {
        parts.push(
          t("bookmarks.sub.linksTo", {
            note: target.query.links_to.replace(/^.*\//, "").replace(/\.typ$/, ""),
          }),
        );
      }
      return parts.join(" · ");
    }
    case "Folder":
      return t("bookmarks.sub.folder");
  }
}

function pathOf(target: BookmarkTarget): string | null {
  return target.kind === "Search" ? null : target.path;
}

export function BookmarksPanel({
  bookmarks,
  readOnly,
  activePath,
  onOpen,
}: BookmarksPanelProps): React.JSX.Element {
  const { t } = useI18n();
  const contextMenu = useContextMenu();
  const [open, setOpen] = useState(true);
  const [editing, setEditing] = useState<{ id: string; group: boolean; value: string } | null>(null);
  const [dragged, setDragged] = useState<{ id: string; from: Slot } | null>(null);
  const [dropAt, setDropAt] = useState<Slot | null>(null);

  const entries: ViewEntry[] = bookmarks.view?.entries ?? [];
  const count = bookmarks.view?.count ?? 0;
  const groups = entries.flatMap((entry) =>
    entry.type === "Group" ? [{ id: entry.id, name: entry.name }] : [],
  );

  /** Позиция для ядра: в том же списке вниз — с поправкой на вынутую закладку. */
  const placeFor = (from: Slot, to: Slot): BookmarkPlace => {
    const index = from.group === to.group && from.index < to.index ? to.index - 1 : to.index;
    return to.group === null ? { index } : { group: to.group, index };
  };

  const drop = (to: Slot) => {
    if (dragged !== null) void bookmarks.move(dragged.id, placeFor(dragged.from, to));
    setDragged(null);
    setDropAt(null);
  };

  const step = (id: string, slot: Slot, delta: -1 | 1, size: number) => {
    const index = slot.index + delta;
    if (index < 0 || index >= size) return;
    void bookmarks.move(id, slot.group === null ? { index } : { group: slot.group, index });
  };

  const rowMenu = (row: BookmarkRow, slot: Slot, size: number): ContextMenuItem[] => {
    const id = row.bookmark.id;
    return [
      { label: t("bookmarks.menu.open"), run: () => onOpen(row) },
      {
        label: t("bookmarks.menu.rename"),
        disabled: readOnly,
        run: () => setEditing({ id, group: false, value: row.bookmark.name }),
      },
      ...groups
        .filter((group) => group.id !== slot.group)
        .map((group) => ({
          label: t("bookmarks.menu.toGroup", { name: group.name }),
          disabled: readOnly,
          run: () => void bookmarks.move(id, { group: group.id, index: 1_000_000 }),
        })),
      ...(slot.group !== null
        ? [
            {
              label: t("bookmarks.menu.outOfGroup"),
              disabled: readOnly,
              run: () => void bookmarks.move(id, { index: 1_000_000 }),
            },
          ]
        : []),
      {
        label: t("bookmarks.menu.up"),
        shortcut: "Alt+↑",
        disabled: readOnly || slot.index === 0,
        run: () => step(id, slot, -1, size),
      },
      {
        label: t("bookmarks.menu.down"),
        shortcut: "Alt+↓",
        disabled: readOnly || slot.index >= size - 1,
        run: () => step(id, slot, 1, size),
      },
      { kind: "separator" },
      {
        label: t("bookmarks.menu.remove"),
        danger: true,
        disabled: readOnly,
        run: () => void bookmarks.remove(id),
      },
    ];
  };

  const renameField = (id: string, group: boolean, value: string) => (
    <input
      id={`ts-bookmark-name-${id}`}
      className="ts-field__input ts-bookmarks__edit"
      aria-label={t("bookmarks.menu.rename")}
      value={value}
      autoFocus
      onChange={(event) => setEditing({ id, group, value: event.target.value })}
      onBlur={() => setEditing(null)}
      onKeyDown={(event) => {
        if (event.key === "Escape") setEditing(null);
        if (event.key === "Enter") {
          if (group) void bookmarks.groupRename(id, value);
          else void bookmarks.rename(id, value);
          setEditing(null);
        }
      }}
    />
  );

  const renderRow = (row: BookmarkRow, slot: Slot, size: number) => {
    const { bookmark, state } = row;
    const sub = subtitle(t, bookmark.target);
    const active = activePath !== null && pathOf(bookmark.target) === activePath;
    const missing = state === "Missing";
    const isDropAt = dropAt?.group === slot.group && dropAt.index === slot.index;
    if (editing !== null && !editing.group && editing.id === bookmark.id) {
      return (
        <li key={bookmark.id} className="ts-bookmarks__row">
          {renameField(bookmark.id, false, editing.value)}
        </li>
      );
    }
    return (
      <li
        key={bookmark.id}
        className={isDropAt ? "ts-bookmarks__row ts-bookmarks__row--drop" : "ts-bookmarks__row"}
        draggable={!readOnly}
        onDragStart={(event) => {
          event.dataTransfer.setData(DRAG_TYPE, bookmark.id);
          event.dataTransfer.effectAllowed = "move";
          setDragged({ id: bookmark.id, from: slot });
        }}
        onDragEnd={() => {
          setDragged(null);
          setDropAt(null);
        }}
        onDragOver={(event) => {
          if (dragged === null) return;
          event.preventDefault();
          setDropAt(slot);
        }}
        onDrop={(event) => {
          event.preventDefault();
          drop(slot);
        }}
        onContextMenu={(event) => contextMenu.open(event, rowMenu(row, slot, size))}
      >
        <button
          type="button"
          className={[
            "ts-bookmarks__item",
            active ? "ts-bookmarks__item--active" : "",
            missing ? "ts-bookmarks__item--missing" : "",
          ].join(" ")}
          aria-current={active ? "true" : undefined}
          onClick={() => {
            if (missing) contextMenu.notify(t("bookmarks.missingHint"));
            else onOpen(row);
          }}
          onKeyDown={(event) => {
            if (event.altKey && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
              event.preventDefault();
              if (!readOnly) step(bookmark.id, slot, event.key === "ArrowUp" ? -1 : 1, size);
            }
          }}
        >
          <Icon name={iconOf(bookmark.target)} size={12} />
          <span className="ts-bookmarks__text">
            <span className="ts-bookmarks__name">{bookmark.name}</span>
            {sub !== "" && <span className="ts-bookmarks__sub">{sub}</span>}
          </span>
          {missing && <span className="ts-bookmarks__flag">{t("bookmarks.missing")}</span>}
          {state === "SectionMissing" && (
            <span className="ts-bookmarks__flag">{t("bookmarks.sectionMissing")}</span>
          )}
        </button>
        {missing && !readOnly && (
          <button
            type="button"
            className="ts-btn ts-btn--quiet ts-bookmarks__remove"
            aria-label={t("bookmarks.menu.remove")}
            onClick={() => void bookmarks.remove(bookmark.id)}
          >
            <Icon name="x" size={11} />
          </button>
        )}
      </li>
    );
  };

  return (
    <section className="ts-bookmarks" aria-label={t("bookmarks.title")}>
      <div className="ts-bookmarks__head">
        <button
          type="button"
          className="ts-bookmarks__toggle"
          aria-expanded={open}
          onClick={() => setOpen((value) => !value)}
        >
          <Icon name="chevron" size={10} />
          {t("bookmarks.title")}
          <span className="ts-bookmarks__count">· {count}</span>
        </button>
        <span className="ts-spacer" />
        {open && !readOnly && (
          <button
            type="button"
            className="ts-btn ts-btn--quiet ts-bookmarks__new"
            onClick={() =>
              void bookmarks.groupCreate(t("bookmarks.newGroup")).then((view) => {
                const created = [...(view?.entries ?? [])].reverse().find((e) => e.type === "Group");
                if (created?.type === "Group") {
                  setEditing({ id: created.id, group: true, value: created.name });
                }
              })
            }
          >
            <Icon name="plus" size={10} />
            {t("bookmarks.group")}
          </button>
        )}
      </div>
      {open &&
        (entries.length === 0 ? (
          <p className="ts-bookmarks__empty">{t("bookmarks.empty")}</p>
        ) : (
          <ul className="ts-bookmarks__list">
            {entries.map((entry, index) => {
              if (entry.type === "Item") return renderRow(entry.row, { group: null, index }, entries.length);
              const slot: Slot = { group: null, index };
              return (
                <li
                  key={entry.id}
                  className="ts-bookmarks__group"
                  draggable={!readOnly}
                  onDragStart={(event) => {
                    event.dataTransfer.setData(DRAG_TYPE, entry.id);
                    setDragged({ id: entry.id, from: slot });
                  }}
                  onDragEnd={() => {
                    setDragged(null);
                    setDropAt(null);
                  }}
                >
                  <div
                    className="ts-bookmarks__ghead"
                    onDragOver={(event) => {
                      if (dragged === null) return;
                      event.preventDefault();
                      setDropAt({ group: entry.id, index: entry.rows.length });
                    }}
                    onDrop={(event) => {
                      event.preventDefault();
                      // Группа на группу — перестановка, закладка — в конец группы.
                      if (dragged !== null && groups.some((group) => group.id === dragged.id)) {
                        drop(slot);
                      } else {
                        drop({ group: entry.id, index: entry.rows.length });
                      }
                    }}
                    onContextMenu={(event) =>
                      contextMenu.open(event, [
                        {
                          label: t("bookmarks.menu.rename"),
                          disabled: readOnly,
                          run: () => setEditing({ id: entry.id, group: true, value: entry.name }),
                        },
                        { kind: "separator" },
                        {
                          label: t("bookmarks.menu.deleteGroup"),
                          danger: true,
                          disabled: readOnly,
                          run: () => void bookmarks.groupDelete(entry.id),
                        },
                      ])
                    }
                  >
                    {editing !== null && editing.group && editing.id === entry.id ? (
                      renameField(entry.id, true, editing.value)
                    ) : (
                      <button
                        type="button"
                        className="ts-bookmarks__gtoggle"
                        aria-expanded={!entry.collapsed}
                        onClick={() => void bookmarks.groupToggle(entry.id)}
                      >
                        <span
                          className={
                            entry.collapsed
                              ? "ts-bookmarks__chev ts-bookmarks__chev--closed"
                              : "ts-bookmarks__chev"
                          }
                        >
                          <Icon name="chevron" size={10} />
                        </span>
                        {entry.name}
                        <span className="ts-bookmarks__count">· {entry.rows.length}</span>
                      </button>
                    )}
                  </div>
                  {!entry.collapsed && (
                    <ul className="ts-bookmarks__list ts-bookmarks__list--inner">
                      {entry.rows.map((row, inner) =>
                        renderRow(row, { group: entry.id, index: inner }, entry.rows.length),
                      )}
                    </ul>
                  )}
                </li>
              );
            })}
            <li
              className={
                dropAt?.group === null && dropAt.index === entries.length
                  ? "ts-bookmarks__tail ts-bookmarks__row--drop"
                  : "ts-bookmarks__tail"
              }
              onDragOver={(event) => {
                if (dragged === null) return;
                event.preventDefault();
                setDropAt({ group: null, index: entries.length });
              }}
              onDrop={(event) => {
                event.preventDefault();
                drop({ group: null, index: entries.length });
              }}
            />
          </ul>
        ))}
      {bookmarks.error !== null && (
        <p role="alert" className="ts-bookmarks__error">
          {bookmarks.error === "ReadOnly" ? t("bookmarks.error.readOnly") : t("bookmarks.error.io")}
        </p>
      )}
    </section>
  );
}
