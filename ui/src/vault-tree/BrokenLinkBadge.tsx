import { useCallback, useEffect, useState } from "react";
import type { Note } from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { useI18n } from "../i18n/index.js";

export interface BrokenLinkBadgeProps {
  /** Цели ссылок заметки, которых нет в vault (`client.brokenLinks(path)`). */
  brokenTargets: string[];
}

/**
 * Пометка заметки, содержащей битые ссылки (T089, FR-030).
 *
 * «Битая» определяется ядром (`brokenLinks`), а не UI: чтобы решить это здесь,
 * пришлось бы во второй раз разбирать содержимое заметки и повторять правило
 * разрешения цели ссылки (`graph::resolve_link_target`) — ровно то
 * дублирование логики, которое запрещает принцип I.
 */
export function BrokenLinkBadge({
  brokenTargets,
}: BrokenLinkBadgeProps): React.JSX.Element | null {
  const { t } = useI18n();
  if (brokenTargets.length === 0) return null;
  const title = t("tree.brokenLinks", { targets: brokenTargets.join(", ") });
  return (
    <span role="img" aria-label={title} title={title}>
      ⚠
    </span>
  );
}

/**
 * Поиск заметок с битыми ссылками (FR-030): «найти» — это не только увидеть
 * значок на конкретной строке, но и получить их список после завершения
 * операции.
 *
 * Пересчёт запускается заново по `LinksInvalidated` (contracts/
 * frontend-events.md): ядро само сообщает, чей набор битых ссылок изменился,
 * поэтому опрашивать все заметки на каждый тик не требуется — `revision`
 * увеличивается вызывающей стороной при получении события.
 */
export function useBrokenLinks(
  client: VaultClient | null,
  notes: Note[],
  revision: number,
): {
  brokenByPath: ReadonlyMap<string, string[]>;
  notesWithBrokenLinks: string[];
  refresh: () => Promise<void>;
} {
  const [brokenByPath, setBrokenByPath] = useState<ReadonlyMap<string, string[]>>(
    new Map(),
  );

  const refresh = useCallback(async () => {
    if (!client) {
      setBrokenByPath(new Map());
      return;
    }
    const next = new Map<string, string[]>();
    for (const note of notes) {
      const broken = await client.brokenLinks(note.path);
      if (broken.length > 0) next.set(note.path, broken);
    }
    setBrokenByPath(next);
  }, [client, notes]);

  useEffect(() => {
    void refresh();
  }, [refresh, revision]);

  return {
    brokenByPath,
    notesWithBrokenLinks: [...brokenByPath.keys()].sort(),
    refresh,
  };
}
