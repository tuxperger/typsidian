#!/usr/bin/env bash
# PostToolUse (Edit|Write): дешёвые проверки одного файла сразу после правки.
#   *.rs                      rustfmt --check этого файла
#   specs/NNN-*/spec.md       остатки шаблона, обязательные разделы, предел уточнений
#   specs/NNN-*/tasks.md      формат и уникальность идентификаторов задач
#   любой артефакт Spec Kit   перегенерация specs/ROADMAP.md
# Находки идут в stderr с кодом 2 — так они возвращаются агенту.
# Хардкод интерфейса проверяет соседний хук check-i18n-edit.sh.

command -v jq >/dev/null 2>&1 || exit 0

input="$(cat)"
file="$(jq -r '.tool_input.file_path // empty' <<<"$input")"
[[ -n "$file" && -f "$file" ]] || exit 0

root="${CLAUDE_PROJECT_DIR:-$(jq -r '.cwd // empty' <<<"$input")}"
rel="${file#"$root"/}"
[[ "$rel" != "$file" ]] || exit 0

problems=()

case "$rel" in
    target/* | */target/* | */gen/*) ;;
    *.rs)
        if command -v rustfmt >/dev/null 2>&1; then
            fmt=(rustfmt)
        elif command -v nix >/dev/null 2>&1; then
            fmt=(nix develop "$root" -c rustfmt)
        else
            exit 0
        fi
        if ! out="$(cd "$root" && "${fmt[@]}" --check --color never --edition 2021 "$file" 2>&1)"; then
            # Важен только diff этого файла: rustfmt обходит и дочерние `mod`.
            if grep -qF "Diff in $file" <<<"$out"; then
                problems+=("rustfmt: $rel не отформатирован — nix develop -c cargo fmt --all")
                problems+=("$(grep -v '^Typst-Obsidian dev shell' <<<"$out" | head -n 30)")
            fi
        fi
        ;;
    specs/[0-9][0-9][0-9]-*/spec.md)
        for ph in '[FEATURE NAME]' '[###-feature-name]' '[DATE]' '$ARGUMENTS' '[Brief Title]'; do
            grep -qF -- "$ph" "$file" && problems+=("spec: остался плейсхолдер шаблона $ph")
        done
        for sec in '## User Scenarios & Testing' '## Requirements' '## Success Criteria'; do
            grep -qF -- "$sec" "$file" || problems+=("spec: нет обязательного раздела «$sec»")
        done
        n="$(grep -c 'NEEDS CLARIFICATION' "$file" || true)"
        [[ "$n" -gt 3 ]] && problems+=("spec: $n маркеров NEEDS CLARIFICATION (максимум 3)")
        # Спеки 001–010 написаны до роадмапа и этих строк не имеют — там в таблице
        # прочерк. Проверяется только заполненность, если строка есть.
        if grep -qE '^\*\*Priority\*\*:' "$file" &&
            ! grep -qE '^\*\*Priority\*\*:[[:space:]]*P[123][[:space:]]*$' "$file"; then
            problems+=("spec: строка **Priority**: должна быть ровно P1, P2 или P3 (её читает specs/ROADMAP.md)")
        fi
        grep -qE '^\*\*Summary\*\*:[[:space:]]*(\[|$)' "$file" &&
            problems+=("spec: строка **Summary**: не заполнена (её читает specs/ROADMAP.md)")
        ;;
    specs/[0-9][0-9][0-9]-*/tasks.md)
        dups="$(grep -oE '^- \[[ xX]\] T[0-9]+' "$file" | grep -oE 'T[0-9]+' | sort | uniq -d | tr '\n' ' ')"
        [[ -n "$dups" ]] && problems+=("tasks: повторяющиеся идентификаторы задач: $dups")
        bad="$(grep -nE '^- \[[ xX]\] ' "$file" | grep -vE '^[0-9]+:- \[[ xX]\] T[0-9]+ ' | head -n 5)"
        [[ -n "$bad" ]] && problems+=("tasks: строки-чекбоксы без идентификатора TNNN:" "$bad")
        ;;
esac

case "$rel" in
    specs/[0-9][0-9][0-9]-*/*)
        "$root/.specify/scripts/typsidian/roadmap.sh" 2>/dev/null || true
        ;;
esac

if [[ ${#problems[@]} -gt 0 ]]; then
    printf '%s\n' "${problems[@]}" >&2
    exit 2
fi
exit 0
