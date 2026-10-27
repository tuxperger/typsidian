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

use typsidian_core::messages::AppMessage;
use typsidian_core::preview::packages::{
    fingerprint, install, installed_templates, recorded_fingerprint, remember_installed,
    source_url, unpack, verify_installed, ArchiveProblem, FingerprintCheck, InstallError,
    InstalledTemplate, TemplateSource, FINGERPRINT_FILE,
};
use typsidian_core::preview::resolve::{download_root, package_root, PACKAGE_MANIFEST};
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
    let archive = make_archive(&[
        ("lib.typ", b"#let codly() = {}"),
        (PACKAGE_MANIFEST, MANIFEST),
    ]);
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
    tar.append(&header, bytes)
        .expect("запись враждебной записи");

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
            matches!(
                error,
                InstallError::ArchiveRejected {
                    reason: ArchiveProblem::EscapesFolder { .. }
                }
            ),
            "{evil} обязан отклонить архив целиком, а не пропустить запись"
        );
    }
}

/// Архив без манифеста отклоняется: без него шаблон всё равно не считался бы
/// установленным, и разложить его файлы значило бы намусорить впустую.
pub fn an_archive_without_a_manifest_is_rejected() {
    let archive = make_archive(&[("lib.typ", b"x")]);
    // Причина — вид, а не текст (фича 010): слова подбирает интерфейс.
    assert_eq!(
        unpack(&archive),
        Err(InstallError::ArchiveRejected {
            reason: ArchiveProblem::NoManifest
        })
    );
}

/// Мусор вместо архива — названная причина, а не паника.
pub fn garbage_is_not_an_archive() {
    assert_eq!(
        unpack("это не архив".as_bytes()),
        Err(InstallError::ArchiveRejected {
            reason: ArchiveProblem::Unparsable
        })
    );
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

    let digest = install(&mut vault, &codly, &files)
        .await
        .expect("установка");
    let root = download_root(&codly);

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

    let root = download_root(&codly);
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
            .read(&download_root(&codly).join(PACKAGE_MANIFEST))
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
    let installed = install(&mut vault, &codly, &files)
        .await
        .expect("установка");

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
        assert!(
            !url.contains(leak),
            "в адресе не должно быть «{leak}»: {url}"
        );
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

// ── Фича 007, вторая половина: сверка отпечатка, реестр, соседние версии ────

/// US1 AS3: установка одной версии не трогает соседнюю.
///
/// Раскладка `<name>/<version>/` делает это свойством путей, а не проверкой
/// внутри установки, — тест закрепляет, что раскладка именно такая.
pub async fn a_neighbouring_version_is_left_alone() {
    let mut vault = MemoryVault::new();
    let old = spec("codly", (0, 1, 0));
    let new = spec("codly", (0, 2, 0));

    let old_files = vec![
        (
            "lib.typ".to_string(),
            b"\xd1\x81\xd1\x82\xd0\xb0\xd1\x80\xd0\xb0\xd1\x8f".to_vec(),
        ),
        (PACKAGE_MANIFEST.to_string(), MANIFEST.to_vec()),
    ];
    let old_digest = install(&mut vault, &old, &old_files).await.expect("0.1.0");

    let new_files = vec![
        (
            "lib.typ".to_string(),
            b"\xd0\xbd\xd0\xbe\xd0\xb2\xd0\xb0\xd1\x8f".to_vec(),
        ),
        (PACKAGE_MANIFEST.to_string(), MANIFEST.to_vec()),
    ];
    install(&mut vault, &new, &new_files).await.expect("0.2.0");

    assert_eq!(
        recorded_fingerprint(&vault, &old).await.as_deref(),
        Some(old_digest.as_str()),
        "прежняя версия осталась нетронутой"
    );
    assert!(
        matches!(
            verify_installed(&vault, &old).await,
            FingerprintCheck::Matches
        ),
        "и сходится сама с собой"
    );
}

/// FR-023: содержимое, изменённое после установки, названо расхождением.
pub async fn tampering_is_noticed_without_reinstalling() {
    let codly = spec("codly", (1, 3, 0));
    let mut vault = MemoryVault::new();
    let files = vec![
        ("lib.typ".to_string(), b"#let codly() = {}".to_vec()),
        (PACKAGE_MANIFEST.to_string(), MANIFEST.to_vec()),
    ];
    let digest = install(&mut vault, &codly, &files)
        .await
        .expect("установка");

    assert!(
        matches!(
            verify_installed(&vault, &codly).await,
            FingerprintCheck::Matches
        ),
        "сразу после установки содержимое сходится"
    );

    // Правка файла руками — ровно то, что фича обязана заметить.
    let root = download_root(&codly);
    vault
        .write_bytes(&root.join("lib.typ"), b"#let codly() = panic()")
        .await
        .expect("правка файла");

    match verify_installed(&vault, &codly).await {
        FingerprintCheck::Diverged { recorded, actual } => {
            assert_eq!(recorded, digest, "записанный отпечаток не переписан");
            assert_ne!(actual, digest, "а пересчитанный отличается");
        }
        other => panic!("ожидалось расхождение, получено {other:?}"),
    }

    // Переустановки не произошло: на диске остался изменённый файл.
    assert_eq!(
        vault
            .read_bytes(&root.join("lib.typ"))
            .await
            .expect("чтение"),
        b"#let codly() = panic()".to_vec(),
        "правка пользователя не затёрта"
    );
}

/// Шаблон, разложенный руками (без файла отпечатка), сверить не с чем.
///
/// И это отдельное состояние, а не «сошлось»: обещать целостность того, чего
/// мы не устанавливали, нельзя.
pub async fn a_hand_placed_template_cannot_be_verified() {
    let codly = spec("codly", (1, 3, 0));
    let mut vault = MemoryVault::new();
    let root = package_root(&codly);
    vault
        .write_bytes(&root.join(PACKAGE_MANIFEST), MANIFEST)
        .await
        .expect("манифест");

    assert!(
        matches!(
            verify_installed(&vault, &codly).await,
            FingerprintCheck::Unknown
        ),
        "сверять не с чем"
    );
}

/// US5 AS1: установленное перечисляется с отпечатком, удалённое — исчезает.
pub async fn the_registry_lists_installed_templates() {
    let codly = spec("codly", (1, 3, 0));
    let mut vault = MemoryVault::new();
    assert!(
        installed_templates(&vault).await.is_empty(),
        "пустой vault — пустой перечень"
    );

    let files = vec![
        ("lib.typ".to_string(), b"#let codly() = {}".to_vec()),
        (PACKAGE_MANIFEST.to_string(), MANIFEST.to_vec()),
    ];
    let digest = install(&mut vault, &codly, &files)
        .await
        .expect("установка");
    remember_installed(
        &mut vault,
        InstalledTemplate {
            spec: codly.to_string(),
            url: "https://packages.typst.org/preview/codly-1.3.0.tar.gz".to_string(),
            fingerprint: digest.clone(),
            file_count: files.len(),
        },
    )
    .await;

    let listed = installed_templates(&vault).await;
    assert_eq!(listed.len(), 1, "установленный шаблон в перечне");
    assert_eq!(listed[0].spec, "@preview/codly:1.3.0");
    assert_eq!(listed[0].fingerprint, digest);

    // Шаблон удалили руками: истина на диске, а не в реестре.
    vault.remove(&download_root(&codly).join(PACKAGE_MANIFEST));
    assert!(
        installed_templates(&vault).await.is_empty(),
        "удалённый руками шаблон из перечня исчез"
    );
}

// ── Путь сборки: когда приложение идёт за шаблоном, а когда — нет ──────────
//
// Сети нет и здесь. Источник шаблона указан адресом-ловушкой, который не
// разбирается как URL ни на одном таргете: попытка загрузки обрывается до
// первого байта в сети, но оставляет диагностику с этим адресом (FR-007).
// Отсюда счётчик обращений без подмены транспорта: **каждая** попытка
// установки — удачная или нет — порождает замечание, называющее адрес, и
// «адрес не упомянут» равносильно «попытки не было».

use typsidian_core::preview::compile::TemplatePolicy;
use typsidian_core::preview::{
    compile_note_with_templates, install_template, CompileOutcome, CompiledDocument, FontSet,
    PackageCache,
};

/// Адрес, по которому уйти в сеть невозможно.
pub const TRAP: &str = "ловушка:не адрес";

const NOTE: &str = "заметка.typ";
const DEMO: &str = "@preview/demo:1.0.0";
const DEMO_MANIFEST: &str =
    "[package]\nname = \"demo\"\nversion = \"1.0.0\"\nentrypoint = \"lib.typ\"\n";
const NOTE_WITH_DEMO: &str = "#import \"@preview/demo:1.0.0\": привет\n#привет\n";

fn trap_policy(auto_download: bool) -> TemplatePolicy {
    TemplatePolicy {
        auto_download,
        sources: vec![(DEMO.to_string(), TRAP.to_string())],
    }
}

async fn install_demo(vault: &mut MemoryVault) -> String {
    let files = vec![
        (
            "lib.typ".to_string(),
            "#let привет = [Привет]\n".as_bytes().to_vec(),
        ),
        (
            PACKAGE_MANIFEST.to_string(),
            DEMO_MANIFEST.as_bytes().to_vec(),
        ),
    ];
    install(vault, &spec("demo", (1, 0, 0)), &files)
        .await
        .expect("установка demo")
}

async fn compile(
    vault: &mut MemoryVault,
    packages: &PackageCache,
    path: &str,
    text: &str,
    policy: &TemplatePolicy,
) -> CompiledDocument {
    compile_note_with_templates(vault, packages, path, text, 1, FontSet::builtin(), policy)
        .await
        .expect("сборка")
}

fn messages(document: &CompiledDocument) -> Vec<String> {
    document
        .diagnostics
        .iter()
        .map(|d| d.message.clone())
        .collect()
}

/// Есть ли замечание о расхождении отпечатка шаблона `spec` — по виду
/// сообщения, а не по тексту: текст собирает интерфейс на языке пользователя.
fn diverged(document: &CompiledDocument, spec: &str) -> bool {
    document.diagnostics.iter().any(|d| {
        matches!(&d.app, Some(AppMessage::TemplateFingerprintDiverged { spec: s, .. }) if s == spec)
    })
}

fn attempted(document: &CompiledDocument) -> bool {
    messages(document).iter().any(|m| m.contains(TRAP))
}

/// SC-002, US1 AS2: установленная версия не перекачивается — ни при первом
/// открытии, ни при повторном.
pub async fn an_installed_template_is_not_fetched_again() {
    let mut vault = MemoryVault::new();
    install_demo(&mut vault).await;
    let packages = PackageCache::new();
    let policy = trap_policy(true);

    for round in 1..=2 {
        let document = compile(&mut vault, &packages, NOTE, NOTE_WITH_DEMO, &policy).await;
        assert_eq!(
            document.outcome,
            CompileOutcome::Succeeded,
            "проход {round}: {:?}",
            messages(&document)
        );
        assert!(
            !attempted(&document),
            "проход {round}: обращения быть не должно"
        );
    }
}

/// FR-008, SC-009, US2 AS3: заметка без подключений — ни обращений, ни
/// сообщений о загрузке.
pub async fn a_note_without_imports_never_reaches_for_the_network() {
    let mut vault = MemoryVault::new();
    let packages = PackageCache::new();
    let document = compile(
        &mut vault,
        &packages,
        NOTE,
        "= Заголовок\n\nПросто текст.\n",
        &trap_policy(true),
    )
    .await;

    assert_eq!(document.outcome, CompileOutcome::Succeeded);
    assert!(
        document.diagnostics.is_empty(),
        "ни одного замечания: {:?}",
        messages(&document)
    );
}

/// FR-021, принцип IX, на пути `fetch_and_install`: адрес, к которому
/// обратилось приложение, — ровно заданный для шаблона, и ничего о заметке в
/// него не добавлено.
pub async fn the_fetch_path_sends_nothing_about_the_note() {
    let mut vault = MemoryVault::new();
    let packages = PackageCache::new();
    let path = "Идеи/Заметка apokryf.typ";
    let text = format!("// секрет-в-тексте\n{NOTE_WITH_DEMO}");

    let document = compile(&mut vault, &packages, path, &text, &trap_policy(true)).await;
    let notes = messages(&document);
    let named: Vec<&String> = notes.iter().filter(|m| m.contains(TRAP)).collect();
    assert_eq!(named.len(), 1, "одна попытка, один адрес: {notes:?}");

    // Всё, что стоит в сообщении после адреса, — формулировка причины, а не
    // хвост запроса: адрес назван ровно тем, что задано, без добавок.
    let message = named[0];
    for leak in ["Идеи", "Заметк", "apokryf", "секрет"] {
        assert!(
            !message.contains(leak),
            "«{leak}» ушло бы наружу: {message}"
        );
    }
}

/// FR-009, SC-004: выключено — в сеть не ходим, причина названа.
pub async fn disabled_auto_download_makes_no_attempt() {
    let mut vault = MemoryVault::new();
    let packages = PackageCache::new();
    let document = compile(
        &mut vault,
        &packages,
        NOTE,
        NOTE_WITH_DEMO,
        &trap_policy(false),
    )
    .await;

    assert!(!attempted(&document), "выключено — обращений нет");
    let notes = messages(&document);
    assert!(
        notes
            .iter()
            .any(|m| m.contains(DEMO) && m.contains("автозагрузка выключена")),
        "шаблон назван по имени и версии: {notes:?}"
    );
    // FR-010: интерфейсу есть что предложить загрузить явно — спецификация
    // приходит полем, а не разбором текста.
    assert!(
        document
            .diagnostics
            .iter()
            .any(|d| d.template.as_deref() == Some(DEMO)),
        "замечание несёт шаблон для явной загрузки"
    );
    // Фича 010: причина уходит в интерфейс видом и параметрами.
    assert!(
        document.diagnostics.iter().any(|d| d.app
            == Some(AppMessage::TemplateInstallFailed {
                spec: DEMO.to_string(),
                reason: InstallError::AutoDownloadDisabled {
                    spec: DEMO.to_string()
                },
            })),
        "{:?}",
        document.diagnostics
    );
}

/// FR-001: включено — попытка есть.
pub async fn enabled_auto_download_makes_an_attempt() {
    let mut vault = MemoryVault::new();
    let packages = PackageCache::new();
    let document = compile(
        &mut vault,
        &packages,
        NOTE,
        NOTE_WITH_DEMO,
        &trap_policy(true),
    )
    .await;
    assert!(
        attempted(&document),
        "включено — попытка обязана быть: {:?}",
        messages(&document)
    );
}

/// FR-010, US3 AS2: явный запрос работает и при выключенной автозагрузке.
pub async fn an_explicit_install_ignores_the_switch() {
    let mut vault = MemoryVault::new();
    let error = install_template(&mut vault, DEMO, &trap_policy(false))
        .await
        .expect_err("ловушка не загружается");

    assert!(
        !matches!(error, InstallError::AutoDownloadDisabled { .. }),
        "выключатель не должен запирать явный запрос"
    );
    assert!(
        error.to_string().contains(TRAP),
        "попытка была, и её адрес назван: {error}"
    );
}

/// FR-004, US4 AS2: указанный адрес не подменяется реестром, а его отказ
/// назван вместе с этим адресом.
pub async fn a_user_url_is_never_replaced_by_the_registry() {
    let mut vault = MemoryVault::new();
    let packages = PackageCache::new();
    let document = compile(
        &mut vault,
        &packages,
        NOTE,
        NOTE_WITH_DEMO,
        &trap_policy(true),
    )
    .await;

    let notes = messages(&document);
    assert!(
        notes.iter().any(|m| m.contains(TRAP)),
        "назван указанный адрес: {notes:?}"
    );
    assert!(
        !notes.iter().any(|m| m.contains("packages.typst.org")),
        "реестр вместо указанного адреса не подставлен: {notes:?}"
    );
}

/// FR-005, US1 AS4: у `@local` источника нет — ни попытки, ни сообщения о
/// загрузке; шаблон назван отсутствующим, как до фичи 007.
pub async fn a_local_namespace_is_only_reported_missing() {
    let mut vault = MemoryVault::new();
    let packages = PackageCache::new();
    let policy = TemplatePolicy {
        auto_download: true,
        sources: Vec::new(),
    };
    let document = compile(
        &mut vault,
        &packages,
        NOTE,
        "#import \"@local/мой:0.1.0\": x\n",
        &policy,
    )
    .await;

    let notes = messages(&document);
    assert_eq!(document.outcome, CompileOutcome::Failed);
    assert!(
        notes.iter().any(|m| m.contains("@local/мой:0.1.0")),
        "шаблон назван: {notes:?}"
    );
    assert!(
        !notes.iter().any(|m| m.contains("источник")
            || m.contains("загружен")
            || m.contains("в источнике нет")),
        "ни слова о загрузке, которой не было: {notes:?}"
    );
}

/// FR-023 на пути сборки: правка установленного шаблона названа, но ничего не
/// переустановлено и не перезаписано.
pub async fn compiling_reports_a_tampered_template_without_reinstalling() {
    let mut vault = MemoryVault::new();
    install_demo(&mut vault).await;
    let lib = download_root(&spec("demo", (1, 0, 0))).join("lib.typ");
    let edited = "#let привет = [Изменено]\n";
    vault
        .write_bytes(&lib, edited.as_bytes())
        .await
        .expect("правка файла");

    let packages = PackageCache::new();
    let document = compile(
        &mut vault,
        &packages,
        NOTE,
        NOTE_WITH_DEMO,
        &trap_policy(true),
    )
    .await;

    let notes = messages(&document);
    assert_eq!(document.outcome, CompileOutcome::Succeeded, "{notes:?}");
    assert!(diverged(&document, DEMO), "расхождение названо: {notes:?}");
    assert!(!attempted(&document), "переустановки не было");
    assert_eq!(
        vault.read_bytes(&lib).await.expect("чтение"),
        edited.as_bytes(),
        "правка пользователя на месте"
    );

    // Сверка — раз за сеанс: вторая сборка не повторяет замечание.
    let again = compile(
        &mut vault,
        &packages,
        NOTE,
        NOTE_WITH_DEMO,
        &trap_policy(true),
    )
    .await;
    assert!(
        !diverged(&again, DEMO),
        "замечание не повторяется на каждое нажатие"
    );
}
