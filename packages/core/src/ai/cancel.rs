//! Отмена запроса (research.md §9). Ключ включает id vault: на сервере его
//! подставляет сервер, поэтому один пользователь не отменит запрос другого.

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

use futures_channel::oneshot;
use futures_util::future::{select, Either};

use super::AiFailure;

/// Занятые id и отмена каждого. Номер нужен, чтобы завершившийся запрос не снял
/// слот следующего с тем же id, занятый уже после отмены.
type Slots = HashMap<String, (u64, oneshot::Sender<()>)>;
static PENDING: LazyLock<Mutex<Slots>> = LazyLock::new(Mutex::default);
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Идущие циклы урока: отмена между ходами цикла не теряется (ревью безопасности 051, L2).
/// Значение — отменён ли цикл.
static CYCLES: LazyLock<Mutex<HashMap<String, bool>>> = LazyLock::new(Mutex::default);

fn key(vault_id: &str, request_id: &str) -> String {
    format!("{vault_id}/{request_id}")
}

/// Снимает слот при любом выходе, в том числе когда ожидание просто бросили (оборвалось
/// соединение, закрыта вкладка): иначе слот остался бы занятым навсегда (M6).
struct Slot {
    key: String,
    ticket: u64,
}

impl Drop for Slot {
    fn drop(&mut self) {
        if let Ok(mut pending) = PENDING.lock() {
            if pending
                .get(&self.key)
                .is_some_and(|(owner, _)| *owner == self.ticket)
            {
                pending.remove(&self.key);
            }
        }
    }
}

/// Ждёт запрос, пока его не отменят. Ожидание прекращается сразу.
///
/// Id, под которым уже идёт запрос, второй раз не занимается: иначе второй
/// затёр бы отмену первого, а первый, завершившись, снял бы отмену второго
/// (ревью безопасности 050, L3). `work` не выполняется, пока слот не занят: всё, что
/// он делает с общим состоянием по этому id, второй запрос испортить не может.
pub async fn cancellable<T, F>(vault_id: &str, request_id: &str, work: F) -> Result<T, AiFailure>
where
    F: Future<Output = Result<T, AiFailure>>,
{
    let (sender, receiver) = oneshot::channel();
    let slot = key(vault_id, request_id);
    let ticket = NEXT.fetch_add(1, Ordering::Relaxed);
    {
        let Ok(mut pending) = PENDING.lock() else {
            return Err(AiFailure::Cancelled);
        };
        if pending.contains_key(&slot) {
            return Err(AiFailure::Cancelled);
        }
        pending.insert(slot.clone(), (ticket, sender));
    }
    let _slot = Slot { key: slot, ticket };
    futures_util::pin_mut!(work);
    match select(work, receiver).await {
        Either::Left((result, _)) => result,
        Either::Right(_) => Err(AiFailure::Cancelled),
    }
}

pub fn cancel(vault_id: &str, request_id: &str) {
    let slot = key(vault_id, request_id);
    if let Ok(mut cycles) = CYCLES.lock() {
        if let Some(cancelled) = cycles.get_mut(&slot) {
            *cancelled = true;
        }
    }
    if let Ok(mut pending) = PENDING.lock() {
        if let Some((_, sender)) = pending.remove(&slot) {
            let _ = sender.send(());
        }
    }
}

/// Цикл ходов урока под одним `request_id`: пока он открыт, отмена запоминается и
/// действует на следующие ходы, даже если пришла между ними, когда сетевого запроса нет.
/// Закрывается при сбросе, в том числе когда соединение оборвалось.
pub struct Cycle {
    key: String,
}

impl Cycle {
    /// `None`, если цикл с этим id уже идёт: второй не должен писать в тот же урок и
    /// трогать буфер первого (ревью безопасности 051, L1).
    pub fn open(vault_id: &str, request_id: &str) -> Option<Cycle> {
        let key = key(vault_id, request_id);
        let mut cycles = CYCLES.lock().ok()?;
        if cycles.contains_key(&key) {
            return None;
        }
        cycles.insert(key.clone(), false);
        Some(Cycle { key })
    }

    pub fn is_cancelled(&self) -> bool {
        CYCLES
            .lock()
            .map(|cycles| cycles.get(&self.key).copied().unwrap_or(true))
            .unwrap_or(true)
    }
}

impl Drop for Cycle {
    fn drop(&mut self) {
        if let Ok(mut cycles) = CYCLES.lock() {
            cycles.remove(&self.key);
        }
    }
}
