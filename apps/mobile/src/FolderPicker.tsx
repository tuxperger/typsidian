import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import "./folder-picker.css";

/**
 * Обзор папок устройства — тот самый жест выбора, который на рабочем столе
 * целиком делает нативный диалог ОС, а в браузере `showDirectoryPicker`.
 *
 * На Android ни того, ни другого нет: `pick_folder` у `tauri-plugin-dialog`
 * реализован только под desktop, а системный SAF отдаёт `content://`-адреса,
 * которых `NativeBackend` не понимает — он построен на `std::fs`. Поэтому
 * список папок приходится показать своим окном; содержимое каталогов даёт
 * оболочка командами `browse_*` (`packages/shell/src/lib.rs`).
 *
 * Правил vault здесь нет и быть не может: обзор не знает, что такое заметка, и
 * ничего не проверяет в выбранной папке. Открывает её ядро — отказ вернётся
 * оттуда теми же словами, что на рабочем столе и на web (FR-008).
 */

/** Папка в списке — `DirEntryView` оболочки. */
interface DirEntry {
  name: string;
  path: string;
}

/** Содержимое открытой папки — `DirListing` оболочки. */
interface DirListing {
  path: string;
  parent: string | null;
  dirs: DirEntry[];
}

/**
 * Отказ обзора — `BrowseError` оболочки.
 *
 * Описан здесь руками, а не взят из сгенерированных определений (принцип IV):
 * генератор есть только у типов ядра (`tsify` → `ui/src/wasm-core`), а эти
 * живут в оболочке, потому что обзор файловой системы — не ядро. Тем же
 * способом разбирается её же `NoVaultOpen` в `useVaultSettings`. Форма держится
 * минимальной ровно затем, чтобы расхождение было заметным: имена вариантов и
 * одно поле.
 */
type BrowseError =
  | { kind: "NotFound" }
  | { kind: "NotADirectory" }
  | { kind: "AccessDenied" }
  | { kind: "AlreadyExists" }
  | { kind: "InvalidName" }
  | { kind: "Io"; message: string };

/** Сужение `unknown` до `BrowseError` без `any` (принцип IV). */
function asBrowseError(cause: unknown): BrowseError | null {
  if (typeof cause !== "object" || cause === null || !("kind" in cause)) return null;
  const { kind } = cause as { kind: unknown };
  switch (kind) {
    case "NotFound":
    case "NotADirectory":
    case "AccessDenied":
    case "AlreadyExists":
    case "InvalidName":
      return { kind };
    case "Io": {
      const { message } = cause as { message?: unknown };
      return { kind: "Io", message: typeof message === "string" ? message : "" };
    }
    default:
      return null;
  }
}

/** Текст на каждый отказ — без общего «что-то пошло не так» (FR-008). */
function describeFailure(cause: unknown): string {
  const error = asBrowseError(cause);
  if (error === null) return `Не удалось прочитать папку: ${String(cause)}`;
  switch (error.kind) {
    case "AccessDenied":
      // Самая частая причина здесь — не права на конкретный каталог, а не
      // выданный доступ ко всей общей памяти: его Android даёт только вручную,
      // отдельным экраном настроек (см. `MainActivity.kt`).
      return "Нет доступа к памяти устройства. Разрешите Typsidian «Доступ ко всем файлам» в настройках Android и откройте обзор заново.";
    case "NotFound":
      return "Этой папки больше нет: её переименовали или удалили.";
    case "NotADirectory":
      return "Это файл, а не папка.";
    case "AlreadyExists":
      return "Папка с таким именем здесь уже есть.";
    case "InvalidName":
      return "Имя папки не может быть пустым и не может содержать «/».";
    case "Io":
      return `Система отказала: ${error.message}`;
    default: {
      const exhaustive: never = error;
      return exhaustive;
    }
  }
}

export interface FolderPickerProps {
  /**
   * Папка, с которой открывается обзор; `null` — начинать с корня устройства.
   * Если её больше нет, обзор молча уходит в корень: пропажа папки, выбранной
   * в прошлый раз, — не отказ этого открытия.
   */
  startAt: string | null;
  onChoose: (path: string) => void;
  onCancel: () => void;
}

export function FolderPicker({
  startAt,
  onChoose,
  onCancel,
}: FolderPickerProps): React.JSX.Element {
  const [listing, setListing] = useState<DirListing | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(true);
  // Имя создаваемой папки; `null` — форма закрыта.
  const [newName, setNewName] = useState<string | null>(null);

  /** Переходит в папку. `false` — не удалось, причина уже показана. */
  const go = useCallback(async (path: string): Promise<boolean> => {
    setBusy(true);
    try {
      const next = await invoke<DirListing>("browse_dir", { path });
      setListing(next);
      setError(null);
      return true;
    } catch (cause) {
      // Список остаётся прежним: уводить пользователя из читаемой папки
      // потому, что соседняя не открылась, незачем.
      setError(describeFailure(cause));
      return false;
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    void (async () => {
      if (startAt !== null && (await go(startAt))) return;
      try {
        await go(await invoke<string>("browse_root"));
      } catch (cause) {
        setError(describeFailure(cause));
        setBusy(false);
      }
    })();
  }, [go, startAt]);

  const create = useCallback(async () => {
    if (listing === null || newName === null) return;
    setBusy(true);
    let created: string;
    try {
      created = await invoke<string>("browse_create_dir", {
        parent: listing.path,
        name: newName,
      });
    } catch (cause) {
      setError(describeFailure(cause));
      setBusy(false);
      return;
    }
    setNewName(null);
    // Созданная папка сразу становится текущей: её и предлагается выбрать.
    await go(created);
  }, [go, listing, newName]);

  const parent = listing?.parent ?? null;
  const dirs = listing?.dirs ?? [];

  return (
    <div className="ts-picker" role="dialog" aria-modal="true" aria-label="Папка с заметками">
      <header className="ts-picker__head">
        <h2 className="ts-picker__title">Папка с заметками</h2>
        <button type="button" className="ts-btn ts-btn--outline" onClick={onCancel}>
          Отмена
        </button>
      </header>

      {/* Путь целиком, а не имя папки: на устройстве «Notes» может быть
          несколько, и различает их только путь. */}
      <p className="ts-picker__path">{listing?.path ?? "…"}</p>

      <ul className="ts-picker__list">
        {parent !== null && (
          <li>
            <button
              type="button"
              className="ts-picker__row ts-picker__row--up"
              onClick={() => void go(parent)}
              disabled={busy}
            >
              ↑ Наверх
            </button>
          </li>
        )}
        {dirs.map((dir) => (
          <li key={dir.path}>
            <button
              type="button"
              className="ts-picker__row"
              onClick={() => void go(dir.path)}
              disabled={busy}
            >
              {dir.name}
            </button>
          </li>
        ))}
      </ul>

      {listing !== null && dirs.length === 0 && (
        <p className="ts-picker__empty">
          Вложенных папок здесь нет — эту можно выбрать целиком или создать в ней новую.
        </p>
      )}
      {error !== null && (
        <p className="ts-picker__error" role="alert">
          {error}
        </p>
      )}

      <footer className="ts-picker__actions">
        {newName === null ? (
          <>
            <button
              type="button"
              className="ts-btn ts-btn--outline"
              onClick={() => setNewName("")}
              disabled={listing === null || busy}
            >
              Создать папку
            </button>
            <button
              type="button"
              className="ts-btn ts-btn--accent"
              onClick={() => {
                if (listing !== null) onChoose(listing.path);
              }}
              disabled={listing === null || busy}
            >
              Выбрать эту папку
            </button>
          </>
        ) : (
          <form
            className="ts-picker__new"
            onSubmit={(event) => {
              event.preventDefault();
              void create();
            }}
          >
            <input
              className="ts-picker__name"
              value={newName}
              onChange={(event) => setNewName(event.target.value)}
              placeholder="Имя новой папки"
              aria-label="Имя новой папки"
              autoFocus
            />
            <button type="submit" className="ts-btn ts-btn--accent" disabled={busy}>
              Создать
            </button>
            <button
              type="button"
              className="ts-btn ts-btn--outline"
              onClick={() => setNewName(null)}
            >
              Отмена
            </button>
          </form>
        )}
      </footer>
    </div>
  );
}
