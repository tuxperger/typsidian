{
  description = "Typst-Obsidian dev environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  # Флейк только связывает части; сами они — в `nix/`:
  #
  #   toolchain.nix     Rust из fenix (+ wasm32, + android по запросу)
  #   android.nix       Android SDK/NDK для `.#android`
  #   gui.nix           WebKitGTK и окружение запуска оболочек Tauri
  #   wasm-bindgen.nix  CLI ровно той версии, что в Cargo.lock
  #   shells.nix        `nix develop`, `nix develop .#android`
  #   desktop.nix       пакет оболочки рабочего стола — `nix build`
  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      fenix,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        toolchain = import ./nix/toolchain.nix { inherit fenix system; };
        gui = import ./nix/gui.nix { inherit pkgs; };
        wasmBindgenCli = import ./nix/wasm-bindgen.nix { inherit pkgs; };

        desktop = import ./nix/desktop.nix {
          inherit pkgs gui wasmBindgenCli;
          rust = toolchain.rustToolchainWith [ ];
        };
      in
      {
        packages = {
          inherit desktop;
          default = desktop;
          wasm-bindgen-cli = wasmBindgenCli;
        };

        # `nix run .#desktop` (или просто `nix run`): Nix собирает пакет, если
        # его ещё нет в хранилище или исходники изменились, и запускает.
        # Окружение запуска (схемы GSettings, модули GIO) пакет несёт сам —
        # обёрткой `wrapGAppsHook3`, — поэтому от dev-shell и от каталога, из
        # которого запускают, он не зависит.
        apps = rec {
          desktop = {
            type = "app";
            meta.description = "Typsidian — оболочка рабочего стола";
            program = pkgs.lib.getExe self.packages.${system}.desktop;
          };
          default = desktop;
        };

        devShells = import ./nix/shells.nix {
          inherit pkgs toolchain gui wasmBindgenCli;
          # SDK+NDK вычисляются лениво: платит только `nix develop .#android`.
          android = import ./nix/android.nix { inherit nixpkgs system; };
        };
      }
    );
}
