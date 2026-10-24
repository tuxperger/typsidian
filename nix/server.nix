# Пакет сервера web-версии (фича 037): `nix build .#server`.
#
# Только бинарь `typsidian-server` — ядро у него нативное, wasm и WebKit не нужны.
# Статика web-версии в пакет не входит: `next build` берёт шрифты интерфейса из
# Google Fonts во время сборки (`next/font/google`), а в песочнице Nix сети нет.
# Её собирают в dev-shell (`pnpm --filter @typsidian/web build:server`) и
# передают серверу флагом `--static` или `static_dir` в настройках — README.
{
  pkgs,
  rust,
}:
let
  inherit (pkgs) lib;

  rustPlatform = pkgs.makeRustPlatform {
    cargo = rust;
    rustc = rust;
  };

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../packages
      ../apps
    ];
  };
in
rustPlatform.buildRustPackage {
  pname = "typsidian-server";
  version = "0.1.0";
  inherit src;

  cargoLock.lockFile = ../Cargo.lock;
  cargoBuildFlags = [ "-p" "typsidian-server" ];

  # Тот же обход `rust-lld` из fenix, что у пакета рабочего стола (`desktop.nix`).
  "CARGO_TARGET_${pkgs.stdenv.hostPlatform.rust.cargoEnvVarTarget}_RUSTFLAGS" =
    "-C linker-features=-lld";

  # Тесты — `cargo test -p typsidian-server` в dev-shell (гейт `core`).
  doCheck = false;

  meta = {
    description = "Typsidian — сервер web-версии: вход, свои vault, команды ядра по HTTP";
    mainProgram = "typsidian-server";
    platforms = lib.platforms.linux;
  };
}
