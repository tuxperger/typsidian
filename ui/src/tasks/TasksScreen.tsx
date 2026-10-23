"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  Condition,
  DueFilter,
  DueState,
  TaskFilter,
  TaskGrouping,
  TaskRow,
  TaskState,
  TasksView,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import type { OpenNoteEditors } from "../note-panel/links/openNoteEditors.js";
import { Icon } from "../icons/Icons.js";
import { useI18n, type MessageKey } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";

// Окно «Задачи» (фича 033, макет T1, T2). Разбор, индекс, фильтры, группы и
// отметку делает ядро (`tasks`); здесь — фильтры как ввод, список, клавиатура.

export interface TasksScreenProps {
  client: VaultClient;
  /** Правки открытых заметок идут через их редакторы (013). */
  editors: OpenNoteEditors;
  /** Папки vault — для выбора папки. */
  folders: readonly string[];
  /** Номер изменений vault: список перечитывается (FR-005). */
  revision: unknown;
  onOpen: (path: string, line: number) => void;
  onClose: () => void;
}

/** «Сегодня» по часам устройства — `ГГГГ-ММ-ДД`. */
function today(): string {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

const STATES: { id: TaskState; label: MessageKey }[] = [
  { id: "Open", label: "tasks.state.open" },
  { id: "Done", label: "tasks.state.done" },
  { id: "All", label: "tasks.state.all" },
];

const DUES: { id: DueFilter; label: MessageKey }[] = [
  { id: "Any", label: "tasks.due.any" },
  { id: "Overdue", label: "tasks.due.overdue" },
  { id: "Today", label: "tasks.due.today" },
  { id: "Week", label: "tasks.due.week" },
  { id: "NoDue", label: "tasks.due.none" },
];

const DUE_GROUPS: Record<string, MessageKey> = {
  Overdue: "tasks.group.overdue",
  Today: "tasks.group.today",
  Week: "tasks.group.week",
  Later: "tasks.group.later",
  NoDue: "tasks.group.none",
};

export function TasksScreen({
  client,
  editors,
  folders,
  revision,
  onOpen,
  onClose,
}: TasksScreenProps): React.JSX.Element {
  const { t, formatNumber } = useI18n();
  useDismissOnEscape(onClose);
  const [state, setState] = useState<TaskState>("Open");
  const [due, setDue] = useState<DueFilter>("Any");
  const [group, setGroup] = useState<TaskGrouping>("ByNote");
  const [folder, setFolder] = useState("");
  const [conditionText, setConditionText] = useState("");
  const [conditions, setConditions] = useState<Condition[]>([]);
  const [view, setView] = useState<TasksView | null>(null);
  const [selected, setSelected] = useState(0);
  const [stale, setStale] = useState<string | null>(null);
  const [round, setRound] = useState(0);
  const listRef = useRef<HTMLDivElement | null>(null);

  // Строка условий: `ключ:значение` — тот же разбор, что в поиске 020.
  useEffect(() => {
    let alive = true;
    const timer = window.setTimeout(() => {
      if (conditionText.trim() === "") {
        setConditions([]);
        return;
      }
      void client.properties
        .parseConditions(conditionText, [])
        .then((parsed) => alive && setConditions(parsed.conditions))
        .catch(() => undefined);
    }, 250);
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [client, conditionText]);

  const filter = useMemo<TaskFilter>(
    () => ({
      state,
      due,
      group,
      conditions,
      ...(folder === "" ? {} : { folder }),
    }),
    [state, due, group, conditions, folder],
  );

  useEffect(() => {
    let alive = true;
    void client.tasks
      .view(filter, today())
      .then((next) => alive && setView(next))
      .catch(() => alive && setView(null));
    return () => {
      alive = false;
    };
  }, [client, filter, revision, round]);

  const rows = useMemo<TaskRow[]>(
    () => (view?.groups ?? []).flatMap((entry) => entry.rows),
    [view],
  );

  const toggle = useCallback(
    async (row: TaskRow) => {
      const { task } = row;
      const request = { path: task.path, line: task.line, raw: task.raw };
      const outcome = await client.tasks.toggle(request, editors.textOf(task.path));
      switch (outcome.kind) {
        case "EditorEdit":
          if (!editors.applyEdit(task.path, outcome.edit).ok) setStale(t("tasks.failed"));
          break;
        case "Written":
          setStale(null);
          break;
        case "Stale":
          setStale(t("tasks.stale", { text: task.text, note: row.note_title }));
          break;
        case "ReadOnly":
          setStale(t("tasks.readOnly"));
          break;
        case "Failed":
          setStale(t("tasks.failed"));
          break;
      }
      setRound((value) => value + 1);
    },
    [client, editors, t],
  );

  const onKeyDown = (event: React.KeyboardEvent) => {
    const row = rows[selected];
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelected((index) => Math.min(rows.length - 1, index + 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelected((index) => Math.max(0, index - 1));
    } else if (event.key === " " && row !== undefined) {
      event.preventDefault();
      void toggle(row);
    } else if (event.key === "Enter" && row !== undefined) {
      event.preventDefault();
      onOpen(row.task.path, row.task.line);
    }
  };

  useEffect(() => {
    listRef.current
      ?.querySelector<HTMLElement>(`[data-task-index="${selected}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  const dueLabel = (row: TaskRow): string | null => {
    if (row.task.due === undefined) return null;
    const [, month, day] = row.task.due.split("-");
    const short = `${day}.${month}`;
    const states: Partial<Record<DueState, string>> = {
      Overdue: t("tasks.badge.overdue", { date: short }),
      Today: t("tasks.badge.today"),
    };
    return states[row.due_state] ?? short;
  };

  let index = -1;
  return (
    <div className="ts-overlay" role="presentation">
      <section className="ts-search ts-tasks" aria-label={t("tasks.title")} onKeyDown={onKeyDown}>
        <div className="ts-search__query">
          <span className="ts-history__title">{t("tasks.title")}</span>
          <span className="ts-search__hit-meta">
            {view !== null &&
              t("tasks.summary", {
                open: formatNumber(view.open),
                overdue: formatNumber(view.overdue),
              })}
          </span>
          <span className="ts-spacer" />
          <span className="ts-tasks__seg" role="group" aria-label={t("tasks.groupBy")}>
            {(["ByNote", "ByDue"] as const).map((id) => (
              <button
                key={id}
                type="button"
                className="ts-chip"
                aria-pressed={group === id}
                onClick={() => setGroup(id)}
              >
                {id === "ByNote" ? t("tasks.byNote") : t("tasks.byDue")}
              </button>
            ))}
          </span>
          <button
            type="button"
            className="ts-btn ts-btn--ghost"
            onClick={onClose}
            aria-label={t("tasks.close")}
          >
            <Icon name="x" size={14} />
          </button>
        </div>

        <div className="ts-tasks__filters">
          <span className="ts-eyebrow">{t("tasks.filter.state")}</span>
          <span className="ts-tasks__seg" role="group" aria-label={t("tasks.filter.state")}>
            {STATES.map((option) => (
              <button
                key={option.id}
                type="button"
                className="ts-chip"
                aria-pressed={state === option.id}
                onClick={() => setState(option.id)}
              >
                {t(option.label)}
              </button>
            ))}
          </span>
          <span className="ts-eyebrow">{t("tasks.filter.due")}</span>
          <span className="ts-tasks__seg" role="group" aria-label={t("tasks.filter.due")}>
            {DUES.map((option) => (
              <button
                key={option.id}
                type="button"
                className="ts-chip"
                aria-pressed={due === option.id}
                onClick={() => setDue(option.id)}
              >
                {t(option.label)}
              </button>
            ))}
          </span>
        </div>
        <div className="ts-tasks__filters">
          <label className="ts-eyebrow" htmlFor="ts-tasks-folder">
            {t("tasks.filter.folder")}
          </label>
          <select
            id="ts-tasks-folder"
            className="ts-field__input ts-tasks__folder"
            value={folder}
            onChange={(event) => setFolder(event.target.value)}
          >
            <option value="">{t("tasks.filter.anyFolder")}</option>
            {folders.map((path) => (
              <option key={path} value={path}>
                {path}
              </option>
            ))}
          </select>
          <input
            id="ts-tasks-conditions"
            className="ts-field__input ts-tasks__conditions"
            value={conditionText}
            placeholder={t("tasks.filter.conditions")}
            aria-label={t("tasks.filter.conditions")}
            onChange={(event) => setConditionText(event.target.value)}
          />
        </div>

        {stale !== null && (
          <div className="ts-replace__panel ts-replace__panel--warn" role="status">
            <span className="ts-replace__title">{stale}</span>
          </div>
        )}

        <div className="ts-search__results ts-tasks__list" ref={listRef} role="list">
          {view !== null && rows.length === 0 && <p className="ts-search__empty">{t("tasks.empty")}</p>}
          {(view?.groups ?? []).map((entry) => (
            <div key={entry.key} className="ts-tasks__group">
              <div className="ts-tasks__ghead">
                <span className="ts-tasks__gtitle">
                  {group === "ByNote" ? entry.title : t(DUE_GROUPS[entry.key] ?? "tasks.group.none")}
                </span>
                <span className="ts-search__hit-meta">
                  {group === "ByNote" && entry.folder !== "" ? `${entry.folder}/ · ` : ""}
                  {formatNumber(entry.rows.length)}
                </span>
              </div>
              {entry.rows.map((row) => {
                index += 1;
                const at = index;
                const label = dueLabel(row);
                return (
                  <div
                    key={`${row.task.path}:${row.task.line}`}
                    role="listitem"
                    data-task-index={at}
                    className={[
                      "ts-tasks__row",
                      at === selected ? "ts-tasks__row--on" : "",
                      row.task.done ? "ts-tasks__row--done" : "",
                      row.task.parent !== undefined && group === "ByNote" ? "ts-tasks__row--sub" : "",
                    ].join(" ")}
                  >
                    <input
                      id={`ts-task-${at}`}
                      type="checkbox"
                      className="ts-tasks__box"
                      checked={row.task.done}
                      aria-label={row.task.text}
                      onChange={() => {
                        setSelected(at);
                        void toggle(row);
                      }}
                    />
                    <button
                      type="button"
                      className="ts-tasks__text"
                      onClick={() => onOpen(row.task.path, row.task.line)}
                      onFocus={() => setSelected(at)}
                    >
                      {row.task.text}
                      {group === "ByDue" && (
                        <span className="ts-tasks__where">
                          {t("tasks.where", { note: row.note_title, line: row.task.line })}
                        </span>
                      )}
                    </button>
                    {label !== null && (
                      <span className={`ts-tasks__due ts-tasks__due--${row.due_state}`}>{label}</span>
                    )}
                    {group === "ByNote" && (
                      <span className="ts-tasks__line">{t("tasks.line", { line: row.task.line })}</span>
                    )}
                  </div>
                );
              })}
            </div>
          ))}
        </div>
        <div className="ts-search__hints ts-replace__foot">
          <span>{t("tasks.hints")}</span>
        </div>
      </section>
    </div>
  );
}
