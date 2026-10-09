//! Файлы внешних шаблонов, пережившие сборку.
//!
//! # Зачем
//!
//! Typst обрывает вычисление на **первом** недостающем файле, поэтому цикл
//! резолва догружает по одному файлу за проход, а каждый проход — это полная
//! сборка документа заново (`compile.rs`, `snapshot.rs` § MAX_RESOLVE_ITERATIONS).
//! Для заметки с `@preview/codly-languages:0.1.1`, где 86 иконок, это 97 сборок
//! на один показ: измерено `examples/preview_profile.rs` на native-release —
//! 240 мс, из них 200 мс в `typst::compile` и 0,7 мс в чтении файлов. Та же
//! заметка по готовому снимку собирается за 2,5 мс.
//!
//! Пока снимок начинался с нуля при каждом вызове, эти 97 проходов повторялись
//! на **каждое** нажатие клавиши. Кэш переносит уже прочитанные файлы шаблонов
//! в следующую сборку: промахов не остаётся, проход остаётся один.
//!
//! # Почему только файлы шаблонов
//!
//! Версия шаблона неизменна по построению: `@preview/codly:1.3.0` — это всегда
//! одни и те же байты, а новая версия лежит в другой папке и имеет другой
//! `FileId`. Файлы vault так себя не ведут: их правит пользователь, и кэш для
//! них потребовал бы сверки с диском на каждой сборке — то есть ровно того
//! чтения, которое он должен был устранить. Их перечитывание и так почти
//! ничего не стоит: дорога здесь пересборка, а не чтение.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use typst::foundations::Bytes;
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, VirtualRoot};

use super::fonts::VaultFontCache;
use super::snapshot::DependencySnapshot;

/// Файлы установленных шаблонов, прочитанные в этом сеансе.
///
/// Живёт столько же, сколько движок предпросмотра: оболочка создаёт его один
/// раз на открытый vault. Между заметками кэш общий — шаблон, поднятый одной
/// заметкой, достаётся соседней уже готовым.
///
/// `Mutex`, а не `RefCell`: движок отдаёт кэш по `&self` (у wasm-bindgen
/// асинхронный метод иначе не описать), а `World` в этом же модуле обязан быть
/// `Send + Sync` — один и тот же выбор по одной и той же причине, что и в
/// [`DependencySnapshot`]. Блокировка не горячая: она берётся дважды за сборку,
/// на заполнение снимка и на сбор результата, и никогда не удерживается через
/// `await`.
#[derive(Debug, Default)]
pub struct PackageCache {
    files: Mutex<HashMap<FileId, Bytes>>,
    /// Содержимое связанных файлов vault от прошлой сборки той же заметки
    /// (005 T122) — см. [`PackageCache::vault_files_of`].
    vault_files: Mutex<VaultFiles>,
    /// Шаблоны, чей отпечаток уже сверен в этом сеансе (фича 007, FR-023).
    verified: Mutex<HashSet<PackageSpec>>,
    /// Шрифты из папки vault (фича 005, FR-027). Здесь, а не отдельным
    /// параметром сборки: время жизни у них то же — сеанс движка, — а каждый
    /// вызывающий `compile_*` уже держит этот кэш.
    vault_fonts: VaultFontCache,
}

/// Отметка файла vault: размер и время изменения ([`VaultBackend::file_mark`]).
pub type FileMark = (u64, i64);

#[derive(Debug, Default)]
struct VaultFiles {
    /// Заметка, к сборке которой относятся файлы. Кэш держит одну заметку:
    /// переход к другой его сбрасывает, и память не копится по мере
    /// хождения по vault.
    note: String,
    files: HashMap<FileId, (FileMark, Bytes)>,
}

impl PackageCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Связанные файлы vault, прочитанные прошлой сборкой заметки.
    ///
    /// Без этого каждый импорт поднимался бы заново отдельным проходом
    /// компилятора на каждое нажатие клавиши: у заметки с сорока импортами это
    /// сорок проходов вместо одного. Годится ли файл, решает вызывающий —
    /// сверкой отметки с диском: содержимое, устаревшее хоть на байт, в
    /// сборку попасть не должно.
    pub fn vault_files_of(&self, note: &str) -> Vec<(FileId, FileMark, Bytes)> {
        let cache = self.vault_files.lock().expect("кэш файлов vault");
        if cache.note != note {
            return Vec::new();
        }
        cache
            .files
            .iter()
            .map(|(id, (mark, bytes))| (*id, *mark, bytes.clone()))
            .collect()
    }

    /// Запоминает связанные файлы vault этой сборки — те, чья отметка известна
    /// и которые компилятор действительно запрашивал. Прежний набор
    /// заменяется целиком: импорт, убранный из текста, из кэша уходит. Ошибиться
    /// здесь можно только в безопасную сторону — забытый файл просто
    /// прочитается заново.
    pub fn remember_vault_files(
        &self,
        note: &str,
        snapshot: &DependencySnapshot,
        marks: &HashMap<FileId, FileMark>,
    ) {
        let files = marks
            .iter()
            .filter(|(id, _)| snapshot.was_accessed(**id))
            .filter_map(|(id, mark)| Some((*id, (*mark, snapshot.get(*id)?.clone()))))
            .collect();
        *self.vault_files.lock().expect("кэш файлов vault") = VaultFiles {
            note: note.to_owned(),
            files,
        };
    }

    /// Кладёт в снимок всё, что уже известно про шаблоны.
    ///
    /// Лишнего не боится: файл, который этому документу не нужен, компилятор
    /// просто не спросит. Цена — запись в карте, выигрыш — отсутствие промаха,
    /// то есть целой пересборки.
    pub fn seed(&self, snapshot: &mut DependencySnapshot) {
        let files = self.files.lock().expect("кэш шаблонов");
        for (id, bytes) in files.iter() {
            // `insert`, а не «если ещё нет»: снимок в этот момент содержит
            // только текст самой заметки, а он лежит в корне vault и с файлами
            // шаблонов пересечься не может.
            snapshot.insert(*id, bytes.clone());
        }
    }

    /// Забирает из снимка файлы шаблонов, чтобы следующая сборка начала с них.
    pub fn absorb(&self, snapshot: &DependencySnapshot) {
        let mut files = self.files.lock().expect("кэш шаблонов");
        for id in snapshot.dependencies() {
            if !matches!(id.root(), VirtualRoot::Package(_)) {
                continue;
            }
            if let Some(bytes) = snapshot.get(id) {
                files.entry(id).or_insert_with(|| bytes.clone());
            }
        }
    }

    /// Забывает всё, что относится к этой версии шаблона.
    ///
    /// Вызывается после установки (фича 007): если ту же версию поставили
    /// поверх, в кэше могли остаться прежние байты, а сборка обязана увидеть
    /// то, что сейчас на диске.
    pub fn forget(&self, spec: &PackageSpec) {
        let mut files = self.files.lock().expect("кэш шаблонов");
        files.retain(|id, _| match id.root() {
            VirtualRoot::Package(known) => *known != *spec,
            VirtualRoot::Project => true,
        });
    }

    /// Шаблоны, чьи файлы этот сеанс уже читал.
    ///
    /// Отсюда сборка узнаёт, у каких шаблонов сверять отпечаток (фича 007,
    /// FR-023): проверять надо то, что документ действительно использовал, а не
    /// всё установленное — перечислить установленное `VaultBackend` всё равно
    /// не умеет.
    pub fn specs(&self) -> Vec<PackageSpec> {
        let files = self.files.lock().expect("кэш шаблонов");
        let mut specs: Vec<PackageSpec> = Vec::new();
        for id in files.keys() {
            if let VirtualRoot::Package(spec) = id.root() {
                if !specs.contains(spec) {
                    specs.push(spec.clone());
                }
            }
        }
        specs
    }

    /// Отмечает шаблон сверенным; `true` — сверяем впервые.
    ///
    /// Сверка читает все файлы шаблона с диска, и делать это на каждую сборку
    /// значило бы возвращать те самые чтения, ради устранения которых заведён
    /// кэш. Раз за сеанс достаточно: подменить файл незаметно для пользователя,
    /// который тут же продолжает печатать, всё равно нельзя — сеанс начнётся
    /// заново при следующем открытии папки.
    pub fn mark_verified(&self, spec: &PackageSpec) -> bool {
        let mut verified = self.verified.lock().expect("сверенные шаблоны");
        verified.insert(spec.clone())
    }

    /// Забывает отметку о сверке — например, после переустановки шаблона.
    pub fn forget_verified(&self, spec: &PackageSpec) {
        let mut verified = self.verified.lock().expect("сверенные шаблоны");
        verified.remove(spec);
    }

    /// Кэш шрифтов vault того же сеанса.
    pub fn vault_fonts(&self) -> &VaultFontCache {
        &self.vault_fonts
    }

    /// Сколько файлов шаблонов сейчас удержано. Нужно проверкам и диагностике.
    pub fn len(&self) -> usize {
        self.files.lock().expect("кэш шаблонов").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
