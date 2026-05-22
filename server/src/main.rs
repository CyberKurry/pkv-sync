use clap::Parser;
use pkv_sync_server::auth::password;
use pkv_sync_server::cli::{Cli, Command, MigrateOp, UserOp};
use pkv_sync_server::config::Config;
use pkv_sync_server::db::repos::{NewUser, UserRepo};
use pkv_sync_server::service::auth::validate_username;
use std::sync::Arc;

fn read_password_stdin() -> anyhow::Result<String> {
    use std::io::{IsTerminal, Write};

    let stdin = std::io::stdin();
    let password = if stdin.is_terminal() {
        eprint!("Password (will not echo): ");
        std::io::stderr().flush().ok();
        rpassword::read_password()?
    } else {
        let config = rpassword::ConfigBuilder::new()
            .input_reader(stdin)
            .output_discard()
            .build();
        rpassword::read_password_with_config(config)?
    };
    if password.is_empty() {
        anyhow::bail!("password cannot be empty");
    }
    Ok(password)
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Serve => {
            let cfg = Arc::new(Config::load(&cli.config)?);
            pkv_sync_server::logging::init_with_config(&cfg.logging);
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(pkv_sync_server::server::run(cfg))?;
        }
        Command::Migrate { op: MigrateOp::Up } => {
            let cfg = Config::load(&cli.config)?;
            pkv_sync_server::logging::init_with_config(&cfg.logging);
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let pool = pkv_sync_server::db::pool::connect(&cfg.storage.db_path).await?;
                pkv_sync_server::db::pool::migrate_up(&pool).await?;
                println!("migrations applied");
                Ok::<_, anyhow::Error>(())
            })?;
        }
        Command::Genkey => {
            pkv_sync_server::logging::init();
            let key = pkv_sync_server::keygen::generate_deployment_key();
            println!("{key}");
            println!();
            println!("Paste this into config.toml:");
            println!();
            println!("[server]");
            println!("deployment_key = \"{key}\"");
        }
        Command::User { op } => {
            let cfg = Config::load(&cli.config)?;
            pkv_sync_server::logging::init_with_config(&cfg.logging);
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let pool = pkv_sync_server::db::pool::connect(&cfg.storage.db_path).await?;
                let users = pkv_sync_server::db::repos::SqliteUserRepo::new(pool);
                match op {
                    UserOp::Add { username, admin } => {
                        validate_username(&username)
                            .map_err(|e| anyhow::anyhow!("{}: {}", e.code, e.message))?;
                        if users.find_by_username(&username).await?.is_some() {
                            anyhow::bail!("user already exists");
                        }
                        let password_hash = password::hash(&read_password_stdin()?)?;
                        let user = users
                            .create(NewUser {
                                username,
                                password_hash,
                                is_admin: admin,
                            })
                            .await?;
                        println!("created user {} ({})", user.username, user.id);
                    }
                    UserOp::Passwd { username } => {
                        let user = users
                            .find_by_username(&username)
                            .await?
                            .ok_or_else(|| anyhow::anyhow!("user not found"))?;
                        let password_hash = password::hash(&read_password_stdin()?)?;
                        users.update_password(&user.id, &password_hash).await?;
                        println!("password updated for {username}");
                    }
                    UserOp::List => {
                        for user in users.list().await? {
                            println!(
                                "{}\t{}\tadmin={}\tactive={}",
                                user.id, user.username, user.is_admin, user.is_active
                            );
                        }
                    }
                    UserOp::SetActive { username, active } => {
                        let user = users
                            .find_by_username(&username)
                            .await?
                            .ok_or_else(|| anyhow::anyhow!("user not found"))?;
                        users.set_active(&user.id, active).await?;
                        println!("set {username} active={active}");
                    }
                }
                Ok::<_, anyhow::Error>(())
            })?;
        }
        Command::Materialize {
            vault_id,
            output,
            at,
        } => {
            let cfg = Config::load(&cli.config)?;
            pkv_sync_server::logging::init_with_config(&cfg.logging);
            pkv_sync_server::cli::materialize::run(&cfg, &vault_id, &output, at.as_deref())?;
        }
        Command::Backup {
            data_dir,
            output,
            gzip,
        } => {
            let mut cfg = Config::load(&cli.config)?;
            if let Some(data_dir) = data_dir {
                cfg.storage.data_dir = data_dir.clone();
                cfg.storage.db_path = data_dir.join("metadata.db");
            }
            pkv_sync_server::logging::init_with_config(&cfg.logging);
            pkv_sync_server::cli::backup::run(&cfg, Some(&cli.config), &output, gzip)?;
        }
        Command::Restore {
            input,
            data_dir,
            force,
        } => {
            pkv_sync_server::logging::init();
            let target =
                data_dir.ok_or_else(|| anyhow::anyhow!("--data-dir is required for restore"))?;
            pkv_sync_server::cli::restore::run(&input, &target, force)?;
        }
        Command::Verify { data_dir, no_fail } => {
            let mut cfg = Config::load(&cli.config)?;
            if let Some(data_dir) = data_dir {
                cfg.storage.data_dir = data_dir.clone();
                cfg.storage.db_path = data_dir.join("metadata.db");
            }
            pkv_sync_server::logging::init_with_config(&cfg.logging);
            let report = pkv_sync_server::cli::verify::run(&cfg, no_fail)?;
            report.print();
            if !report.should_exit_success(no_fail) {
                anyhow::bail!("verification failed");
            }
        }
        Command::Mcp {
            transport,
            vault,
            token,
            bind,
        } => {
            let cfg = Config::load(&cli.config)?;
            pkv_sync_server::logging::init_with_config(&cfg.logging);
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                std::fs::create_dir_all(&cfg.storage.data_dir)?;
                let pool = pkv_sync_server::db::pool::connect(&cfg.storage.db_path).await?;
                pkv_sync_server::db::pool::migrate_up(&pool).await?;
                let default_name = cfg
                    .server
                    .public_host
                    .clone()
                    .unwrap_or_else(|| "PKV Sync".into());
                let git_available = std::process::Command::new("git")
                    .arg("--version")
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);
                let state = pkv_sync_server::service::AppState::new(
                    pool,
                    cfg.storage.data_dir.clone(),
                    default_name,
                    git_available,
                )
                .await?;
                let transport = match transport.as_str() {
                    "stdio" => {
                        let vault_id = vault.ok_or_else(|| {
                            anyhow::anyhow!("--vault is required for stdio transport")
                        })?;
                        let token = token
                            .or_else(|| std::env::var("PKV_TOKEN").ok())
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "--token or PKV_TOKEN is required for stdio transport"
                                )
                            })?;
                        pkv_sync_server::mcp::McpTransport::Stdio { vault_id, token }
                    }
                    "http" => pkv_sync_server::mcp::McpTransport::Http {
                        bind: bind.parse()?,
                    },
                    other => anyhow::bail!(
                        "unsupported MCP transport '{other}', expected 'stdio' or 'http'"
                    ),
                };
                pkv_sync_server::mcp::run(state, transport).await
            })?;
        }
        Command::Upgrade {
            dry_run,
            yes,
            version,
        } => {
            pkv_sync_server::logging::init();
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(pkv_sync_server::cli::upgrade::run(
                pkv_sync_server::cli::upgrade::RunOptions {
                    dry_run,
                    yes,
                    version,
                },
            ))?;
        }
    }
    Ok(())
}
