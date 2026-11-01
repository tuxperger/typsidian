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
static PENDING: LazyLock<Mutex<HashMap<String, (u64, oneshot::Sender<()>)>>> =
    LazyLock::new(Mutex::default);
static NEXT: AtomicU64 = AtomicU64::new(0);

fn key(vault_id: &str, request_id: &str) -> String {
    format!("{vault_id}/{request_id}")
}

/// Ждёт запрос, пока его не отменят. Ожидание прекращается сразу.
///
/// Id, под которым уже идёт запрос, второй раз не занимается: иначе второй
/// затёр бы отмену первого, а первый, завершившись, снял бы отмену второго
/// (ревью безопасности 050, L3).
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
    futures_util::pin_mut!(work);
    let result = match select(work, receiver).await {
        Either::Left((result, _)) => result,
        Either::Right(_) => Err(AiFailure::Cancelled),
    };
    if let Ok(mut pending) = PENDING.lock() {
        if pending.get(&slot).is_some_and(|(owner, _)| *owner == ticket) {
            pending.remove(&slot);
        }
    }
    result
}

pub fn cancel(vault_id: &str, request_id: &str) {
    if let Ok(mut pending) = PENDING.lock() {
        if let Some((_, sender)) = pending.remove(&key(vault_id, request_id)) {
            let _ = sender.send(());
        }
    }
}
