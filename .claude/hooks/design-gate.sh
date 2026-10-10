#!/usr/bin/env bash
# PreToolUse (Edit|Write|NotebookEdit): вёрстка идёт по макету. Компонент или
# стиль интерфейса правится, когда в plan.md активной фичи записано, по какому
# макету его верстать, — строкой «Макет — …» (скилл design-typsidian). Иначе
# правка требует явного согласия пользователя.
#
# Правило действует с фичи FIRST_FEATURE: более ранние спланированы до него, и
# дописывать им макет задним числом незачем.
#
# Выключатель для пользователя: TYPSIDIAN_DESIGN_GATE=off в окружении.

FIRST_FEATURE=13

[[ "${TYPSIDIAN_DESIGN_GATE:-on}" == "off" ]] && exit 0
command -v jq >/dev/null 2>&1 || exit 0 # без jq гейт молчит: он подсказка, а не замок

input="$(cat)"
file="$(jq -r '.tool_input.file_path // .tool_input.notebook_path // empty' <<<"$input")"
[[ -n "$file" ]] || exit 0

root="${CLAUDE_PROJECT_DIR:-$(jq -r '.cwd // empty' <<<"$input")}"
rel="${file#"$root"/}"
[[ "$rel" != "$file" ]] || exit 0 # вне проекта

# Только то, что рисует интерфейс: клиенты ядра, типы и тесты макета не требуют.
case "$rel" in
    ui/src/i18n/* | *.test.tsx) exit 0 ;;
    ui/*.tsx | ui/*.css | apps/*.tsx | apps/*.css) ;;
    *) exit 0 ;;
esac

fj="$root/.specify/feature.json"
dir=""
[[ -f "$fj" ]] && dir="$(jq -r '.feature_directory // empty' "$fj" 2>/dev/null)"
plan="$root/$dir/plan.md"
# Нет активной фичи или плана — об этом уже спросит spec-gate.
[[ -n "$dir" && -f "$plan" ]] || exit 0

num="$(basename "$dir")"
num="${num%%-*}"
[[ "$num" =~ ^[0-9]+$ ]] || exit 0
((10#$num >= FIRST_FEATURE)) || exit 0

# Незаполненная строка шаблона («Макет — [...]») макетом не считается.
grep -qE '^Макет — [^[]' "$plan" && exit 0

jq -n --arg r "design-gate: в $dir/plan.md нет строки «Макет — …», а правится интерфейс ($rel). \
Сначала нарисуй макет (скилл design-typsidian) и запиши ссылку в Summary плана; \
если фиче макет не нужен — «Макет — не требуется: <причина>». Либо подтверди правку без макета." \
    '{hookSpecificOutput: {hookEventName: "PreToolUse",
        permissionDecision: "ask", permissionDecisionReason: $r}}'
exit 0
