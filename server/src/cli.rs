use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "pkvsyncd", version, about = "PKV Sync server daemon")]
pub struct Cli {
    /// Path to config file (default: /etc/pkv-sync/config.toml)
    #[arg(
        short,
        long,
        global = true,
        default_value = "/etc/pkv-sync/config.toml"
    )]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Start the HTTP server.
    Serve,
    /// Database migration commands.
    Migrate {
        #[command(subcommand)]
        op: MigrateOp,
    },
    /// Generate a random deployment key (paste into config.toml).
    Genkey,
    /// User management commands.
    User {
        #[command(subcommand)]
        op: UserOp,
    },
}

#[derive(Subcommand, Debug)]
pub enum MigrateOp {
    /// Apply all pending migrations.
    Up,
}

#[derive(Subcommand, Debug)]
pub enum UserOp {
    /// Create a user.
    Add {
        username: String,
        #[arg(long)]
        admin: bool,
    },
    /// Reset a user's password.
    Passwd { username: String },
    /// List all users.
    List,
    /// Disable or re-enable a user.
    SetActive {
        username: String,
        #[arg(long)]
        active: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use std::path::PathBuf;

    #[test]
    fn cli_parses_serve() {
        let cli = Cli::try_parse_from(["pkvsyncd", "serve"]).unwrap();
        assert!(matches!(cli.command, Command::Serve));
    }

    #[test]
    fn cli_parses_migrate_up() {
        let cli = Cli::try_parse_from(["pkvsyncd", "migrate", "up"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Migrate { op: MigrateOp::Up }
        ));
    }

    #[test]
    fn cli_parses_genkey() {
        let cli = Cli::try_parse_from(["pkvsyncd", "genkey"]).unwrap();
        assert!(matches!(cli.command, Command::Genkey));
    }

    #[test]
    fn cli_parses_custom_config_path() {
        let cli = Cli::try_parse_from(["pkvsyncd", "-c", "/tmp/x.toml", "serve"]).unwrap();
        assert_eq!(cli.config, PathBuf::from("/tmp/x.toml"));
    }

    #[test]
    fn cli_help_compiles() {
        Cli::command().debug_assert();
    }

    #[test]
    fn cli_parses_user_add() {
        let cli = Cli::try_parse_from(["pkvsyncd", "user", "add", "alice", "--admin"]).unwrap();
        match cli.command {
            Command::User {
                op: UserOp::Add { username, admin },
            } => {
                assert_eq!(username, "alice");
                assert!(admin);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn cli_parses_user_list() {
        let cli = Cli::try_parse_from(["pkvsyncd", "user", "list"]).unwrap();
        assert!(matches!(cli.command, Command::User { op: UserOp::List }));
    }
}
