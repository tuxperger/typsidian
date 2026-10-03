//! Набор шрифтов, доступный документу (FR-027).
//!
//! Правило спеки: встроенный набор приложения **плюс** шрифты из папки vault.
//! Шрифты, установленные в системе пользователя, не используются — иначе один
//! и тот же vault выглядел бы по-разному на web и на desktop (spec.md →
//! Clarifications).
//!
//! Сейчас встроенный набор берётся из `typst-assets`, вшитого в модуль. Это
//! временно и стоит 9,6 МБ веса: T029 переносит его в отдельные кэшируемые
//! файлы, а сюда байты будут приходить снаружи — форма [`FontSet::from_bytes`]
//! рассчитана именно на это и меняться не будет.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use typst::foundations::Bytes;
use typst::text::Font;

use super::resolve::ResolveError;
use crate::messages::{AppMessage, FontProblem, ResolveReason};
use crate::vault::backend::VaultBackend;

/// Шрифты сборки.
#[derive(Debug, Clone, Default)]
pub struct FontSet {
    fonts: Vec<Font>,
}

impl FontSet {
    /// Собирает набор из готовых байтов.
    ///
    /// Один файл может содержать несколько начертаний (TTC), поэтому из каждого
    /// извлекается всё, что в нём есть.
    pub fn from_bytes(files: impl IntoIterator<Item = Bytes>) -> Self {
        let fonts = files
            .into_iter()
            .flat_map(|data| (0..).map_while(move |index| Font::new(data.clone(), index)))
            .collect();
        Self { fonts }
    }

    /// Встроенный набор приложения.
    pub fn builtin() -> Self {
        Self::from_bytes(typst_assets::fonts().map(|bytes| Bytes::new(bytes.to_vec())))
    }

    /// Добавляет шрифты пользователя из папки vault (FR-027, FR-029).
    pub fn with_vault_fonts(mut self, files: impl IntoIterator<Item = Bytes>) -> Self {
        self.fonts.extend(Self::from_bytes(files).fonts);
        self
    }

    /// Добавляет уже разобранные шрифты — из [`VaultFontCache`], где байты
    /// разбираются один раз на изменение файла, а не на каждую сборку.
    pub fn with_fonts(mut self, fonts: impl IntoIterator<Item = Font>) -> Self {
        self.fonts.extend(fonts);
        self
    }

    pub fn into_fonts(self) -> Vec<Font> {
        self.fonts
    }

    pub fn len(&self) -> usize {
        self.fonts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }
}

/// Шрифты vault, разобранные в этом сеансе (FR-027, FR-029).
///
/// Список файлов спрашивается у vault на каждую сборку — иначе шрифт,
/// положенный в папку, не появился бы без переоткрытия (FR-029). А вот байты
/// перечитываются, только когда отпечаток `{size, modified}` разошёлся с
/// запомненным: шрифты весят мегабайты, и разбирать их на каждое нажатие
/// клавиши значило бы платить за чтение, которое ничего не меняет.
///
/// `Mutex` по той же причине, что в [`super::cache::PackageCache`]:
/// блокировка не удерживается через `await`.
#[derive(Debug, Default)]
pub struct VaultFontCache {
    entries: Mutex<HashMap<PathBuf, CachedFont>>,
}

#[derive(Debug, Clone)]
struct CachedFont {
    size: u64,
    modified_ms: i64,
    /// Пусто, если файл не разобрался как шрифт: повторять разбор тех же
    /// байтов незачем, отпечаток изменится вместе с файлом.
    fonts: Vec<Font>,
}

/// Файл шрифта vault, который не удалось использовать (spec.md → Edge Cases).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultFontProblem {
    pub path: String,
    pub reason: VaultFontProblemReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultFontProblemReason {
    /// Файл не прочитался.
    Unreadable(ResolveError),
    /// Прочитался, но шрифтом не разобрался.
    Invalid,
}

impl VaultFontProblem {
    /// Сообщение пользователю: какой файл и почему он не вошёл в набор
    /// (фича 010: вид и параметры, слова — в интерфейсе).
    pub fn app_message(&self) -> AppMessage {
        let reason = match &self.reason {
            VaultFontProblemReason::Unreadable(error) => FontProblem::Unreadable {
                // Шрифт — не шаблон: `PackageMissing` здесь не наступает, а
                // если наступит, это всё равно «файл не читается».
                reason: ResolveReason::of(error).unwrap_or(ResolveReason::Unreadable),
            },
            VaultFontProblemReason::Invalid => FontProblem::Invalid,
        };
        AppMessage::VaultFontUnusable {
            path: self.path.clone(),
            reason,
        }
    }
}

impl VaultFontCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Шрифты vault для очередной сборки и файлы, которые не пригодились.
    ///
    /// Порядок шрифтов — по пути файла (его задаёт `list_fonts`): тогда выбор
    /// начертания при совпадении имён одинаков на всех клиентах (FR-027).
    pub async fn load<B: VaultBackend>(&self, backend: &B) -> (Vec<Font>, Vec<VaultFontProblem>) {
        let listed = backend.list_fonts().await;
        let mut fonts = Vec::new();
        let mut problems = Vec::new();

        for file in &listed {
            let cached = self
                .entries
                .lock()
                .expect("кэш шрифтов vault")
                .get(&file.path)
                .filter(|known| known.size == file.size && known.modified_ms == file.modified_ms)
                .cloned();

            let entry = match cached {
                Some(entry) => entry,
                None => {
                    let parsed = match backend.read_bytes(&file.path).await {
                        Ok(bytes) => FontSet::from_bytes([Bytes::new(bytes)]).fonts,
                        Err(error) => {
                            // Нечитаемое не кэшируется: причина вроде
                            // отозванного доступа проходит без смены файла.
                            problems.push(VaultFontProblem {
                                path: display_path(&file.path),
                                reason: VaultFontProblemReason::Unreadable(error.into()),
                            });
                            continue;
                        }
                    };
                    let entry = CachedFont {
                        size: file.size,
                        modified_ms: file.modified_ms,
                        fonts: parsed,
                    };
                    self.entries
                        .lock()
                        .expect("кэш шрифтов vault")
                        .insert(file.path.clone(), entry.clone());
                    entry
                }
            };

            if entry.fonts.is_empty() {
                problems.push(VaultFontProblem {
                    path: display_path(&file.path),
                    reason: VaultFontProblemReason::Invalid,
                });
            }
            fonts.extend(entry.fonts);
        }

        // Удалённые из vault файлы уходят и из кэша (FR-029).
        self.entries
            .lock()
            .expect("кэш шрифтов vault")
            .retain(|path, _| listed.iter().any(|file| &file.path == path));

        (fonts, problems)
    }
}

/// Путь для сообщения — с прямыми разделителями на любой платформе, как и
/// остальные пути в диагностиках.
fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
