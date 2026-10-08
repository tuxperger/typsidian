# Пакет оболочки рабочего стола: `nix build` (`.#desktop`).
#
# Сборка целиком внутри Nix, без сети и без того, что лежит в рабочей копии
# (`target/`, `node_modules/`, `ui/dist/`): Rust-зависимости — из `Cargo.lock`,
# npm-зависимости — из `pnpm-lock.yaml` по хэшу ниже.
#
# Порядок тот же, что руками в dev-shell, и по тем же причинам:
# 1. ядро под wasm32 и `wasm-bindgen` → `ui/src/wasm-core`. Оболочке рабочего
#    стола wasm не нужен (ядро у неё нативное), но `ui` без сгенерированных
#    типов не проходит `tsc`;
# 2. `@typsidian/ui` → `ui/dist`, откуда его берёт Vite оболочки;
# 3. `cargo tauri build` — его `beforeBuildCommand` сам собирает фронтенд
#    (`pnpm build:vite`), и деб-пакет разворачивается в `$out` хуком nixpkgs.
#
# После изменения `pnpm-lock.yaml` сборка остановится на `pnpmDeps` с
# несовпадением хэша: переписать `hash` ниже тем, что назовёт ошибка.
{
  pkgs,
  rust,
  gui,
  wasmBindgenCli,
}:
let
  inherit (pkgs) lib;

  rustPlatform = pkgs.makeRustPlatform {
    cargo = rust;
    rustc = rust;
  };

  # Тот же pnpm, что в dev-shell: лок-файл пишется им.
  pnpm = pkgs.pnpm;

  # Только то, что участвует в сборке. Спеки, настройки агентов и прочее к
  # пакету отношения не имеют, и их правка не должна его пересобирать.
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../package.json
      ../pnpm-lock.yaml
      ../pnpm-workspace.yaml
      ../packages
      ../apps
      ../ui
    ];
  };

  # Оболочка и то, от чего она зависит (`@typsidian/ui`). Next.js веб-клиента
  # сюда не попадает — ему в пакете рабочего стола делать нечего.
  workspaces = [ "@typsidian/desktop..." ];

  tauriConf = builtins.fromJSON (builtins.readFile ../apps/desktop/src-tauri/tauri.conf.json);
in
pkgs.stdenv.mkDerivation (finalAttrs: {
  pname = "typsidian-desktop";
  inherit (tauriConf) version;
  inherit src;

  cargoDeps = rustPlatform.importCargoLock { lockFile = ../Cargo.lock; };

  pnpmWorkspaces = workspaces;
  pnpmDeps = pkgs.fetchPnpmDeps {
    inherit (finalAttrs) pname version src;
    inherit pnpm;
    pnpmWorkspaces = workspaces;
    fetcherVersion = 4;
    hash = "sha256-6S6kXIdXPBIq/mixu4hBn36rWLn/mUa9A1wc/Ui0wWQ=";
  };

  nativeBuildInputs = [
    # Хук nixpkgs, но с нашим cargo: тулчейн fenix несёт rust-std под wasm32,
    # а cargo из nixpkgs — только под хост.
    (pkgs.cargo-tauri.hook.override { cargo = rust; })
    rust
    rustPlatform.cargoSetupHook
    pkgs.nodejs_22
    pnpm
    pkgs.pnpmConfigHook
    wasmBindgenCli
    pkgs.pkg-config
    # Схемы GSettings и модули GIO — то, что dev-shell выставляет руками в
    # XDG_DATA_DIRS; без них оболочка падает на первом диалоге выбора папки.
    pkgs.wrapGAppsHook3
  ];

  buildInputs = gui.libs ++ [
    # TLS для WebKit (загрузка шаблонов Typst из сети идёт через ядро, но
    # WebKit без модуля GIO сыплет предупреждениями и не открывает https).
    pkgs.glib-networking
  ];

  buildAndTestSubdir = "apps/desktop/src-tauri";

  # С Rust 1.90 на x86_64-linux линкует встроенный `rust-lld` в обход
  # ld-wrapper'а Nix — и в бинарнике не остаётся RPATH к библиотекам из
  # хранилища (`libgdk-3.so.0: cannot open shared object file` при запуске).
  # rustc из nixpkgs это отключает патчем, rustc из fenix — нет. Только для
  # хоста: wasm32 линкуется своим lld и RPATH ему не нужен.
  "CARGO_TARGET_${pkgs.stdenv.hostPlatform.rust.cargoEnvVarTarget}_RUSTFLAGS" =
    "-C linker-features=-lld";

  preBuild = ''
    cargo build -p typsidian-core --lib --release \
      --target wasm32-unknown-unknown --offline -j "$NIX_BUILD_CORES"
    wasm-bindgen --target web --out-dir ui/src/wasm-core \
      target/wasm32-unknown-unknown/release/typsidian_core.wasm
    pnpm --filter @typsidian/ui build
  '';

  # Тесты — в dev-shell на обоих таргетах (принцип II); сборку пакета они бы
  # только удвоили.
  doCheck = false;

  meta = {
    description = "Typsidian — заметки на Typst, оболочка рабочего стола";
    mainProgram = "typsidian-desktop";
    platforms = lib.platforms.linux;
  };
})
