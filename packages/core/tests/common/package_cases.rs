//! Разрешение внешних шаблонов Typst — общая таблица для обоих таргетов
//! (фича 006, contracts/package-resolve.md).
//!
//! Сети здесь нет ни в одной проверке и быть не может: чтение идёт через
//! `MemoryVault`, у которого сетевого пути не существует (FR-039).

#![allow(dead_code)]

use typsidian_core::messages::AppMessage;
use typsidian_core::preview::resolve::{
    download_root, installed_root, package_path, package_root, read_dependency, ResolveError,
};
use typst::syntax::package::{PackageSpec, PackageVersion};
use typst::syntax::{FileId, RootedPath, VirtualPath, VirtualRoot};

use super::MemoryVault;

fn spec(namespace: &str, name: &str, version: (u32, u32, u32)) -> PackageSpec {
    PackageSpec {
        namespace: namespace.into(),
        name: name.into(),
        version: PackageVersion {
            major: version.0,
            minor: version.1,
            patch: version.2,
        },
    }
}

fn file_id(spec: &PackageSpec, inside: &str) -> FileId {
    RootedPath::new(
        VirtualRoot::Package(spec.clone()),
        VirtualPath::new(inside).expect("путь внутри пакета"),
    )
    .intern()
}

/// Раскладка совпадает с каталогом пакетов самого Typst: скачанную папку
/// пользователь копирует как есть.
pub fn the_layout_mirrors_the_typst_package_cache() {
    let codly = spec("preview", "codly", (1, 3, 0));
    assert_eq!(
        package_root(&codly).to_string_lossy().replace('\\', "/"),
        ".typsidian/packages/preview/codly/1.3.0"
    );
    assert_eq!(
        package_path(&codly, &VirtualPath::new("lib.typ").unwrap())
            .to_string_lossy()
            .replace('\\', "/"),
        ".typsidian/packages/preview/codly/1.3.0/lib.typ"
    );
}

/// Namespace не зашит: `@local` работает тем же путём, что `@preview`.
pub fn the_namespace_is_not_hardcoded() {
    let local = spec("local", "мой", (0, 1, 0));
    assert_eq!(
        package_root(&local).to_string_lossy().replace('\\', "/"),
        ".typsidian/packages/local/мой/0.1.0"
    );
}

/// FR-036: шаблон лежит локально — документ его получает.
pub async fn an_installed_template_is_read() {
    let codly = spec("preview", "codly", (1, 3, 0));
    let root = package_root(&codly).to_string_lossy().into_owned();
    let vault = MemoryVault::new()
        .with_file(&format!("{root}/typst.toml"), b"[package]".to_vec())
        .with_file(&format!("{root}/lib.typ"), b"#let codly() = {}".to_vec());

    let bytes = read_dependency(&vault, file_id(&codly, "lib.typ"))
        .await
        .expect("установленный шаблон читается");
    assert!(!bytes.is_empty());
}

/// FR-037: шаблона нет — причина называет его имя и версию слово в слово так,
/// как пользователь написал в `#import`.
pub async fn a_missing_template_is_named() {
    let codly = spec("preview", "codly", (1, 3, 0));
    let vault = MemoryVault::new();

    let error = read_dependency(&vault, file_id(&codly, "lib.typ"))
        .await
        .expect_err("несуществующий шаблон обязан отказать");
    assert_eq!(
        error,
        ResolveError::PackageMissing {
            spec: "@preview/codly:1.3.0".to_string()
        }
    );
    // Сообщение называет шаблон так, как его написал пользователь (FR-037).
    assert_eq!(
        AppMessage::from_resolve(".typsidian/packages/preview/codly/1.3.0/lib.typ", &error),
        AppMessage::TemplateMissing {
            spec: "@preview/codly:1.3.0".to_string()
        }
    );
}

/// FR-038: локально лежит **другая** версия — это по-прежнему «нужной нет», а
/// не молчаливая подстановка имеющейся.
pub async fn a_different_version_is_not_substituted() {
    let wanted = spec("preview", "codly", (1, 3, 0));
    let installed = spec("preview", "codly", (1, 2, 0));
    let root = package_root(&installed).to_string_lossy().into_owned();
    let vault = MemoryVault::new()
        .with_file(&format!("{root}/typst.toml"), b"[package]".to_vec())
        .with_file(&format!("{root}/lib.typ"), b"#let codly() = {}".to_vec());

    let error = read_dependency(&vault, file_id(&wanted, "lib.typ"))
        .await
        .expect_err("версия сверяется точно");
    assert_eq!(
        error,
        ResolveError::PackageMissing {
            spec: "@preview/codly:1.3.0".to_string()
        },
        "сообщение называет запрошенную версию, а не имеющуюся"
    );
}

/// Шаблон установлен, но нужного файла в нём нет — причина другая: чинится
/// правкой заметки, а не установкой шаблона.
pub async fn a_missing_file_inside_an_installed_template_is_a_different_reason() {
    let codly = spec("preview", "codly", (1, 3, 0));
    let root = package_root(&codly).to_string_lossy().into_owned();
    let vault = MemoryVault::new().with_file(&format!("{root}/typst.toml"), b"[package]".to_vec());

    assert_eq!(
        read_dependency(&vault, file_id(&codly, "нет-такого.typ")).await,
        Err(ResolveError::NotFound)
    );
}

/// FR-039: заметка без подключений от отсутствия папки шаблонов не страдает.
pub async fn a_document_without_imports_is_unaffected() {
    let vault = MemoryVault::new().with_file("Идея.typ", "= Заголовок".as_bytes().to_vec());
    let id = RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new("Идея.typ").expect("путь внутри vault"),
    )
    .intern();
    assert!(read_dependency(&vault, id).await.is_ok());
}

// ── Фича 041: скачанное отдельно от положенного вручную ──────────────────────

/// Скачанный шаблон лежит в папке скачанного — она не синхронизируется.
pub fn downloads_have_their_own_folder() {
    let codly = spec("preview", "codly", (1, 3, 0));
    assert_eq!(
        download_root(&codly).to_string_lossy().replace('\\', "/"),
        ".typsidian/downloads/preview/codly/1.3.0"
    );
}

/// FR-004: сборка находит скачанный шаблон, а при наличии обоих берёт ручной.
pub async fn a_downloaded_template_is_read_and_a_hand_placed_one_wins() {
    let codly = spec("preview", "codly", (1, 3, 0));
    let downloaded = download_root(&codly).to_string_lossy().into_owned();
    let vault = MemoryVault::new()
        .with_file(&format!("{downloaded}/typst.toml"), b"[package]".to_vec())
        .with_file(&format!("{downloaded}/lib.typ"), b"// downloaded".to_vec());
    let bytes = read_dependency(&vault, file_id(&codly, "lib.typ"))
        .await
        .expect("скачанный шаблон читается");
    assert_eq!(bytes.as_slice(), b"// downloaded");

    let manual = package_root(&codly).to_string_lossy().into_owned();
    let vault = vault
        .with_file(&format!("{manual}/typst.toml"), b"[package]".to_vec())
        .with_file(&format!("{manual}/lib.typ"), b"// by hand".to_vec());
    let bytes = read_dependency(&vault, file_id(&codly, "lib.typ"))
        .await
        .expect("ручной шаблон читается");
    assert_eq!(bytes.as_slice(), b"// by hand", "ручной важнее скачанного");
    assert_eq!(
        installed_root(&vault, &codly).await,
        Some(package_root(&codly))
    );
}
