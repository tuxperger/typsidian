//! Синхронизация vault на сервере (фича 038): окружение аккаунта, id vault и
//! проверка адреса удалённого репозитория.
//!
//! Команды — те же `typsidian-host`, что у оболочки. Здесь только то, чем сервер
//! отличается от устройства: настройки устройства и секреты лежат в папке
//! аккаунта (`<данные>/sync/<аккаунт>/`), id vault назначает сервер, а адреса,
//! ведущие в сеть самого сервера, отклоняются (research.md §2–§4).

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;
use typsidian_core::sync::engine::server_policy;
use typsidian_core::sync::ShellCapabilities;
use typsidian_host::sync::{DevicePlace, FileSecrets, SyncHome};

use crate::Shared;

/// Окружение синхронизации аккаунта.
struct AccountSync {
    home: Arc<SyncHome>,
    secrets: Arc<FileSecrets>,
}

/// Окружения аккаунтов: одно на аккаунт, общее для его открытых vault — у них
/// одно хранилище настроек устройства, и две его копии теряли бы записи друг
/// друга.
#[derive(Default)]
pub struct SyncHomes(Mutex<HashMap<String, AccountSync>>);

/// Папка синхронизации аккаунта.
pub fn account_sync_dir(state: &Shared, account_id: &str) -> PathBuf {
    state.config.sync_dir().join(account_id)
}

impl SyncHomes {
    /// Окружение аккаунта; первое обращение создаёт его папку (0700).
    pub async fn home(&self, state: &Shared, account_id: &str) -> anyhow::Result<Arc<SyncHome>> {
        Ok(self.account(state, account_id).await?.0)
    }

    /// Окружение и файл секретов аккаунта — один экземпляр на аккаунт: две копии
    /// со своими замками теряли бы записи друг друга.
    async fn account(
        &self,
        state: &Shared,
        account_id: &str,
    ) -> anyhow::Result<(Arc<SyncHome>, Arc<FileSecrets>)> {
        let mut map = self.0.lock().await;
        if let Some(found) = map.get(account_id) {
            return Ok((found.home.clone(), found.secrets.clone()));
        }
        crate::private_dir(&state.config.sync_dir())?;
        let dir = account_sync_dir(state, account_id);
        crate::private_dir(&dir)?;
        let secrets = Arc::new(FileSecrets::new(dir.join("secrets.json")));
        let home = Arc::new(SyncHome::new(
            Arc::default(),
            DevicePlace::In(dir.join("device")),
            Some(dir.join("mirrors")),
            secrets.clone(),
            ShellCapabilities::server(),
        ));
        map.insert(
            account_id.to_owned(),
            AccountSync {
                home: home.clone(),
                secrets: secrets.clone(),
            },
        );
        Ok((home, secrets))
    }

    /// Удаляет секреты и зеркало vault (удаление vault).
    pub async fn forget_vault(&self, state: &Shared, account_id: &str, vault_id: &str) {
        let dir = account_sync_dir(state, account_id);
        let secrets = match self.account(state, account_id).await {
            Ok((_, secrets)) => secrets,
            Err(error) => {
                tracing::error!("секреты удалённого vault не удалены: {error:#}");
                return;
            }
        };
        if secrets.forget_vault(vault_id).await.is_err() {
            tracing::error!("секреты удалённого vault не удалены");
        }
        if let Some(mirror) = typsidian_host::sync::mirror_dir(&dir.join("mirrors"), vault_id) {
            if mirror.exists() {
                if let Err(error) = std::fs::remove_dir_all(&mirror) {
                    tracing::error!("зеркало удалённого vault не удалено: {error}");
                }
            }
        }
    }
}

/// Ведёт ли адрес удалённого репозитория в сеть самого сервера: `localhost`,
/// внутренние адреса ([`server_policy::is_internal_ip`]).
///
/// Это ранний отказ с понятной причиной. Имя, которое DNS разрешает во
/// внутренний адрес, ловит уже транспорт ядра — после разрешения, на каждом
/// соединении (research.md §3).
pub fn internal_remote(url: &str) -> bool {
    // Без `://` — ssh-адрес `git@host:путь` или путь на диске: им откажет ядро
    // (на сервере нет ssh и каталогов) и назовёт причину точнее.
    let Some(host) = remote_host(url) else {
        return false;
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") {
        return true;
    }
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    match bare.parse::<IpAddr>() {
        Ok(ip) => server_policy::is_internal_ip(ip),
        // Число без точек или в шестнадцатеричном виде (`https://2130706433/`,
        // `https://0x7f000001/`) HTTP-клиент тоже понимает как адрес, а не имя.
        Err(_) => {
            host.starts_with(|c: char| c.is_ascii_digit())
                && host
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() || c == '.' || c == 'x')
        }
    }
}

/// Хост адреса `scheme://[user@]host[:port]/…`; `None` — не такой адрес.
fn remote_host(url: &str) -> Option<String> {
    let (_, rest) = url.trim().split_once("://")?;
    let authority = rest.split('/').next()?;
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = if host_port.starts_with('[') {
        host_port.split_once(']').map(|(h, _)| format!("{h}]"))?
    } else {
        host_port.split(':').next()?.to_owned()
    };
    (!host.is_empty()).then_some(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_addresses_are_recognised() {
        for url in [
            "https://localhost/me/notes.git",
            "https://LOCALHOST./x",
            "https://git.localhost/x",
            "https://127.0.0.1/x",
            "https://user:token@10.1.2.3:8443/x",
            "https://192.168.0.5/x",
            "https://172.16.0.1/x",
            "https://169.254.169.254/latest",
            "https://100.100.1.1/x",
            "https://0.0.0.0/x",
            "https://[::1]/x",
            "https://[fd00::1]/x",
            "https://[fe80::1]/x",
            "https://[::ffff:127.0.0.1]/x",
            "https://2130706433/x",
            "https://0x7f000001/x",
        ] {
            assert!(internal_remote(url), "{url}");
        }
        for url in [
            "https://github.com/me/notes.git",
            "https://user:token@git.example.org:8443/me/notes.git",
            "https://8.8.8.8/x",
            "https://[2001:db8::1]/x",
        ] {
            assert!(!internal_remote(url), "{url}");
        }
    }
}
