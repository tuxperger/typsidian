// Движок git в браузере (фича 011, research §3): isomorphic-git поверх папки,
// которую пользователь открыл как vault.
//
// Это адаптер, а не политика (принцип VIII): методы и формы значений повторяют
// трейт `GitEngine` ядра один в один, а решения — что снимать, как совмещать,
// в каком порядке писать файлы — принимает `SyncSession` в WASM. Ошибки
// бросаются объектами формы `EngineError` (`{ kind: "Offline" }`), чтобы ядро
// разобрало их тем же типом, что и на native.
//
// История лежит в `.git` самой папки (FR-002) — её читает и стандартный git.
// Зеркало не-хранителя (FR-033) — тот же адаптер поверх каталога в OPFS.

import { Buffer } from "buffer";
import git from "isomorphic-git";
import http from "isomorphic-git/http/web";
import type {
  Author,
  HeadInfo,
  LogEntry,
  LogPage,
  RawChange,
  RemoteSpec,
  RepoProbe,
} from "../wasm-core/typsidian_core.js";

// isomorphic-git читает пакеты через `Buffer` и в браузере требует его
// глобально (их issue #1855). Ставится один раз, до первого вызова.
const scope = globalThis as unknown as { Buffer?: typeof Buffer };
if (scope.Buffer === undefined) scope.Buffer = Buffer;

/** Данные доступа на время одной операции; движок их не хранит. */
interface Auth {
  user?: string | null;
  token?: string | null;
}

/** Ошибка формы `EngineError` ядра. */
type EngineError =
  | { kind: "Offline" }
  | { kind: "Timeout" }
  | { kind: "AuthRequired" }
  | { kind: "AuthRejected" }
  | { kind: "NotFound" }
  | { kind: "RejectedNonFastForward" }
  | { kind: "CorsBlocked" }
  | { kind: "Cancelled" }
  | { kind: "NotARepository" }
  | { kind: "Io"; message: string };

// ── fs поверх FileSystemDirectoryHandle ──────────────────────────────────────

interface FsError extends Error {
  code: string;
}

function fsError(code: string, path: string): FsError {
  const error = new Error(`${code}: ${path}`) as FsError;
  error.code = code;
  return error;
}

interface Stat {
  type: "file" | "dir";
  mode: number;
  size: number;
  ino: number;
  mtimeMs: number;
  ctimeMs: number;
  uid: number;
  gid: number;
  dev: number;
  isFile(): boolean;
  isDirectory(): boolean;
  isSymbolicLink(): boolean;
}

function makeStat(type: "file" | "dir", size: number, mtimeMs: number): Stat {
  return {
    type,
    mode: type === "dir" ? 0o40755 : 0o100644,
    size,
    ino: 0,
    mtimeMs,
    ctimeMs: mtimeMs,
    uid: 1,
    gid: 1,
    dev: 1,
    isFile: () => type === "file",
    isDirectory: () => type === "dir",
    isSymbolicLink: () => false,
  };
}

type Entry =
  | { kind: "file"; handle: FileSystemFileHandle }
  | { kind: "directory"; handle: FileSystemDirectoryHandle };

interface KeyedDirectory {
  keys(): AsyncIterableIterator<string>;
}

/** Файловая система для isomorphic-git поверх папки браузера. */
export class HandleFs {
  readonly promises: {
    readFile: (path: string, options?: unknown) => Promise<Uint8Array | string>;
    writeFile: (path: string, data: Uint8Array | string, options?: unknown) => Promise<void>;
    unlink: (path: string) => Promise<void>;
    readdir: (path: string) => Promise<string[]>;
    mkdir: (path: string) => Promise<void>;
    rmdir: (path: string) => Promise<void>;
    stat: (path: string) => Promise<Stat>;
    lstat: (path: string) => Promise<Stat>;
    readlink: (path: string) => Promise<string>;
    symlink: (target: string, path: string) => Promise<void>;
  };

  constructor(private readonly root: FileSystemDirectoryHandle) {
    this.promises = {
      readFile: (path, options) => this.readFile(path, options),
      writeFile: (path, data) => this.writeFile(path, data),
      unlink: (path) => this.unlink(path),
      readdir: (path) => this.readdir(path),
      mkdir: (path) => this.mkdir(path),
      rmdir: (path) => this.rmdir(path),
      stat: (path) => this.stat(path),
      lstat: (path) => this.stat(path),
      readlink: (path) => Promise.reject(fsError("ENOENT", path)),
      symlink: (_target, path) => Promise.reject(fsError("EPERM", path)),
    };
  }

  private parts(path: string): string[] {
    return path.split("/").filter((part) => part !== "" && part !== ".");
  }

  private async directory(
    parts: string[],
    path: string,
    create = false,
  ): Promise<FileSystemDirectoryHandle> {
    let handle = this.root;
    for (const part of parts) {
      try {
        handle = await handle.getDirectoryHandle(part, { create });
      } catch (cause) {
        throw translate(cause, path);
      }
    }
    return handle;
  }

  private async entry(path: string): Promise<Entry> {
    const parts = this.parts(path);
    const name = parts.pop();
    if (name === undefined) return { kind: "directory", handle: this.root };
    const parent = await this.directory(parts, path);
    try {
      return { kind: "file", handle: await parent.getFileHandle(name) };
    } catch {
      try {
        return { kind: "directory", handle: await parent.getDirectoryHandle(name) };
      } catch (cause) {
        throw translate(cause, path);
      }
    }
  }

  private async readFile(path: string, options?: unknown): Promise<Uint8Array | string> {
    const entry = await this.entry(path);
    if (entry.kind !== "file") throw fsError("EISDIR", path);
    const file = await entry.handle.getFile();
    const bytes = new Uint8Array(await file.arrayBuffer());
    const encoding =
      typeof options === "string"
        ? options
        : typeof options === "object" && options !== null && "encoding" in options
          ? (options as { encoding?: string }).encoding
          : undefined;
    return encoding === "utf8" ? new TextDecoder().decode(bytes) : bytes;
  }

  private async writeFile(path: string, data: Uint8Array | string): Promise<void> {
    const parts = this.parts(path);
    const name = parts.pop();
    if (name === undefined) throw fsError("EISDIR", path);
    const parent = await this.directory(parts, path);
    const handle = await parent.getFileHandle(name, { create: true });
    const writable = await handle.createWritable();
    const bytes = typeof data === "string" ? new TextEncoder().encode(data) : data;
    // Копия в собственный `ArrayBuffer`: `Buffer` из пула isomorphic-git
    // может смотреть в чужой общий буфер.
    await writable.write(new Uint8Array(bytes).buffer as ArrayBuffer);
    await writable.close();
  }

  private async unlink(path: string): Promise<void> {
    const parts = this.parts(path);
    const name = parts.pop();
    if (name === undefined) throw fsError("EPERM", path);
    const parent = await this.directory(parts, path);
    try {
      await parent.removeEntry(name);
    } catch (cause) {
      throw translate(cause, path);
    }
  }

  private async readdir(path: string): Promise<string[]> {
    const entry = await this.entry(path);
    if (entry.kind !== "directory") throw fsError("ENOTDIR", path);
    const names: string[] = [];
    for await (const name of (entry.handle as unknown as KeyedDirectory).keys()) {
      names.push(name);
    }
    return names;
  }

  private async mkdir(path: string): Promise<void> {
    const parts = this.parts(path);
    const name = parts.pop();
    if (name === undefined) return;
    const parent = await this.directory(parts, path);
    try {
      await parent.getFileHandle(name);
      throw fsError("EEXIST", path);
    } catch (cause) {
      if ((cause as Partial<FsError>).code === "EEXIST") throw cause;
    }
    try {
      await parent.getDirectoryHandle(name);
      throw fsError("EEXIST", path);
    } catch (cause) {
      if ((cause as Partial<FsError>).code === "EEXIST") throw cause;
    }
    await parent.getDirectoryHandle(name, { create: true });
  }

  private async rmdir(path: string): Promise<void> {
    const parts = this.parts(path);
    const name = parts.pop();
    if (name === undefined) throw fsError("EPERM", path);
    const parent = await this.directory(parts, path);
    try {
      await parent.removeEntry(name);
    } catch (cause) {
      throw translate(cause, path);
    }
  }

  private async stat(path: string): Promise<Stat> {
    const entry = await this.entry(path);
    if (entry.kind === "directory") return makeStat("dir", 0, 0);
    const file = await entry.handle.getFile();
    return makeStat("file", file.size, file.lastModified);
  }
}

function translate(cause: unknown, path: string): FsError {
  const name = cause instanceof DOMException ? cause.name : "";
  if (name === "NotFoundError") return fsError("ENOENT", path);
  if (name === "TypeMismatchError") return fsError("ENOTDIR", path);
  if (name === "InvalidModificationError") return fsError("ENOTEMPTY", path);
  if ((cause as Partial<FsError>).code !== undefined) return cause as FsError;
  return fsError("EIO", path);
}

// ── Адаптер движка ───────────────────────────────────────────────────────────

const DIR = "/";

function ioError(cause: unknown): EngineError {
  return { kind: "Io", message: cause instanceof Error ? cause.message : String(cause) };
}

/** Перевод ошибок isomorphic-git и сети в `EngineError`. */
function networkError(cause: unknown, viaProxy: boolean): EngineError {
  const code = (cause as { code?: string }).code;
  const data = (cause as { data?: { statusCode?: number; reason?: string } }).data;
  if (code === "UserCanceledError") return { kind: "Cancelled" };
  if (code === "PushRejectedError") return { kind: "RejectedNonFastForward" };
  if (code === "HttpError") {
    const status = data?.statusCode ?? 0;
    if (status === 401) return { kind: "AuthRequired" };
    if (status === 403) return { kind: "AuthRejected" };
    if (status === 404) return { kind: "NotFound" };
    return ioError(cause);
  }
  if (code === "NotFoundError") return { kind: "NotFound" };
  // `fetch` браузера роняет одно и то же `TypeError` и без сети, и на отказе
  // CORS. Различает их только состояние сети: при сети и без посредника это
  // почти наверняка CORS (FR-037).
  if (cause instanceof TypeError) {
    if (!navigator.onLine) return { kind: "Offline" };
    return viaProxy ? { kind: "Offline" } : { kind: "CorsBlocked" };
  }
  return ioError(cause);
}

function onAuth(auth: Auth): (() => { username: string; password: string }) | undefined {
  if (auth.token === undefined || auth.token === null) return undefined;
  const token = auth.token;
  return () => ({ username: auth.user ?? "x-access-token", password: token });
}

/**
 * Адаптер движка для `WebSyncSession`. `mirror` — голое зеркало удалённого
 * репозитория для не-хранителя: рабочих файлов у него нет.
 */
export function createWebGitEngine(
  root: FileSystemDirectoryHandle,
  options: { mirror?: boolean } = {},
): Record<string, unknown> {
  const fs = new HandleFs(root);
  const mirror = options.mirror === true;
  let aborted = false;

  async function hasRepo(): Promise<boolean> {
    try {
      await fs.promises.stat("/.git/HEAD");
      return true;
    } catch {
      return false;
    }
  }

  async function requireRepo(): Promise<void> {
    if (!(await hasRepo())) throw { kind: "NotARepository" } satisfies EngineError;
  }

  async function resolve(rev: string): Promise<string | null> {
    try {
      return await git.resolveRef({ fs, dir: DIR, ref: rev });
    } catch {
      // Полный идентификатор снимка `resolveRef` не принимает — проверяем
      // сам объект.
      try {
        const expanded = await git.expandOid({ fs, dir: DIR, oid: rev });
        return expanded;
      } catch {
        return null;
      }
    }
  }

  async function branch(): Promise<string | null> {
    try {
      return (await git.currentBranch({ fs, dir: DIR, fullname: false })) ?? null;
    } catch {
      return null;
    }
  }

  async function treeFiles(oid: string | null): Promise<Map<string, string>> {
    const files = new Map<string, string>();
    if (oid === null) return files;
    await git.walk({
      fs,
      dir: DIR,
      trees: [git.TREE({ ref: oid })],
      map: async (path, [entry]) => {
        if (entry === null || entry === undefined || path === ".") return true;
        if ((await entry.type()) === "blob") files.set(path, await entry.oid());
        return true;
      },
    });
    return files;
  }

  async function ancestors(oid: string): Promise<Set<string>> {
    const log = await git.log({ fs, dir: DIR, ref: oid });
    return new Set(log.map((entry) => entry.oid));
  }

  function entryOf(result: Awaited<ReturnType<typeof git.log>>[number]): LogEntry {
    return {
      id: result.oid,
      message: result.commit.message.replace(/\n$/, ""),
      author: { name: result.commit.author.name, email: result.commit.author.email },
      time: result.commit.author.timestamp * 1000,
      parents: result.commit.parent,
    };
  }

  return {
    abort(): void {
      aborted = true;
    },

    async probe(): Promise<RepoProbe> {
      if (!(await hasRepo())) return { kind: "Absent" };
      return mirror ? { kind: "Mirror" } : { kind: "Local" };
    },

    async init(defaultBranch: string): Promise<void> {
      await git.init({ fs, dir: DIR, defaultBranch });
    },

    async head(): Promise<HeadInfo> {
      await requireRepo();
      const oid = await resolve("HEAD");
      let merging = false;
      try {
        await fs.promises.stat("/.git/MERGE_HEAD");
        merging = true;
      } catch {
        merging = false;
      }
      return {
        branch: (await branch()) ?? undefined,
        oid: oid ?? undefined,
        in_progress: merging ? "Merge" : undefined,
      } as HeadInfo;
    },

    async status(): Promise<RawChange[]> {
      await requireRepo();
      const matrix = await git.statusMatrix({ fs, dir: DIR });
      const changes: RawChange[] = [];
      for (const [path, head, workdir] of matrix) {
        if (head === 0 && workdir === 2) {
          changes.push({ path, status: "Added" } as RawChange);
        } else if (head === 1 && workdir === 0) {
          changes.push({ path, status: "Deleted" } as RawChange);
        } else if (head === 1 && workdir === 2) {
          changes.push({ path, status: "Modified" } as RawChange);
        }
      }
      return changes;
    },

    async commit(
      paths: string[],
      message: string,
      author: Author,
      time: number,
      mergeParent: string | null,
    ): Promise<string> {
      await requireRepo();
      for (const filepath of paths) {
        try {
          await fs.promises.stat(`/${filepath}`);
          await git.add({ fs, dir: DIR, filepath, force: true });
        } catch {
          await git.remove({ fs, dir: DIR, filepath });
        }
      }
      const head = await resolve("HEAD");
      const parent = [
        ...(head === null ? [] : [head]),
        ...(mergeParent === null ? [] : [mergeParent]),
      ];
      const signature = {
        name: author.name,
        email: author.email,
        timestamp: Math.floor(time / 1000),
        timezoneOffset: new Date(time).getTimezoneOffset(),
      };
      return git.commit({
        fs,
        dir: DIR,
        message,
        author: signature,
        committer: signature,
        parent,
      });
    },

    async log(start: string | null, offset: number, limit: number): Promise<LogPage> {
      await requireRepo();
      const ref = start === null ? await resolve("HEAD") : await resolve(start);
      if (ref === null) return { entries: [] } as unknown as LogPage;
      const entries = await git.log({ fs, dir: DIR, ref, depth: offset + limit + 1 });
      const page = entries.slice(offset, offset + limit).map(entryOf);
      const more = entries.length > offset + limit;
      return { entries: page, next: more ? offset + page.length : undefined } as LogPage;
    },

    async readBlob(rev: string, path: string): Promise<Uint8Array | null> {
      const oid = await resolve(rev);
      if (oid === null) return null;
      try {
        const { blob } = await git.readBlob({ fs, dir: DIR, oid, filepath: path });
        return blob;
      } catch {
        return null;
      }
    },

    async treeDiff(from: string | null, to: string): Promise<RawChange[]> {
      const before = await treeFiles(from === null ? null : await resolve(from));
      const target = await resolve(to);
      if (target === null) throw { kind: "NotFound" } satisfies EngineError;
      const after = await treeFiles(target);
      const changes: RawChange[] = [];
      for (const [path, oid] of after) {
        const old = before.get(path);
        if (old === undefined) {
          changes.push({ path, status: "Added", new_oid: oid } as RawChange);
        } else if (old !== oid) {
          changes.push({ path, status: "Modified", old_oid: old, new_oid: oid } as RawChange);
        }
      }
      for (const [path, oid] of before) {
        if (!after.has(path)) {
          changes.push({ path, status: "Deleted", old_oid: oid } as RawChange);
        }
      }
      return changes;
    },

    async mergeBase(a: string, b: string): Promise<string | null> {
      const left = await resolve(a);
      const right = await resolve(b);
      if (left === null || right === null) return null;
      const bases = await git.findMergeBase({ fs, dir: DIR, oids: [left, right] });
      return bases[0] ?? null;
    },

    async setHead(oid: string): Promise<void> {
      const previous = await resolve("HEAD");
      const name = (await branch()) ?? "main";
      await git.writeRef({ fs, dir: DIR, ref: `refs/heads/${name}`, value: oid, force: true });
      // Индекс — к дереву снимка по изменившимся путям; рабочие файлы уже
      // записала сессия.
      const before = await treeFiles(previous);
      const after = await treeFiles(oid);
      const paths = new Set([...before.keys(), ...after.keys()]);
      for (const filepath of paths) {
        if (before.get(filepath) !== after.get(filepath)) {
          await git.resetIndex({ fs, dir: DIR, filepath, ref: oid });
        }
      }
    },

    async remoteHead(branchName: string): Promise<string | null> {
      const ref = mirror ? `refs/heads/${branchName}` : `refs/remotes/origin/${branchName}`;
      return resolve(ref);
    },

    async aheadBehind(local: string, upstream: string): Promise<[number, number]> {
      const ours = await ancestors(local);
      const theirs = await ancestors(upstream);
      let ahead = 0;
      let behind = 0;
      for (const oid of ours) if (!theirs.has(oid)) ahead += 1;
      for (const oid of theirs) if (!ours.has(oid)) behind += 1;
      return [ahead, behind];
    },

    async fetch(remote: RemoteSpec, auth: Auth): Promise<string | null> {
      aborted = false;
      const corsProxy = remote.cors_proxy ?? undefined;
      try {
        const authenticate = onAuth(auth);
        await git.fetch({
          fs,
          http,
          dir: DIR,
          url: remote.url,
          remote: "origin",
          ref: remote.branch,
          singleBranch: true,
          tags: false,
          ...(corsProxy === undefined ? {} : { corsProxy }),
          ...(authenticate === undefined ? {} : { onAuth: authenticate }),
          onProgress: () => {
            if (aborted) throw { code: "UserCanceledError" };
          },
        });
      } catch (cause) {
        throw networkError(cause, corsProxy !== undefined);
      }
      const tracking = await resolve(`refs/remotes/origin/${remote.branch}`);
      if (tracking !== null && mirror) {
        await git.writeRef({
          fs,
          dir: DIR,
          ref: `refs/heads/${remote.branch}`,
          value: tracking,
          force: true,
        });
      }
      return tracking;
    },

    async push(remote: RemoteSpec, auth: Auth): Promise<void> {
      aborted = false;
      const corsProxy = remote.cors_proxy ?? undefined;
      let result: Awaited<ReturnType<typeof git.push>>;
      try {
        const authenticate = onAuth(auth);
        result = await git.push({
          fs,
          http,
          dir: DIR,
          url: remote.url,
          remote: "origin",
          ref: remote.branch,
          remoteRef: remote.branch,
          ...(corsProxy === undefined ? {} : { corsProxy }),
          ...(authenticate === undefined ? {} : { onAuth: authenticate }),
        });
      } catch (cause) {
        throw networkError(cause, corsProxy !== undefined);
      }
      if (!result.ok) throw { kind: "RejectedNonFastForward" } satisfies EngineError;
      const head = await resolve("HEAD");
      if (head !== null) {
        await git.writeRef({
          fs,
          dir: DIR,
          ref: `refs/remotes/origin/${remote.branch}`,
          value: head,
          force: true,
        });
      }
    },

    async cloneInto(remote: RemoteSpec, auth: Auth, bare: boolean): Promise<void> {
      aborted = false;
      const corsProxy = remote.cors_proxy ?? undefined;
      try {
        const authenticate = onAuth(auth);
        await git.clone({
          fs,
          http,
          dir: DIR,
          url: remote.url,
          ref: remote.branch,
          singleBranch: true,
          noCheckout: bare,
          ...(corsProxy === undefined ? {} : { corsProxy }),
          ...(authenticate === undefined ? {} : { onAuth: authenticate }),
        });
      } catch (cause) {
        throw networkError(cause, corsProxy !== undefined);
      }
    },

    async setRemoteUrl(url: string | null): Promise<void> {
      if (url === null) {
        try {
          await git.deleteRemote({ fs, dir: DIR, remote: "origin" });
        } catch {
          // Удалять нечего.
        }
        return;
      }
      await git.addRemote({ fs, dir: DIR, remote: "origin", url, force: true });
    },
  };
}

/** Каталог зеркала не-хранителя в OPFS (FR-033). */
export async function mirrorDirectory(vaultId: string): Promise<FileSystemDirectoryHandle> {
  const root = await navigator.storage.getDirectory();
  const mirrors = await root.getDirectoryHandle("typsidian-mirrors", { create: true });
  return mirrors.getDirectoryHandle(vaultId.replace(/[^A-Za-z0-9_-]/g, "_"), { create: true });
}
