//! Фаза резолва: догрузка связанных файлов в снимок
//! (contracts/compile-engine.md § Фаза резолва).
//!
//! Своего доступа к файлам эта фича не заводит — читает через `VaultBackend`
//! фичи 001 (принцип I). Отсюда же берётся и предел размера, и различение
//! отказов: «файла нет» и «нет прав» обязаны дойти до пользователя разными
//! словами (FR-021).

use std::path::PathBuf;

use typst::foundations::Bytes;
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, VirtualPath, VirtualRoot};

use crate::vault::backend::{ReadError, VaultBackend};

/// Почему связанный файл не попал в снимок (contracts/compile-engine.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    NotFound,
    Unreadable,
    AccessDenied,
    /// Ссылка за пределы папки vault (FR-023).
    OutsideVault,
    /// Наследуется от предела фичи 001 (FR-030).
    TooLarge,
    /// Внешний шаблон Typst не установлен локально (фича 006, FR-037).
    ///
    /// `spec` — то, что пользователь написал в `#import`, слово в слово:
    /// `@preview/codly:1.3.0`. Берётся у `Display for PackageSpec`, а не
    /// собирается вручную, иначе сообщение разошлось бы с текстом заметки.
    ///
    /// Отдельный вариант, а не `OutsideVault`: «шаблон не установлен» и
    /// «ссылка наружу» — разные факты и разные слова, а FR-037 запрещает общий
    /// отказ.
    PackageMissing {
        spec: String,
    },
}

impl From<ReadError> for ResolveError {
    fn from(error: ReadError) -> Self {
        match error {
            ReadError::NotFound => ResolveError::NotFound,
            ReadError::TooLarge => ResolveError::TooLarge,
            ReadError::Io => ResolveError::Unreadable,
        }
    }
}

/// Путь файла внутри vault по идентификатору компилятора.
///
/// `None` — идентификатор указывает на пакет Typst. Такие файлы тоже лежат
/// внутри vault, но в служебном месте, и путь к ним считает
/// [`package_path`]: раскладка там своя.
pub fn vault_path(id: FileId) -> Option<String> {
    if !matches!(id.root(), VirtualRoot::Project) {
        return None;
    }
    Some(id.vpath().get_without_slash().to_owned())
}

/// Путь файла внешнего шаблона относительно корня vault (фича 006, FR-036).
///
/// ```text
/// .typsidian/packages/<namespace>/<name>/<major>.<minor>.<patch>/<vpath>
/// ```
///
/// Раскладка `<namespace>/<name>/<version>/` совпадает с каталогом пакетов
/// самого Typst: папку, уже скачанную штатным `typst`, пользователь копирует
/// как есть, а не раскладывает по нашей выдуманной схеме.
pub fn package_root(spec: &PackageSpec) -> PathBuf {
    let version = format!(
        "{}.{}.{}",
        spec.version.major, spec.version.minor, spec.version.patch
    );
    crate::vault::service_dir::packages_dir()
        .join(spec.namespace.as_str())
        .join(spec.name.as_str())
        .join(version)
}

/// Путь конкретного файла внутри шаблона.
pub fn package_path(spec: &PackageSpec, vpath: &VirtualPath) -> PathBuf {
    package_root(spec).join(vpath.get_without_slash())
}

/// Файл, по которому опознаётся **установленный** шаблон.
///
/// Манифест есть у каждого пакета Typst, поэтому его отсутствие означает
/// «этой версии шаблона здесь нет», а не «в шаблоне нет такого файла». Без
/// такой проверки различить эти случаи было бы нечем: перечислить содержимое
/// папки `VaultBackend` не умеет — у него нет операции обхода произвольного
/// каталога, только индекс заметок, куда служебное место не попадает.
pub const PACKAGE_MANIFEST: &str = "typst.toml";

/// Читает связанный файл байтами.
///
/// Именно байтами, а не текстом: `#image("схема.png")` собирается из байтов, и
/// пропускать их через `String` нельзя.
pub async fn read_dependency<B: VaultBackend>(
    backend: &B,
    id: FileId,
) -> Result<Bytes, ResolveError> {
    let path = match id.root() {
        VirtualRoot::Project => PathBuf::from(
            vault_path(id).expect("Project-идентификатор всегда даёт путь внутри vault"),
        ),
        // Фича 006 сняла прежний отказ: шаблон лежит не «наружу», а в
        // служебном месте самого vault. Сети при этом по-прежнему нет — читаем
        // тем же `VaultBackend`, у которого сетевого пути не существует
        // (FR-039, принцип VII).
        VirtualRoot::Package(spec) => {
            let file = package_path(&spec, id.vpath());
            // Отличаем «шаблона нет» от «в шаблоне нет такого файла»: первое
            // чинится установкой шаблона, второе — правкой заметки, и общее
            // «не удалось собрать» запрещено (FR-037).
            let manifest = package_root(&spec).join(PACKAGE_MANIFEST);
            if backend.read_bytes(&manifest).await.is_err() {
                return Err(ResolveError::PackageMissing {
                    spec: spec.to_string(),
                });
            }
            file
        }
    };

    // Изображение читается без предела размера заметки (фича 016, research.md
    // §13): предел защищает редактор от заметки на десятки мегабайт, а к
    // картинке применялся по совпадению — вставленная после подтверждения
    // картинка на 34 МБ обязана показаться (FR-008).
    let bytes = if crate::vault::is_image_file(&path) {
        backend.read_bytes_any_size(&path).await?
    } else {
        backend.read_bytes(&path).await?
    };
    Ok(Bytes::new(bytes))
}
