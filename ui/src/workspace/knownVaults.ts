import type { VaultRecord } from "../wasm-core/typsidian_core.js";

/**
 * Короткое имя vault для шапки и стартового экрана (фича 043): подпись, если
 * задана, иначе последняя часть расположения — расположение показывается
 * отдельной строкой рядом. В панели настроек имя другое — подпись или полный
 * путь (004 FR-018), там строки расположения нет.
 */
export function vaultDisplayName(record: VaultRecord): string {
  const label = record.label?.trim();
  if (label) return label;
  const parts = record.location.split(/[\\/]/).filter((part) => part !== "");
  return parts[parts.length - 1] ?? record.location;
}
