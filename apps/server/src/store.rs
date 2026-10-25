//! Аккаунты, привязки провайдера входа и сессии — SQLite в папке данных
//! (data-model.md). Попытки входа здесь не хранятся: они в памяти ([`crate::auth`]).
//!
//! Токен сессии в базе не лежит — только его blake3 (конституция XI): утечка файла
//! базы не даёт войти чужой сессией.

use std::path::Path;
use std::str::FromStr;

use rand::RngCore;
use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS accounts (
    id TEXT PRIMARY KEY,
    login TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    password_hash TEXT,
    blocked INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS oidc_links (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    issuer TEXT NOT NULL,
    subject TEXT NOT NULL,
    PRIMARY KEY (issuer, subject)
);
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    token_hash TEXT NOT NULL UNIQUE,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    csrf TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_seen INTEGER NOT NULL,
    user_agent TEXT NOT NULL,
    ip TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS sessions_account ON sessions(account_id);
CREATE TABLE IF NOT EXISTS vault_sync_ids (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    id TEXT NOT NULL UNIQUE,
    PRIMARY KEY (account_id, name)
);
"#;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub login: String,
    pub display_name: String,
    #[serde(skip)]
    pub password_hash: Option<String>,
    pub blocked: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    #[serde(skip)]
    pub account_id: String,
    #[serde(skip)]
    pub csrf: String,
    pub created_at: i64,
    pub last_seen: i64,
    pub user_agent: String,
    pub ip: String,
}

/// Новая сессия: токен уходит в cookie и больше нигде не хранится.
pub struct NewSession {
    pub token: String,
    pub session: Session,
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

/// 32 случайных байта в hex.
pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn token_hash(token: &str) -> String {
    blake3::hash(token.as_bytes()).to_hex().to_string()
}

/// Логин — то, что задал администратор: латиница, цифры, `.`, `_`, `-`.
pub fn valid_login(login: &str) -> bool {
    (1..=64).contains(&login.len())
        && login
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
}

#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
}

fn account(row: &sqlx::sqlite::SqliteRow) -> Account {
    Account {
        id: row.get("id"),
        login: row.get("login"),
        display_name: row.get("display_name"),
        password_hash: row.get("password_hash"),
        blocked: row.get::<i64, _>("blocked") != 0,
        created_at: row.get("created_at"),
    }
}

fn session(row: &sqlx::sqlite::SqliteRow) -> Session {
    Session {
        id: row.get("id"),
        account_id: row.get("account_id"),
        csrf: row.get("csrf"),
        created_at: row.get("created_at"),
        last_seen: row.get("last_seen"),
        user_agent: row.get("user_agent"),
        ip: row.get("ip"),
    }
}

impl Store {
    pub async fn open(path: &Path) -> anyhow::Result<Store> {
        if let Some(parent) = path.parent() {
            crate::private_dir(parent)?;
        }
        let options = SqliteConnectOptions::from_str("sqlite:")?
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true);
        let store = Self::with(options, 4).await?;
        // База — хеши паролей и токены CSRF: только для процесса сервера.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(store)
    }

    /// База в памяти — для тестов.
    pub async fn memory() -> anyhow::Result<Store> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        Self::with(options, 1).await
    }

    async fn with(options: SqliteConnectOptions, connections: u32) -> anyhow::Result<Store> {
        let pool = SqlitePoolOptions::new()
            .max_connections(connections)
            // База в памяти живёт, пока жив хотя бы один connection.
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(options)
            .await?;
        sqlx::raw_sql(SCHEMA).execute(&pool).await?;
        Ok(Store { pool })
    }

    // ── Аккаунты ──────────────────────────────────────────────────────────

    pub async fn add_account(
        &self,
        login: &str,
        display_name: &str,
        password_hash: Option<&str>,
    ) -> anyhow::Result<Account> {
        if !valid_login(login) {
            anyhow::bail!("логин — строчная латиница, цифры, «.», «_», «-», до 64 знаков");
        }
        if self.account_by_login(login).await?.is_some() {
            anyhow::bail!("аккаунт «{login}» уже есть");
        }
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO accounts (id, login, display_name, password_hash, blocked, created_at)
             VALUES (?, ?, ?, ?, 0, ?)",
        )
        .bind(&id)
        .bind(login)
        .bind(display_name)
        .bind(password_hash)
        .bind(now())
        .execute(&self.pool)
        .await?;
        self.account_by_id(&id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("аккаунт пропал сразу после записи"))
    }

    pub async fn account_by_login(&self, login: &str) -> anyhow::Result<Option<Account>> {
        let row = sqlx::query("SELECT * FROM accounts WHERE login = ?")
            .bind(login)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().map(account))
    }

    pub async fn account_by_id(&self, id: &str) -> anyhow::Result<Option<Account>> {
        let row = sqlx::query("SELECT * FROM accounts WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().map(account))
    }

    pub async fn accounts(&self) -> anyhow::Result<Vec<Account>> {
        let rows = sqlx::query("SELECT * FROM accounts ORDER BY login")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(account).collect())
    }

    pub async fn set_password(&self, id: &str, hash: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE accounts SET password_hash = ? WHERE id = ?")
            .bind(hash)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Блокировка завершает все сессии аккаунта (US6 AS2).
    pub async fn set_blocked(&self, id: &str, blocked: bool) -> anyhow::Result<()> {
        sqlx::query("UPDATE accounts SET blocked = ? WHERE id = ?")
            .bind(i64::from(blocked))
            .bind(id)
            .execute(&self.pool)
            .await?;
        if blocked {
            self.end_sessions(id, None).await?;
        }
        Ok(())
    }

    /// Удаляет аккаунт с его сессиями и привязками; папки vault не трогает.
    pub async fn delete_account(&self, id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM accounts WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Привязки провайдера входа ─────────────────────────────────────────

    pub async fn link_oidc(&self, id: &str, issuer: &str, subject: &str) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO oidc_links (account_id, issuer, subject) VALUES (?, ?, ?)
             ON CONFLICT (issuer, subject) DO UPDATE SET account_id = excluded.account_id",
        )
        .bind(id)
        .bind(issuer)
        .bind(subject)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn account_by_oidc(
        &self,
        issuer: &str,
        subject: &str,
    ) -> anyhow::Result<Option<Account>> {
        let row = sqlx::query(
            "SELECT accounts.* FROM accounts JOIN oidc_links ON oidc_links.account_id = accounts.id
             WHERE oidc_links.issuer = ? AND oidc_links.subject = ?",
        )
        .bind(issuer)
        .bind(subject)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.as_ref().map(account))
    }

    pub async fn oidc_links(&self, id: &str) -> anyhow::Result<Vec<(String, String)>> {
        let rows = sqlx::query("SELECT issuer, subject FROM oidc_links WHERE account_id = ?")
            .bind(id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .map(|row| (row.get("issuer"), row.get("subject")))
            .collect())
    }

    // ── Сессии ────────────────────────────────────────────────────────────

    pub async fn create_session(
        &self,
        account_id: &str,
        user_agent: &str,
        ip: &str,
    ) -> anyhow::Result<NewSession> {
        let token = random_token();
        let now = now();
        let session = Session {
            id: uuid::Uuid::new_v4().to_string(),
            account_id: account_id.to_owned(),
            csrf: random_token(),
            created_at: now,
            last_seen: now,
            user_agent: user_agent.chars().take(200).collect(),
            ip: ip.to_owned(),
        };
        sqlx::query(
            "INSERT INTO sessions (id, token_hash, account_id, csrf, created_at, last_seen, user_agent, ip)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&session.id)
        .bind(token_hash(&token))
        .bind(&session.account_id)
        .bind(&session.csrf)
        .bind(session.created_at)
        .bind(session.last_seen)
        .bind(&session.user_agent)
        .bind(&session.ip)
        .execute(&self.pool)
        .await?;
        Ok(NewSession { token, session })
    }

    /// Сессия по токену из cookie, если она не истекла и аккаунт не заблокирован.
    /// Истёкшую удаляет.
    pub async fn session_by_token(
        &self,
        token: &str,
        idle_secs: i64,
    ) -> anyhow::Result<Option<(Session, Account)>> {
        let hash = token_hash(token);
        let Some(row) = sqlx::query("SELECT * FROM sessions WHERE token_hash = ?")
            .bind(&hash)
            .fetch_optional(&self.pool)
            .await?
        else {
            return Ok(None);
        };
        let found = session(&row);
        if now() - found.last_seen > idle_secs {
            sqlx::query("DELETE FROM sessions WHERE id = ?")
                .bind(&found.id)
                .execute(&self.pool)
                .await?;
            return Ok(None);
        }
        let Some(account) = self.account_by_id(&found.account_id).await? else {
            return Ok(None);
        };
        if account.blocked {
            return Ok(None);
        }
        Ok(Some((found, account)))
    }

    pub async fn touch_session(&self, id: &str, at: i64) -> anyhow::Result<()> {
        sqlx::query("UPDATE sessions SET last_seen = ? WHERE id = ?")
            .bind(at)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn sessions(&self, account_id: &str) -> anyhow::Result<Vec<Session>> {
        let rows =
            sqlx::query("SELECT * FROM sessions WHERE account_id = ? ORDER BY last_seen DESC")
                .bind(account_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.iter().map(session).collect())
    }

    /// Завершает сессию аккаунта; `false` — такой у аккаунта нет.
    pub async fn end_session(&self, account_id: &str, id: &str) -> anyhow::Result<bool> {
        let done = sqlx::query("DELETE FROM sessions WHERE id = ? AND account_id = ?")
            .bind(id)
            .bind(account_id)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Завершает все сессии аккаунта, кроме `keep`.
    pub async fn end_sessions(&self, account_id: &str, keep: Option<&str>) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM sessions WHERE account_id = ? AND id != ?")
            .bind(account_id)
            .bind(keep.unwrap_or(""))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Удаляет истёкшие сессии.
    pub async fn purge_sessions(&self, idle_secs: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM sessions WHERE last_seen < ?")
            .bind(now() - idle_secs)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Id синхронизации vault (фича 038, research.md §4) ─────────────────
    //
    // Ключ секретов и зеркала vault. Его назначает сервер, а не клиент, и
    // переименование vault его сохраняет.

    /// Id синхронизации vault; первое обращение его создаёт.
    pub async fn vault_sync_id(&self, account_id: &str, name: &str) -> anyhow::Result<String> {
        sqlx::query("INSERT OR IGNORE INTO vault_sync_ids (account_id, name, id) VALUES (?, ?, ?)")
            .bind(account_id)
            .bind(name)
            .bind(&random_token()[..32])
            .execute(&self.pool)
            .await?;
        Ok(
            sqlx::query("SELECT id FROM vault_sync_ids WHERE account_id = ? AND name = ?")
                .bind(account_id)
                .bind(name)
                .fetch_one(&self.pool)
                .await?
                .get("id"),
        )
    }

    /// Переносит id синхронизации на новое имя vault.
    pub async fn rename_vault_sync_id(
        &self,
        account_id: &str,
        from: &str,
        to: &str,
    ) -> anyhow::Result<()> {
        sqlx::query("UPDATE vault_sync_ids SET name = ? WHERE account_id = ? AND name = ?")
            .bind(to)
            .bind(account_id)
            .bind(from)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Забывает id синхронизации удалённого vault; возвращает его, если был.
    pub async fn forget_vault_sync_id(
        &self,
        account_id: &str,
        name: &str,
    ) -> anyhow::Result<Option<String>> {
        Ok(
            sqlx::query(
                "DELETE FROM vault_sync_ids WHERE account_id = ? AND name = ? RETURNING id",
            )
            .bind(account_id)
            .bind(name)
            .fetch_optional(&self.pool)
            .await?
            .map(|row| row.get("id")),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn vault_sync_ids_are_stable_and_per_account() {
        let store = Store::memory().await.unwrap();
        let anna = store.add_account("anna", "Анна", None).await.unwrap();
        let boris = store.add_account("boris", "Борис", None).await.unwrap();
        let first = store.vault_sync_id(&anna.id, "Заметки").await.unwrap();
        assert_eq!(first.len(), 32);
        assert_eq!(
            store.vault_sync_id(&anna.id, "Заметки").await.unwrap(),
            first
        );
        assert_ne!(
            store.vault_sync_id(&boris.id, "Заметки").await.unwrap(),
            first
        );
        store
            .rename_vault_sync_id(&anna.id, "Заметки", "Архив")
            .await
            .unwrap();
        assert_eq!(store.vault_sync_id(&anna.id, "Архив").await.unwrap(), first);
        assert_eq!(
            store.forget_vault_sync_id(&anna.id, "Архив").await.unwrap(),
            Some(first.clone())
        );
        assert_ne!(store.vault_sync_id(&anna.id, "Архив").await.unwrap(), first);
        store.delete_account(&anna.id).await.unwrap();
        assert_eq!(
            store.forget_vault_sync_id(&anna.id, "Архив").await.unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn accounts_and_sessions() {
        let store = Store::memory().await.unwrap();
        let anna = store.add_account("anna", "Анна", Some("h")).await.unwrap();
        assert!(store.add_account("anna", "Ещё", None).await.is_err());
        assert!(store.add_account("Anna/..", "", None).await.is_err());

        let new = store
            .create_session(&anna.id, "test", "127.0.0.1")
            .await
            .unwrap();
        let (found, account) = store
            .session_by_token(&new.token, 3600)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.id, new.session.id);
        assert_eq!(account.login, "anna");
        assert!(store
            .session_by_token("чужой", 3600)
            .await
            .unwrap()
            .is_none());

        // Блокировка завершает сессии.
        store.set_blocked(&anna.id, true).await.unwrap();
        assert!(store
            .session_by_token(&new.token, 3600)
            .await
            .unwrap()
            .is_none());
        store.set_blocked(&anna.id, false).await.unwrap();

        let a = store.create_session(&anna.id, "a", "").await.unwrap();
        let b = store.create_session(&anna.id, "b", "").await.unwrap();
        store
            .end_sessions(&anna.id, Some(&a.session.id))
            .await
            .unwrap();
        assert!(store
            .session_by_token(&a.token, 3600)
            .await
            .unwrap()
            .is_some());
        assert!(store
            .session_by_token(&b.token, 3600)
            .await
            .unwrap()
            .is_none());

        // Истёкшая по простою сессия не находится.
        store
            .touch_session(&a.session.id, now() - 7200)
            .await
            .unwrap();
        assert!(store
            .session_by_token(&a.token, 3600)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn oidc_links_map_to_accounts() {
        let store = Store::memory().await.unwrap();
        let anna = store.add_account("anna", "Анна", None).await.unwrap();
        store
            .link_oidc(&anna.id, "https://id", "sub-1")
            .await
            .unwrap();
        let found = store.account_by_oidc("https://id", "sub-1").await.unwrap();
        assert_eq!(found.unwrap().id, anna.id);
        assert!(store
            .account_by_oidc("https://other", "sub-1")
            .await
            .unwrap()
            .is_none());

        store.delete_account(&anna.id).await.unwrap();
        assert!(store
            .account_by_oidc("https://id", "sub-1")
            .await
            .unwrap()
            .is_none());
    }
}
