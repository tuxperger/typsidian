import type { Translate } from "../i18n/index.js";
import type {
  CompletionItem,
  GroupKind,
  LabelKind,
  MarkKind,
  NoDefinition,
} from "../wasm-core/typsidian_core.js";

// Значения ядра словами (фича 018). Правил здесь нет — только перевод.

export function groupTitle(t: Translate, kind: GroupKind): string {
  switch (kind) {
    case "Labels":
      return t("completion.group.labels");
    case "Sources":
      return t("completion.group.sources");
    case "Notes":
      return t("completion.group.notes");
  }
}

function labelKind(t: Translate, kind: LabelKind): string {
  switch (kind) {
    case "Heading":
      return t("completion.kind.heading");
    case "Figure":
      return t("completion.kind.figure");
    case "Table":
      return t("completion.kind.table");
    case "Equation":
      return t("completion.kind.equation");
    case "Other":
      return t("completion.kind.other");
  }
}

/** Тег вида, текст элемента и место справа для строки списка. */
export function itemFacts(
  t: Translate,
  item: CompletionItem,
): { tag: string; text: string; side: string; duplicate: boolean } {
  const detail = item.detail;
  switch (detail.kind) {
    case "Label":
      return {
        tag: labelKind(t, detail.label_kind),
        text: detail.text,
        side: detail.file ?? "",
        duplicate: detail.duplicate,
      };
    case "Source":
      return { tag: t("completion.kind.source"), text: detail.summary, side: "", duplicate: false };
    case "Note":
      return {
        tag: t("completion.kind.note"),
        text: "",
        side: detail.same_folder
          ? t("completion.thisFolder")
          : detail.folder === ""
            ? t("completion.rootFolder")
            : `/${detail.folder}/`,
        duplicate: false,
      };
  }
}

export function gotoReason(t: Translate, reason: NoDefinition): string {
  switch (reason.kind) {
    case "NoReferenceAtCaret":
      return t("completion.goto.noReference");
    case "LabelMissing":
      return t("completion.goto.labelMissing", { name: reason.name });
    case "LabelFromBuild":
      return t("completion.goto.labelFromBuild", { name: reason.name });
    case "NoteMissing":
      return t("completion.goto.noteMissing", { target: reason.target });
    case "OutsideVault":
      return t("completion.goto.outsideVault", { target: reason.target });
  }
}

/** Причина отметки и строка-описание (макет 1c). */
export function markText(t: Translate, kind: MarkKind): { title: string; detail: string } {
  switch (kind.kind) {
    case "MissingLabel":
      return {
        title: t("completion.mark.missingLabel", { name: kind.name }),
        detail: t("completion.mark.missingLabelDetail"),
      };
    case "MissingNote":
      return {
        title: t("completion.mark.missingNote", { target: kind.target }),
        detail: t("completion.mark.missingNoteDetail", { path: kind.create_path }),
      };
    case "LinkOutsideVault":
      return {
        title: t("completion.mark.outside", { target: kind.target }),
        detail: t("completion.mark.outsideDetail"),
      };
    case "DuplicateLabel": {
      const places = kind.others
        .map((other) =>
          other.path === undefined
            ? t("completion.mark.line", { line: other.line })
            : t("completion.mark.fileLine", { path: other.path, line: other.line }),
        )
        .join(", ");
      return {
        title: t("completion.mark.duplicate", { name: kind.name }),
        detail: t("completion.mark.duplicateDetail", { places }),
      };
    }
  }
}
