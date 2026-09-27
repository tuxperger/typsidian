# Тулчейн Rust из fenix.
#
# Список таргетов — параметр, потому что цена у них разная: wasm нужен всем и
# всегда (принцип II), а четыре android-таргета имеет смысл тянуть только в
# android-оболочку.
{ fenix, system }:
let
  f = fenix.packages.${system};
in
{
  rustToolchainWith =
    extraTargets:
    f.combine (
      [
        (f.stable.withComponents [
          "cargo"
          "clippy"
          "rust-src"
          "rustc"
          "rustfmt"
          "rust-analyzer"
        ])
        f.targets.wasm32-unknown-unknown.stable.rust-std
      ]
      ++ map (t: f.targets.${t}.stable.rust-std) extraTargets
    );
}
