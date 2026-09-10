//! Установка внешних шаблонов — общая таблица для обоих таргетов (фича 007,
//! contracts/template-install.md → «Проверяемые сценарии»).
//!
//! # Сети здесь нет ни в одной проверке
//!
//! И это не упрощение, а следствие устройства фичи: разбор архива, проверка
//! путей, отпечаток и запись — чистые функции над байтами, а сетевой вызов
//! остаётся тонкой оболочкой вокруг них. Тест, которому понадобился бы
//! настоящий запрос, означал бы, что логика утекла в оболочку.

#![allow(dead_code)]

use std::io::Write;

use typsidian_core::preview::packages::{
    fingerprint, install, recorded_fingerprint, source_url, unpack, InstallError, TemplateSource,
    FINGERPRINT_FILE,
};
use typsidian_core::preview::resolve::{package_root, PACKAGE_MANIFEST};
use typst::syntax::package::{PackageSpec, PackageVersion};

use super::MemoryVault;
use typsidian_core::vault::backend::VaultBackend;

pub fn spec(name: &str, version: (u32, u32, u32)) -> PackageSpec {
    PackageSpec {
        namespace: "preview".into(),
        name: name.into(),
        version: PackageVersion {
            major: version.0,
            minor: version.1,
            patch: version.2,
        },
    }
}

/// Собирает gzip+tar в памяти — настоящий архив, а не заглушка: проверять надо
/// тот же путь разбора, которым пойдут настоящие данные.
pub fn make_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut tar = tar::Builder::new(Vec::new());
    for (path, bytes) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, path, *bytes)
            .expect("запись в tar");
    }
    let raw = tar.into_inner().expect("сборка tar");

    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&raw).expect("сжатие");
    gz.finish().expect("завершение сжатия")
}

const MANIFEST: &[u8] = b"[package]\nname = \"codly\"\n";

/// FR-001, FR-036: обычный архив разбирается, файлы на местах.
pub fn a_plain_archive_unpacks() {
    let archive = make_archive(&[("lib.typ", b"#let codly() = {}"), (PACKAGE_MANIFEST, MANIFEST)]);
    let files = unpack(&archive).expect("обычный архив разбирается");

    let names: Vec<&str> = files.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(names, vec!["lib.typ", "typst.toml"], "и отсортированы");
}

/// Собирает архив с **враждебным** путём записи.
///
/// Отдельная функция, потому что `tar::Builder::append_data` такой путь
/// записать не даёт — и правильно делает. Имя пишется прямо в поле заголовка,
/// минуя проверку сборщика: нам нужен именно тот архив, от которого мы
/// защищаемся, а не его безопасный суррогат.
fn make_hostile_archive(path: &str, bytes: &[u8]) -> Vec<u8> {
    let mut tar = tar::Builder::new(Vec::new());

    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    {
        let name = &mut header.as_old_mut().name;
        let raw = path.as_bytes();
        assert!(raw.len() < name.len(), "имя не помещается в заголовок tar");
        name[..raw.len()].copy_from_slice(raw);
    }
    header.set_cksum();
    tar.append(&header, bytes).expect("запись враждебной записи");

    let mut manifest = tar::Header::new_gnu();
    manifest.set_size(MANIFEST.len() as u64);
    manifest.set_mode(0o644);
    manifest.set_cksum();
    tar.append_data(&mut manifest, PACKAGE_MANIFEST, MANIFEST)
        .expect("запись манифеста");

    let raw = tar.into_inner().expect("сборка tar");
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&raw).expect("сжатие");
    gz.finish().expect("завершение сжатия")
}

/// FR-019: запись, ведущая за пределы папки версии, отклоняет **весь** архив.
pub fn a_path_traversal_rejects_the_whole_archive() {
    for evil in ["../снаружи.typ", "a/../../снаружи.typ", "/etc/passwd"] {
        let archive = make_hostile_archive(evil, "зло".as_bytes());
        let error = unpack(&archive).expect_err(evil);
        assert!(
            matches!(error, InstallError::ArchiveRejected { .. }),
            "{evil} обязан отклонить архив целиком, а не пропустить запись"
        );
    }
}

/// Архив без манифеста отклоняется: без него шаблон всё равно не считался бы
/// установленным, и разложить его файлы значило бы намусорить впустую.
pub fn an_archive_without_a_manifest_is_rejected() {
    let archive = make_archive(&[("lib.typ", b"x")]);
    assert!(matches!(
        unpack(&archive),
        Err(InstallError::ArchiveRejected { .. })
    ));
}

/// Мусор вместо архива — названная причина, а не паника.
pub fn garbage_is_not_an_archive() {
    assert!(matches!(
        unpack("это не архив".as_bytes()),
        Err(InstallError::ArchiveRejected { .. })
    ));
}

/// Обёртка-папка срезается: иначе `lib.typ` лежал бы глубже, чем его ищет
/// компилятор, и шаблон установился бы нерабочим.
pub fn a_wrapper_folder_is_stripped() {
    let archive = make_archive(&[
        ("codly-1.3.0/lib.typ", b"x"),
        (&format!("codly-1.3.0/{PACKAGE_MANIFEST}"), MANIFEST),
    ]);
    let files = unpack(&archive).expect("архив с обёрткой разбирается");
    let names: Vec<&str> = files.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(names, vec!["lib.typ", "typst.toml"]);
}

/// FR-020: манифест пишется последним, поэтому установка — всё или ничего.
///
/// Проверяется наблюдаемо: после установки признак «шаблон установлен» из фичи
/// 006 (наличие манифеста) истинен, и рядом лежат все файлы.
pub async fn install_writes_the_manifest_last() {
    let codly = spec("codly", (1, 3, 0));
    let mut vault = MemoryVault::new();
    let files = vec![
        ("lib.typ".to_string(), b"#let codly() = {}".to_vec()),
        (PACKAGE_MANIFEST.to_string(), MANIFEST.to_vec()),
    ];

    let digest = install(&mut vault, &codly, &files).await.expect("установка");
    let root = package_root(&codly);

    assert!(vault.read(&root.join("lib.typ")).await.is_ok());
    assert!(vault.read(&root.join(PACKAGE_MANIFEST)).await.is_ok());
    assert_eq!(
        recorded_fingerprint(&vault, &codly).await.as_deref(),
        Some(digest.as_str()),
        "FR-022: отпечаток записан рядом с шаблоном"
    );
}

/// Шаблон с **бинарным** файлом устанавливается целиком.
///
/// Регрессия, найденная на живом `@preview/codly:1.3.0`: в нём лежит
/// `src/typst-small.png`, а установка писала файлы через `VaultBackend::write`,
/// принимающий `&str`. PNG не проходил через `String::from_utf8`, и весь
/// шаблон отклонялся с «файл не является текстом».
///
/// Шаблон Typst — это не только `.typ`: картинки и шрифты в нём обычное дело.
pub async fn a_template_with_binary_files_installs() {
    let codly = spec("codly", (1, 3, 0));
    let mut vault = MemoryVault::new();

    // Начало настоящего PNG: сигнатура заведомо не UTF-8.
    let png: Vec<u8> = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0xFF, 0xFE];
    let files = vec![
        ("lib.typ".to_string(), b"#let codly() = {}".to_vec()),
        ("src/typst-small.png".to_string(), png.clone()),
        (PACKAGE_MANIFEST.to_string(), MANIFEST.to_vec()),
    ];

    install(&mut vault, &codly, &files)
        .await
        .expect("шаблон с картинкой обязан установиться");

    let root = package_root(&codly);
    assert_eq!(
        vault
            .read_bytes(&root.join("src/typst-small.png"))
            .await
            .expect("картинка на месте"),
        png,
        "байты картинки не искажены"
    );
    assert!(
        vault.read(&root.join(PACKAGE_MANIFEST)).await.is_ok(),
        "манифест записан — значит установка состоялась целиком"
    );
}

/// FR-018: vault только на чтение — установить некуда, причина названа.
pub async fn a_read_only_vault_refuses_installation() {
    let codly = spec("codly", (1, 3, 0));
    let mut vault = MemoryVault::new().read_only();
    let files = vec![(PACKAGE_MANIFEST.to_string(), MANIFEST.to_vec())];

    assert_eq!(
        install(&mut vault, &codly, &files).await,
        Err(InstallError::NotWritable)
    );
    assert!(
        vault
            .read(&package_root(&codly).join(PACKAGE_MANIFEST))
            .await
            .is_err(),
        "ни одного файла не создано"
    );
}

/// FR-024: отпечаток зависит от содержимого, а не от порядка записей.
pub fn the_fingerprint_is_deterministic() {
    let forward = vec![
        ("a.typ".to_string(), b"one".to_vec()),
        ("b.typ".to_string(), b"two".to_vec()),
    ];
    let backward = vec![
        ("b.typ".to_string(), b"two".to_vec()),
        ("a.typ".to_string(), b"one".to_vec()),
    ];
    assert_eq!(fingerprint(&forward), fingerprint(&backward));

    let changed = vec![
        ("a.typ".to_string(), b"one".to_vec()),
        ("b.typ".to_string(), b"TWO".to_vec()),
    ];
    assert_ne!(fingerprint(&forward), fingerprint(&changed));
}

/// FR-023: содержимое изменили после установки — расхождение с отпечатком
/// видно, и переустановки при этом не происходит.
pub async fn a_changed_template_diverges_from_its_fingerprint() {
    let codly = spec("codly", (1, 3, 0));
    let mut vault = MemoryVault::new();
    let files = vec![
        ("lib.typ".to_string(), "исходный".as_bytes().to_vec()),
        (PACKAGE_MANIFEST.to_string(), MANIFEST.to_vec()),
    ];
    let installed = install(&mut vault, &codly, &files).await.expect("установка");

    // Пользователь (или кто-то ещё) поправил файл на диске.
    let tampered = vec![
        ("lib.typ".to_string(), "подменённый".as_bytes().to_vec()),
        (PACKAGE_MANIFEST.to_string(), MANIFEST.to_vec()),
    ];
    assert_ne!(
        fingerprint(&tampered),
        installed,
        "расхождение обязано быть заметным"
    );
    assert_eq!(
        recorded_fingerprint(&vault, &codly).await.as_deref(),
        Some(installed.as_str()),
        "записанный отпечаток при этом не переписан"
    );
}

/// FR-005: за пределами пространства имён реестра обращаться некуда.
pub fn only_the_registry_namespace_has_a_registry_url() {
    let local = PackageSpec {
        namespace: "local".into(),
        name: "мой".into(),
        version: PackageVersion {
            major: 0,
            minor: 1,
            patch: 0,
        },
    };
    assert_eq!(source_url(&local, &TemplateSource::Registry), None);
    assert!(source_url(&spec("codly", (1, 3, 0)), &TemplateSource::Registry).is_some());
}

/// FR-021, принцип IX: наружу уходит только имя и версия шаблона.
///
/// Проверяется по составу адреса: в нём нет ни пути заметки, ни её имени, ни
/// чего-либо ещё об устройстве пользователя.
pub fn the_request_carries_nothing_but_the_template_name() {
    let url = source_url(&spec("codly", (1, 3, 0)), &TemplateSource::Registry).unwrap();
    assert!(url.contains("codly"));
    assert!(url.contains("1.3.0"));
    // Проверяются токены, относящиеся к **пользователю и его файлам**. Строку
    // `.typ` сюда включать нельзя: она встречается внутри `packages.typst.org`,
    // и проверка ловила бы сам адрес реестра вместо утечки.
    for leak in ["Идея", "Заметк", "vault", "/home/", "notes", "apokryf"] {
        assert!(!url.contains(leak), "в адресе не должно быть «{leak}»: {url}");
    }
    // Адрес состоит ровно из реестра, имени и версии — ничего больше.
    assert_eq!(url, "https://packages.typst.org/preview/codly-1.3.0.tar.gz");
}

/// Файл отпечатка не участвует в собственном хэше.
pub fn the_fingerprint_file_is_excluded() {
    let bare = vec![("lib.typ".to_string(), b"x".to_vec())];
    let mut with_marker = bare.clone();
    with_marker.push((FINGERPRINT_FILE.to_string(), b"aaa".to_vec()));
    assert_eq!(fingerprint(&bare), fingerprint(&with_marker));
}
