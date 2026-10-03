#!/usr/bin/env bash
# Хардкод-литералы интерфейса вне каталогов переводов (фича 010, FR-028).
#
# Ищет строки кода с кириллицей в зоне интерфейса: всё, что показывается
# пользователю, обязано приходить из `ui/src/i18n/locales/`. Комментарии не в
# счёт — они не показываются. Строку можно явно исключить маркером
# `i18n-ignore` в ней самой (например, образец текста для распознавания языка).
#
# Только bash, grep и perl (есть в системе): скрипт запускает и хук агента,
# а он работает вне `nix develop`, где node нет.
#
#   scripts/check-i18n.sh            — весь интерфейс
#   scripts/check-i18n.sh FILE...    — только эти файлы (вне зоны — пропуск)
#
# Код возврата: 0 — чисто, 1 — найден хардкод.

set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Относительные пути в аргументах — от каталога вызова, а не от корня репо:
# иначе файл «не находится», и скрипт молча отвечает «ok».
caller="$PWD"
cd "$root" || exit 2

zones=(ui/src apps/desktop/src apps/mobile/src apps/web/app)

in_zone() {
  local file="$1"
  case "$file" in
    ui/src/i18n/locales/*|ui/src/wasm-core/*) return 1 ;;
    *.ts|*.tsx) ;;
    *) return 1 ;;
  esac
  local zone
  for zone in "${zones[@]}"; do
    [[ "$file" == "$zone"/* ]] && return 0
  done
  return 1
}

files=()
if (($# > 0)); then
  for arg in "$@"; do
    [[ "$arg" == /* ]] || arg="$caller/$arg"
    rel="${arg#"$root"/}"
    in_zone "$rel" && [[ -f "$rel" ]] && files+=("$rel")
  done
else
  while IFS= read -r file; do
    in_zone "$file" && files+=("$file")
  done < <(find "${zones[@]}" -type f \( -name '*.ts' -o -name '*.tsx' \) \
             -not -path '*/node_modules/*' 2>/dev/null | sort)
fi

# Комментарии вырезаются с сохранением номеров строк: блочные `/* … */`
# (включая JSX `{/* … */}`) заменяются переводами строк, строчные `// …` —
# только после пробела или в начале строки, чтобы не задеть `https://` в
# строках. После этого остаются строки кода, и кириллица в них — хардкод.
strip_comments() {
  perl -CSD -Mutf8 -0777 -ne '
    my @original = split /\n/, $_, -1;
    s{/\*.*?\*/}{ my $c = $&; $c =~ tr/\n//cd; $c }gse;
    s{(^|[ \t])//[^\n]*}{$1}gm;
    my $n = 0;
    for my $line (split /\n/, $_, -1) {
      $n++;
      next if ($original[$n - 1] // "") =~ /i18n-ignore/;
      if ($line =~ /[А-Яа-яЁё]/) {
        print "$n:$line\n";
        next;
      }
      next unless $ENV{JSX};
      # Английский хардкод кириллицей не пойман: в разметке ищется латиница в
      # подписях (`title="Vault"`) и в текстовом узле на одной строке
      # (`<h2>Vault</h2>`). `>` после `=`, `-` или пробела — стрелка, дженерик
      # или сравнение, а не тег; адреса (`https://…`) — пример, не текст.
      # Содержимое `<code>` и `<Math>` — код и формулы, их не переводят.
      (my $text = $line) =~ s{<(code|Math)\b[^>]*>.*?</\1>}{}g;
      if ($text =~ /\b(?:title|aria-label|placeholder|alt|label)="(?!https?:)[^"]*[A-Za-z]{2}/
          || $text =~ /(?<![=\-\s])>[^<>{}]*[A-Za-z]{2}[^<>{}]*<\//) {
        print "$n:$line\n";
      }
    }
  ' "$1"
}

found=0
for file in "${files[@]}"; do
  jsx=""
  [[ "$file" == *.tsx ]] && jsx=1
  hits="$(JSX="$jsx" strip_comments "$file")"
  if [[ -n "$hits" ]]; then
    while IFS= read -r line; do
      printf '%s:%s\n' "$file" "$line"
    done <<<"$hits"
    found=1
  fi
done

if ((found)); then
  echo "i18n: текст интерфейса вне ui/src/i18n/locales/ — перенесите в каталог и вызывайте t(...)" >&2
  exit 1
fi

# Полнота каталогов (FR-028) — только при полном прогоне: хуку на правку
# отдельного файла она ни к чему. Недостающие ключи в полном каталоге ловит
# `tsc` (тип `Catalog`); здесь — честность пометки `complete` в реестре:
# язык в работе (`complete: false`) — предупреждение, «полный» язык с
# каталогом `Partial<Catalog>` — провал: он выпустил бы английские дыры.
if (($# == 0)); then
  registry=ui/src/i18n/index.ts
  while IFS=' ' read -r code complete catalog; do
    file="ui/src/i18n/locales/$catalog.ts"
    partial=0
    [[ -f "$file" ]] && grep -Eq "^export const $catalog: Partial<Catalog>" "$file" && partial=1
    if [[ "$complete" == false ]]; then
      echo "i18n: предупреждение: язык $code помечен неполным — недостающие тексты покажутся по-английски" >&2
    elif ((partial)); then
      echo "i18n: язык $code помечен полным (complete: true), но $file объявлен Partial<Catalog>" >&2
      found=1
    fi
  done < <(perl -ne 'print "$1 $2 $3\n" if /\{\s*code:\s*"([^"]+)".*?complete:\s*(true|false).*?catalog:\s*(\w+)\s*\}/' "$registry")
  if ((found)); then
    exit 1
  fi
fi
echo "i18n: ok"
