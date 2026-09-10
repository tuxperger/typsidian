//! Снимок зависимостей документа (data-model.md «DependencySnapshot»).
//!
//! Единственный источник байтов для [`crate::preview::PreviewWorld`]. Всё, что
//! компилятор попросит и чего здесь не окажется, попадает в [`missing`] — это
//! и есть сигнал фазе резолва сделать ещё один проход (research.md §2).
//!
//! [`missing`]: DependencySnapshot::missing

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use typst::foundations::Bytes;
use typst::syntax::{FileId, RootedPath, VirtualPath, VirtualRoot};

/// Предел чередования резолва и сборки.
///
/// Патологический документ (импорт, чьё имя вычисляется от предыдущего
/// импорта) мог бы пополнять снимок бесконечно. Предел превращает это в
/// внятный отказ вместо зависания — contracts/compile-engine.md.
///
/// # Почему 8 оказалось мало (фича 007)
///
/// Значение выбиралось фичей 005, когда связанными файлами были только
/// картинки и импорты внутри vault: там глубина графа — единицы, и один проход
/// догружал сразу всё, что всплыло.
///
/// С появлением внешних шаблонов это перестало быть верным. Typst обрывает
/// вычисление на **первом** недостающем файле, поэтому пакет, который строит
/// словарь из десятков `image(...)` — а именно так устроен `codly-languages` с
/// его 86 иконками, — отдаёт по одному недостающему файлу за проход. Восьми
/// проходов хватало на восемь иконок, после чего сборка останавливалась с
/// «file not found» на девятой, и выглядело это как потерянный файл шаблона.
///
/// Поэтому предел считает не глубину графа, а **работу**: каждый проход
/// догружает хотя бы один файл (иначе цикл выходит по отсутствию прогресса),
/// значит число проходов ограничивает и число файлов, вошедших в сборку.
/// 512 — заведомо больше любого разумного документа и по-прежнему конечно.
pub const MAX_RESOLVE_ITERATIONS: u16 = 512;

/// Почему снимок не удалось довести до пригодного состояния.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotError {
    /// Путь не удалось выразить относительно корня vault.
    ///
    /// Соответствует FR-023: ссылка за пределы папки vault. Отказ структурный —
    /// `VirtualPath` нормализует сегменты и сам отвергает выход за корень
    /// (`typst-syntax-0.15.1/src/path.rs:195`), поэтому до чтения файла дело не
    /// доходит.
    OutsideVault { path: String },
}

/// Набор байтов, из которого собирается документ.
///
/// Ключ — `FileId` компилятора, значение — содержимое файла vault. Ни одного
/// хендла, ни одной ссылки на файловую систему: `World` обязан быть
/// `Send + Sync`, и это ограничение здесь работает страховкой (research.md §11).
#[derive(Debug, Default)]
pub struct DependencySnapshot {
    files: HashMap<FileId, Bytes>,
    /// Запрошено компилятором, но в снимке отсутствует.
    ///
    /// `Mutex`, а не `RefCell`: `World` отдаёт файлы через `&self`, промах надо
    /// записать, а трейт требует `Send + Sync` — `RefCell` этого не даёт и не
    /// компилируется (research.md §11, предохранитель сработал буквально).
    /// Блокировка не горячая: она берётся только на промахе, то есть считаное
    /// число раз за сборку.
    missing: Mutex<HashSet<FileId>>,
    iterations: u16,
}

impl Clone for DependencySnapshot {
    fn clone(&self) -> Self {
        Self {
            files: self.files.clone(),
            missing: Mutex::new(self.missing.lock().expect("snapshot mutex").clone()),
            iterations: self.iterations,
        }
    }
}

impl DependencySnapshot {
    pub fn new() -> Self {
        Self::default()
    }

    /// Переводит путь внутри vault в `FileId` компилятора.
    ///
    /// Корень (`VirtualRoot::Project`) — это папка vault: другого корня у нас
    /// нет, а пакеты Typst не поддерживаются, потому что их загрузка требует
    /// сети (research.md §8, принципы VII и IX).
    pub fn file_id(path: &str) -> Result<FileId, SnapshotError> {
        let vpath = VirtualPath::new(path).map_err(|_| SnapshotError::OutsideVault {
            path: path.to_owned(),
        })?;
        Ok(RootedPath::new(VirtualRoot::Project, vpath).intern())
    }

    /// Кладёт содержимое файла в снимок и снимает его с учёта недостающих.
    pub fn insert(&mut self, id: FileId, bytes: Bytes) {
        self.missing.get_mut().expect("snapshot mutex").remove(&id);
        self.files.insert(id, bytes);
    }

    pub fn get(&self, id: FileId) -> Option<&Bytes> {
        self.files.get(&id)
    }

    pub fn contains(&self, id: FileId) -> bool {
        self.files.contains_key(&id)
    }

    /// Отмечает, что компилятор просил файл, которого в снимке нет.
    pub fn record_missing(&self, id: FileId) {
        self.missing
            .lock()
            .expect("snapshot mutex")
            .insert(id);
    }

    /// Снимает файл с учёта недостающих, не добавляя его содержимого.
    ///
    /// Нужно резолву: файл, который не прочитался, повторно запрашивать в
    /// пределах одной сборки незачем — ответ не изменится, а цикл иначе
    /// потратил бы на него все проходы.
    pub fn forget_missing(&mut self, id: FileId) {
        self.missing.get_mut().expect("snapshot mutex").remove(&id);
    }

    /// Что осталось догрузить, чтобы сборка продвинулась дальше.
    pub fn missing(&self) -> Vec<FileId> {
        self.missing
            .lock()
            .expect("snapshot mutex")
            .iter()
            .copied()
            .collect()
    }

    pub fn has_missing(&self) -> bool {
        !self.missing.lock().expect("snapshot mutex").is_empty()
    }

    pub fn iterations(&self) -> u16 {
        self.iterations
    }

    /// Отмечает ещё один проход резолв↔сборка.
    ///
    /// Возвращает `false`, когда предел исчерпан: вызывающая сторона обязана
    /// прекратить чередование и отдать `DependencyBudgetExhausted`.
    pub fn begin_iteration(&mut self) -> bool {
        if self.iterations >= MAX_RESOLVE_ITERATIONS {
            return false;
        }
        self.iterations += 1;
        true
    }

    /// Файлы vault, вошедшие в сборку. Нужны, чтобы знать, чьё изменение
    /// обязано пересобрать эту заметку (FR-020).
    pub fn dependencies(&self) -> Vec<FileId> {
        self.files.keys().copied().collect()
    }
}
