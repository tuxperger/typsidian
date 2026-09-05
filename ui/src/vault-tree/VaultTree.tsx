import { useMemo, useState } from "react";
import type { Note } from "../wasm-core/typsidian_core.js";
import { buildTree, type TreeFolder, type TreeNode } from "./buildTree.js";

export interface VaultTreeProps {
  notes: Note[];
  onOpenNote: (note: Note) => void;
}

/**
 * Дерево vault (FR-003, FR-006). Clarifications: «пригодность для навигации» —
 * верхний уровень виден и раскрываем сразу, содержимое вложенной папки
 * появляется по раскрытию (ленивая отрисовка, не блокирующая на глубоких
 * поддеревьях).
 */
export function VaultTree({ notes, onOpenNote }: VaultTreeProps): React.JSX.Element {
  const tree = useMemo(() => buildTree(notes), [notes]);

  if (notes.length === 0) {
    return <p role="status">Vault пуст — создайте первую заметку.</p>;
  }

  return (
    <ul role="tree" aria-label="Дерево заметок">
      {tree.map((node) => (
        <TreeNodeView key={nodeKey(node)} node={node} onOpenNote={onOpenNote} />
      ))}
    </ul>
  );
}

function nodeKey(node: TreeNode): string {
  return node.kind === "folder" ? `folder:${node.path}` : `note:${node.note.path}`;
}

function TreeNodeView({
  node,
  onOpenNote,
}: {
  node: TreeNode;
  onOpenNote: (note: Note) => void;
}): React.JSX.Element {
  if (node.kind === "note") {
    return (
      <li role="treeitem">
        <button type="button" onClick={() => onOpenNote(node.note)}>
          {node.note.name}
        </button>
      </li>
    );
  }
  return <FolderNodeView folder={node} onOpenNote={onOpenNote} />;
}

function FolderNodeView({
  folder,
  onOpenNote,
}: {
  folder: TreeFolder;
  onOpenNote: (note: Note) => void;
}): React.JSX.Element {
  // Верхний уровень раскрыт по умолчанию — соответствует «дерево пригодно для
  // навигации сразу» (SC-002); более глубокие уровни сворачиваются по клику.
  const [expanded, setExpanded] = useState(true);

  return (
    <li role="treeitem" aria-expanded={expanded}>
      <button type="button" onClick={() => setExpanded((v) => !v)}>
        {expanded ? "▾" : "▸"} {folder.name || "/"}
      </button>
      {expanded && (
        <ul role="group">
          {folder.children.map((child) => (
            <TreeNodeView key={nodeKey(child)} node={child} onOpenNote={onOpenNote} />
          ))}
        </ul>
      )}
    </li>
  );
}
