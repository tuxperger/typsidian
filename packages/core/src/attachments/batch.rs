//! Пачка вложений — машина состояний одного действия пользователя (FR-005,
//! FR-013, FR-023, research.md §10).
//!
//! Одна картинка из буфера — пачка из одного элемента, отдельного пути нет.
//! Состояние ходит через границу значением: UI держит его, просит `step`,
//! читает байты выданного файла и отдаёт их в `store`. Переходы — чистые
//! функции и проверяются на обоих таргетах; отмена действует между файлами.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::folder::target_folder;
use super::name::InsertMoment;
use super::LARGE_IMAGE_BYTES;
use crate::sync::ignore::IgnoreFile;
use crate::vault::backend::VaultBackend;
use crate::vault::vault_settings::VaultOverrides;
use crate::vault::{classify, AccessState, FileKind, ImageFile};

/// Откуда пришёл файл: от этого зависит его имя (FR-012).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Origin {
    Clipboard,
    Camera,
    /// Перетащен, выбран в диалоге или взят из галереи — у него своё имя.
    File,
}

/// Входящий файл до чтения байтов.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct IncomingItem {
    /// Имя, как его дала платформа; для буфера и камеры не используется.
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub name: Option<String>,
    /// Размер известен до чтения байтов (`File.size`).
    pub size: u64,
    pub origin: Origin,
}

/// Элемент пачки и его судьба.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct BatchItem {
    pub incoming: IncomingItem,
    pub status: ItemStatus,
}

/// Состояние элемента. `Stored` и `Reused` — «вставляемые»: только их пути
/// попадают в текст.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum ItemStatus {
    Queued,
    /// Больше `LARGE_IMAGE_BYTES`: ждёт ответа пользователя (FR-013).
    AwaitingConfirmation,
    Copying,
    /// Записан. `path` — от корня vault, `markup_path` — от папки заметки;
    /// `saved_as` — имя на диске, если оно отличается от исходного.
    Stored {
        path: String,
        markup_path: String,
        bytes: u64,
        #[cfg_attr(target_arch = "wasm32", tsify(optional))]
        saved_as: Option<String>,
    },
    /// Такой файл уже лежит в vault — копия не создана (FR-010, FR-011).
    Reused {
        path: String,
        markup_path: String,
    },
    /// Вид не поддерживается, файл не записан (FR-006).
    Skipped,
    /// Пользователь отказался от большого файла.
    Declined,
    Failed {
        problem: StoreProblem,
    },
    Cancelled,
}

/// Почему файл не записан (FR-014).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum StoreProblem {
    /// Запись отклонена: vault стал недоступен на запись.
    AccessDenied,
    /// Запись не удалась — нет места и прочее.
    Io,
    /// Свободного имени не нашлось.
    NameBusy,
    /// UI не смог прочитать исходный файл (`source_failed`).
    SourceUnreadable,
}

/// Состояние пачки (data-model.md «Пачка»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct AttachmentBatchState {
    pub note_path: String,
    /// Папка назначения от корня vault, через `/` — для подписи прогресса и
    /// диалога.
    pub target_folder: String,
    pub items: Vec<BatchItem>,
    pub moment: InsertMoment,
    pub phase: AttachmentBatchPhase,
    /// Файлы правил, исключающие папку назначения (FR-020); пусто — не
    /// исключена. Записи не мешает: это предупреждение.
    pub excluded_by: Vec<IgnoreFile>,
    /// Кандидаты сопоставления по содержимому (research.md §7).
    pub known: Vec<ImageFile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum AttachmentBatchPhase {
    /// Ничего не будет сделано.
    Refused {
        reason: AttachmentRefusalReason,
    },
    Running,
    Cancelled,
    Finished,
}

/// Почему пачка не начата (FR-024).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub enum AttachmentRefusalReason {
    ReadOnly,
    NotANote,
}

/// Что UI делает дальше.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
#[serde(tag = "kind")]
pub enum NextAction {
    /// Спросить пользователя о большом файле.
    Confirm { index: u32 },
    /// Прочитать байты файла и отдать их в `store`.
    Copy { index: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct AttachmentBatchStep {
    pub state: AttachmentBatchState,
    #[cfg_attr(target_arch = "wasm32", tsify(optional))]
    pub next: Option<NextAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct AttachmentBatchView {
    pub state: AttachmentBatchState,
    /// Всё, что уже не `Queued`, не `AwaitingConfirmation` и не `Copying`.
    pub processed: u32,
    pub total: u32,
    pub is_over: bool,
    pub summary: AttachmentBatchSummary,
    /// Пути `Stored` и `Reused` в порядке элементов — их вставляет UI.
    pub markup_paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct AttachmentBatchSummary {
    pub stored: u32,
    pub reused: u32,
    pub skipped: u32,
    pub declined: u32,
    pub failed: u32,
    pub cancelled: u32,
}

/// Начинает пачку (контракт «`start`, по порядку»).
pub async fn start<B: VaultBackend>(
    backend: &B,
    overrides: &VaultOverrides,
    note_path: &str,
    items: Vec<IncomingItem>,
    moment: InsertMoment,
) -> AttachmentBatchState {
    let note = Path::new(note_path);
    let folder = target_folder(overrides, note);
    let mut state = AttachmentBatchState {
        note_path: note_path.to_owned(),
        target_folder: slash(&folder),
        items: items
            .into_iter()
            .map(|incoming| BatchItem {
                status: if incoming.size > LARGE_IMAGE_BYTES {
                    ItemStatus::AwaitingConfirmation
                } else {
                    ItemStatus::Queued
                },
                incoming,
            })
            .collect(),
        moment,
        phase: AttachmentBatchPhase::Running,
        excluded_by: Vec::new(),
        known: Vec::new(),
    };

    // 1. Отказы — без обращений к диску.
    if classify(note) != FileKind::Note {
        state.phase = AttachmentBatchPhase::Refused {
            reason: AttachmentRefusalReason::NotANote,
        };
        return state;
    }
    if backend.access_state() != AccessState::ReadWrite {
        state.phase = AttachmentBatchPhase::Refused {
            reason: AttachmentRefusalReason::ReadOnly,
        };
        return state;
    }
    if state.items.is_empty() {
        state.phase = AttachmentBatchPhase::Finished;
        return state;
    }

    // 3. Кандидаты: папка назначения и, для файлов со своим именем, весь vault
    //    по совпадению имени и размера — обход один раз на действие.
    let mut known = backend.list_images(Some(&folder)).await;
    if state
        .items
        .iter()
        .any(|item| item.incoming.origin == Origin::File)
    {
        for image in backend.list_images(None).await {
            let same = state.items.iter().any(|item| {
                item.incoming.origin == Origin::File
                    && item.incoming.size == image.size
                    && item
                        .incoming
                        .name
                        .as_deref()
                        .is_some_and(|name| same_file_name(&image.path, name))
            });
            if same && !known.contains(&image) {
                known.push(image);
            }
        }
    }
    state.known = known;

    // 4. Предупреждение о правилах (FR-020).
    state.excluded_by = exclusion(backend, &folder).await;
    state
}

/// Следующее действие. Сначала все подтверждения, потом копирование:
/// пользователь отвечает до начала работы, а не посреди неё.
pub fn step(mut state: AttachmentBatchState) -> AttachmentBatchStep {
    if state.phase != AttachmentBatchPhase::Running
        || state
            .items
            .iter()
            .any(|item| item.status == ItemStatus::Copying)
    {
        return AttachmentBatchStep { state, next: None };
    }
    if let Some(index) = position(&state, &ItemStatus::AwaitingConfirmation) {
        return AttachmentBatchStep {
            state,
            next: Some(NextAction::Confirm { index }),
        };
    }
    if let Some(index) = position(&state, &ItemStatus::Queued) {
        state.items[index as usize].status = ItemStatus::Copying;
        return AttachmentBatchStep {
            state,
            next: Some(NextAction::Copy { index }),
        };
    }
    state.phase = AttachmentBatchPhase::Finished;
    AttachmentBatchStep { state, next: None }
}

/// Ответ о большом файле (FR-013). Не для ожидающего — состояние не меняется.
pub fn confirm(mut state: AttachmentBatchState, index: u32, accept: bool) -> AttachmentBatchState {
    if let Some(item) = state.items.get_mut(index as usize) {
        if item.status == ItemStatus::AwaitingConfirmation {
            item.status = if accept {
                ItemStatus::Queued
            } else {
                ItemStatus::Declined
            };
        }
    }
    state
}

/// Записывает файл элемента в `Copying`; иначе состояние не меняется.
pub async fn store<B: VaultBackend>(
    backend: &mut B,
    mut state: AttachmentBatchState,
    index: u32,
    bytes: &[u8],
) -> AttachmentBatchState {
    let Some(item) = state.items.get(index as usize) else {
        return state;
    };
    if item.status != ItemStatus::Copying {
        return state;
    }
    let incoming = item.incoming.clone();
    let folder = PathBuf::from(&state.target_folder);
    let status = super::store::store_item(
        backend,
        Path::new(&state.note_path),
        &folder,
        &incoming,
        &state.moment,
        &state.known,
        bytes,
    )
    .await;
    if let ItemStatus::Stored { path, bytes, .. } = &status {
        // Повтор содержимого внутри пачки даёт `Reused`.
        state.known.push(ImageFile {
            path: PathBuf::from(path),
            size: *bytes,
        });
    }
    state.items[index as usize].status = status;
    state
}

/// UI не смог прочитать исходный файл.
pub fn source_failed(mut state: AttachmentBatchState, index: u32) -> AttachmentBatchState {
    if let Some(item) = state.items.get_mut(index as usize) {
        if item.status == ItemStatus::Copying {
            item.status = ItemStatus::Failed {
                problem: StoreProblem::SourceUnreadable,
            };
        }
    }
    state
}

/// Отмена (FR-023): текущий файл доводится до конца, остальные отменяются.
pub fn cancel(mut state: AttachmentBatchState) -> AttachmentBatchState {
    if state.phase != AttachmentBatchPhase::Running {
        return state;
    }
    state.phase = AttachmentBatchPhase::Cancelled;
    for item in &mut state.items {
        if matches!(
            item.status,
            ItemStatus::Queued | ItemStatus::AwaitingConfirmation
        ) {
            item.status = ItemStatus::Cancelled;
        }
    }
    state
}

/// Прогресс, итог и пути для вставки.
pub fn view(state: AttachmentBatchState) -> AttachmentBatchView {
    let mut summary = AttachmentBatchSummary::default();
    let mut processed = 0;
    let mut copying = false;
    let mut markup_paths = Vec::new();
    for item in &state.items {
        match &item.status {
            ItemStatus::Queued | ItemStatus::AwaitingConfirmation => continue,
            ItemStatus::Copying => {
                copying = true;
                continue;
            }
            ItemStatus::Stored { markup_path, .. } => {
                summary.stored += 1;
                markup_paths.push(markup_path.clone());
            }
            ItemStatus::Reused { markup_path, .. } => {
                summary.reused += 1;
                markup_paths.push(markup_path.clone());
            }
            ItemStatus::Skipped => summary.skipped += 1,
            ItemStatus::Declined => summary.declined += 1,
            ItemStatus::Failed { .. } => summary.failed += 1,
            ItemStatus::Cancelled => summary.cancelled += 1,
        }
        processed += 1;
    }
    let is_over = !copying && state.phase != AttachmentBatchPhase::Running;
    AttachmentBatchView {
        processed,
        total: state.items.len() as u32,
        is_over,
        summary,
        markup_paths,
        state,
    }
}

/// Файлы правил в корне vault, исключающие папку назначения (FR-020,
/// research.md §12). Файла нет — он ничего не исключает.
pub async fn exclusion<B: VaultBackend>(backend: &B, folder: &Path) -> Vec<IgnoreFile> {
    let folder = slash(folder);
    let mut excluded = Vec::new();
    for file in [IgnoreFile::GitIgnore, IgnoreFile::StIgnore] {
        if let Ok(text) = backend.read(Path::new(file.path())).await {
            if crate::sync::ignore_match::excludes(file, &text, &folder) {
                excluded.push(file);
            }
        }
    }
    excluded
}

fn position(state: &AttachmentBatchState, status: &ItemStatus) -> Option<u32> {
    state
        .items
        .iter()
        .position(|item| &item.status == status)
        .map(|index| index as u32)
}

/// Путь от корня vault через `/` — одинаковый на всех платформах.
pub(crate) fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Совпадает ли имя файла на диске с именем входящего, без учёта регистра:
/// vault может доставляться на файловую систему без различения регистра.
pub(crate) fn same_file_name(path: &Path, name: &str) -> bool {
    let incoming = name.rsplit(['/', '\\']).next().unwrap_or_default();
    path.file_name()
        .and_then(|own| own.to_str())
        .is_some_and(|own| own.to_lowercase() == incoming.to_lowercase())
}
