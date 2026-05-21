pub mod backup;
pub mod materialize;
pub mod restore;
pub mod verify;

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
    /// Expand a vault's git+blob storage into a plain file tree.
    Materialize {
        /// Vault ID to materialize
        vault_id: String,
        /// Output directory (must not exist or be empty)
        #[arg(short, long)]
        output: std::path::PathBuf,
        /// Specific commit SHA to materialize (default: HEAD)
        #[arg(long)]
        at: Option<String>,
    },
    /// Snapshot server data into a portable backup directory.
    Backup {
        /// Data directory override for offline operations
        #[arg(long)]
        data_dir: Option<std::path::PathBuf>,
        /// Backup output directory (must not exist or be empty)
        #[arg(short, long)]
        output: std::path::PathBuf,
        /// Also create a .tar.gz archive next to the backup directory
        #[arg(long)]
        gzip: bool,
    },
    /// Restore a backup directory into a data directory.
    Restore {
        /// Backup directory containing MANIFEST.json
        #[arg(short, long)]
        input: std::path::PathBuf,
        /// Target data directory override
        #[arg(long)]
        data_dir: Option<std::path::PathBuf>,
        /// Clear a non-empty data directory before restoring
        #[arg(long)]
        force: bool,
    },
    /// Verify vault git repositories and content-addressed blobs.
    Verify {
        /// Data directory override for offline operations
        #[arg(long)]
        data_dir: Option<std::path::PathBuf>,
        /// Return success even when verification finds errors
        #[arg(long)]
        no_fail: bool,
    },
    /// Start the read-only MCP server.
    Mcp {
        /// Transport: "stdio" or "http".
        #[arg(long, default_value = "stdio")]
        transport: String,
        /// Stdio only: vault ID to expose.
        #[arg(long)]
        vault: Option<String>,
        /// Stdio only: bearer token. If omitted, PKV_TOKEN is used.
        #[arg(long)]
        token: Option<String>,
        /// HTTP only: bind address.
        #[arg(long, default_value = "127.0.0.1:6711")]
        bind: String,
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

    #[test]
    fn cli_parses_materialize() {
        let cli =
            Cli::try_parse_from(["pkvsyncd", "materialize", "vault1", "-o", "/tmp/out"]).unwrap();
        match cli.command {
            Command::Materialize {
                vault_id,
                output,
                at,
            } => {
                assert_eq!(vault_id, "vault1");
                assert_eq!(output, PathBuf::from("/tmp/out"));
                assert!(at.is_none());
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn cli_parses_materialize_with_at() {
        let cli = Cli::try_parse_from([
            "pkvsyncd",
            "materialize",
            "vault1",
            "-o",
            "/tmp/out",
            "--at",
            "abc123",
        ])
        .unwrap();
        match cli.command {
            Command::Materialize {
                vault_id,
                output,
                at,
            } => {
                assert_eq!(vault_id, "vault1");
                assert_eq!(output, PathBuf::from("/tmp/out"));
                assert_eq!(at.as_deref(), Some("abc123"));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn cli_parses_backup() {
        let cli = Cli::try_parse_from([
            "pkvsyncd",
            "backup",
            "--data-dir",
            "/tmp/data",
            "--output",
            "/tmp/backup",
            "--gzip",
        ])
        .unwrap();
        match cli.command {
            Command::Backup {
                data_dir,
                output,
                gzip,
            } => {
                assert_eq!(data_dir, Some(PathBuf::from("/tmp/data")));
                assert_eq!(output, PathBuf::from("/tmp/backup"));
                assert!(gzip);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn cli_parses_restore_with_force() {
        let cli = Cli::try_parse_from([
            "pkvsyncd",
            "restore",
            "--input",
            "/tmp/backup",
            "--data-dir",
            "/tmp/data",
            "--force",
        ])
        .unwrap();
        match cli.command {
            Command::Restore {
                input,
                data_dir,
                force,
            } => {
                assert_eq!(input, PathBuf::from("/tmp/backup"));
                assert_eq!(data_dir, Some(PathBuf::from("/tmp/data")));
                assert!(force);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn cli_parses_verify_no_fail() {
        let cli =
            Cli::try_parse_from(["pkvsyncd", "verify", "--data-dir", "/tmp/data", "--no-fail"])
                .unwrap();
        match cli.command {
            Command::Verify { data_dir, no_fail } => {
                assert_eq!(data_dir, Some(PathBuf::from("/tmp/data")));
                assert!(no_fail);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn cli_parses_mcp_stdio_defaults() {
        let cli = Cli::try_parse_from(["pkvsyncd", "mcp", "--vault", "vault1"]).unwrap();
        match cli.command {
            Command::Mcp {
                transport,
                vault,
                token,
                bind,
            } => {
                assert_eq!(transport, "stdio");
                assert_eq!(vault.as_deref(), Some("vault1"));
                assert!(token.is_none());
                assert_eq!(bind, "127.0.0.1:6711");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn cli_parses_mcp_http_bind() {
        let cli = Cli::try_parse_from([
            "pkvsyncd",
            "mcp",
            "--transport",
            "http",
            "--bind",
            "0.0.0.0:6711",
        ])
        .unwrap();
        match cli.command {
            Command::Mcp {
                transport, bind, ..
            } => {
                assert_eq!(transport, "http");
                assert_eq!(bind, "0.0.0.0:6711");
            }
            _ => panic!("wrong variant"),
        }
    }
}
