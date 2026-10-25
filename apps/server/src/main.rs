//! `typsidian-server`: `serve` и команды администратора (US6, research.md §2).
//!
//! Открытой регистрации нет: аккаунты заводит администратор на машине сервера.

use std::io::{BufRead, IsTerminal};
use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use typsidian_server::auth::{check_new_password, hash_password, MIN_PASSWORD_CHARS};
use typsidian_server::config::Config;
use typsidian_server::store::Store;
use typsidian_server::{router, spawn_housekeeping, AppState};

#[derive(Parser)]
#[command(name = "typsidian-server", version, about = "Сервер Typsidian")]
struct Cli {
    /// Файл настроек (TOML).
    #[arg(long, short, global = true, env = "TYPSIDIAN_CONFIG")]
    config: Option<PathBuf>,
    /// Папка данных (поверх настроек).
    #[arg(long, global = true)]
    data: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Запустить сервер.
    Serve {
        /// Адрес, например 127.0.0.1:8787.
        #[arg(long)]
        listen: Option<SocketAddr>,
        /// Собранная статикой web-версия.
        #[arg(long = "static")]
        static_dir: Option<PathBuf>,
        /// Демо-сервер: только гости с временным vault, без входа в профиль.
        #[arg(long)]
        demo: bool,
        /// Своя папка-образец демо-vault (иначе встроенная).
        #[arg(long)]
        demo_vault: Option<PathBuf>,
    },
    /// Аккаунты.
    User {
        #[command(subcommand)]
        command: UserCommand,
    },
}

#[derive(Subcommand)]
enum UserCommand {
    /// Завести аккаунт; пароль спрашивается (или из stdin).
    Add {
        login: String,
        /// Отображаемое имя.
        #[arg(long)]
        name: Option<String>,
        /// Без пароля — только вход через провайдера.
        #[arg(long)]
        no_password: bool,
    },
    /// Задать или сбросить пароль.
    Passwd {
        login: String,
    },
    /// Заблокировать: сессии завершаются, вход невозможен.
    Block {
        login: String,
    },
    Unblock {
        login: String,
    },
    /// Удалить аккаунт; папки vault остаются на диске без `--with-vaults`.
    Delete {
        login: String,
        #[arg(long)]
        with_vaults: bool,
    },
    List,
    /// Привязать пользователя провайдера входа (издатель и `sub`).
    LinkOidc {
        login: String,
        #[arg(long)]
        issuer: String,
        #[arg(long)]
        subject: String,
    },
}

fn read_password() -> anyhow::Result<String> {
    let password = if std::io::stdin().is_terminal() {
        let first = rpassword::prompt_password("Пароль: ")?;
        let second = rpassword::prompt_password("Ещё раз: ")?;
        if first != second {
            anyhow::bail!("пароли не совпали");
        }
        first
    } else {
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line)?;
        line.trim_end_matches(['\r', '\n']).to_owned()
    };
    if check_new_password(&password).is_err() {
        anyhow::bail!("пароль не короче {MIN_PASSWORD_CHARS} знаков");
    }
    Ok(password)
}

async fn account(store: &Store, login: &str) -> anyhow::Result<typsidian_server::store::Account> {
    store
        .account_by_login(login)
        .await?
        .ok_or_else(|| anyhow::anyhow!("аккаунта «{login}» нет"))
}

async fn user(config: &Config, command: UserCommand) -> anyhow::Result<()> {
    let store = Store::open(&config.database()).await?;
    match command {
        UserCommand::Add {
            login,
            name,
            no_password,
        } => {
            let hash = if no_password {
                None
            } else {
                Some(hash_password(&read_password()?)?)
            };
            let created = store
                .add_account(&login, name.as_deref().unwrap_or(&login), hash.as_deref())
                .await?;
            println!("аккаунт «{}» заведён ({})", created.login, created.id);
        }
        UserCommand::Passwd { login } => {
            let found = account(&store, &login).await?;
            let hash = hash_password(&read_password()?)?;
            store.set_password(&found.id, &hash).await?;
            store.end_sessions(&found.id, None).await?;
            println!("пароль «{login}» задан, сессии завершены");
        }
        UserCommand::Block { login } => {
            let found = account(&store, &login).await?;
            store.set_blocked(&found.id, true).await?;
            println!("«{login}» заблокирован, сессии завершены");
        }
        UserCommand::Unblock { login } => {
            let found = account(&store, &login).await?;
            store.set_blocked(&found.id, false).await?;
            println!("«{login}» разблокирован");
        }
        UserCommand::Delete { login, with_vaults } => {
            let found = account(&store, &login).await?;
            store.delete_account(&found.id).await?;
            // Секреты и настройки синхронизации удаляются всегда, даже если vault
            // остаются (038 FR-002): без аккаунта они ничьи.
            let sync_dir = config.sync_dir().join(&found.id);
            if sync_dir.exists() {
                std::fs::remove_dir_all(&sync_dir)?;
            }
            let dir = config.users_dir().join(&found.id);
            if with_vaults && dir.exists() {
                std::fs::remove_dir_all(&dir)?;
                println!("«{login}» удалён вместе с vault");
            } else {
                println!("«{login}» удалён; vault остались в {}", dir.display());
            }
        }
        UserCommand::List => {
            for found in store.accounts().await? {
                let links = store.oidc_links(&found.id).await?;
                println!(
                    "{}\t{}\t{}{}{}",
                    found.login,
                    found.display_name,
                    if found.blocked {
                        "заблокирован"
                    } else {
                        "активен"
                    },
                    if found.password_hash.is_some() {
                        "\tпароль"
                    } else {
                        ""
                    },
                    links
                        .iter()
                        .map(|(issuer, subject)| format!("\t{issuer} {subject}"))
                        .collect::<String>()
                );
            }
        }
        UserCommand::LinkOidc {
            login,
            issuer,
            subject,
        } => {
            let found = account(&store, &login).await?;
            store.link_oidc(&found.id, &issuer, &subject).await?;
            println!("«{login}» привязан к {issuer} {subject}");
        }
    }
    Ok(())
}

async fn serve(config: Config) -> anyhow::Result<()> {
    config.check_transport()?;
    typsidian_server::private_dir(&config.data)?;
    typsidian_server::private_dir(&config.users_dir())?;
    let store = Store::open(&config.database()).await?;
    let listen = config.listen;
    let state = AppState::new(config, store);
    // Гости, чей срок истёк, пока сервер стоял (US2 AS3), — в любом режиме.
    typsidian_server::demo::sweep(&state).await;
    if state.config.demo_mode {
        typsidian_server::demo::check(&state)?;
        tracing::info!(
            "демо-сервер: гости на {} мин, объём {} МБ",
            state.config.demo.ttl_minutes,
            state.config.demo.quota_mb
        );
    }
    spawn_housekeeping(state.clone());
    let listener = tokio::net::TcpListener::bind(listen).await?;
    tracing::info!("слушаю {listen}");
    axum::serve(
        listener,
        router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let cli = Cli::parse();
    let mut config = Config::load(cli.config.as_deref())?;
    if let Some(data) = cli.data {
        config.data = data;
    }
    match cli.command {
        Command::Serve {
            listen,
            static_dir,
            demo,
            demo_vault,
        } => {
            if demo {
                config.demo_mode = true;
            }
            if let Some(dir) = demo_vault {
                config.demo.vault = Some(dir);
            }
            if let Some(listen) = listen {
                config.listen = listen;
            }
            if let Some(dir) = static_dir {
                config.static_dir = Some(dir);
            }
            serve(config).await
        }
        Command::User { command } => user(&config, command).await,
    }
}
