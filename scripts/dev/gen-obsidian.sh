#!/usr/bin/env bash
# Генератор тестового хранилища Obsidian для замера SC-004 (фича 021, T023).
#
#   scripts/dev/gen-obsidian.sh <папка> [N=2000]
#
# N заметок Markdown в дереве из 20 папок: заголовки, списки, таблица, вики-ссылки
# на другие заметки (в том числе `[[Имя|текст]]`), формулы `$…$`, YAML-шапка у
# каждой третьей, картинка у каждой десятой. Без внешних зависимостей: bash и
# base64. Папка должна быть пустой или несуществующей.
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
  echo "использование: $0 <папка> [N=2000]" >&2
  exit 2
fi
root=$1
count=${2:-2000}
folders=20

if [[ -e $root && -n $(ls -A "$root" 2>/dev/null || true) ]]; then
  echo "папка $root не пуста" >&2
  exit 1
fi
mkdir -p "$root/.obsidian"
printf '{}\n' >"$root/.obsidian/app.json"

# Крохотный PNG (1x1), один на все картинки.
png_b64='iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg=='

folder_of() { printf 'Папка %02d' $(($1 % folders)); }
name_of() { printf 'Заметка %04d' "$1"; }

for ((f = 0; f < folders; f++)); do
  mkdir -p "$root/$(folder_of "$f")"
done

for ((i = 0; i < count; i++)); do
  dir="$root/$(folder_of "$i")"
  name=$(name_of "$i")
  a=$(((i * 7 + 1) % count))
  b=$(((i * 13 + 5) % count))
  c=$(((i * 31 + 11) % count))
  {
    if ((i % 3 == 0)); then
      printf -- '---\ntitle: %s\ntags: [тест, группа%d]\nid: %d\ndraft: false\n---\n\n' "$name" $((i % 5)) "$i"
    fi
    printf '# %s\n\n' "$name"
    printf 'Вводный абзац заметки %d с **жирным**, *курсивом* и `кодом`. Ссылка на [[%s]], ещё на [[%s|вторую заметку]] и на [[%s#Раздел]].\n\n' \
      "$i" "$(name_of "$a")" "$(name_of "$b")" "$(name_of "$c")"
    printf '## Раздел\n\n- первый пункт\n- второй пункт с формулой $x_%d^2 + y^2 = z^2$\n  - вложенный пункт\n- третий пункт\n\n' "$i"
    printf '1. шаг один\n2. шаг два\n\n'
    printf 'Формула отдельной строкой:\n\n$$\\sum_{k=1}^{%d} k = \\frac{%d(%d+1)}{2}$$\n\n' $((i % 50 + 1)) $((i % 50 + 1)) $((i % 50 + 1))
    printf '| Колонка A | Колонка B | Колонка C |\n| --- | --- | --- |\n| %d | %d | %d |\n| а | б | в |\n\n' "$i" "$a" "$b"
    printf '> Цитата к заметке %d.\n\n' "$i"
    if ((i % 10 == 0)); then
      printf '![[рис-%04d.png]]\n\n' "$i"
    fi
    printf 'Конец заметки.\n'
  } >"$dir/$name.md"
  if ((i % 10 == 0)); then
    printf '%s' "$png_b64" | base64 -d >"$dir/рис-$(printf '%04d' "$i").png"
  fi
done

echo "готово: $count заметок, $folders папок, $(((count + 9) / 10)) картинок в $root"
