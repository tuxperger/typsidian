# Окружения разработки: `nix develop` и `nix develop .#android`.
{
  pkgs,
  toolchain,
  android,
  gui,
  wasmBindgenCli,
}:
let
  # Пакеты, общие для всех оболочек.
  commonBuildInputs = [
    pkgs.wasm-pack
    # Та же версия, что у крейта в `Cargo.lock` (`wasm-bindgen.nix`): с ней
    # wasm-pack берёт CLI из PATH и не скачивает свой.
    wasmBindgenCli
    pkgs.binaryen # wasm-opt
    # wasm-bindgen-test гоняет тесты в реальном браузере. Свой chromedriver
    # wasm-pack скачивает generic-linux бинарём, который на NixOS не стартует
    # ("libglib-2.0.so.0: cannot open shared object file") — берём оба из
    # nixpkgs и указываем на них явно ниже.
    pkgs.chromium
    pkgs.chromedriver
    pkgs.nodejs_22
    pkgs.pnpm
    pkgs.uv # для specify-cli (Spec Kit)
    pkgs.python3 # даём uv нативный, собранный под NixOS python
    pkgs.neovim # vim-мост, headless backend
    pkgs.sqlite
    pkgs.git

    # ── Сборка Tauri-оболочек (001 T111) ──────────────────────────────────
    pkgs.cargo-tauri
    pkgs.pkg-config
    pkgs.gobject-introspection
  ]
  ++ gui.libs;

  commonShellHook = ''
    export PATH="$HOME/.local/bin:$PATH"
    # без этого uv пытается скачать generic-linux python и падает на NixOS с
    # "Could not start dynamically linked executable"
    export UV_PYTHON_PREFERENCE="only-system"
    # Без этих двух wasm-pack скачивает собственный chromedriver, не
    # запускающийся на NixOS (принцип II требует, чтобы wasm-таргет реально
    # прогонялся, а не только собирался).
    export CHROMEDRIVER="${pkgs.chromedriver}/bin/chromedriver"
    export CHROME_PATH="${pkgs.chromium}/bin/chromium"
    # Сборка Tauri ищет системные библиотеки через pkg-config, а собранная
    # оболочка догружает WebKit во время работы. mkShell выставляет
    # PKG_CONFIG_PATH сам, но `cargo tauri` запускает сборку своим процессом и
    # до `webkit2gtk-4.1` без явного пути не добирается — как и до
    # `libwebkit2gtk` при запуске (001 T111).
    export PKG_CONFIG_PATH="${pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" gui.libs}:$PKG_CONFIG_PATH"
    export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath gui.libs}:$LD_LIBRARY_PATH"
    # Схемы GSettings для GTK3.
    #
    # `mkShell` выставляет GSETTINGS_SCHEMAS_PATH сам (setup-hook у glib и
    # gtk3), но в рантайме GLib читает не его, а XDG_DATA_DIRS — связать одно с
    # другим обычно берётся `wrapGAppsHook`, которого в dev-shell нет. Без этой
    # строки оболочка, запущенная из shell, падает в тот момент, когда GTK
    # впервые трогает схему (на практике — диалог выбора папки): "Settings
    # schema 'org.gtk.Settings.FileChooser' is not installed". У GLib это
    # уровень ERROR, то есть abort с образом памяти, а не предупреждение.
    #
    # Системный XDG_DATA_DIRS не выручает: в него попадает `-dev`-выход gtk3
    # (заголовки, без схем), а сами схемы лежат в обычном выходе.
    export XDG_DATA_DIRS="${gui.gsettingsSchemaDirs}:$XDG_DATA_DIRS"
  '';
in
{
  default = pkgs.mkShell {
    buildInputs = [ (toolchain.rustToolchainWith [ ]) ] ++ commonBuildInputs;

    shellHook = commonShellHook + ''
      echo "Typst-Obsidian dev shell готов. Проверка: specify check"
    '';
  };

  # Оболочка для мобильного таргета: всё из default плюс Android SDK/NDK, JDK и
  # rust-std под четыре ABI Android.
  #
  # Отдельный devShell, а не добавка к default: SDK+NDK весят ~3 ГБ, и платить
  # их каждому `nix develop` ради web/desktop незачем.
  android = pkgs.mkShell {
    buildInputs = [
      (toolchain.rustToolchainWith [
        "aarch64-linux-android"
        "armv7-linux-androideabi"
        "i686-linux-android"
        "x86_64-linux-android"
      ])
      android.sdk
      # Gradle из шаблона Tauri требует JDK 17+; берём его же, а не системный,
      # — системного на NixOS нет вовсе.
      pkgs.jdk17
    ]
    ++ commonBuildInputs;

    shellHook = commonShellHook + ''
      export ANDROID_HOME="${android.sdkRoot}"
      export ANDROID_SDK_ROOT="$ANDROID_HOME"
      # `cargo-mobile2` внутри tauri-cli ищет NDK именно по NDK_HOME;
      # gradle-плагин Android — по ANDROID_NDK_ROOT. Задаём обе.
      export NDK_HOME="$ANDROID_HOME/ndk/${android.ndkVersion}"
      export ANDROID_NDK_ROOT="$NDK_HOME"
      export JAVA_HOME="${pkgs.jdk17.home}"
      # Gradle по умолчанию тянет aapt2 из Maven generic-linux бинарём, который
      # на NixOS не стартует (та же болезнь, что у chromedriver выше).
      # Подсовываем пропатченный из nixpkgs.
      export GRADLE_OPTS="-Dorg.gradle.project.android.aapt2FromMavenOverride=$ANDROID_HOME/build-tools/${android.buildToolsVersion}/aapt2 $GRADLE_OPTS"
      echo "Typst-Obsidian android shell готов. Сборка: pnpm --filter @typsidian/mobile build"
    '';
  };
}
