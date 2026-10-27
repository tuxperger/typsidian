import type { Note } from "../wasm-core/typsidian_core.js";

export interface TreeFolder {
  kind: "folder";
  name: string;
  path: string;
  children: TreeNode[];
}

export interface TreeNoteNode {
  kind: "note";
  note: Note;
}

export type TreeNode = TreeFolder | TreeNoteNode;

/**
 * Строит иерархию из плоского списка заметок (FR-003) — папки не хранятся
 * отдельно, выводятся из путей (data-model.md «Папка»).
 */
export function buildTree(notes: Note[]): TreeNode[] {
  const root: TreeFolder = { kind: "folder", name: "", path: "", children: [] };
  const folders = new Map<string, TreeFolder>([["", root]]);

  const ensureFolder = (path: string): TreeFolder => {
    const existing = folders.get(path);
    if (existing) return existing;

    const segments = path.split("/");
    const name = segments[segments.length - 1] ?? "";
    const parentPath = segments.slice(0, -1).join("/");
    const parent = ensureFolder(parentPath);
    const folder: TreeFolder = { kind: "folder", name, path, children: [] };
    parent.children.push(folder);
    folders.set(path, folder);
    return folder;
  };

  for (const note of [...notes].sort((a, b) => a.path.localeCompare(b.path))) {
    const segments = note.path.split("/");
    const parentPath = segments.slice(0, -1).join("/");
    const parent = ensureFolder(parentPath);
    parent.children.push({ kind: "note", note });
  }

  return root.children;
}

/**
 * Пути всех папок среди `nodes` и под ними, на всех уровнях (фича 042): для
 * «Свернуть все папки» — по всему дереву, для «Свернуть вложенные» — по папке.
 */
export function folderPaths(nodes: readonly TreeNode[]): string[] {
  const paths: string[] = [];
  const walk = (list: readonly TreeNode[]): void => {
    for (const node of list) {
      if (node.kind !== "folder") continue;
      paths.push(node.path);
      walk(node.children);
    }
  };
  walk(nodes);
  return paths;
}
