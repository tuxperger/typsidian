#!/usr/bin/env bash
# Stop: не заканчивать ход с деревом, которое отвергают самые дешёвые гейты.
#   - изменённые в рабочем дереве .rs проходят `cargo fmt --all --check`
#   - specs/ROADMAP.md соответствует артефактам на диске
# Блокирует не больше одного раза за остановку (stop_hook_active) — зациклиться
# не может. cargo fmt не берёт лок target/, так что чужой сборке не мешает.

command -v jq >/dev/null 2>&1 || exit 0

input="$(cat)"
[[ "$(jq -r '.stop_hook_active // false' <<<"$input")" == "true" ]] && exit 0

root="${CLAUDE_PROJECT_DIR:-$(jq -r '.cwd // empty' <<<"$input")}"
cd "$root" 2>/dev/null || exit 0

problems=()

if git status --porcelain 2>/dev/null | grep -qE '\.rs$'; then
    if command -v cargo >/dev/null 2>&1; then
        out="$(cargo fmt --all --check 2>/dev/null)"
    elif command -v nix >/dev/null 2>&1; then
        out="$(nix develop -c cargo fmt --all --check 2>/dev/null)"
    else
        out=""
    fi
    files="$(sed -n 's/^Diff in \(.*\):[0-9]*:$/\1/p' <<<"$out" | sed "s#^$root/##" | sort -u | tr '\n' ' ')"
    [[ -n "$files" ]] &&
        problems+=("cargo fmt --all --check не проходит: ${files}(nix develop -c cargo fmt --all)")
fi

if [[ -x .specify/scripts/typsidian/roadmap.sh ]]; then
    .specify/scripts/typsidian/roadmap.sh --check 2>/dev/null ||
        problems+=("specs/ROADMAP.md устарел — запусти .specify/scripts/typsidian/roadmap.sh")
fi

[[ ${#problems[@]} -eq 0 ]] && exit 0
jq -n --arg r "stop-gate: $(printf '%s; ' "${problems[@]}")исправь или явно сообщи пользователю, почему оставляешь так." \
    '{decision: "block", reason: $r}'
exit 0
