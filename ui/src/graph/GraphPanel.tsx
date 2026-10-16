import { useState } from "react";
import { useI18n } from "../i18n/index.js";
import type { GraphGroup, GroupColor } from "../wasm-core/typsidian_core.js";
import type { GraphSettingsState, ResolvedGraphSettings } from "./useGraphSettings.js";

// Панель настроек графа (фича 025, T022, макет G1): фильтр, показ видов,
// группы, вид, силы. Меняется рабочая копия — холст и воркер откликаются сразу,
// запись в `vault.json` идёт с прореживанием (`useGraphSettings`).

export const GROUP_COLORS: readonly GroupColor[] = [
  "orange",
  "blue",
  "green",
  "amber",
  "red",
  "hivis",
  "steel",
];

type ForceKey = keyof ResolvedGraphSettings["forces"];
const FORCES: { key: ForceKey; label: "graph.forces.center" | "graph.forces.repel" | "graph.forces.link" | "graph.forces.distance" }[] = [
  { key: "center", label: "graph.forces.center" },
  { key: "repel", label: "graph.forces.repel" },
  { key: "link", label: "graph.forces.link" },
  { key: "distance", label: "graph.forces.distance" },
];

export function GraphPanel({ state }: { state: GraphSettingsState }): React.JSX.Element {
  const { t } = useI18n();
  const { settings, update, reset, save } = state;
  const [confirming, setConfirming] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState<number | null>(null);
  const [dragFrom, setDragFrom] = useState<number | null>(null);

  const number = (value: number): string => value.toFixed(2);
  const setGroups = (groups: GraphGroup[]): void => update({ groups });
  const patchGroup = (index: number, patch: Partial<GraphGroup>): void =>
    setGroups(settings.groups.map((group, i) => (i === index ? { ...group, ...patch } : group)));
  const move = (from: number, to: number): void => {
    if (to < 0 || to >= settings.groups.length || from === to) return;
    const next = [...settings.groups];
    const [item] = next.splice(from, 1);
    if (item !== undefined) next.splice(to, 0, item);
    setGroups(next);
  };
  const addGroup = (): void => {
    const used = new Set(settings.groups.map((group) => group.color));
    const color = GROUP_COLORS.find((candidate) => !used.has(candidate)) ?? "orange";
    setGroups([...settings.groups, { query: "", color }]);
  };

  return (
    <aside className="ts-graph__panel" aria-label={t("graph.panel.label")}>
      <section className="ts-graph__sec">
        <h3>{t("graph.filter.title")}</h3>
        <input
          className="ts-graph__input"
          type="text"
          value={settings.filter}
          aria-label={t("graph.filter.label")}
          placeholder={t("graph.filter.placeholder")}
          onChange={(event) => update({ filter: event.target.value })}
        />
        <Toggle
          label={t("graph.show.orphans")}
          checked={settings.show_orphans}
          onChange={(show_orphans) => update({ show_orphans })}
        />
        <Toggle
          label={t("graph.show.attachments")}
          checked={settings.show_attachments}
          onChange={(show_attachments) => update({ show_attachments })}
        />
        <Toggle
          label={t("graph.show.tags")}
          checked={settings.show_tags}
          onChange={(show_tags) => update({ show_tags })}
        />
      </section>

      <section className="ts-graph__sec">
        <h3>
          {t("graph.groups.title")}
          <button type="button" className="ts-graph__link" onClick={addGroup}>
            {t("graph.groups.add")}
          </button>
        </h3>
        {settings.groups.map((group, index) => (
          <div
            key={index}
            className="ts-graph__group"
            data-dragging={dragFrom === index}
            onDragOver={(event) => {
              if (dragFrom !== null) event.preventDefault();
            }}
            onDrop={() => {
              if (dragFrom !== null) move(dragFrom, index);
              setDragFrom(null);
            }}
          >
            <div className="ts-graph__group-row">
              <span
                className="ts-graph__grip"
                draggable
                title={t("graph.groups.drag")}
                aria-hidden="true"
                onDragStart={() => setDragFrom(index)}
                onDragEnd={() => setDragFrom(null)}
              >
                ⋮⋮
              </span>
              <button
                type="button"
                className="ts-graph__swatch"
                data-color={group.color}
                aria-label={`${t("graph.groups.color")}: ${t(`graph.color.${group.color}`)}`}
                aria-expanded={paletteOpen === index}
                onClick={() => setPaletteOpen(paletteOpen === index ? null : index)}
              />
              <input
                className="ts-graph__input ts-graph__input--code"
                type="text"
                value={group.query}
                aria-label={t("graph.groups.query")}
                placeholder={t("graph.groups.queryPlaceholder")}
                onChange={(event) => patchGroup(index, { query: event.target.value })}
              />
              <button
                type="button"
                className="ts-graph__icon"
                aria-label={t("graph.groups.up")}
                disabled={index === 0}
                onClick={() => move(index, index - 1)}
              >
                ↑
              </button>
              <button
                type="button"
                className="ts-graph__icon"
                aria-label={t("graph.groups.down")}
                disabled={index === settings.groups.length - 1}
                onClick={() => move(index, index + 1)}
              >
                ↓
              </button>
              <button
                type="button"
                className="ts-graph__icon"
                aria-label={t("graph.groups.remove")}
                onClick={() => {
                  setGroups(settings.groups.filter((_, i) => i !== index));
                  setPaletteOpen(null);
                }}
              >
                ×
              </button>
            </div>
            {paletteOpen === index && (
              <div className="ts-graph__palette" role="radiogroup" aria-label={t("graph.groups.color")}>
                {GROUP_COLORS.map((color) => (
                  <button
                    key={color}
                    type="button"
                    role="radio"
                    aria-checked={group.color === color}
                    aria-label={t(`graph.color.${color}`)}
                    className="ts-graph__swatch"
                    data-color={color}
                    onClick={() => {
                      patchGroup(index, { color });
                      setPaletteOpen(null);
                    }}
                  />
                ))}
              </div>
            )}
          </div>
        ))}
      </section>

      <section className="ts-graph__sec">
        <h3>{t("graph.view.title")}</h3>
        <Toggle
          label={t("graph.view.arrows")}
          checked={settings.arrows}
          onChange={(arrows) => update({ arrows })}
        />
        <Slider
          label={t("graph.view.labelZoom")}
          hint={t("graph.view.labelZoomValue", {
            percent: Math.round((0.5 + 1.75 * settings.label_zoom) * 100),
          })}
          value={settings.label_zoom}
          onChange={(label_zoom) => update({ label_zoom })}
        />
        <Slider
          label={t("graph.view.nodeSize")}
          hint={number(0.55 + 0.9 * settings.node_size)}
          value={settings.node_size}
          onChange={(node_size) => update({ node_size })}
        />
      </section>

      <section className="ts-graph__sec ts-graph__sec--last">
        <h3>{t("graph.forces.title")}</h3>
        {FORCES.map(({ key, label }) => (
          <Slider
            key={key}
            label={t(label)}
            hint={number(settings.forces[key])}
            value={settings.forces[key]}
            onChange={(value) => update({ forces: { ...settings.forces, [key]: value } })}
          />
        ))}
        {confirming ? (
          <div className="ts-graph__confirm" role="alertdialog" aria-label={t("graph.reset.ask")}>
            <span>{t("graph.reset.ask")}</span>
            <button
              type="button"
              className="ts-btn ts-btn--accent"
              onClick={() => {
                reset();
                setConfirming(false);
              }}
            >
              {t("graph.reset.yes")}
            </button>
            <button type="button" className="ts-btn" onClick={() => setConfirming(false)}>
              {t("graph.reset.no")}
            </button>
          </div>
        ) : (
          <div className="ts-graph__foot">
            <button type="button" className="ts-btn ts-btn--outline" onClick={() => setConfirming(true)}>
              {t("graph.reset")}
            </button>
            <span role="status" data-state={save}>
              {save === "saved" ? t("graph.saved") : save === "failed" ? t("graph.saveFailed") : ""}
            </span>
          </div>
        )}
      </section>
    </aside>
  );
}

function Toggle({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}): React.JSX.Element {
  return (
    <label className="ts-graph__toggle">
      <span>{label}</span>
      <input
        type="checkbox"
        className="ts-switch"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
    </label>
  );
}

function Slider({
  label,
  hint,
  value,
  onChange,
}: {
  label: string;
  hint: string;
  value: number;
  onChange: (value: number) => void;
}): React.JSX.Element {
  return (
    <label className="ts-graph__slider">
      <span>
        {label}
        <small>{hint}</small>
      </span>
      <input
        type="range"
        min={0}
        max={100}
        value={Math.round(value * 100)}
        onChange={(event) => onChange(Number(event.target.value) / 100)}
      />
    </label>
  );
}
