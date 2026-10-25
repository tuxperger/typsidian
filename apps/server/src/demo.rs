//! Демо-сервер (фича 039): гости вместо аккаунтов.
//!
//! `serve --demo` открывает сервер только для гостей: «Попробовать без
//! регистрации» заводит временный аккаунт с одним vault — копией демо-vault — и
//! открывает ему обычную сессию 037. Вход паролем и через провайдера, сессии,
//! пароль и операции над vault на демо-сервере закрыты; удалённый репозиторий
//! закрыт возможностью ядра `remote = false`. Срок, квота и предел по адресу —
//! здесь же, как и уборка просроченных гостей (research.md §1–§6).

use std::net::IpAddr;
use std::path::{Path, PathBuf};

use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Serialize;

use crate::auth::{client_ip, open_session, Authed, Peer};
use crate::error::{ApiError, ApiResult};
use crate::store::now;
use crate::vaults::OpenVault;
use crate::Shared;

/// Имя vault гостя.
pub const DEMO_VAULT: &str = "Демо";

/// Встроенный демо-vault: путь в vault и содержимое (research.md §6).
const FILES: &[(&str, &[u8])] = &[
    (
        "Начните здесь.typ",
        include_bytes!("../demo-vault/Начните здесь.typ"),
    ),
    (
        "Исследование/Методика.typ",
        include_bytes!("../demo-vault/Исследование/Методика.typ"),
    ),
    (
        "Исследование/Выборка.typ",
        include_bytes!("../demo-vault/Исследование/Выборка.typ"),
    ),
    (
        "Обзор проектов.typ",
        include_bytes!("../demo-vault/Обзор проектов.typ"),
    ),
    (
        "Задачи недели.typ",
        include_bytes!("../demo-vault/Задачи недели.typ"),
    ),
    ("Доклад.typ", include_bytes!("../demo-vault/Доклад.typ")),
    (
        "источники.bib",
        include_bytes!("../demo-vault/источники.bib"),
    ),
    (
        "Схема опроса.excalidraw",
        include_bytes!("../demo-vault/Схема опроса.excalidraw"),
    ),
];

/// Глубже образец не копируется: папку задаёт администратор, но цикл в ней не
/// должен ронять старт гостя.
const MAX_SAMPLE_DEPTH: usize = 16;

/// Сколько «весит» каждая запись папки в квоте: иначе миллион пустых заметок
/// весил бы ноль, а исчерпывал бы inode диска (ревью безопасности 039).
const ENTRY_WEIGHT: u64 = 4096;

/// Проверка при запуске: своя папка-образец администратора существует, не
/// пуста и помещается в квоту гостя — иначе демо не включается, причина
/// названа. Тут же — секрет ключей адресов.
pub fn check(state: &Shared) -> anyhow::Result<()> {
    if let Some(dir) = &state.config.demo.vault {
        let mut entries = std::fs::read_dir(dir)
            .map_err(|error| anyhow::anyhow!("демо-vault {}: {error}", dir.display()))?;
        if entries.next().is_none() {
            anyhow::bail!("демо-vault {} пуст", dir.display());
        }
        let size = usage_of(&[dir.clone()]);
        if size >= state.config.demo.quota_bytes() {
            anyhow::bail!(
                "демо-vault {} больше квоты гостя ({} МБ)",
                dir.display(),
                state.config.demo.quota_mb
            );
        }
    }
    secret(state)?;
    Ok(())
}

/// Секрет ключей адресов в папке данных: по ключу адрес не восстановить
/// перебором всего IPv4 (ревью безопасности 039).
fn secret(state: &Shared) -> anyhow::Result<String> {
    if let Some(found) = state.demo_secret.get() {
        return Ok(found.clone());
    }
    let path = state.config.data.join("demo-secret");
    let value = match std::fs::read_to_string(&path) {
        Ok(text) if text.trim().len() == 64 => text.trim().to_owned(),
        _ => {
            crate::private_dir(&state.config.data)?;
            let value = crate::store::random_token();
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            use std::io::Write;
            options.open(&path)?.write_all(value.as_bytes())?;
            value
        }
    };
    Ok(state.demo_secret.get_or_init(|| value).clone())
}

/// Кладёт копию демо-vault в пустую папку `target`.
fn seed(sample: Option<&Path>, target: &Path) -> anyhow::Result<()> {
    match sample {
        Some(source) => copy_tree(source, target, 0)?,
        None => {
            for (path, bytes) in FILES {
                let file = target.join(path);
                if let Some(parent) = file.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(file, bytes)?;
            }
        }
    }
    // Библиотеки выборок и встраивания — те же, что кладёт приложение.
    let lib = target.join(".typsidian/lib");
    std::fs::create_dir_all(&lib)?;
    for (path, text) in [
        (
            typsidian_core::properties::select::LIBRARY_PATH,
            typsidian_core::properties::select::LIBRARY_TEXT,
        ),
        (
            typsidian_core::embeds::LIBRARY_PATH,
            typsidian_core::embeds::LIBRARY_TEXT,
        ),
    ] {
        std::fs::write(target.join(path), text)?;
    }
    Ok(())
}

/// Копия папки администратора: обычные файлы и папки, без `.git`, ссылок и
/// спецфайлов (`file_type` не идёт по ссылкам).
fn copy_tree(source: &Path, target: &Path, depth: usize) -> anyhow::Result<()> {
    if depth > MAX_SAMPLE_DEPTH {
        anyhow::bail!("демо-vault глубже {MAX_SAMPLE_DEPTH} папок");
    }
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        if typsidian_core::vault::names_git_dir(&name.to_string_lossy()) {
            continue;
        }
        let kind = entry.file_type()?;
        let to = target.join(&name);
        if kind.is_dir() {
            std::fs::create_dir_all(&to)?;
            copy_tree(&entry.path(), &to, depth + 1)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

/// Объём папок со всем содержимым: байты файлов плюс [`ENTRY_WEIGHT`] на
/// каждую запись. Ссылки не обходятся.
fn usage_of(dirs: &[PathBuf]) -> u64 {
    let mut total = 0u64;
    let mut stack: Vec<PathBuf> = dirs.to_vec();
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            total = total.saturating_add(ENTRY_WEIGHT);
            if meta.is_dir() {
                stack.push(entry.path());
            } else if meta.is_file() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    total
}

/// Всё, что гость занимает на диске: vault, корзина удалённых заметок и данные
/// синхронизации. Корзина — в квоте: иначе «вставить и удалить» забивало бы диск
/// без предела (ревью безопасности 039, К1). Обход — в пуле блокирующих потоков.
pub async fn account_usage(state: &Shared, account_id: &str) -> u64 {
    let dirs = vec![
        crate::vaults::account_dir(state, account_id),
        state.config.trash_dir().join("notes").join(account_id),
        crate::sync::account_sync_dir(state, account_id),
    ];
    tokio::task::spawn_blocking(move || usage_of(&dirs))
        .await
        .unwrap_or(u64::MAX)
}

/// Ключ адреса для предела стартов: IPv4 целиком, IPv6 — по сети /64 (адресов в
/// ней больше, чем гостей у сервера), с секретом сервера. Строка, которая не
/// разбирается как адрес, — один общий ключ: подделанный заголовок не даёт
/// нового адреса на каждый запрос.
fn ip_key(secret: &str, ip: &str) -> String {
    let bucket = match ip.trim().parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => v4.to_string(),
        Ok(IpAddr::V6(v6)) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => {
                let s = v6.segments();
                format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
            }
        },
        Err(_) => "unknown".to_owned(),
    };
    blake3::keyed_hash(
        blake3::hash(secret.as_bytes()).as_bytes(),
        bucket.as_bytes(),
    )
    .to_hex()
    .to_string()
}

// ── Обработчики ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Started {
    #[serde(flatten)]
    me: crate::auth::Me,
    vault: &'static str,
}

/// Тело `POST /api/demo/start` — пустой объект JSON. Требование JSON закрывает
/// старт формой с чужого сайта (CSRF входа): такая форма без предварительного
/// запроса CORS JSON не отправит.
#[derive(serde::Deserialize)]
pub struct StartRequest {}

/// `POST /api/demo/start`: гость, его vault и сессия.
pub async fn start(
    State(state): State<Shared>,
    Peer(peer): Peer,
    headers: HeaderMap,
    Json(_): Json<StartRequest>,
) -> ApiResult<Response> {
    if !state.config.demo_mode {
        return Err(ApiError::NotFound);
    }
    let ip = client_ip(&state, &headers, peer);
    let key = ip_key(&secret(&state)?, &ip);
    let hour = 60 * 60;
    let at = now();
    let per_ip = i64::from(state.config.demo.per_ip_per_hour.max(1));
    // Живых гостей меньше, чем открытых vault на сервер: гостям всегда хватает
    // места, и гости не вытесняют друг друга (ревью безопасности 039, В2).
    let max_guests =
        i64::from(state.config.demo.max_guests.max(1)).min(crate::vaults::MAX_OPEN as i64 - 16);
    // Замок на проверку и запись: параллельные старты иначе прошли бы пределы
    // вместе.
    let guard = state.demo_starts.lock().await;
    let (count, first) = state.store.starts_from(&key, at - hour).await?;
    if count >= per_ip {
        let retry_after_s = (first.unwrap_or(at) + hour - at).max(1) as u64;
        tracing::warn!("отказ: предел демо с одного адреса");
        return Err(ApiError::TooManyDemos { retry_after_s });
    }
    if state.store.live_guests(at).await? >= max_guests {
        tracing::warn!("отказ: демо-сервер заполнен");
        return Err(ApiError::DemoFull);
    }
    state.store.record_start(&key, at).await?;
    let account = state.store.add_guest(state.config.demo.ttl_secs()).await?;
    drop(guard);
    let parent = crate::vaults::account_dir(&state, &account.id);
    crate::private_dir(&state.config.users_dir())?;
    crate::private_dir(&parent)?;
    let dir = parent.join(DEMO_VAULT);
    std::fs::create_dir(&dir)?;
    let sample = state.config.demo.vault.clone();
    let seeded = tokio::task::spawn_blocking(move || seed(sample.as_deref(), &dir))
        .await
        .map_err(|error| anyhow::anyhow!("копия демо-vault: {error}"))?;
    if let Err(error) = seeded {
        remove_guest(&state, &account.id).await;
        return Err(error.into());
    }
    tracing::info!(login = %account.login, "гость демо");
    // Адрес гостя в списке сессий не хранится: входов и списка сессий у гостя
    // нет, а предел ведёт ключ с секретом.
    let (cookie, me) = open_session(&state, account, &headers, "").await?;
    Ok((
        [(SET_COOKIE, cookie)],
        Json(Started {
            me,
            vault: DEMO_VAULT,
        }),
    )
        .into_response())
}

#[derive(Serialize)]
pub struct Usage {
    used: u64,
    quota: u64,
}

/// `GET /api/demo/usage`: объём гостя.
pub async fn usage(
    State(state): State<Shared>,
    Extension(authed): Extension<Authed>,
) -> ApiResult<Json<Usage>> {
    if !state.config.demo_mode {
        return Err(ApiError::NotFound);
    }
    Ok(Json(Usage {
        used: account_usage(&state, &authed.account.id).await,
        quota: state.config.demo.quota_bytes(),
    }))
}

// ── Квота (research.md §4) ──────────────────────────────────────────────────

/// Команды, которые пропускаются и сверх квоты: читают или освобождают место.
/// Не попавшая сюда читающая команда сверх квоты получит отказ — неудобство, а
/// не дыра; записывающая сюда попадать не должна.
const WITHIN_QUOTA: &[&str] = &[
    "list",
    "read",
    "rename",
    "move_targets",
    "move_impact",
    "move_entry",
    "delete",
    "backlinks",
    "broken_links",
    "access_state",
    "search",
    "replace_preview",
    "links_health",
    "snapshot_history",
    "snapshot_diff",
    "snapshot_text",
    "snapshot_deleted_notes",
    "snapshot_usage",
    "snapshots_clear",
    "tasks_view",
    "bookmarks_view",
    "bookmark_remove",
    "spell_suggest",
    "spell_remove_word",
    "syntax_session_open",
    "syntax_session_update",
    "syntax_session_close",
    "syntax_session_spelling",
    "board_write_profile",
    "file_kind",
    "discard_edit",
    "pending_conflict",
    "unresolved_edit_paths",
    "conflict_diff",
    "read_board",
    "embeds_of",
    "embeds_in_text",
    "embeds_changed",
    "plan_embed",
    "vault_settings",
    "unmark_section",
    "keybindings",
    "template_library",
    "read_template",
    "preview_compile",
    "preview_dependency_mark",
    "preview_installed_templates",
    "export_candidates",
    "export_batch_cancel",
    "export_batch_view",
    "sources_summary",
    "sources_search",
    "sources_for_completion",
    "sources_scan_view",
    "sources_scan_cancel",
    "sources_citing",
    "sources_missing_keys",
    "sources_check_note",
    "attachments_batch_cancel",
    "attachments_batch_view",
    "links_backlinks_page",
    "links_mentions_cancel",
    "workspace_view",
    "structure_outline",
    "structure_outline_view",
    "structure_outline_active",
    "render_element",
    "plan_element_edit",
    "template_event",
    "sync_capabilities",
    "sync_overview",
    "sync_poll_events",
    "sync_progress",
    "sync_note_change",
    "sync_cancel",
    "sync_ignore_reports",
    "sync_ignore_open",
    "sync_ignore_check",
    "sync_ignore_restore",
    "sync_changes",
    "sync_history",
    "sync_diff",
    "sync_snapshot_changes",
    "sync_file_at",
    "sync_conflicts",
    "sync_device_values",
    "sync_open",
];

/// Отказ гостю, если команда может записать, а vault с телом запроса выйдет за
/// квоту. `None` — можно выполнять.
///
/// Записи гостя идут по одной — под замком `demo_write` его vault, который
/// держится до конца команды: параллельные записи иначе прошли бы проверку при
/// одном и том же объёме (ревью безопасности 039, С2). Одна команда может выйти
/// за квоту на свой собственный выход (экспорт), следующая уже не пройдёт.
pub async fn write_guard<'a>(
    state: &Shared,
    authed: &Authed,
    open: &'a OpenVault,
    command: &str,
    body: usize,
) -> ApiResult<Option<tokio::sync::MutexGuard<'a, ()>>> {
    if !state.config.demo_mode || WITHIN_QUOTA.contains(&command) {
        return Ok(None);
    }
    let guard = open.demo_write.lock().await;
    let used = account_usage(state, &authed.account.id).await;
    if used.saturating_add(body as u64) > state.config.demo.quota_bytes() {
        return Err(ApiError::DemoQuota {
            quota_mb: state.config.demo.quota_mb,
        });
    }
    Ok(Some(guard))
}

/// После удаления у гостя корзина сразу пустеет: удаление на демо окончательное,
/// и освободить место сверх квоты можно (корзина входит в квоту, К1).
pub async fn after_delete(state: &Shared, account_id: &str) {
    let trash = state.config.trash_dir().join("notes").join(account_id);
    if trash.exists() {
        let _ = tokio::task::spawn_blocking(move || std::fs::remove_dir_all(trash)).await;
    }
}

// ── Уборка (research.md §5) ─────────────────────────────────────────────────

/// Удаляет гостя целиком: vault (с сессией синхронизации), корзину, данные
/// синхронизации и строку аккаунта — сессии и id синхронизации каскадом.
pub async fn remove_guest(state: &Shared, account_id: &str) {
    state.vaults.close_with_sync(account_id, DEMO_VAULT).await;
    state.vaults.close_account(account_id).await;
    state.sync.forget_account(account_id).await;
    let dirs: [PathBuf; 3] = [
        crate::vaults::account_dir(state, account_id),
        crate::sync::account_sync_dir(state, account_id),
        state.config.trash_dir().join("notes").join(account_id),
    ];
    for dir in dirs {
        if dir.exists() {
            if let Err(error) = std::fs::remove_dir_all(&dir) {
                tracing::error!("папка гостя не удалена: {error}");
            }
        }
    }
    if let Err(error) = state.store.delete_account(account_id).await {
        tracing::error!("гость не удалён из базы: {error:#}");
    }
}

/// Удаляет просроченных гостей.
///
/// Просроченные гости убираются в любом режиме сервера: сервер, перезапущенный
/// без `--demo`, не оставляет им ни функций, ни файлов (Н4). Папки без хозяина
/// (запрос, шедший во время уборки, пересоздал папку) и старые старты убираются
/// только на демо-сервере: на обычном папка удалённого аккаунта — намеренно
/// оставленные vault (С1).
pub async fn sweep(state: &Shared) {
    match state.store.expired_guests(now()).await {
        Ok(expired) => {
            for id in expired {
                remove_guest(state, &id).await;
            }
        }
        Err(error) => tracing::error!("уборка гостей: {error:#}"),
    }
    if !state.config.demo_mode {
        return;
    }
    if let Err(error) = state.store.prune_starts(now() - 60 * 60).await {
        tracing::error!("уборка стартов демо: {error:#}");
    }
    for root in [
        state.config.users_dir(),
        state.config.sync_dir(),
        state.config.trash_dir().join("notes"),
    ] {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let id = entry.file_name().to_string_lossy().into_owned();
            if matches!(state.store.account_exists(&id).await, Ok(false)) {
                state.sync.forget_account(&id).await;
                let path = entry.path();
                let _ = tokio::task::spawn_blocking(move || std::fs::remove_dir_all(path)).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ip_key;

    #[test]
    fn addresses_are_bucketed_before_counting() {
        let key = |ip: &str| ip_key("секрет", ip);
        // Вся сеть IPv6 /64 — один адрес.
        assert_eq!(key("2001:db8:1:2::1"), key("2001:db8:1:2:ffff::9"));
        assert_ne!(key("2001:db8:1:2::1"), key("2001:db8:1:3::1"));
        // IPv4 в IPv6 — тот же IPv4.
        assert_eq!(key("::ffff:203.0.113.7"), key("203.0.113.7"));
        assert_ne!(key("203.0.113.7"), key("203.0.113.8"));
        // Не адрес — один общий ключ, а не новый на каждую строку.
        assert_eq!(key("evil-1"), key("evil-2"));
        // Ключ зависит от секрета сервера.
        assert_ne!(ip_key("а", "203.0.113.7"), ip_key("б", "203.0.113.7"));
    }
}
