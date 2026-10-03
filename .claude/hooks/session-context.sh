#!/usr/bin/env bash
# SessionStart: состояние Spec Kit перед глазами агента — какая фича активна, на
# какой она стадии и какая команда следующая. stdout уходит в контекст сессии.

root="${CLAUDE_PROJECT_DIR:-$(pwd)}"
feature="$root/.specify/scripts/typsidian/feature.sh"
[[ -x "$feature" ]] || exit 0

active="$("$feature" show 2>/dev/null)"
next_cmd=""
case "$active" in
    *"— spec") next_cmd="/speckit-clarify → /speckit-plan" ;;
    *"— plan") next_cmd="/speckit-tasks" ;;
    *"— implement"*) next_cmd="/speckit-implement (затем /speckit-converge)" ;;
    *"— done") next_cmd="фича закрыта — выбери следующую: .specify/scripts/typsidian/feature.sh next" ;;
esac

echo "## Spec Kit: состояние проекта"
echo
echo "Активная фича: ${active:-нет}"
[[ -n "$next_cmd" ]] && echo "Следующий шаг: $next_cmd"
echo
echo "Все фичи:"
"$feature" list 2>/dev/null | sed 's/^/  /'
echo
echo "feature.json — след последней /speckit-* команды, а не приоритет: что делать"
echo "дальше, решают незакрытые задачи выше (скилл roadmap-typsidian)."
echo "Код в packages/, ui/, apps/, nix/ меняется под задачи tasks.md активной фичи"
echo "(скилл flow-typsidian); тулчейн — только через nix develop -c."

dirty="$(git -C "$root" status --short 2>/dev/null | grep -vc '^?? ' || true)"
if [[ "${dirty:-0}" -gt 0 ]]; then
    echo
    echo "В дереве $dirty незакоммиченных изменений в отслеживаемых файлах — это"
    echo "накопленная работа пользователя: не откатывать и не коммитить заодно."
fi
exit 0
