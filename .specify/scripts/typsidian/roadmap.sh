#!/usr/bin/env bash
# Перегенерировать specs/ROADMAP.md из артефактов Spec Kit на диске.
#
# Роадмап ПРОИЗВОДНЫЙ, руками не правится: стадия фичи — то, что доказывает её
# каталог (spec.md → plan.md → tasks.md → все задачи отмечены), приоритет и суть —
# строки `**Priority**:` / `**Summary**:` в шапке spec.md (у спек без них —
# прочерк). Править спеку, а не роадмап.
#
# Usage: roadmap.sh [--check] [--stdout]
#   --check   код 1, если specs/ROADMAP.md устарел (не пишет)
#   --stdout  напечатать роадмап вместо записи

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
out="$root/specs/ROADMAP.md"
mode=write
for arg in "$@"; do
    case "$arg" in
        --check) mode=check ;;
        --stdout) mode=stdout ;;
        *) echo "roadmap.sh: unknown option $arg" >&2; exit 2 ;;
    esac
done

# Значение строки шапки `**Key**: value` (первое совпадение в первых 40 строках).
header() {
    sed -n '1,40p' "$1" | sed -n "s/^\*\*$2\*\*:[[:space:]]*//p" | head -n1
}

# Активная фича в роадмап не попадает: `.specify/feature.json` — локальное
# состояние рабочей копии (в `.gitignore`), а этот файл коммитится и сверяется в CI.
render() {
    cat <<'EOF'
# Roadmap

<!-- СГЕНЕРИРОВАНО .specify/scripts/typsidian/roadmap.sh — руками не править.
     Стадия выводится из артефактов каталога фичи; приоритет и суть —
     строки `**Priority**` / `**Summary**` её spec.md. -->

Все фичи проекта ведутся через Spec Kit: одна фича — один каталог `specs/NNN-имя/`.
Стадия: `spec` → `plan` → `tasks` → `implement N/M` → `done`.

| № | Фича | Приоритет | Стадия | Вопросы | Суть |
|---|------|-----------|--------|---------|------|
EOF
    local dir name num title prio summary stage total done_n open
    for dir in "$root"/specs/[0-9][0-9][0-9]-*/; do
        [[ -f "$dir/spec.md" ]] || continue
        name="$(basename "$dir")"
        num="${name%%-*}"
        title="$(sed -n '1s/^# Feature Specification:[[:space:]]*//p' "$dir/spec.md")"
        [[ -n "$title" ]] || title="${name#*-}"
        prio="$(header "$dir/spec.md" Priority)"
        summary="$(header "$dir/spec.md" Summary)"
        open="$(grep -c 'NEEDS CLARIFICATION' "$dir/spec.md" || true)"

        if [[ -f "$dir/tasks.md" ]]; then
            total="$(grep -cE '^- \[[ xX]\] T[0-9]+' "$dir/tasks.md" || true)"
            done_n="$(grep -cE '^- \[[xX]\] T[0-9]+' "$dir/tasks.md" || true)"
            if [[ "$total" -gt 0 && "$done_n" -eq "$total" ]]; then
                stage="done"
            elif [[ "$done_n" -eq 0 ]]; then
                stage="tasks 0/$total"
            else
                stage="implement $done_n/$total"
            fi
        elif [[ -f "$dir/plan.md" ]]; then
            stage="plan"
        else
            stage="spec"
        fi

        # Буквальный `|` разорвал бы ячейку таблицы.
        title="${title//|/\\|}"
        summary="${summary//|/\\|}"

        printf '| %s | [%s](%s/spec.md) | %s | %s | %s | %s |\n' \
            "$num" "$title" "$name" "${prio:-—}" "$stage" \
            "$([[ "$open" -gt 0 ]] && echo "$open" || echo "—")" "${summary:-—}"
    done
    printf '\nАктивная фича: `.specify/scripts/typsidian/feature.sh`.\n'
}

case "$mode" in
    stdout) render ;;
    check)
        if [[ -f "$out" ]] && diff -q <(render) "$out" >/dev/null; then
            exit 0
        fi
        echo "specs/ROADMAP.md is stale — run .specify/scripts/typsidian/roadmap.sh" >&2
        exit 1
        ;;
    write)
        tmp="$(mktemp "$out.XXXXXX")"
        render >"$tmp"
        mv "$tmp" "$out"
        ;;
esac
