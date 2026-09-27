# Android SDK/NDK (решение T116 пересмотрено).
#
# 001/plan.md откладывал мобильную оболочку с обоснованием «SDK нет в
# окружении, а класть в воспроизводимую среду тулчейн, который здесь нельзя ни
# собрать, ни запустить, — обещание без прогона». Обоснование снято ровно в той
# части, в которой оно было верным: `androidenv` собирает SDK и NDK
# декларативно, из того же `flake.lock`, и сборка проверяема здесь же
# (`nix develop .#android`).
#
# Лицензия Google на SDK несвободна. Разрешение выдаётся не глобальным
# `allowUnfree`, а точечным предикатом по `meta.homepage`: все пакеты
# `androidenv` объявлены одним общим `meta`
# (nixpkgs/pkgs/development/mobile/androidenv/default.nix), поэтому предикат
# пропускает ровно их и ничего сверх.
{ nixpkgs, system }:
let
  pkgsAndroid = import nixpkgs {
    inherit system;
    config = {
      android_sdk.accept_license = true;
      allowUnfreePredicate =
        pkg: (pkg.meta.homepage or "") == "https://developer.android.com/tools";
    };
  };

  # NDK 27 — LTS-ветка Google и та, на которой у Rust нет известных
  # расхождений по `libunwind`/`libgcc`. 29 (latest на момент правки) свежее,
  # чем то, на чём проверен `cargo-mobile2` внутри tauri-cli.
  ndkVersion = "27.2.12479018";
  # 35.0.0 — то, что берёт по умолчанию Android Gradle Plugin 8.11 из шаблона
  # Tauri. Версия участвует не только в сборке: на неё же указывает обход aapt2
  # в `shells.nix`, и разъехавшись, они дали бы «файл не найден» на ровном месте.
  buildToolsVersion = "35.0.0";

  composition = pkgsAndroid.androidenv.composeAndroidPackages {
    # 36 — `compileSdk`/`targetSdk` шаблона Tauri
    # (`gen/android/app/build.gradle.kts`). Платформа нужна ровно та, против
    # которой компилируется проект; `minSdk` (24) в SDK ничего не требует.
    platformVersions = [ "36" ];
    buildToolsVersions = [ buildToolsVersion ];
    includeNDK = true;
    ndkVersions = [ ndkVersion ];
    # Эмулятор и образы системы — отдельные гигабайты, а проверять сборку можно
    # на устройстве. Если понадобится эмулятор, включается здесь.
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
in
{
  inherit ndkVersion buildToolsVersion;
  sdk = composition.androidsdk;
  sdkRoot = "${composition.androidsdk}/libexec/android-sdk";
}
