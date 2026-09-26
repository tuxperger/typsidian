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

        # Общий тулчейн Rust. Список таргетов — параметр, потому что цена у них
        # разная: wasm нужен всем и всегда (принцип II), а четыре
        # android-таргета имеет смысл тянуть только в android-оболочку.
        rustToolchainWith =
          extraTargets:
          fenix.packages.${system}.combine (
            [
              (fenix.packages.${system}.stable.withComponents [
                "cargo"
                "clippy"
                "rust-src"
                "rustc"
                "rustfmt"
                "rust-analyzer"
              ])
              fenix.packages.${system}.targets.wasm32-unknown-unknown.stable.rust-std
            ]
            ++ map (t: fenix.packages.${system}.targets.${t}.stable.rust-std) extraTargets
          );

        # ── Android SDK/NDK (решение T116 пересмотрено) ─────────────────────
        #
        # 001/plan.md откладывал мобильную оболочку с обоснованием «SDK нет в
        # окружении, а класть в воспроизводимую среду тулчейн, который здесь
        # нельзя ни собрать, ни запустить, — обещание без прогона». Обоснование
        # снято ровно в той части, в которой оно было верным: `androidenv`
        # собирает SDK и NDK декларативно, из того же `flake.lock`, и сборка
        # проверяема здесь же (`nix develop .#android`).
        #
        # Отдельный devShell, а не добавка к default: SDK+NDK весят ~3 ГБ, и
        # платить их каждому `nix develop` ради web/desktop незачем.
        #
        # Лицензия Google на SDK несвободна. Разрешение выдаётся не глобальным
        # `allowUnfree`, а точечным предикатом по `meta.homepage`: все пакеты
        # `androidenv` объявлены одним общим `meta`
        # (nixpkgs/pkgs/development/mobile/androidenv/default.nix), поэтому
        # предикат пропускает ровно их и ничего сверх.
        pkgsAndroid = import nixpkgs {
          inherit system;
          config = {
            android_sdk.accept_license = true;
            allowUnfreePredicate =
              pkg: (pkg.meta.homepage or "") == "https://developer.android.com/tools";
          };
        };

        # NDK 27 — LTS-ветка Google и та, на которой у Rust нет известных
        # расхождений по `libunwind`/`libgcc`. 29 (latest на момент правки)
        # свежее, чем то, на чём проверен `cargo-mobile2` внутри tauri-cli.
        androidNdkVersion = "27.2.12479018";
        # 35.0.0 — то, что берёт по умолчанию Android Gradle Plugin 8.11 из
        # шаблона Tauri. Версия участвует не только в сборке: на неё же указывает
        # обход aapt2 ниже, и разъехавшись, они дали бы «файл не найден» на
        # ровном месте.
        androidBuildToolsVersion = "35.0.0";

        androidComposition = pkgsAndroid.androidenv.composeAndroidPackages {
          # 36 — `compileSdk`/`targetSdk` шаблона Tauri
          # (`gen/android/app/build.gradle.kts`). Платформа нужна ровно та,
          # против которой компилируется проект; `minSdk` (24) в SDK ничего не
          # требует.
          platformVersions = [ "36" ];
          buildToolsVersions = [ androidBuildToolsVersion ];
          includeNDK = true;
          ndkVersions = [ androidNdkVersion ];
          # Эмулятор и образы системы — отдельные гигабайты, а проверять сборку
          # можно на устройстве. Если понадобится эмулятор, включается здесь.
          includeEmulator = false;
          includeSystemImages = false;
          # CMake из SDK не нужен: нативные зависимости ядра (`libsqlite3-sys`)
          # собираются через `cc` компилятором из NDK.
          includeCmake = false;
          abiVersions = [
            "arm64-v8a"
            "x86_64"
          ];
        };

        androidSdkRoot = "${androidComposition.androidsdk}/libexec/android-sdk";

        # Пакеты, общие для всех оболочек.
        commonBuildInputs = [
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

        # Библиотеки, которые ищутся и при сборке (pkg-config), и при запуске.
        graphicalLibs = [
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
        ];

        # Каталоги со схемами GSettings, пригодные для XDG_DATA_DIRS.
        #
        # В XDG_DATA_DIRS кладётся каталог, СОДЕРЖАЩИЙ `glib-2.0/schemas`, а в
        # nixpkgs схемы лежат на уровень глубже — в
        # `share/gsettings-schemas/<имя-пакета-с-версией>`. Отсюда `.name`:
        # просто `${pkgs.gtk3}/share` не сработает.
        gsettingsSchemaDirs = pkgs.lib.concatStringsSep ":" [
          "${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}"
          "${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}"
        ];

        commonShellHook = ''
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
          export PKG_CONFIG_PATH="${pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" graphicalLibs}:$PKG_CONFIG_PATH"
          export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath graphicalLibs}:$LD_LIBRARY_PATH"
          # Схемы GSettings для GTK3.
          #
          # `mkShell` выставляет GSETTINGS_SCHEMAS_PATH сам (setup-hook у glib и
          # gtk3), но в рантайме GLib читает не его, а XDG_DATA_DIRS — связать
          # одно с другим обычно берётся `wrapGAppsHook`, которого в dev-shell
          # нет. Без этой строки оболочка падает в тот момент, когда GTK впервые
          # трогает схему (на практике — диалог выбора папки):
          # "Settings schema 'org.gtk.Settings.FileChooser' is not installed".
          # У GLib это уровень ERROR, то есть abort с образом памяти, а не
          # предупреждение.
          #
          # Системный XDG_DATA_DIRS не выручает: в него попадает `-dev`-выход
          # gtk3 (заголовки, без схем), а сами схемы лежат в обычном выходе.
          #
          # Значение то же, что у обёртки `nix run .#desktop` ниже: разойдясь,
          # они дали бы «у одного работает, у другого нет» на одном и том же
          # бинаре.
          export XDG_DATA_DIRS="${gsettingsSchemaDirs}:$XDG_DATA_DIRS"
        '';
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = [ (rustToolchainWith [ ]) ] ++ commonBuildInputs;

          shellHook = commonShellHook + ''
            echo "Typst-Obsidian dev shell готов. Проверка: specify check"
          '';
        };

        # Оболочка для мобильного таргета: всё из default плюс Android SDK/NDK,
        # JDK и rust-std под четыре ABI Android.
        #
        # Вход: `nix develop .#android`.
        devShells.android = pkgs.mkShell {
          buildInputs =
            [
              (rustToolchainWith [
                "aarch64-linux-android"
                "armv7-linux-androideabi"
                "i686-linux-android"
                "x86_64-linux-android"
              ])
              androidComposition.androidsdk
              # Gradle из шаблона Tauri требует JDK 17+; берём его же, а не
              # системный, — системного на NixOS нет вовсе.
              pkgs.jdk17
            ]
            ++ commonBuildInputs;

          shellHook = commonShellHook + ''
            export ANDROID_HOME="${androidSdkRoot}"
            export ANDROID_SDK_ROOT="$ANDROID_HOME"
            # `cargo-mobile2` внутри tauri-cli ищет NDK именно по NDK_HOME;
            # gradle-плагин Android — по ANDROID_NDK_ROOT. Задаём обе.
            export NDK_HOME="$ANDROID_HOME/ndk/${androidNdkVersion}"
            export ANDROID_NDK_ROOT="$NDK_HOME"
            export JAVA_HOME="${pkgs.jdk17.home}"
            # Gradle по умолчанию тянет aapt2 из Maven generic-linux бинарём,
            # который на NixOS не стартует (та же болезнь, что у chromedriver
            # выше). Подсовываем пропатченный из nixpkgs.
            export GRADLE_OPTS="-Dorg.gradle.project.android.aapt2FromMavenOverride=$ANDROID_HOME/build-tools/${androidBuildToolsVersion}/aapt2 $GRADLE_OPTS"
            echo "Typst-Obsidian android shell готов. Сборка: pnpm --filter @typsidian/mobile build"
          '';
        };

        # Запуск собранной оболочки: `nix run .#desktop` из корня репозитория.
        #
        # Зачем отдельный вход, если есть dev-shell. Бинарь в `target/release/`
        # на NixOS не самодостаточен: ему нужны и `LD_LIBRARY_PATH` (libgdk,
        # WebKit), и `XDG_DATA_DIRS` (схемы GSettings). Пока оба приходили
        # только из `shellHook`, запуск зависел от того, КОГДА пользователь
        # вошёл в shell: вход до правки флейка давал старое окружение, и один и
        # тот же бинарь у одного работал, а у другого падал с abort'ом. Здесь
        # окружение вычисляется в момент запуска и устареть не может.
        #
        # Это не замена пакету: деривации, собирающей проект, тут нет, и бинарь
        # обязан быть собран заранее (`pnpm --filter @typsidian/desktop build`).
        # Обёртка ровно над запуском — поэтому и проверка пути ниже явная, с
        # внятным текстом вместо «No such file or directory».
        apps.desktop = {
          type = "app";
          meta.description = "Запуск собранной оболочки рабочего стола Typsidian";
          program = "${pkgs.writeShellScript "typsidian-desktop" ''
            set -eu
            binary="''${PWD}/target/release/typsidian-desktop"
            if [ ! -x "$binary" ]; then
              echo "typsidian: не найден $binary" >&2
              echo "Сначала собрать: nix develop --command pnpm --filter @typsidian/desktop build" >&2
              echo "(и запускать из корня репозитория)" >&2
              exit 1
            fi
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath graphicalLibs}:''${LD_LIBRARY_PATH:-}"
            export XDG_DATA_DIRS="${gsettingsSchemaDirs}:''${XDG_DATA_DIRS:-}"
            exec "$binary" "$@"
          ''}";
        };
      }
    );
}
