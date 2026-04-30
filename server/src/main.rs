use clap::Parser;
use pkv_sync_server::auth::password;
use pkv_sync_server::cli::{Cli, Command, MigrateOp, UserOp};
use pkv_sync_server::config::Config;
use pkv_sync_server::db::repos::{NewUser, UserRepo};
use std::sync::Arc;

fn read_password_stdin() -> anyhow::Result<String> {
    use std::io::{BufReader, IsTerminal, Write};

    let stdin = std::io::stdin();
    let password = if stdin.is_terminal() {
        eprint!("Password (will not echo): ");
        std::io::stderr().flush().ok();
        rpassword::read_password()?
    } else {
        let mut reader = BufReader::new(stdin.lock());
        rpassword::read_password_from_bufread(&mut reader)?
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
    }
    Ok(())
}
