# `wasm-bindgen-cli` ровно той версии, что записана в `Cargo.lock`.
#
# Версия CLI обязана совпадать с крейтом `wasm-bindgen` до патча: иначе он
# отказывается обрабатывать модуль («rust wasm file schema version … doesn't
# match»). nixpkgs держит одну, свою, и она уезжает вперёд `Cargo.lock` при
# каждом обновлении флейка — поэтому CLI собирается здесь под версию из
# лок-файла.
#
# После `cargo update -p wasm-bindgen` сборка остановится с просьбой добавить
# хэши новой версии в `hashes` ниже: подставить `lib.fakeHash`, собрать
# (`nix build .#wasm-bindgen-cli`) и переписать хэш из сообщения об ошибке.
{ pkgs }:
let
  inherit (pkgs) lib;

  lock = builtins.fromTOML (builtins.readFile ../Cargo.lock);
  version = (lib.findFirst (p: p.name == "wasm-bindgen") (throw "в Cargo.lock нет wasm-bindgen") lock.package).version;

  hashes = {
    "0.2.126" = {
      src = "sha256-H6Is3fiZVxZCfOMWK5dWMSrtn50VGv0sfdnsT+cTtyk=";
      cargo = "sha256-VucqkXbCi4qtQzY/HrXiDnbSURsagPsdNVMn1Tw3UiY=";
    };
  };

  pinned =
    hashes.${version}
      or (throw "nix/wasm-bindgen.nix: нет хэшей для wasm-bindgen-cli ${version} — добавьте их в `hashes`");

  src = pkgs.fetchCrate {
    pname = "wasm-bindgen-cli";
    inherit version;
    hash = pinned.src;
  };
in
pkgs.buildWasmBindgenCli {
  inherit src version;
  cargoDeps = pkgs.rustPlatform.fetchCargoVendor {
    inherit src;
    pname = "wasm-bindgen-cli";
    inherit version;
    hash = pinned.cargo;
  };
}
