#!/usr/bin/env bash
# Гейты качества проекта (конституция, «Рабочий процесс и гейты качества»).
#
# Usage: gates.sh [fast|core|ui|all]   (по умолчанию: fast)
#   fast  fmt --check + хардкод интерфейса + cargo check ядра под оба таргета,
#         свежесть обёрток команд host, cargo check host и сервера
#   core  fast + cargo test (ядро, host, сервер) + wasm-bindgen-test в браузере
#   ui    свежий wasm → сборка ui → tsc --noEmit по всем пакетам
#   all   core + ui
#
# Тулчейн живёт только в dev-shell: когда cargo нет в PATH, шаги идут через
# `nix develop -c`. Шаги строго последовательны — два cargo на один target/
# вешают друг друга на .cargo-lock. Параллельно с другим cargo не запускать.

set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$root" || exit 1

run() {
    if command -v cargo >/dev/null 2>&1 && command -v pnpm >/dev/null 2>&1; then
        "$@"
    elif command -v nix >/dev/null 2>&1; then
        nix develop -c "$@"
    else
        echo "gates.sh: нет ни тулчейна в PATH, ни nix" >&2
        return 127
    fi
}

failed=()
step() {
    local label="$1"
    shift
    printf '\n▸ %s\n' "$label"
    if ! "$@"; then
        failed+=("$label")
    fi
}

fast() {
    step "fmt" run cargo fmt --all --check
    step "i18n (хардкод интерфейса)" scripts/check-i18n.sh
    step "check (native)" run cargo check -p typsidian-core --all-targets
    step "check (wasm32)" run cargo check -p typsidian-core --target wasm32-unknown-unknown
    step "обёртки команд (host)" run node scripts/dev/gen-host-dispatch.mjs --check
    step "check (host, server)" run cargo check -p typsidian-host -p typsidian-server --all-targets
}

core() {
    fast
    step "cargo test (native)" run cargo test -p typsidian-core
    step "cargo test (host, server)" run cargo test -p typsidian-host -p typsidian-server
    step "wasm-bindgen-test (chrome)" run wasm-pack test --headless --chrome packages/core
}

# Порядок важен: типы приложений берутся из ui/dist, а тот — из ui/src/wasm-core.
# Без обеих пересборок зелёный ui при красном apps/* означает устаревший dist.
ui() {
    step "build:wasm" run pnpm build:wasm
    step "build ui" run pnpm --filter @typsidian/ui build
    step "typecheck" run pnpm typecheck
}

case "${1:-fast}" in
    fast) fast ;;
    core) core ;;
    ui) ui ;;
    all)
        core
        ui
        ;;
    *)
        sed -n '2,12p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
        exit 2
        ;;
esac

echo
if [[ ${#failed[@]} -eq 0 ]]; then
    echo "gates: OK (${1:-fast})"
else
    echo "gates: FAILED (${1:-fast})"
    printf '  - %s\n' "${failed[@]}"
    exit 1
fi
