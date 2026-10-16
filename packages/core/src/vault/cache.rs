use crate::properties::PropertyEntry;
#[allow(unused_imports)] // FileStamp используется в native_cache (native-only), не в web_cache
use crate::vault::index::{FileStamp, Note};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct LinkRecord {
    pub source_path: PathBuf,
    pub target_path: PathBuf,
}

/// Один переезд заметки в кэше. `links` — новые исходящие ссылки, разобранные
/// из текста под новым путём; `None` — ссылки не пересчитываются (доска: её
/// содержимое не Typst).
#[derive(Debug, Clone)]
pub struct CacheMove {
    pub from: PathBuf,
    pub to: PathBuf,
    pub links: Option<Vec<PathBuf>>,
    /// Вставленные файлы `#image` под новым путём (фича 025): относительные
    /// пути разрешаются от папки заметки, поэтому после переезда пересчитываются.
    /// `None` — не пересчитываются.
    pub embeds: Option<Vec<PathBuf>>,
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

    /// Переезд заметок при переносе (фича 023, T007): каждая запись меняет
    /// путь, а исходящие ссылки переносимой заметки пересчитываются.
    ///
    /// Пересчёт обязателен: кэш хранит уже **разрешённые** цели, а
    /// относительная ссылка `[[имя]]` после переезда в другую папку находит
    /// другую заметку. Входящие ссылки не трогаются — их текст не менялся, и
    /// ссылки на старый путь теперь честно битые (FR-016, FR-019).
    ///
    /// Одним вызовом, потому что на native это одна транзакция: перенос папки
    /// в тысячу заметок иначе платил бы за запись на диск тысячи раз.
    async fn relocate(&mut self, moves: &[CacheMove]);

    async fn set_links(&mut self, source: &Path, targets: &[PathBuf]);
    async fn backlinks(&self, target: &Path) -> Vec<PathBuf>;
    /// Исходящие ссылки заметки. Нужны, чтобы отличить «битую» ссылку от
    /// просто отсутствующей: сама таблица `links` не знает, существует ли
    /// цель, — это решает индекс заметок (data-model.md «Ссылка», `is_broken`).
    async fn links_from(&self, source: &Path) -> Vec<PathBuf>;

    /// Все исходящие ссылки vault одним вопросом: источник -> цели (фича 025).
    async fn all_links(&self) -> HashMap<PathBuf, Vec<PathBuf>>;

    /// Заменяет список файлов, вставленных в заметку вызовами `#image` (фича 025).
    async fn set_embeds(&mut self, source: &Path, targets: &[PathBuf]);

    /// Все вставки файлов vault: заметка -> вставленные пути (фича 025).
    async fn all_embeds(&self) -> HashMap<PathBuf, Vec<PathBuf>>;

    // ── Свойства заметок (фича 020) ───────────────────────────────────────

    /// Заменяет свойства заметки в индексе. Пустой список — свойств нет.
    async fn set_properties(&mut self, path: &Path, entries: &[PropertyEntry]);

    /// Свойства всех заметок. Копия: вызывающий отбирает по ней, не держа кэш.
    async fn all_properties(&self) -> HashMap<PathBuf, Vec<PropertyEntry>>;

    /// Построены ли свойства по текущему формату
    /// ([`crate::properties::PROPERTIES_FORMAT_VERSION`]).
    async fn properties_current(&self) -> bool;

    /// Отмечает, что свойства всех заметок пересчитаны по текущему формату.
    async fn mark_properties_current(&mut self);
}

/// Всё, что индекс выводит из текста заметки: ссылки (фича 013) и свойства
/// (фича 020). Считается за один проход по уже прочитанному тексту.
#[derive(Debug, Clone, Default)]
pub struct Derived {
    pub links: Vec<PathBuf>,
    pub properties: Vec<PropertyEntry>,
    /// Вставленные файлы `#image("…")` (фича 025).
    pub embeds: Vec<PathBuf>,
}

impl Derived {
    /// Выводит сведения из текста заметки по пути `source`. Свойства есть
    /// только у текстовых заметок: содержимое доски — не Typst.
    pub fn of(source: &Path, content: &str) -> Self {
        let is_note =
            source.extension().and_then(|ext| ext.to_str()) == Some(super::NOTE_EXTENSION);
        Self {
            links: crate::graph::links_in(source, content),
            embeds: if is_note {
                crate::graph::embeds_in(source, content)
            } else {
                Vec::new()
            },
            properties: if is_note {
                crate::properties::entries_of(content)
            } else {
                Vec::new()
            },
        }
    }
}

/// Записывает в кэш всё, выведенное из текста заметки.
pub async fn record_derived<C: IndexCache + ?Sized>(
    cache: &mut C,
    source: &Path,
    derived: &Derived,
) {
    cache.set_links(source, &derived.links).await;
    cache.set_properties(source, &derived.properties).await;
    cache.set_embeds(source, &derived.embeds).await;
}

/// Пересчитывает исходящие ссылки и свойства заметки по её содержимому и
/// записывает их в кэш (T085, FR-031; фича 020). Единственная точка, где кэш
/// узнаёт о ссылках: разбор содержимого принадлежит модулю `graph`,
/// vault-manager — только потребитель и хранилище (research.md §8).
pub async fn record_links<C: IndexCache + ?Sized>(cache: &mut C, source: &Path, content: &str) {
    record_derived(cache, source, &Derived::of(source, content)).await;
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
        /// Свойства заметок в памяти (фича 020): отбор по ним идёт без запросов
        /// к SQLite, на 10 000 заметок это разница между миллисекундами и
        /// десятками миллисекунд (SC-001). Таблица `properties` — их
        /// персистентная копия, из которой память наполняется при открытии.
        properties: HashMap<PathBuf, Vec<PropertyEntry>>,
        /// Формат свойств в таблице совпадает с текущим.
        properties_current: bool,
        /// Вставленные файлы `#image` в памяти (фича 025): граф берёт их одним
        /// вопросом, а запись заметки без вставок обходится без запроса к SQLite.
        embeds: HashMap<PathBuf, Vec<PathBuf>>,
    }

    const META_PROPERTIES_VERSION: &str = "properties_version";

    impl SqliteCache {
        pub async fn open(db_path: &Path) -> Result<Self, sqlx::Error> {
            let url = format!("sqlite://{}?mode=rwc", db_path.display());
            let pool = SqlitePoolOptions::new().connect(&url).await?;
            Self::prepare(pool).await
        }

        pub async fn in_memory() -> Result<Self, sqlx::Error> {
            let pool = SqlitePoolOptions::new().connect("sqlite::memory:").await?;
            Self::prepare(pool).await
        }

        /// Схема, затем свойства в память. Таблица с другим номером формата
        /// очищается: производное состояние проще построить заново, чем
        /// мигрировать (research.md §5).
        async fn prepare(pool: SqlitePool) -> Result<Self, sqlx::Error> {
            for migration in [
                include_str!("../../migrations/0001_vault_cache.sql"),
                include_str!("../../migrations/0002_properties.sql"),
                include_str!("../../migrations/0003_embeds.sql"),
            ] {
                for stmt in migration.split(';') {
                    let stmt = stmt.trim();
                    if !stmt.is_empty() {
                        sqlx::query(stmt).execute(&pool).await?;
                    }
                }
            }
            let version: Option<String> =
                sqlx::query("SELECT value FROM cache_meta WHERE name = ?")
                    .bind(META_PROPERTIES_VERSION)
                    .fetch_optional(&pool)
                    .await?
                    .and_then(|row| row.try_get("value").ok());
            let current = version.as_deref()
                == Some(
                    crate::properties::PROPERTIES_FORMAT_VERSION
                        .to_string()
                        .as_str(),
                );
            let mut cache = Self {
                pool,
                properties: HashMap::new(),
                properties_current: current,
                embeds: HashMap::new(),
            };
            if current {
                cache.load_properties().await;
                cache.embeds = cache.all_pairs("embeds").await;
            } else {
                let _ = sqlx::query("DELETE FROM properties")
                    .execute(&cache.pool)
                    .await;
                let _ = sqlx::query("DELETE FROM embeds").execute(&cache.pool).await;
            }
            Ok(cache)
        }

        async fn load_properties(&mut self) {
            let rows = sqlx::query(
                "SELECT path, key, kind, value_text, value_num FROM properties ORDER BY path, ord",
            )
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();
            for row in rows {
                let (Ok(path), Ok(key), Ok(kind)) = (
                    row.try_get::<String, _>("path"),
                    row.try_get::<String, _>("key"),
                    row.try_get::<String, _>("kind"),
                ) else {
                    continue;
                };
                let text: Option<String> = row.try_get("value_text").ok().flatten();
                let num: Option<f64> = row.try_get("value_num").ok().flatten();
                if let Some(value) = decode_value(&kind, text, num) {
                    self.properties
                        .entry(PathBuf::from(path))
                        .or_default()
                        .push(PropertyEntry { key, value });
                }
            }
        }
    }

    impl SqliteCache {
        /// Все пары «источник -> цель» таблицы `links` или `embeds` одним запросом.
        async fn all_pairs(&self, table: &str) -> HashMap<PathBuf, Vec<PathBuf>> {
            // Имя таблицы — одна из двух констант выше, не ввод пользователя.
            let rows = sqlx::query(&format!(
                "SELECT source_path, target_path FROM {table} ORDER BY source_path, target_path"
            ))
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();
            let mut out: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
            for row in rows {
                let (Ok(source), Ok(target)) = (
                    row.try_get::<String, _>("source_path"),
                    row.try_get::<String, _>("target_path"),
                ) else {
                    continue;
                };
                out.entry(PathBuf::from(source))
                    .or_default()
                    .push(PathBuf::from(target));
            }
            out
        }
    }

    /// Значение свойства → колонки таблицы.
    fn encode_value(
        value: &crate::properties::PropertyValue,
    ) -> (&'static str, Option<String>, Option<f64>) {
        use crate::properties::PropertyValue as V;
        match value {
            V::Text { value } => ("text", Some(value.clone()), None),
            V::Number { value } => ("number", None, Some(*value)),
            V::Date { year, month, day } => {
                ("date", Some(format!("{year:04}-{month:02}-{day:02}")), None)
            }
            V::Flag { value } => ("flag", None, Some(if *value { 1.0 } else { 0.0 })),
            V::List { values } => (
                "list",
                Some(serde_json::to_string(values).unwrap_or_default()),
                None,
            ),
            V::Unknown { source } => ("unknown", Some(source.clone()), None),
        }
    }

    fn decode_value(
        kind: &str,
        text: Option<String>,
        num: Option<f64>,
    ) -> Option<crate::properties::PropertyValue> {
        use crate::properties::PropertyValue as V;
        Some(match kind {
            "text" => V::Text { value: text? },
            "number" => V::Number { value: num? },
            "date" => {
                let text = text?;
                let mut parts = text.split('-');
                V::Date {
                    year: parts.next()?.parse().ok()?,
                    month: parts.next()?.parse().ok()?,
                    day: parts.next()?.parse().ok()?,
                }
            }
            "flag" => V::Flag { value: num? != 0.0 },
            "list" => V::List {
                values: serde_json::from_str(&text?).ok()?,
            },
            _ => return None,
        })
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
            self.properties.remove(path);
            let _ = sqlx::query("DELETE FROM properties WHERE path = ?")
                .bind(path.to_string_lossy().into_owned())
                .execute(&self.pool)
                .await;
            if self.embeds.remove(path).is_some() {
                let _ = sqlx::query("DELETE FROM embeds WHERE source_path = ?")
                    .bind(path.to_string_lossy().into_owned())
                    .execute(&self.pool)
                    .await;
            }
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
            // Свойства — часть текста заметки, а он при переименовании не
            // менялся: они переезжают вместе с путём.
            if let Some(entries) = self.properties.remove(from) {
                self.properties.insert(to.to_path_buf(), entries);
            }
            let _ = sqlx::query("UPDATE properties SET path = ? WHERE path = ?")
                .bind(to.to_string_lossy().into_owned())
                .bind(from.to_string_lossy().into_owned())
                .execute(&self.pool)
                .await;
            if let Some(targets) = self.embeds.remove(from) {
                self.embeds.insert(to.to_path_buf(), targets);
                let _ = sqlx::query("UPDATE embeds SET source_path = ? WHERE source_path = ?")
                    .bind(to.to_string_lossy().into_owned())
                    .bind(from.to_string_lossy().into_owned())
                    .execute(&self.pool)
                    .await;
            }
        }

        async fn relocate(&mut self, moves: &[CacheMove]) {
            // Одна транзакция на весь перенос: без неё каждое из тысяч
            // обновлений платило бы за синхронизацию с диском отдельно.
            let Ok(mut tx) = self.pool.begin().await else {
                for item in moves {
                    self.rename_note(&item.from, &item.to).await;
                    if let Some(links) = &item.links {
                        self.set_links(&item.to, links).await;
                    }
                    if let Some(embeds) = &item.embeds {
                        self.set_embeds(&item.to, embeds).await;
                    }
                }
                return;
            };
            for item in moves {
                let from = item.from.to_string_lossy().into_owned();
                let to = item.to.to_string_lossy().into_owned();
                let name = item
                    .to
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let _ = sqlx::query("UPDATE notes SET path = ?, name = ? WHERE path = ?")
                    .bind(&to)
                    .bind(&name)
                    .bind(&from)
                    .execute(&mut *tx)
                    .await;
                let _ = sqlx::query("UPDATE links SET source_path = ? WHERE source_path = ?")
                    .bind(&to)
                    .bind(&from)
                    .execute(&mut *tx)
                    .await;
                let _ = sqlx::query("UPDATE properties SET path = ? WHERE path = ?")
                    .bind(&to)
                    .bind(&from)
                    .execute(&mut *tx)
                    .await;
                let _ = sqlx::query("UPDATE embeds SET source_path = ? WHERE source_path = ?")
                    .bind(&to)
                    .bind(&from)
                    .execute(&mut *tx)
                    .await;
                if let Some(entries) = self.properties.remove(&item.from) {
                    self.properties.insert(item.to.clone(), entries);
                }
                if let Some(targets) = self.embeds.remove(&item.from) {
                    self.embeds.insert(item.to.clone(), targets);
                }
                if let Some(embeds) = &item.embeds {
                    let _ = sqlx::query("DELETE FROM embeds WHERE source_path = ?")
                        .bind(&to)
                        .execute(&mut *tx)
                        .await;
                    for target in embeds {
                        let _ = sqlx::query(
                            "INSERT OR IGNORE INTO embeds (source_path, target_path) VALUES (?, ?)",
                        )
                        .bind(&to)
                        .bind(target.to_string_lossy().into_owned())
                        .execute(&mut *tx)
                        .await;
                    }
                    if embeds.is_empty() {
                        self.embeds.remove(&item.to);
                    } else {
                        self.embeds.insert(item.to.clone(), embeds.clone());
                    }
                }
                if let Some(links) = &item.links {
                    let _ = sqlx::query("DELETE FROM links WHERE source_path = ?")
                        .bind(&to)
                        .execute(&mut *tx)
                        .await;
                    for target in links {
                        let _ = sqlx::query(
                            "INSERT OR IGNORE INTO links (source_path, target_path) VALUES (?, ?)",
                        )
                        .bind(&to)
                        .bind(target.to_string_lossy().into_owned())
                        .execute(&mut *tx)
                        .await;
                    }
                }
            }
            let _ = tx.commit().await;
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

        async fn all_links(&self) -> HashMap<PathBuf, Vec<PathBuf>> {
            self.all_pairs("links").await
        }

        async fn set_embeds(&mut self, source: &Path, targets: &[PathBuf]) {
            let source_str = source.to_string_lossy().into_owned();
            // Подавляющему большинству заметок вставлять нечего и стирать тоже:
            // запрос к SQLite нужен, только если вставки там были.
            if targets.is_empty() {
                if self.embeds.remove(source).is_some() {
                    let _ = sqlx::query("DELETE FROM embeds WHERE source_path = ?")
                        .bind(&source_str)
                        .execute(&self.pool)
                        .await;
                }
                return;
            }
            if self
                .embeds
                .get(source)
                .is_some_and(|known| known == targets)
            {
                return;
            }
            self.embeds.insert(source.to_path_buf(), targets.to_vec());
            let _ = sqlx::query("DELETE FROM embeds WHERE source_path = ?")
                .bind(&source_str)
                .execute(&self.pool)
                .await;
            for target in targets {
                let _ = sqlx::query(
                    "INSERT OR IGNORE INTO embeds (source_path, target_path) VALUES (?, ?)",
                )
                .bind(&source_str)
                .bind(target.to_string_lossy().into_owned())
                .execute(&self.pool)
                .await;
            }
        }

        async fn all_embeds(&self) -> HashMap<PathBuf, Vec<PathBuf>> {
            self.embeds.clone()
        }

        async fn set_properties(&mut self, path: &Path, entries: &[PropertyEntry]) {
            let path_text = path.to_string_lossy().into_owned();
            // Заметка без свойств — подавляющее большинство; для неё запись в
            // SQLite нужна, только если там что-то было.
            let had = self.properties.contains_key(path);
            if entries.is_empty() {
                self.properties.remove(path);
                if had {
                    let _ = sqlx::query("DELETE FROM properties WHERE path = ?")
                        .bind(&path_text)
                        .execute(&self.pool)
                        .await;
                }
                return;
            }
            if self
                .properties
                .get(path)
                .is_some_and(|known| known == entries)
            {
                return;
            }
            self.properties.insert(path.to_path_buf(), entries.to_vec());
            let _ = sqlx::query("DELETE FROM properties WHERE path = ?")
                .bind(&path_text)
                .execute(&self.pool)
                .await;
            for (ord, entry) in entries.iter().enumerate() {
                let (kind, text, num) = encode_value(&entry.value);
                let _ = sqlx::query(
                    "INSERT INTO properties (path, ord, key, kind, value_text, value_num) VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(&path_text)
                .bind(ord as i64)
                .bind(&entry.key)
                .bind(kind)
                .bind(text)
                .bind(num)
                .execute(&self.pool)
                .await;
            }
        }

        async fn all_properties(&self) -> HashMap<PathBuf, Vec<PropertyEntry>> {
            self.properties.clone()
        }

        async fn properties_current(&self) -> bool {
            self.properties_current
        }

        async fn mark_properties_current(&mut self) {
            self.properties_current = true;
            let _ = sqlx::query(
                "INSERT INTO cache_meta (name, value) VALUES (?, ?)
                 ON CONFLICT(name) DO UPDATE SET value = excluded.value",
            )
            .bind(META_PROPERTIES_VERSION)
            .bind(crate::properties::PROPERTIES_FORMAT_VERSION.to_string())
            .execute(&self.pool)
            .await;
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
        /// Вставленные файлы `#image` (фича 025).
        embeds: HashMap<PathBuf, Vec<PathBuf>>,
        /// Свойства заметок (фича 020). Как и ссылки, не переживают
        /// перезагрузку страницы и пересчитываются при открытии vault.
        properties: HashMap<PathBuf, Vec<PropertyEntry>>,
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
            self.properties.remove(path);
            self.embeds.remove(path);
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
            if let Some(targets) = self.embeds.remove(from) {
                self.embeds.insert(to.to_path_buf(), targets);
            }
            if let Some(entries) = self.properties.remove(from) {
                self.properties.insert(to.to_path_buf(), entries);
            }
        }

        async fn relocate(&mut self, moves: &[CacheMove]) {
            for item in moves {
                self.rename_note(&item.from, &item.to).await;
                if let Some(links) = &item.links {
                    self.links.insert(item.to.clone(), links.clone());
                }
                if let Some(embeds) = &item.embeds {
                    self.set_embeds(&item.to, embeds).await;
                }
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

        async fn all_links(&self) -> HashMap<PathBuf, Vec<PathBuf>> {
            self.links.clone()
        }

        async fn set_embeds(&mut self, source: &Path, targets: &[PathBuf]) {
            if targets.is_empty() {
                self.embeds.remove(source);
            } else {
                self.embeds.insert(source.to_path_buf(), targets.to_vec());
            }
        }

        async fn all_embeds(&self) -> HashMap<PathBuf, Vec<PathBuf>> {
            self.embeds.clone()
        }

        async fn set_properties(&mut self, path: &Path, entries: &[PropertyEntry]) {
            if entries.is_empty() {
                self.properties.remove(path);
            } else {
                self.properties.insert(path.to_path_buf(), entries.to_vec());
            }
        }

        async fn all_properties(&self) -> HashMap<PathBuf, Vec<PropertyEntry>> {
            self.properties.clone()
        }

        async fn properties_current(&self) -> bool {
            // Кэш пуст при каждом открытии и наполняется обходом по текущему
            // формату: устаревших данных в нём не бывает.
            true
        }

        async fn mark_properties_current(&mut self) {}
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod relocate_tests {
    use super::*;

    fn stamp() -> FileStamp {
        FileStamp::from_content(0, b"")
    }

    #[tokio::test]
    async fn relocate_moves_rows_and_re_resolves_outgoing_links_only() {
        let mut cache = SqliteCache::in_memory().await.unwrap();
        for path in ["A/x.typ", "A/y.typ", "other.typ"] {
            cache
                .upsert_note(&Note::new(PathBuf::from(path), stamp()))
                .await;
        }
        // x ссылается на соседа y и на заметку вне папки; other ссылается на x.
        cache
            .set_links(
                Path::new("A/x.typ"),
                &[PathBuf::from("A/y.typ"), PathBuf::from("A/z.typ")],
            )
            .await;
        cache
            .set_links(Path::new("other.typ"), &[PathBuf::from("A/x.typ")])
            .await;

        cache
            .relocate(&[
                CacheMove {
                    from: PathBuf::from("A/x.typ"),
                    to: PathBuf::from("B/A/x.typ"),
                    // После переезда `[[y]]` разрешается в B/A/y.typ.
                    links: Some(vec![PathBuf::from("B/A/y.typ")]),
                    embeds: None,
                },
                CacheMove {
                    from: PathBuf::from("A/y.typ"),
                    to: PathBuf::from("B/A/y.typ"),
                    links: None,
                    embeds: None,
                },
            ])
            .await;

        let mut paths: Vec<_> = cache
            .load_notes()
            .await
            .into_iter()
            .map(|n| n.path.to_string_lossy().into_owned())
            .collect();
        paths.sort();
        assert_eq!(paths, ["B/A/x.typ", "B/A/y.typ", "other.typ"]);
        assert_eq!(
            cache.links_from(Path::new("B/A/x.typ")).await,
            vec![PathBuf::from("B/A/y.typ")]
        );
        assert!(cache.links_from(Path::new("A/x.typ")).await.is_empty());
        // Входящая ссылка не переписана: теперь она честно битая.
        assert_eq!(
            cache.backlinks(Path::new("A/x.typ")).await,
            vec![PathBuf::from("other.typ")]
        );
        assert!(cache.backlinks(Path::new("B/A/x.typ")).await.is_empty());
    }
}
