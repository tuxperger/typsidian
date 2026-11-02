"use client";

import { useEffect, useId, useMemo, useState } from "react";
import type { PlanStepState, PlanStepView, PlanVersionInfo, PlanView } from "../wasm-core/typsidian_core.js";
import { useI18n } from "../i18n/index.js";

// Колонка плана урока (фича 052, макет D, E, F). Раскладку по слоям отдаёт ядро
// (`x` — слой, `y` — место в слое); здесь карта рисуется SVG: слои сверху вниз, как в
// макете, зависимости стрелками. Тот же план — списком: он и на узком экране, и для
// чтения без карты (FR-009). Нажатие на шаг с якорем прокручивает ленту к его месту.

export interface LessonPlanProps {
  plan: PlanView;
  history: readonly PlanVersionInfo[];
  /** Лист узкого экрана: только список и кнопка закрытия. */
  sheet: boolean;
  onClose(): void;
  /** Прокрутить ленту к событию урока с этим номером. */
  onJump(anchor: number): void;
  /** План прежней версии; `undefined` — такой версии нет. */
  loadVersion(version: number): Promise<PlanView | undefined>;
}

/** Шаги в порядке слоёв: сверху вниз, внутри слоя — как в раскладке. */
export function orderedSteps(plan: PlanView): PlanStepView[] {
  return [...plan.steps].sort((a, b) => a.x - b.x || a.y - b.y);
}

/** Номер текущего шага (с единицы; 0 — текущего нет) и число шагов. */
export function planProgressOf(plan: PlanView): { current: number; total: number } {
  const steps = orderedSteps(plan);
  return { current: steps.findIndex((step) => step.state === "Current") + 1, total: steps.length };
}

const SLOT_W = 132;
const LAYER_H = 100;
const PAD_X = 12;
const PAD_TOP = 22;
const R = 7;
const R_CURRENT = 9;
const LINE_CHARS = 17;
const LINE_H = 13;
const MAX_LINES = 3;

/** Название в несколько коротких строк: SVG сам текст не переносит. */
function wrap(title: string): string[] {
  const lines: string[] = [];
  let line = "";
  for (const word of title.split(/\s+/).filter((item) => item !== "")) {
    const next = line === "" ? word : `${line} ${word}`;
    if (next.length > LINE_CHARS && line !== "") {
      lines.push(line);
      line = word;
    } else {
      line = next;
    }
  }
  if (line !== "") lines.push(line);
  if (lines.length > MAX_LINES) {
    const kept = lines.slice(0, MAX_LINES);
    kept[MAX_LINES - 1] = `${kept[MAX_LINES - 1] ?? ""}…`;
    return kept;
  }
  return lines;
}

interface Place {
  step: PlanStepView;
  cx: number;
  cy: number;
}

function placesOf(plan: PlanView): Map<string, Place> {
  const layerSize = new Map<number, number>();
  for (const step of plan.steps) layerSize.set(step.x, (layerSize.get(step.x) ?? 0) + 1);
  const places = new Map<string, Place>();
  for (const step of plan.steps) {
    // Слой с меньшим числом шагов стоит по центру карты.
    const offset = (plan.rows - (layerSize.get(step.x) ?? 1)) / 2;
    places.set(step.id, {
      step,
      cx: PAD_X + (offset + step.y + 0.5) * SLOT_W,
      cy: PAD_TOP + step.x * LAYER_H,
    });
  }
  return places;
}

function statusClass(step: PlanStepView, goal: string): string {
  const parts = [`ts-plan__node--${step.state}`];
  if (step.added) parts.push("ts-plan__node--added");
  if (step.id === goal) parts.push("ts-plan__node--goal");
  return parts.join(" ");
}

function PlanMap({
  plan,
  onJump,
}: {
  plan: PlanView;
  onJump(anchor: number): void;
}): React.JSX.Element {
  const { t } = useI18n();
  const markerId = useId();
  const places = useMemo(() => placesOf(plan), [plan]);
  const width = Math.max(plan.rows, 1) * SLOT_W + PAD_X * 2;
  const height = PAD_TOP + Math.max(plan.cols, 1) * LAYER_H;

  return (
    <div className="ts-plan__scroll" tabIndex={0} role="group" aria-label={t("lessons.plan.mapLabel")}>
      <svg
        className="ts-plan__svg"
        viewBox={`0 0 ${width} ${height}`}
        width={width}
        height={height}
        role="presentation"
      >
        <defs>
          <marker
            id={markerId}
            viewBox="0 0 8 8"
            refX="7"
            refY="4"
            markerWidth="6"
            markerHeight="6"
            orient="auto"
          >
            <path className="ts-plan__arrow" d="M0,0 L8,4 L0,8 z" />
          </marker>
        </defs>
        <g className="ts-plan__edges" markerEnd={`url(#${markerId})`}>
          {plan.edges.map((edge) => {
            const from = places.get(edge.from);
            const to = places.get(edge.to);
            if (from === undefined || to === undefined) return null;
            const y1 = from.cy + (from.step.state === "Current" ? R_CURRENT : R);
            const y2 = to.cy - (to.step.state === "Current" ? R_CURRENT : R) - 4;
            const mid = (y1 + y2) / 2;
            return (
              <path
                key={`${edge.from}>${edge.to}`}
                d={`M${from.cx},${y1} C${from.cx},${mid} ${to.cx},${mid} ${to.cx},${y2}`}
              />
            );
          })}
        </g>
        {[...places.values()].map(({ step, cx, cy }) => {
          const lines = wrap(step.title);
          const state = t(`lessons.plan.state.${step.state}`);
          const note = step.state === "Current" ? t("lessons.plan.now") : step.added ? t("lessons.plan.added") : null;
          const anchor = step.anchor ?? undefined;
          const jumpable = anchor !== undefined;
          return (
            <g
              key={step.id}
              className={`ts-plan__node ${statusClass(step, plan.goal)}`}
              {...(jumpable
                ? {
                    role: "button",
                    tabIndex: 0,
                    "aria-label": t("lessons.plan.jump", { title: step.title }),
                    onClick: () => onJump(anchor),
                    onKeyDown: (event: React.KeyboardEvent) => {
                      if (event.key === "Enter" || event.key === " ") {
                        event.preventDefault();
                        onJump(anchor);
                      }
                    },
                  }
                : { role: "img", "aria-label": `${step.title}, ${state}` })}
            >
              <title>{`${step.title} — ${state}`}</title>
              <rect
                className="ts-plan__hit"
                x={cx - SLOT_W / 2 + 4}
                y={cy - R_CURRENT - 4}
                width={SLOT_W - 8}
                height={LINE_H * (lines.length + (note === null ? 0 : 1)) + R_CURRENT * 2 + 14}
                rx={6}
              />
              <circle className="ts-plan__dot" cx={cx} cy={cy} r={step.state === "Current" ? R_CURRENT : R} />
              <text className="ts-plan__label" x={cx} y={cy + R_CURRENT + 14} textAnchor="middle">
                {lines.map((line, index) => (
                  <tspan key={index} x={cx} dy={index === 0 ? 0 : LINE_H}>
                    {line}
                  </tspan>
                ))}
              </text>
              {note !== null && (
                <text
                  className="ts-plan__sub"
                  x={cx}
                  y={cy + R_CURRENT + 14 + LINE_H * lines.length}
                  textAnchor="middle"
                >
                  {note}
                </text>
              )}
            </g>
          );
        })}
      </svg>
    </div>
  );
}

function PlanList({
  plan,
  onJump,
}: {
  plan: PlanView;
  onJump(anchor: number): void;
}): React.JSX.Element {
  const { t } = useI18n();
  const steps = useMemo(() => orderedSteps(plan), [plan]);
  const numberOf = useMemo(() => new Map(steps.map((step, index) => [step.id, index + 1])), [steps]);
  return (
    <>
      <ol className="ts-plan__list" aria-label={t("lessons.plan.listLabel")}>
        {steps.map((step) => {
          const after = step.after
            .map((id) => numberOf.get(id))
            .filter((value): value is number => value !== undefined)
            .join(", ");
          const side = [
            step.known ? t("lessons.plan.fromProbe") : null,
            after !== "" ? t("lessons.plan.after", { steps: after }) : null,
            step.added ? t("lessons.plan.added") : null,
            step.id === plan.goal ? t("lessons.plan.goal") : null,
          ]
            .filter((item): item is string => item !== null)
            .join(" · ");
          const anchor = step.anchor ?? undefined;
          const body = (
            <>
              <span
                className={`ts-plan__mark ts-plan__mark--${step.state}${step.added ? " ts-plan__mark--added" : ""}${
                  step.id === plan.goal ? " ts-plan__mark--goal" : ""
                }`}
                aria-hidden="true"
              />
              <span className="ts-plan__title">
                {step.title}
                <span className="ts-plan__sr"> — {t(`lessons.plan.state.${step.state}`)}</span>
              </span>
              {side !== "" && <span className="ts-plan__dep">{side}</span>}
            </>
          );
          return (
            <li
              key={step.id}
              className="ts-plan__item"
              aria-current={step.state === "Current" ? "step" : undefined}
              data-state={step.state}
            >
              {anchor !== undefined ? (
                <button
                  type="button"
                  className="ts-plan__row"
                  aria-label={`${step.title}. ${t("lessons.plan.jump", { title: step.title })}`}
                  onClick={() => onJump(anchor)}
                >
                  {body}
                </button>
              ) : (
                <div className="ts-plan__row">{body}</div>
              )}
            </li>
          );
        })}
      </ol>
      <p className="ts-lesson__hint ts-plan__hint">{t("lessons.plan.hint")}</p>
    </>
  );
}

function History({
  history,
  current,
  onOpen,
  onBack,
}: {
  history: readonly PlanVersionInfo[];
  current: number;
  onOpen(version: number): void;
  onBack(): void;
}): React.JSX.Element {
  const { t, lang } = useI18n();
  const format = new Intl.DateTimeFormat(lang, { hour: "2-digit", minute: "2-digit" });
  return (
    <div className="ts-plan__history">
      <div className="ts-plan__history-head">
        <b>{t("lessons.plan.history.title")}</b>
        <button type="button" className="ts-btn ts-btn--sm" onClick={onBack}>
          {t("lessons.plan.history.back")}
        </button>
      </div>
      <ul className="ts-plan__versions">
        {[...history].reverse().map((item) => (
          <li key={item.version} className="ts-plan__ver">
            <span className="ts-plan__when">{format.format(new Date(item.at))}</span>
            <span className="ts-plan__ver-text">
              {t("lessons.plan.history.version", { version: item.version, steps: item.steps })}
              {item.reason != null && item.reason !== "" && (
                <small className="ts-plan__reason">{item.reason}</small>
              )}
            </span>
            <button
              type="button"
              className="ts-btn ts-btn--sm"
              aria-label={t("lessons.plan.history.openLabel", { version: item.version })}
              aria-current={item.version === current ? "true" : undefined}
              onClick={() => onOpen(item.version)}
            >
              {t("lessons.plan.history.open")}
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

export function LessonPlan({ plan, history, sheet, onClose, onJump, loadVersion }: LessonPlanProps): React.JSX.Element {
  const { t } = useI18n();
  const [mode, setMode] = useState<"map" | "list">("map");
  const [historyOpen, setHistoryOpen] = useState(false);
  const [opened, setOpened] = useState<PlanView | null>(null);
  const [failed, setFailed] = useState(false);

  // Учитель принял новую версию — смотрим текущую.
  useEffect(() => {
    setOpened(null);
    setFailed(false);
  }, [plan.version]);

  const shown = opened ?? plan;
  const effective = sheet ? "list" : mode;
  const total = history.length > 0 ? history.length : plan.version;

  const open = (version: number): void => {
    setFailed(false);
    if (version === plan.version) {
      setOpened(null);
      setHistoryOpen(false);
      return;
    }
    void loadVersion(version)
      .then((loaded) => {
        if (loaded === undefined) {
          setFailed(true);
          return;
        }
        setOpened(loaded);
        setHistoryOpen(false);
      })
      .catch(() => setFailed(true));
  };

  return (
    <aside
      className={sheet ? "ts-plan ts-plan--sheet" : "ts-plan"}
      aria-label={t("lessons.plan.title")}
      {...(sheet ? { role: "dialog", "aria-modal": true } : {})}
    >
      <div className="ts-plan__head">
        <b>{t("lessons.plan.title")}</b>
        {!sheet && (
          <span className="ts-lesson__seg" role="group" aria-label={t("lessons.plan.view")}>
            {(["map", "list"] as const).map((item) => (
              <button key={item} type="button" aria-pressed={mode === item} onClick={() => setMode(item)}>
                {item === "map" ? t("lessons.plan.map") : t("lessons.plan.list")}
              </button>
            ))}
          </span>
        )}
        {sheet && (
          <button type="button" className="ts-btn ts-btn--sm" autoFocus onClick={onClose}>
            {t("lessons.plan.close")}
          </button>
        )}
      </div>

      {historyOpen ? (
        <History
          history={history}
          current={shown.version}
          onOpen={open}
          onBack={() => setHistoryOpen(false)}
        />
      ) : (
        <>
          {opened !== null && (
            <div className="ts-plan__viewing" role="status">
              {t("lessons.plan.history.viewing", { version: opened.version, total })}{" "}
              <button type="button" className="ts-plan__linkbtn" onClick={() => setOpened(null)}>
                {t("lessons.plan.history.current")}
              </button>
            </div>
          )}
          <div className="ts-plan__body">
            {effective === "map" ? (
              <PlanMap plan={shown} onJump={onJump} />
            ) : (
              <PlanList plan={shown} onJump={onJump} />
            )}
          </div>
          {shown.changes > 0 && (
            <div className="ts-plan__chg">
              <b>{t("lessons.plan.changed")}</b>{" "}
              {shown.last_reason != null && shown.last_reason !== ""
                ? shown.last_reason
                : t("lessons.plan.noReason")}{" "}
              <button type="button" className="ts-plan__linkbtn" onClick={() => setHistoryOpen(true)}>
                {t("lessons.plan.history")}
              </button>
            </div>
          )}
          {failed && (
            <p className="ts-lesson__bad ts-plan__fail" role="alert">
              {t("lessons.plan.history.failed")}
            </p>
          )}
        </>
      )}
    </aside>
  );
}

export type { PlanStepState };
