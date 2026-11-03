//! Прогресс идущего хода (фича 051, research.md §3, §5).
//!
//! Два слоя. Буфер в памяти процесса по ключу `vault_id/request_id`: интерфейс
//! опрашивает его раз в 150 мс. Промежуточный файл устройства
//! `.typsidian/lesson-partial/<хэш пути>.txt`: интерфейс раз в секунду просит
//! переписать в него накопленное, чтобы закрытие приложения посреди ответа не
//! стоило полученного текста.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::ai::access::normalize_folder;
use crate::ai::AiFailure;
use crate::vault::backend::VaultBackend;
use crate::vault::service_dir;

/// Через сколько без единой записи забытый буфер убирается (ревью безопасности 051, M6).
/// Дольше общего предела потока (10 минут): идущий ход под уборку не попадает.
pub const BUFFER_TTL_MS: u64 = 20 * 60 * 1000;

/// Что видит интерфейс, пока идёт ответ.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LessonProgress {
    pub text: String,
    /// Заметка, которую учитель просит прочитать.
    pub reading: Option<String>,
}

struct Entry {
    vault_id: String,
    path: String,
    progress: LessonProgress,
    touched_ms: u64,
}

static BUFFERS: LazyLock<Mutex<HashMap<String, Entry>>> = LazyLock::new(Mutex::default);

fn key(vault_id: &str, request_id: &str) -> String {
    format!("{vault_id}/{request_id}")
}

/// Монотонные миллисекунды: на wasm32 `Instant` недоступен.
fn now_ms() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        static START: LazyLock<std::time::Instant> = LazyLock::new(std::time::Instant::now);
        START.elapsed().as_millis() as u64
    }
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as u64
    }
}

/// Путь урока в одной форме: `Уроки/./а.lesson` и `Уроки/а.lesson` — один урок, иначе
/// `is_active` и промежуточный файл расходились бы по написанию (ревью безопасности 051, L3).
pub fn canonical(path: &str) -> String {
    let normalized = normalize_folder(Path::new(path)).unwrap_or_else(|| path.to_owned());
    normalized.nfc().collect()
}

/// Убирает буферы, к которым не прикасались дольше `max_age_ms`. Зовётся при открытии
/// каждого нового буфера.
pub fn sweep_stale(max_age_ms: u64) {
    let now = now_ms();
    if let Ok(mut buffers) = BUFFERS.lock() {
        buffers.retain(|_, entry| now.saturating_sub(entry.touched_ms) < max_age_ms);
    }
}

/// Открывает буфер хода. Прежний буфер с тем же ключом (следующий ход цикла) сбрасывается.
pub fn start(vault_id: &str, request_id: &str, path: &str) {
    sweep_stale(BUFFER_TTL_MS);
    if let Ok(mut buffers) = BUFFERS.lock() {
        buffers.insert(
            key(vault_id, request_id),
            Entry {
                vault_id: vault_id.to_owned(),
                path: canonical(path),
                progress: LessonProgress {
                    text: String::new(),
                    reading: None,
                },
                touched_ms: now_ms(),
            },
        );
    }
}

/// Дописывает в буфер полученное: передаётся только новая часть, копировать накопленное
/// на каждом куске потока незачем (M5).
pub fn update(vault_id: &str, request_id: &str, delta: &str, reading: Option<String>) {
    if let Ok(mut buffers) = BUFFERS.lock() {
        if let Some(entry) = buffers.get_mut(&key(vault_id, request_id)) {
            entry.progress.text.push_str(delta);
            if reading.is_some() {
                entry.progress.reading = reading;
            }
            entry.touched_ms = now_ms();
        }
    }
}

pub fn progress(vault_id: &str, request_id: &str) -> Option<LessonProgress> {
    BUFFERS
        .lock()
        .ok()?
        .get(&key(vault_id, request_id))
        .map(|entry| entry.progress.clone())
}

/// Забирает накопленный текст хода и закрывает буфер.
pub fn take(vault_id: &str, request_id: &str) -> Option<String> {
    BUFFERS
        .lock()
        .ok()?
        .remove(&key(vault_id, request_id))
        .map(|entry| entry.progress.text)
}

/// Закрывает буфер хода при любом выходе, в том числе когда ожидание ответа бросили
/// (оборвалось соединение): иначе буфер остался бы навсегда, а `is_active` считал бы ход
/// идущим и не давал восстановить оборванный ответ (ревью безопасности 051, M6).
pub struct BufferGuard {
    key: String,
}

impl BufferGuard {
    pub fn new(vault_id: &str, request_id: &str) -> Self {
        BufferGuard {
            key: key(vault_id, request_id),
        }
    }
}

impl Drop for BufferGuard {
    fn drop(&mut self) {
        if let Ok(mut buffers) = BUFFERS.lock() {
            buffers.remove(&self.key);
        }
    }
}

/// Идёт ли сейчас в этом процессе ход этого урока. Пока идёт, оборванный ответ из
/// промежуточного файла не восстанавливается: он ещё не оборван.
pub fn is_active(path: &str) -> bool {
    let path = canonical(path);
    BUFFERS
        .lock()
        .map(|buffers| buffers.values().any(|entry| entry.path == path))
        .unwrap_or(false)
}

/// То же в пределах одного vault: ход чужого vault на том же пути (на сервере — чужой
/// аккаунт) не виден и удалению урока не мешает (ревью безопасности 054, L2). `None` — любой
/// vault: оболочка без идентификатора vault (рабочий стол, web) однопользовательская.
pub fn is_active_in(vault_id: Option<&str>, path: &str) -> bool {
    let path = canonical(path);
    BUFFERS
        .lock()
        .map(|buffers| {
            buffers.values().any(|entry| {
                entry.path == path && vault_id.is_none_or(|vault| entry.vault_id == vault)
            })
        })
        .unwrap_or(false)
}

/// Файл устройства для урока по `path`.
pub fn partial_path(lesson_path: &str) -> PathBuf {
    let hash = blake3::hash(canonical(lesson_path).as_bytes()).to_hex();
    service_dir::lesson_partial_dir().join(format!("{}.txt", &hash.as_str()[..16]))
}

/// Переписывает промежуточный файл накопленным текстом хода. Нет хода или текста — ничего
/// не делает. Сбой записи отказом не считается: это страховка, а не основная запись.
///
/// Путь урока берётся из буфера хода, а не от клиента: иначе чужой путь стёр бы или
/// подменил промежуточный файл другого урока (ревью безопасности 051, L3). `lesson_path`
/// клиента служит только сверкой: расхождение — ничего не пишется.
pub async fn checkpoint<B: VaultBackend>(
    backend: &mut B,
    vault_id: &str,
    lesson_path: &str,
    request_id: &str,
) -> Result<(), AiFailure> {
    let (path, text) = {
        let Ok(buffers) = BUFFERS.lock() else {
            return Ok(());
        };
        let Some(entry) = buffers.get(&key(vault_id, request_id)) else {
            return Ok(());
        };
        (entry.path.clone(), entry.progress.text.clone())
    };
    if text.is_empty() || canonical(lesson_path) != path {
        return Ok(());
    }
    let _ = backend
        .ensure_folder(&service_dir::lesson_partial_dir())
        .await;
    let _ = backend
        .write_bytes(&partial_path(&path), text.as_bytes())
        .await;
    Ok(())
}

/// Текст оборванного ответа, если промежуточный файл есть и не пуст.
pub async fn read_partial<B: VaultBackend>(backend: &B, lesson_path: &str) -> Option<String> {
    let bytes = backend.read_bytes(&partial_path(lesson_path)).await.ok()?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    (!text.is_empty()).then_some(text)
}

pub async fn clear_partial<B: VaultBackend>(backend: &mut B, lesson_path: &str) {
    let _ = backend
        .remove_service_file(&partial_path(lesson_path))
        .await;
}
