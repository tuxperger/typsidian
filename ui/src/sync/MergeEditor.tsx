"use client";

import { useMemo, useState } from "react";
import type {
  Conflict,
  Content,
  Hunk,
  Resolution,
} from "../wasm-core/typsidian_core.js";
import { useI18n, type I18n } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import { describeSyncError } from "./describe.js";
import type { SyncControl } from "./useSync.js";

export interface MergeEditorProps {
  sync: SyncControl;
  conflict: Conflict;
  onClose: () => void;
}

/** Чью версию взять в спорном участке. */
type Side = "ours" | "theirs" | "both";

/**
 * Редактор слияния (FR-016, FR-025, FR-034): своя версия, их версия и — когда
 * история её знает — общая; ниже результат.
 *
 * Совмещает версии ядро (`merge3`): сюда приходят уже готовые участки — общие
 * и спорные. Интерфейс только складывает результат из выбранных пользователем
 * сторон и отдаёт ядру решение; что с ним делать (записать файл, удалить
 * копию, закрыть получение), решает оно.
 */
export function MergeEditor({ sync, conflict, onClose }: MergeEditorProps): React.JSX.Element {
  const i18n = useI18n();
  const { t } = i18n;
  const busy = sync.activity !== null;
  useDismissOnEscape(busy ? null : onClose);

  const hunks: Hunk[] | null =
    conflict.auto_merge?.kind === "Conflicted" ? conflict.auto_merge.hunks : null;
  const cleanText = conflict.auto_merge?.kind === "Clean" ? conflict.auto_merge.text : null;
  const disputed = useMemo(
    () => (hunks ?? []).filter((hunk) => hunk.kind === "Conflict").length,
    [hunks],
  );

  const [sides, setSides] = useState<Side[]>(() => Array<Side>(disputed).fill("ours"));
  // Ручная правка результата. `null` — результат собирается из выбора сторон.
  const [manual, setManual] = useState<string | null>(null);

  const assembled = useMemo(() => {
    if (cleanText !== null) return cleanText;
    if (hunks === null) return null;
    let index = 0;
    return hunks
      .map((hunk) => {
        if (hunk.kind === "Same") return hunk.text;
        const side = sides[index++] ?? "ours";
        if (side === "ours") return hunk.ours;
        if (side === "theirs") return hunk.theirs;
        return hunk.ours + hunk.theirs;
      })
      .join("");
  }, [cleanText, hunks, sides]);

  const result = manual ?? assembled;
  const isSyncthing = conflict.source.kind === "Syncthing";
  const [copyPath, setCopyPath] = useState(() =>
    withSuffix(conflict.path, t("sync.merge.copySuffix")),
  );

  const resolve = async (resolution: Resolution): Promise<void> => {
    const done = await sync.run("resolve", async () => {
      await sync.client.resolve(conflict.id, resolution);
      return true;
    });
    if (done === true) onClose();
  };

  const abortPull = async (): Promise<void> => {
    const done = await sync.run("resolve", async () => {
      await sync.client.abortPull();
      return true;
    });
    if (done === true) onClose();
  };

  let disputedIndex = 0;

  return (
    <div
      className="ts-dialog ts-sync-merge"
      role="dialog"
      aria-modal="true"
      aria-label={t("sync.merge.title", { path: conflict.path })}
    >
      <div className="ts-dialog__head">
        <span className="ts-eyebrow">{t("sync.merge.title", { path: conflict.path })}</span>
        <p className="ts-dialog__hint">{sourceText(i18n, conflict)}</p>
      </div>

      <div className="ts-dialog__body">
        <div
          className="ts-sync-merge__columns"
          data-columns={conflict.base === undefined ? 2 : 3}
        >
          <VersionColumn i18n={i18n} title={t("sync.merge.ours")} content={conflict.ours} />
          {conflict.base !== undefined && (
            <VersionColumn i18n={i18n} title={t("sync.merge.base")} content={conflict.base} />
          )}
          <VersionColumn i18n={i18n} title={t("sync.merge.theirs")} content={conflict.theirs} />
        </div>

        {cleanText !== null && <p className="ts-dialog__hint">{t("sync.merge.autoClean")}</p>}

        {hunks !== null && disputed > 0 && (
          <section aria-label={t("sync.merge.disputed")}>
            <h3 className="ts-eyebrow">{t("sync.merge.disputedCount", { count: disputed })}</h3>
            <ol className="ts-sync-merge__hunks">
              {hunks.map((hunk, position) => {
                if (hunk.kind !== "Conflict") return null;
                const index = disputedIndex++;
                const chosen = sides[index] ?? "ours";
                const choose = (side: Side): void => {
                  setManual(null);
                  setSides((current) => current.map((old, at) => (at === index ? side : old)));
                };
                return (
                  <li key={position}>
                    <div className="ts-sync-merge__pair">
                      <pre>{hunk.ours === "" ? t("sync.merge.emptySide") : hunk.ours}</pre>
                      <pre>{hunk.theirs === "" ? t("sync.merge.emptySide") : hunk.theirs}</pre>
                    </div>
                    <div className="ts-seg" role="group" aria-label={t("sync.merge.choose")}>
                      {(["ours", "theirs", "both"] as const).map((side) => (
                        <button
                          key={side}
                          type="button"
                          className="ts-seg__item"
                          aria-pressed={manual === null && chosen === side}
                          onClick={() => choose(side)}
                        >
                          {side === "ours"
                            ? t("sync.merge.takeOurs")
                            : side === "theirs"
                              ? t("sync.merge.takeTheirs")
                              : t("sync.merge.takeBoth")}
                        </button>
                      ))}
                    </div>
                  </li>
                );
              })}
            </ol>
          </section>
        )}

        {result !== null ? (
          <label className="ts-field">
            <span className="ts-field__label">{t("sync.merge.result")}</span>
            <textarea
              className="ts-field__input ts-field__input--code ts-sync-merge__result"
              value={result}
              disabled={busy}
              spellCheck={false}
              onChange={(event) => setManual(event.target.value)}
            />
            {manual !== null && (
              <span className="ts-field__hint">{t("sync.merge.manualHint")}</span>
            )}
          </label>
        ) : (
          // Совмещать нечего: двоичный файл или одной из версий нет — только
          // выбор версии целиком (research §11).
          <p className="ts-dialog__hint">{t("sync.merge.wholeOnly")}</p>
        )}

        {isSyncthing && (
          <label className="ts-field">
            <span className="ts-field__label">{t("sync.merge.keepBothPath")}</span>
            <input
              className="ts-field__input ts-field__input--code"
              value={copyPath}
              disabled={busy}
              spellCheck={false}
              onChange={(event) => setCopyPath(event.target.value)}
            />
          </label>
        )}

        {sync.error !== null && (
          <p className="ts-dialog__error" role="alert">
            {describeSyncError(t, sync.error)}
          </p>
        )}
      </div>

      <div className="ts-dialog__foot">
        <button type="button" className="ts-btn" disabled={busy} onClick={onClose}>
          {t("common.close")}
        </button>
        {conflict.source.kind === "Pull" && (
          <button
            type="button"
            className="ts-btn ts-btn--danger"
            disabled={busy}
            title={t("sync.merge.abortPullHint")}
            onClick={() => void abortPull()}
          >
            {t("sync.merge.abortPull")}
          </button>
        )}
        <span className="ts-spacer" />
        {isSyncthing && (
          <button
            type="button"
            className="ts-btn"
            disabled={busy || copyPath.trim() === ""}
            onClick={() => void resolve({ kind: "KeepBothAsFile", new_path: copyPath.trim() })}
          >
            {t("sync.merge.keepBoth")}
          </button>
        )}
        <button
          type="button"
          className="ts-btn"
          disabled={busy}
          onClick={() => void resolve({ kind: "KeepOurs" })}
        >
          {t("sync.merge.keepOurs")}
        </button>
        <button
          type="button"
          className="ts-btn"
          disabled={busy}
          onClick={() => void resolve({ kind: "TakeTheirs" })}
        >
          {t("sync.merge.useTheirs")}
        </button>
        {result !== null && (
          <button
            type="button"
            className="ts-btn ts-btn--accent"
            disabled={busy}
            onClick={() => void resolve({ kind: "Merged", text: result })}
          >
            {t("sync.merge.saveMerged")}
          </button>
        )}
      </div>
    </div>
  );
}

function VersionColumn({
  i18n,
  title,
  content,
}: {
  i18n: I18n;
  title: string;
  content: Content;
}): React.JSX.Element {
  const { t } = i18n;
  return (
    <section className="ts-sync-merge__column" aria-label={title}>
      <h3 className="ts-eyebrow">{title}</h3>
      {content.kind === "Text" && <pre>{content.text}</pre>}
      {content.kind === "Binary" && (
        <p className="ts-dialog__hint">
          {t("sync.content.binary", { size: i18n.formatBytes(content.size) })}
        </p>
      )}
      {content.kind === "Missing" && <p className="ts-dialog__hint">{t("sync.content.missing")}</p>}
    </section>
  );
}

/** Откуда конфликт — одной строкой под заголовком. */
function sourceText(i18n: I18n, conflict: Conflict): string {
  const { t } = i18n;
  const source = conflict.source;
  switch (source.kind) {
    case "Pull":
      return t("sync.merge.sourcePull", { snapshot: source.theirs_snapshot.slice(0, 7) });
    case "Syncthing":
      return t("sync.merge.sourceSyncthing", { device: source.device, stamp: source.stamp });
    default: {
      const exhaustive: never = source;
      return exhaustive;
    }
  }
}

/** `папка/имя.typ` → `папка/имя<суффикс>.typ` — имя по умолчанию для второй версии. */
function withSuffix(path: string, suffix: string): string {
  const slash = path.lastIndexOf("/");
  const dot = path.lastIndexOf(".");
  return dot > slash + 1 ? `${path.slice(0, dot)}${suffix}${path.slice(dot)}` : `${path}${suffix}`;
}
