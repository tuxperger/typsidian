#!/usr/bin/env bash
# Хук агента PostToolUse (Edit|Write): не даёт хардкоду интерфейса появиться
# заново (фича 010). Проверяет только изменённый файл тем же скриптом, что и
# гейт `scripts/check-i18n.sh`; файлы вне зоны интерфейса пропускаются молча.
#
# Код 2 — находки уходят агенту обратной связью; 0 — всё чисто или не наш файл.

set -uo pipefail

input="$(cat)"
# Путь без jq: jq в окружении не гарантирован.
file="$(printf '%s' "$input" | grep -oP '"file_path"\s*:\s*"\K[^"]+' | head -n1)"
[[ -z "$file" ]] && exit 0

root="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
output="$("$root/scripts/check-i18n.sh" "$file" 2>&1)"
status=$?
if ((status == 1)); then
  # Серия правок одного файла вызывает хук на каждой: без предела агент получал
  # бы весь остаток хардкода по многу раз. Первые находки + счётчик остальных.
  hits="$(printf '%s\n' "$output" | grep -v '^i18n: ')"
  total="$(printf '%s\n' "$hits" | wc -l)"
  printf '%s\n' "$hits" | head -n 10 >&2
  ((total > 10)) && printf '… и ещё %d строк(и) с кириллицей в этом файле\n' "$((total - 10))" >&2
  printf '%s\n' "$output" | grep '^i18n: ' >&2
  exit 2
fi
exit 0
