#[allow(unused_imports)] // FileStamp используется в native_cache (native-only), не в web_cache
use crate::vault::index::{FileStamp, Note};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct LinkRecord {
    pub source_path: PathBuf,
    pub target_path: PathBuf,
}

/// Персистентный кэш — производное, полностью восстановимое состояние
/// (research.md §6): при рассинхронизации или повреждении система откатывается на
/// полный обход (FR-002), пользовательские данные не теряются, потому что кэш
/// никогда не хранит содержимое заметок — только метаданные и обратные ссылки.
#[allow(async_fn_in_trait)]
pub trait IndexCache {
    async fn load_notes(&self) -> Vec<Note>;
    async fn upsert_note(&mut self, note: &Note);
    async fn remove_note(&mut self, path: &Path);
    async fn rename_note(&mut self, from: &Path, to: &Path);

    async fn set_links(&mut self, source: &Path, targets: &[PathBuf]);
    async fn backlinks(&self, target: &Path) -> Vec<PathBuf>;
    /// Исходящие ссылки заметки. Нужны, чтобы отличить «битую» ссылку от
    /// просто отсутствующей: сама таблица `links` не знает, существует ли
    /// цель, — это решает индекс заметок (data-model.md «Ссылка», `is_broken`).
    async fn links_from(&self, source: &Path) -> Vec<PathBuf>;
}

/// Пересчитывает исходящие ссылки заметки по её содержимому и записывает их в
/// таблицу `links` (T085, FR-031). Единственная точка, где кэш узнаёт о
/// ссылках: разбор содержимого принадлежит модулю `graph`, vault-manager —
/// только потребитель и хранилище (research.md §8).
pub async fn record_links<C: IndexCache + ?Sized>(cache: &mut C, source: &Path, content: &str) {
    cache
        .set_links(source, &crate::graph::links_in(source, content))
        .await;
}

/// Заметки, чьи ссылки становятся битыми или, наоборот, чинятся при появлении
/// или исчезновении `target` (FR-030). Отдельный помощник, чтобы обе
/// реализации backend спрашивали об этом одинаково.
pub async fn backlink_sources<C: IndexCache + ?Sized>(cache: &C, target: &Path) -> Vec<PathBuf> {
    cache.backlinks(target).await
}

#[cfg(not(target_arch = "wasm32"))]
pub use native_cache::SqliteCache;

#[cfg(not(target_arch = "wasm32"))]
mod native_cache {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::{Row, SqlitePool};

    /// Реализация `IndexCache` на `sqlx`/SQLite (T013) — единственная
    /// авторизованная native-реализация персистентного кэша (research.md §6).
    pub struct SqliteCache {
        pool: SqlitePool,
    }

    impl SqliteCache {
        pub async fn open(db_path: &Path) -> Result<Self, sqlx::Error> {
            let url = format!("sqlite://{}?mode=rwc", db_path.display());
            let pool = SqlitePoolOptions::new().connect(&url).await?;
            sqlx::query(include_str!("../../migrations/0001_vault_cache.sql"))
                .execute(&pool)
                .await
                .ok();
            for stmt in include_str!("../../migrations/0001_vault_cache.sql").split(';') {
                let stmt = stmt.trim();
                if !stmt.is_empty() {
                    sqlx::query(stmt).execute(&pool).await?;
                }
            }
            Ok(Self { pool })
        }

        pub async fn in_memory() -> Result<Self, sqlx::Error> {
            let pool = SqlitePoolOptions::new().connect("sqlite::memory:").await?;
            for stmt in include_str!("../../migrations/0001_vault_cache.sql").split(';') {
                let stmt = stmt.trim();
                if !stmt.is_empty() {
                    sqlx::query(stmt).execute(&pool).await?;
                }
            }
            Ok(Self { pool })
        }
    }

    impl IndexCache for SqliteCache {
        async fn load_notes(&self) -> Vec<Note> {
            let rows =
                sqlx::query("SELECT path, name, mtime_millis, size, content_hash FROM notes")
                    .fetch_all(&self.pool)
                    .await
                    .unwrap_or_default();
            rows.into_iter()
                .filter_map(|row| {
                    let path: String = row.try_get("path").ok()?;
                    let name: String = row.try_get("name").ok()?;
                    let mtime_millis: i64 = row.try_get("mtime_millis").ok()?;
                    let size: i64 = row.try_get("size").ok()?;
                    let hash_bytes: Vec<u8> = row.try_get("content_hash").ok()?;
                    let mut content_hash = [0u8; 32];
                    if hash_bytes.len() == 32 {
                        content_hash.copy_from_slice(&hash_bytes);
                    }
                    Some(Note::with_name(
                        PathBuf::from(path),
                        name,
                        FileStamp {
                            mtime_millis,
                            size: size as u64,
                            content_hash,
                        },
                    ))
                })
                .collect()
        }

        async fn upsert_note(&mut self, note: &Note) {
            let _ = sqlx::query(
                "INSERT INTO notes (path, name, mtime_millis, size, content_hash) VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT(path) DO UPDATE SET name = excluded.name, mtime_millis = excluded.mtime_millis,
                 size = excluded.size, content_hash = excluded.content_hash",
            )
            .bind(note.path.to_string_lossy().into_owned())
            .bind(&note.name)
            .bind(note.stamp.mtime_millis)
            .bind(note.stamp.size as i64)
            .bind(note.stamp.content_hash.to_vec())
            .execute(&self.pool)
            .await;
        }

        async fn remove_note(&mut self, path: &Path) {
            let _ = sqlx::query("DELETE FROM notes WHERE path = ?")
                .bind(path.to_string_lossy().into_owned())
                .execute(&self.pool)
                .await;
            // Исходящие ссылки исчезнувшей заметки исчезают вместе с ней;
            // входящие остаются и становятся битыми (FR-030).
            let _ = sqlx::query("DELETE FROM links WHERE source_path = ?")
                .bind(path.to_string_lossy().into_owned())
                .execute(&self.pool)
                .await;
        }

        async fn rename_note(&mut self, from: &Path, to: &Path) {
            let _ = sqlx::query("UPDATE notes SET path = ?, name = ? WHERE path = ?")
                .bind(to.to_string_lossy().into_owned())
                .bind(
                    to.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                )
                .bind(from.to_string_lossy().into_owned())
                .execute(&self.pool)
                .await;
            // FR-031: исходящие ссылки переезжают вместе с заметкой. Входящие
            // (`target_path = from`) намеренно НЕ переписываются: содержимое
            // ссылающихся заметок на диске не изменилось, значит их ссылки
            // теперь битые — ровно тот случай, который FR-030 требует дать
            // пользователю найти.
            let _ = sqlx::query("UPDATE links SET source_path = ? WHERE source_path = ?")
                .bind(to.to_string_lossy().into_owned())
                .bind(from.to_string_lossy().into_owned())
                .execute(&self.pool)
                .await;
        }

        async fn set_links(&mut self, source: &Path, targets: &[PathBuf]) {
            let source_str = source.to_string_lossy().into_owned();
            let _ = sqlx::query("DELETE FROM links WHERE source_path = ?")
                .bind(&source_str)
                .execute(&self.pool)
                .await;
            for target in targets {
                let _ = sqlx::query(
                    "INSERT OR IGNORE INTO links (source_path, target_path) VALUES (?, ?)",
                )
                .bind(&source_str)
                .bind(target.to_string_lossy().into_owned())
                .execute(&self.pool)
                .await;
            }
        }

        async fn backlinks(&self, target: &Path) -> Vec<PathBuf> {
            let rows = sqlx::query("SELECT source_path FROM links WHERE target_path = ?")
                .bind(target.to_string_lossy().into_owned())
                .fetch_all(&self.pool)
                .await
                .unwrap_or_default();
            rows.into_iter()
                .filter_map(|row| row.try_get::<String, _>("source_path").ok())
                .map(PathBuf::from)
                .collect()
        }

        async fn links_from(&self, source: &Path) -> Vec<PathBuf> {
            let rows = sqlx::query("SELECT target_path FROM links WHERE source_path = ?")
                .bind(source.to_string_lossy().into_owned())
                .fetch_all(&self.pool)
                .await
                .unwrap_or_default();
            rows.into_iter()
                .filter_map(|row| row.try_get::<String, _>("target_path").ok())
                .map(PathBuf::from)
                .collect()
        }
    }
}

/// Плейсхолдер персистентного кэша для web-таргета: полноценная реализация
/// поверх `wa-sqlite`/OPFS (research.md §6) требует JS-интеропа с внешней
/// JS-библиотекой (`wa-sqlite` — не Rust-крейт) и является отдельным шагом
/// доработки. Здесь — рабочая in-memory реализация того же трейта `IndexCache`,
/// достаточная для прохождения паритета сборки (принцип II): данные не
/// переживают перезагрузку страницы, SC-003 (быстрое повторное открытие) для
/// web пока не достигается этой реализацией.
#[cfg(target_arch = "wasm32")]
pub use web_cache::MemoryCache;

#[cfg(target_arch = "wasm32")]
mod web_cache {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    pub struct MemoryCache {
        notes: HashMap<PathBuf, Note>,
        links: HashMap<PathBuf, Vec<PathBuf>>,
    }

    impl MemoryCache {
        pub fn new() -> Self {
            Self::default()
        }
    }

    impl IndexCache for MemoryCache {
        async fn load_notes(&self) -> Vec<Note> {
            self.notes.values().cloned().collect()
        }

        async fn upsert_note(&mut self, note: &Note) {
            self.notes.insert(note.path.clone(), note.clone());
        }

        async fn remove_note(&mut self, path: &Path) {
            self.notes.remove(path);
            self.links.remove(path);
        }

        async fn rename_note(&mut self, from: &Path, to: &Path) {
            if let Some(mut note) = self.notes.remove(from) {
                note.path = to.to_path_buf();
                note.name = to
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.notes.insert(to.to_path_buf(), note);
            }
            // Паритет с `SqliteCache::rename_note`: исходящие ссылки переезжают,
            // входящие остаются битыми (FR-030, FR-031).
            if let Some(targets) = self.links.remove(from) {
                self.links.insert(to.to_path_buf(), targets);
            }
        }

        async fn set_links(&mut self, source: &Path, targets: &[PathBuf]) {
            self.links.insert(source.to_path_buf(), targets.to_vec());
        }

        async fn backlinks(&self, target: &Path) -> Vec<PathBuf> {
            self.links
                .iter()
                .filter(|(_, targets)| targets.iter().any(|t| t == target))
                .map(|(source, _)| source.clone())
                .collect()
        }

        async fn links_from(&self, source: &Path) -> Vec<PathBuf> {
            self.links.get(source).cloned().unwrap_or_default()
        }
    }
}
