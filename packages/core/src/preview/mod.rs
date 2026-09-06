//! Рендер заметок и живой предпросмотр (фича 005).
//!
//! Границу здесь провело одно свойство `typst`: трейт [`typst::World`]
//! **синхронный** (`typst-library-0.15.1/src/lib.rs:60`), а web-бэкенд vault
//! асинхронный целиком. Прямо подружить их нельзя, поэтому сборка разделена на
//! две фазы (research.md §2):
//!
//! 1. **Резолв** — асинхронный, собирает [`DependencySnapshot`]: набор
//!    `FileId → Bytes`, вычитанный через существующий `VaultBackend` фичи 001.
//! 2. **Сборка** — синхронная, работает над снимком и к файловой системе не
//!    обращается вовсе.
//!
//! Замыкание зависимостей заранее неизвестно — какой файл понадобится,
//! выясняется по ходу вычисления, — поэтому фазы чередуются, пока снимок не
//! перестанет пополняться.
//!
//! Дополнительное следствие `World: Send + Sync`: в снимке лежат простые байты
//! и ничего больше. Браузерный хендл туда не положить даже случайно — код
//! просто не скомпилируется (research.md §11).

pub mod compile;
pub mod diagnostics;
pub mod fonts;
pub mod pages;
pub mod snapshot;
pub mod world;

#[cfg(target_arch = "wasm32")]
pub mod web;

pub use compile::{compile_note, CompileOutcome, CompiledDocument};
pub use diagnostics::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};
pub use fonts::FontSet;
pub use pages::{render_pages, RenderedPage};
pub use snapshot::{DependencySnapshot, SnapshotError, MAX_RESOLVE_ITERATIONS};
pub use world::PreviewWorld;
