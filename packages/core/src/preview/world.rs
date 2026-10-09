//! Реализация [`typst::World`] над снимком зависимостей.
//!
//! Ключевое свойство: **ни один метод не обращается к файловой системе**. Всё,
//! что компилятор может попросить, либо уже лежит в снимке, либо отмечается
//! недостающим и возвращается как `FileError::NotFound` — тогда фаза резолва
//! догрузит его и сборка повторится (research.md §2).
//!
//! Так эта конструкция работает одинаково на обоих таргетах: на wasm32
//! окружающей файловой системы нет вовсе, и `World`, который бы к ней ходил,
//! там просто не существовал бы (принцип II).

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, Source};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

use super::snapshot::DependencySnapshot;

/// `World`, читающий только из снимка.
pub struct PreviewWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    main: FileId,
    snapshot: DependencySnapshot,
}

impl PreviewWorld {
    /// Собирает мир вокруг открытой заметки.
    ///
    /// `fonts` приходят снаружи готовыми байтами: встроенный набор
    /// поставляется файлами рядом с модулем, а не внутри него, и шрифты из
    /// папки vault приходят тем же путём, что прочие связанные файлы
    /// (research.md §6, FR-027).
    pub fn new(main: FileId, snapshot: DependencySnapshot, fonts: Vec<Font>) -> Self {
        let book = FontBook::from_fonts(&fonts);
        Self {
            library: LazyHash::new(<Library as LibraryExt>::default()),
            book: LazyHash::new(book),
            fonts,
            main,
            snapshot,
        }
    }

    /// Снимок обратно — вместе с тем, что компилятор попросил и не нашёл.
    pub fn into_snapshot(self) -> DependencySnapshot {
        self.snapshot
    }

    pub fn snapshot(&self) -> &DependencySnapshot {
        &self.snapshot
    }

    /// Общий путь промаха: отметить недостающее и отказать компилятору.
    ///
    /// Отказ здесь — не ошибка пользователя, а сигнал управляющему циклу. Если
    /// после догрузки файл так и не появится, тот же `NotFound` дойдёт до
    /// пользователя уже как названная причина с именем файла (FR-021).
    fn miss<T>(&self, id: FileId) -> FileResult<T> {
        self.snapshot.record_missing(id);
        Err(FileError::NotFound(id.vpath().get_without_slash().into()))
    }
}

impl World for PreviewWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        let Some(bytes) = self.snapshot.get(id) else {
            return self.miss(id);
        };
        self.snapshot.record_access(id);
        let text = std::str::from_utf8(bytes)
            .map_err(|_| FileError::InvalidUtf8)?
            .to_owned();
        Ok(Source::new(id, text))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        match self.snapshot.get(id) {
            Some(bytes) => {
                self.snapshot.record_access(id);
                Ok(bytes.clone())
            }
            None => self.miss(id),
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        // Дата документа намеренно недоступна: она сделала бы результат сборки
        // невоспроизводимым, а `digest` страницы — нестабильным между
        // пересборками одного и того же текста (research.md §5). Пользователь
        // получит внятную ошибку от самого `datetime`, а не молчаливое
        // расхождение вида.
        None
    }
}
