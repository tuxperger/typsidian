"use client";

import { useEffect, useRef } from "react";
import { EditorState } from "@codemirror/state";
import { Decoration, EditorView, keymap, type DecorationSet } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import type {
  Editability,
  IgnoreFile,
  IgnoreRefusal,
  LineRange,
} from "../wasm-core/typsidian_core.js";
import { DiffView } from "../editor/DiffView.js";
import { useI18n, type Translate } from "../i18n/index.js";
import { useDismissOnEscape } from "../workspace/useDismissOnEscape.js";
import { describeSyncError } from "./describe.js";
import { useIgnoreEditor, type IgnoreEditorState } from "./useIgnoreEditor.js";
import type { SyncControl } from "./useSync.js";

export interface IgnoreEditorProps {
  sync: SyncControl;
  file: IgnoreFile;
  onClose: () => void;
}

/** Имя файла на диске — не перевод, а путь. */
function fileName(file: IgnoreFile): string {
  switch (file) {
    case "GitIgnore":
      return ".gitignore";
    case "StIgnore":
      return ".stignore";
    default: {
      const exhaustive: never = file;
      return exhaustive;
    }
  }
}

function describeRefusal(t: Translate, reason: Exclude<Editability, "Editable"> | IgnoreRefusal): string {
  switch (reason) {
    case "NotText":
      return t("sync.rules.cannot.NotText");
    case "TooLarge":
      return t("sync.rules.cannot.TooLarge");
    case "NotAFile":
      return t("sync.rules.cannot.NotAFile");
    case "ReadOnly":
      return t("sync.rules.cannot.ReadOnly");
    case "HistoryBusy":
      return t("sync.rules.cannot.HistoryBusy");
    default: {
      const exhaustive: never = reason;
      return exhaustive;
    }
  }
}

/**
 * Редактор файла правил исключения (фича 012, contracts/frontend-ignore.md).
 *
 * Диалог показывает ответы ядра: документ, проверку текста и исход
 * сохранения. Подтверждение неполных правил, расхождение с диском и вопрос
 * при закрытии — экраны того же диалога, а не окна поверх него: правка под
 * ними остаётся на месте, и «вернуться» ничего не теряет.
 */
export function IgnoreEditor({ sync, file, onClose }: IgnoreEditorProps): React.JSX.Element {
  const { t } = useI18n();
  const editor = useIgnoreEditor(sync, file);
  const { document, stage } = editor;
  const name = fileName(file);
  const title = file === "GitIgnore" ? t("sync.rules.gitignore") : t("sync.rules.stignore");

  // «Сохранить» из вопроса при закрытии: закрываемся, когда запись состоялась.
  const closeAfterSave = useRef(false);
  useEffect(() => {
    if (closeAfterSave.current && editor.notice === "saved") onClose();
  }, [editor.notice, onClose]);
  useEffect(() => {
    if (stage.kind === "edit" && editor.notice !== "saved" && !editor.saving) {
      closeAfterSave.current = false;
    }
  }, [stage.kind, editor.notice, editor.saving]);

  const close = (): void => {
    if (editor.requestClose()) onClose();
  };
  useDismissOnEscape(stage.kind === "edit" ? close : editor.backToEdit);

  const editable = document?.editable === "Editable";

  return (
    <div
      className="ts-dialog ts-sync-rules-editor"
      role="dialog"
      aria-modal="true"
      aria-label={`${title} — ${name}`}
    >
      <div className="ts-dialog__head">
        <span className="ts-eyebrow">{title}</span>
        <code className="ts-sync-rules-editor__file">{name}</code>
      </div>

      {document === null ? (
        <div className="ts-dialog__body">
          {editor.error === null ? (
            <p role="status">{t("common.loading")}</p>
          ) : (
            <p className="ts-dialog__error" role="alert">
              {describeSyncError(t, editor.error)}
            </p>
          )}
        </div>
      ) : (
        <div className="ts-dialog__body">
          {/* Поле остаётся в дереве на всех экранах: скрыть его — значит
              потерять курсор и историю правок при «вернуться». */}
          <div hidden={stage.kind !== "edit"} className="ts-sync-rules-editor__main">
            <p className="ts-dialog__hint">
              {file === "GitIgnore" ? t("sync.rules.sharedGit") : t("sync.rules.perDeviceSt")}
            </p>
            {!document.in_effect && (
              <p className="ts-dialog__hint">
                {file === "GitIgnore"
                  ? t("sync.rules.notInEffectGit")
                  : t("sync.rules.notInEffectSt")}
              </p>
            )}
            {document.foreign_parent && (
              <p className="ts-dialog__hint">{t("sync.rules.foreignParent")}</p>
            )}

            {document.editable !== "Editable" && (
              <p className="ts-dialog__warn" role="alert">
                {describeRefusal(t, document.editable)}
              </p>
            )}

            {/* Без права записи файл виден, остальные причины содержимого не дают. */}
            {(editable || document.editable === "ReadOnly") && (
              <>
                {!document.exists && (
                  <p className="ts-dialog__hint" role="status">
                    {t("sync.rules.willCreate")}
                  </p>
                )}
                <RulesField
                  value={editor.text}
                  block={editor.check?.block}
                  blockLabel={t("sync.rules.blockLabel")}
                  ariaLabel={t("sync.rules.field", { file: name })}
                  readOnly={!editable || editor.saving}
                  onChange={editor.setText}
                  onSave={() => {
                    if (editor.canSave) editor.save();
                  }}
                />
                {editor.check?.block !== undefined && (
                  <p className="ts-dialog__hint">{t("sync.rules.blockHint")}</p>
                )}
              </>
            )}

            {editable && editor.check !== null && <CheckNotice editor={editor} file={file} />}

            <div role="status">
              {editor.saving && <p className="ts-dialog__hint">{t("sync.rules.saving")}</p>}
              {editor.notice === "saved" && (
                <p className="ts-dialog__hint">{t("sync.rules.saved")}</p>
              )}
              {editor.notice === "updatedOutside" && (
                <p className="ts-dialog__hint">{t("sync.rules.updatedOutside")}</p>
              )}
              {editor.notice === "staleOutside" && (
                <p className="ts-dialog__warn">{t("sync.rules.staleOutside")}</p>
              )}
            </div>
            {editor.refusal !== null && (
              <p className="ts-dialog__error" role="alert">
                {describeRefusal(t, editor.refusal)}
              </p>
            )}
            {editor.error !== null && (
              <p className="ts-dialog__error" role="alert">
                {describeSyncError(t, editor.error)}
              </p>
            )}
          </div>

          {stage.kind === "confirm" && (
            <div role="alertdialog" aria-label={t("sync.rules.confirmTitle")}>
              <p className="ts-dialog__warn">{t("sync.rules.confirmTitle")}</p>
              <MissingRules missing={stage.missing} file={file} />
            </div>
          )}

          {stage.kind === "diverged" && (
            <div role="alertdialog" aria-label={t("sync.rules.divergedTitle")}>
              <p className="ts-dialog__warn">{t("sync.rules.divergedTitle")}</p>
              <p>
                {stage.disk.exists
                  ? t("sync.rules.divergedExplain")
                  : t("sync.rules.deletedExplain")}
              </p>
              {stage.disk.exists && (
                <div className="ts-sync-diff">
                  <DiffView diff={stage.diff} />
                </div>
              )}
            </div>
          )}

          {stage.kind === "closing" && (
            <div role="alertdialog" aria-label={t("sync.rules.closeTitle")}>
              <p className="ts-dialog__warn">{t("sync.rules.closeTitle")}</p>
              <p>{t("sync.rules.closeExplain", { file: name })}</p>
            </div>
          )}
        </div>
      )}

      <div className="ts-dialog__foot">
        {stage.kind === "edit" && (
          <>
            <button type="button" className="ts-btn" onClick={close}>
              {t("common.close")}
            </button>
            <span className="ts-spacer" />
            <button
              type="button"
              className="ts-btn ts-btn--accent"
              disabled={!editor.canSave}
              onClick={editor.save}
            >
              {t("common.save")}
            </button>
          </>
        )}

        {stage.kind === "confirm" && (
          <>
            <button type="button" className="ts-btn" onClick={editor.backToEdit}>
              {t("sync.rules.backToEdit")}
            </button>
            <span className="ts-spacer" />
            <button
              type="button"
              className="ts-btn ts-btn--danger"
              disabled={editor.saving}
              onClick={editor.confirmIncomplete}
            >
              {t("sync.rules.confirmSave")}
            </button>
          </>
        )}

        {stage.kind === "diverged" && (
          <>
            <button type="button" className="ts-btn" onClick={editor.backToEdit}>
              {t("sync.rules.backToEdit")}
            </button>
            <span className="ts-spacer" />
            <button type="button" className="ts-btn" onClick={editor.takeDisk}>
              {stage.disk.exists ? t("sync.rules.takeDisk") : t("sync.rules.dropMine")}
            </button>
            <button
              type="button"
              className="ts-btn ts-btn--danger"
              disabled={editor.saving}
              onClick={editor.overwrite}
            >
              {stage.disk.exists ? t("sync.rules.overwrite") : t("sync.rules.recreate")}
            </button>
          </>
        )}

        {stage.kind === "closing" && (
          <>
            <button type="button" className="ts-btn" onClick={editor.backToEdit}>
              {t("sync.rules.backToEdit")}
            </button>
            <span className="ts-spacer" />
            <button type="button" className="ts-btn ts-btn--danger" onClick={onClose}>
              {t("sync.rules.discard")}
            </button>
            <button
              type="button"
              className="ts-btn ts-btn--accent"
              disabled={editor.saving}
              onClick={() => {
                closeAfterSave.current = true;
                editor.save();
              }}
            >
              {t("common.save")}
            </button>
          </>
        )}
      </div>
    </div>
  );
}

/** Каких правил приложения не хватает и чем это грозит (FR-010). */
function MissingRules({
  missing,
  file,
}: {
  missing: string[];
  file: IgnoreFile;
}): React.JSX.Element {
  const { t } = useI18n();
  return (
    <>
      <p>{t("sync.rules.missing")}</p>
      <pre className="ts-sync-rules">{missing.join("\n")}</pre>
      <p className="ts-dialog__hint">
        {file === "StIgnore" ? t("sync.rules.riskStIgnore") : t("sync.rules.riskGitIgnore")}
      </p>
    </>
  );
}

function CheckNotice({
  editor,
  file,
}: {
  editor: IgnoreEditorState;
  file: IgnoreFile;
}): React.JSX.Element {
  const { t } = useI18n();
  const missing = editor.check?.missing ?? [];
  if (missing.length === 0) {
    return (
      <p className="ts-dialog__hint" role="status">
        {t("sync.rules.complete")}
      </p>
    );
  }
  return (
    <div className="ts-sync__notice" data-tone="attention" role="status">
      <MissingRules missing={missing} file={file} />
      <div className="ts-sync__actions">
        <button
          type="button"
          className="ts-btn"
          disabled={editor.saving}
          onClick={editor.restoreBlock}
        >
          {t("sync.rules.restore")}
        </button>
      </div>
    </div>
  );
}

// ── Поле ───────────────────────────────────────────────────────────────────

const blockLine = Decoration.line({ class: "ts-sync-rules-editor__block" });

/**
 * Подсветка строк блока приложения; диапазон приходит от ядра. Первая строка
 * несёт подпись блока — её рисуют стили.
 */
function blockDecorations(
  state: EditorState,
  block: LineRange | undefined,
  label: string,
): DecorationSet {
  if (block === undefined) return Decoration.none;
  const first = block.start + 1;
  const last = Math.min(block.end, state.doc.lines);
  const ranges = [];
  for (let line = first; line <= last; line += 1) {
    const decoration =
      line === first
        ? Decoration.line({
            class: "ts-sync-rules-editor__block",
            attributes: { "data-label": label },
          })
        : blockLine;
    ranges.push(decoration.range(state.doc.line(line).from));
  }
  return Decoration.set(ranges);
}

interface RulesFieldProps {
  value: string;
  block: LineRange | undefined;
  blockLabel: string;
  ariaLabel: string;
  readOnly: boolean;
  onChange: (next: string) => void;
  onSave: () => void;
}

/**
 * Текстовое поле на весь файл (FR-002). CodeMirror, а не `<textarea>`: блок
 * приложения должен быть виден в самом тексте, а не рядом с ним.
 */
function RulesField({
  value,
  block,
  blockLabel,
  ariaLabel,
  readOnly,
  onChange,
  onSave,
}: RulesFieldProps): React.JSX.Element {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const viewRef = useRef<EditorView | null>(null);
  // Свежие значения для расширений, созданных один раз.
  const latest = useRef({ block, blockLabel, ariaLabel, readOnly, onChange, onSave });
  latest.current = { block, blockLabel, ariaLabel, readOnly, onChange, onSave };

  useEffect(() => {
    const host = hostRef.current;
    if (host === null) return undefined;
    const view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: value,
        extensions: [
          history(),
          keymap.of([
            {
              key: "Mod-s",
              preventDefault: true,
              run: () => {
                latest.current.onSave();
                return true;
              },
            },
            ...defaultKeymap,
            ...historyKeymap,
          ]),
          EditorView.contentAttributes.of(() => ({ "aria-label": latest.current.ariaLabel })),
          EditorState.changeFilter.of(() => !latest.current.readOnly),
          EditorView.decorations.of((current) =>
            blockDecorations(current.state, latest.current.block, latest.current.blockLabel),
          ),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) latest.current.onChange(update.state.doc.toString());
          }),
        ],
      }),
    });
    viewRef.current = view;
    view.focus();
    return () => {
      view.destroy();
      viewRef.current = null;
    };
    // Экземпляр один на жизнь поля: текст и подсветка обновляются ниже.
  }, []);

  // Текст сменился не набором (загрузка, версия с диска, возврат блока).
  useEffect(() => {
    const view = viewRef.current;
    if (view === null || view.state.doc.toString() === value) return;
    const wasReadOnly = latest.current.readOnly;
    latest.current.readOnly = false;
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: value } });
    latest.current.readOnly = wasReadOnly;
  }, [value]);

  // Диапазон блока пришёл от ядра — перерисовать подсветку.
  useEffect(() => {
    viewRef.current?.dispatch({});
  }, [block?.start, block?.end, blockLabel, ariaLabel]);

  return (
    <div
      ref={hostRef}
      className="ts-sync-rules-editor__field"
      data-readonly={readOnly ? "true" : undefined}
    />
  );
}
