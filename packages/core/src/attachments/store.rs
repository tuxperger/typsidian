//! Запись одного вложения (контракт «`store` для элемента в `Copying`»).
//!
//! Порядок шагов — часть контракта и проверяется тестами: существующий файл
//! не изменяется ни в одном исходе (SC-003), потому что `write_bytes`
//! вызывается только по пути, для которого шаг 5 подтвердил отсутствие файла.
//! При `Skipped`, `Reused` и `Failed` число файлов в vault не растёт.

use std::path::Path;

use super::batch::{same_file_name, slash, IncomingItem, ItemStatus, Origin, StoreProblem};
use super::folder::markup_path;
use super::kind::sniff;
use super::name::{free_name, generated_name, kept_name, InsertMoment};
use crate::vault::backend::{VaultBackend, WriteError};
use crate::vault::ImageFile;

/// Определяет вид, сопоставляет по содержимому, выбирает имя и пишет.
pub async fn store_item<B: VaultBackend>(
    backend: &mut B,
    note_path: &Path,
    folder: &Path,
    incoming: &IncomingItem,
    moment: &InsertMoment,
    known: &[ImageFile],
    bytes: &[u8],
) -> ItemStatus {
    // 1. Вид — по содержимому (FR-006, FR-007).
    let Some(kind) = sniff(bytes) else {
        return ItemStatus::Skipped;
    };

    // 2. Имя: из заметки и момента либо собственное.
    let original = match incoming.origin {
        Origin::Clipboard | Origin::Camera => generated_name(note_path, moment, kind),
        Origin::File => kept_name(incoming.name.as_deref().unwrap_or_default(), kind),
    };
    let size = bytes.len() as u64;
    let reused = |path: &Path| ItemStatus::Reused {
        path: slash(path),
        markup_path: markup_path(note_path, path),
    };

    // 3. Файл со своим именем уже лежит где-то в vault (FR-011).
    if incoming.origin == Origin::File {
        for candidate in known
            .iter()
            .filter(|image| image.size == size && same_file_name(&image.path, &original))
        {
            if same_bytes(backend, &candidate.path, bytes).await {
                return reused(&candidate.path);
            }
        }
    }

    // 4. То же содержимое в папке назначения под любым именем (FR-010).
    for candidate in known
        .iter()
        .filter(|image| image.size == size && image.path.parent() == Some(folder))
    {
        if same_bytes(backend, &candidate.path, bytes).await {
            return reused(&candidate.path);
        }
    }

    // 5. Свободное имя: занято, если файл есть или в папке есть изображение
    //    с тем же именем без учёта регистра (FR-009).
    let mut taken = Vec::new();
    let mut candidate_name = original.clone();
    let name = loop {
        let Some(name) = free_name(&candidate_name, |name| {
            taken.iter().any(|busy: &String| busy == name)
                || known.iter().any(|image| {
                    image.path.parent() == Some(folder) && same_file_name(&image.path, name)
                })
        }) else {
            return ItemStatus::Failed {
                problem: StoreProblem::NameBusy,
            };
        };
        // Отметка файла — асинхронная, поэтому проверяется здесь, а не в
        // замыкании; найденное занятым имя запоминается и поиск повторяется.
        if backend.file_mark(&folder.join(&name)).await.is_none() {
            break name;
        }
        taken.push(name);
        candidate_name = original.clone();
    };

    // 6. Папка и атомарная запись (FR-014).
    let path = folder.join(&name);
    if let Err(error) = backend.ensure_folder(folder).await {
        return failed(error);
    }
    if let Err(error) = backend.write_bytes(&path, bytes).await {
        return failed(error);
    }

    // 7. Записан.
    ItemStatus::Stored {
        markup_path: markup_path(note_path, &path),
        path: slash(&path),
        bytes: size,
        saved_as: (name != original_name(incoming, &original)).then_some(name),
    }
}

/// С каким именем сравнивать итоговое, чтобы сказать «сохранён как …»: для
/// файла — его исходное имя (замена расширения тоже переименование), для
/// буфера и камеры — придуманное приложением.
fn original_name(incoming: &IncomingItem, generated: &str) -> String {
    match (incoming.origin, incoming.name.as_deref()) {
        (Origin::File, Some(name)) => name.rsplit(['/', '\\']).next().unwrap_or(name).to_owned(),
        _ => generated.to_owned(),
    }
}

async fn same_bytes<B: VaultBackend>(backend: &B, path: &Path, bytes: &[u8]) -> bool {
    backend
        .read_bytes_any_size(path)
        .await
        .is_ok_and(|existing| existing == bytes)
}

fn failed(error: WriteError) -> ItemStatus {
    ItemStatus::Failed {
        problem: match error {
            WriteError::AccessDenied => StoreProblem::AccessDenied,
            _ => StoreProblem::Io,
        },
    }
}
