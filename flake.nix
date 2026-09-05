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

        rustToolchain = fenix.packages.${system}.combine [
          (fenix.packages.${system}.stable.withComponents [
            "cargo"
            "clippy"
            "rust-src"
            "rustc"
            "rustfmt"
            "rust-analyzer"
          ])
          fenix.packages.${system}.targets.wasm32-unknown-unknown.stable.rust-std
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.wasm-pack
            pkgs.wasm-bindgen-cli
            pkgs.binaryen # wasm-opt
            # wasm-bindgen-test гоняет тесты в реальном браузере. Свой
            # chromedriver wasm-pack скачивает generic-linux бинарём, который на
            # NixOS не стартует ("libglib-2.0.so.0: cannot open shared object
            # file") — берём оба из nixpkgs и указываем на них явно ниже.
            pkgs.chromium
            pkgs.chromedriver
            pkgs.nodejs_22
            pkgs.pnpm
            pkgs.uv # для specify-cli (Spec Kit)
            pkgs.python3 # даём uv нативный, собранный под NixOS python
            pkgs.neovim # vim-мост, headless backend
            pkgs.sqlite
            pkgs.git

            # ── Сборка Tauri-оболочек (001 T111) ────────────────────────────
            # Tauri на Linux линкуется с системным WebKitGTK, а не тащит свой
            # движок. Без этих пакетов `cargo tauri build` падает на этапе
            # линковки, не найдя `webkit2gtk-4.1` через pkg-config, — то есть
            # `apps/desktop` невозможно даже собрать, а без него не выполнима
            # native-половина ручных прогонов quickstart (принцип II).
            pkgs.cargo-tauri
            pkgs.pkg-config
            pkgs.gobject-introspection
            pkgs.webkitgtk_4_1
            pkgs.libsoup_3
            pkgs.gtk3
            pkgs.glib
            pkgs.cairo
            pkgs.pango
            pkgs.atk
            pkgs.gdk-pixbuf
            pkgs.librsvg
            pkgs.openssl
            # Tauri/GTK обращаются к шине сообщений (портал выбора файлов,
            # уведомления). Без libdbus собранная оболочка не находит
            # libdbus-1.so.3 во время работы — сборка при этом проходит, и
            # отсутствие проявилось бы только при запуске.
            pkgs.dbus
          ];

          shellHook = ''
            export PATH="$HOME/.local/bin:$PATH"
            # без этого uv пытается скачать generic-linux python и падает
            # на NixOS с "Could not start dynamically linked executable"
            export UV_PYTHON_PREFERENCE="only-system"
            # Без этих двух wasm-pack скачивает собственный chromedriver, не
            # запускающийся на NixOS (принцип II требует, чтобы wasm-таргет
            # реально прогонялся, а не только собирался).
            export CHROMEDRIVER="${pkgs.chromedriver}/bin/chromedriver"
            export CHROME_PATH="${pkgs.chromium}/bin/chromium"
            # Сборка Tauri ищет системные библиотеки через pkg-config, а
            # собранная оболочка догружает WebKit во время работы. mkShell
            # выставляет PKG_CONFIG_PATH сам, но `cargo tauri` запускает сборку
            # своим процессом и до `webkit2gtk-4.1` без явного пути не
            # добирается — как и до `libwebkit2gtk` при запуске (001 T111).
            export PKG_CONFIG_PATH="${
              pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" [
                pkgs.webkitgtk_4_1
                pkgs.libsoup_3
                pkgs.gtk3
                pkgs.glib
                pkgs.cairo
                pkgs.pango
                pkgs.atk
                pkgs.gdk-pixbuf
                pkgs.librsvg
                pkgs.openssl
                pkgs.dbus
              ]
            }:$PKG_CONFIG_PATH"
            export LD_LIBRARY_PATH="${
              pkgs.lib.makeLibraryPath [
                pkgs.webkitgtk_4_1
                pkgs.libsoup_3
                pkgs.gtk3
                pkgs.glib
                pkgs.cairo
                pkgs.pango
                pkgs.atk
                pkgs.gdk-pixbuf
                pkgs.librsvg
                pkgs.openssl
                pkgs.dbus
              ]
            }:$LD_LIBRARY_PATH"
            echo "Typst-Obsidian dev shell готов. Проверка: specify check"
          '';
        };
      }
    );
}
