use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

fn wait_with_timeout(args: &[&str], mut child: std::process::Child) -> Output {
    let start = Instant::now();
    loop {
        match child.try_wait().unwrap() {
            Some(_) => return child.wait_with_output().unwrap(),
            None if start.elapsed() > COMMAND_TIMEOUT => {
                let _ = child.kill();
                let output = child.wait_with_output().unwrap();
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                panic!(
                    "pkvsyncd command timed out after {COMMAND_TIMEOUT:?}: {args:?}\nstdout={stdout}\nstderr={stderr}"
                );
            }
            None => thread::sleep(Duration::from_millis(50)),
        }
    }
}

fn run(args: &[&str], input: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pkvsyncd"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = input {
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    drop(child.stdin.take());
    wait_with_timeout(args, child)
}

/// Same as `run`, but with environment variables set — the MCP stdio entry
/// point is documented as reading `PKV_TOKEN`/`PKV_VAULT_ID`, so the registry
/// configuration has to be exercised through the real process.
fn run_with_env(args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_pkvsyncd"));
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        command.env(key, value);
    }
    let mut child = command.spawn().unwrap();
    drop(child.stdin.take());
    wait_with_timeout(args, child)
}

fn assert_success(output: Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        output.status.success(),
        "status={:?}\nstdout={stdout}\nstderr={stderr}",
        output.status.code()
    );
    stdout
}

fn assert_failure(output: Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        !output.status.success(),
        "unexpected success\nstdout={stdout}\nstderr={stderr}"
    );
    stderr
}

const STRONG_PASSWORD: &str = "Str0ngPassw0rd!";
const NEW_STRONG_PASSWORD: &str = "N3wStr0ngPass!";

fn write_test_config(tmp: &tempfile::TempDir) -> (String, String) {
    let data = tmp.path().join("data");
    std::fs::create_dir_all(&data).unwrap();
    let db_path = data.join("metadata.db");
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        format!(
            r#"
[server]
bind_addr = "127.0.0.1:0"
deployment_key = "k_0123456789abcdef0123456789abcdef"

[storage]
data_dir = "{}"
db_path = "{}"

[network]
trusted_proxies = ["127.0.0.1/32"]
"#,
            data.display().to_string().replace('\\', "/"),
            db_path.display().to_string().replace('\\', "/")
        ),
    )
    .unwrap();
    (
        cfg.to_str().unwrap().to_string(),
        db_path.to_str().unwrap().to_string(),
    )
}

#[test]
fn user_commands_accept_password_from_stdin() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    std::fs::create_dir_all(&data).unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        format!(
            r#"
[server]
bind_addr = "127.0.0.1:0"
deployment_key = "k_0123456789abcdef0123456789abcdef"

[storage]
data_dir = "{}"
db_path = "{}"

[network]
trusted_proxies = ["127.0.0.1/32"]
"#,
            data.display().to_string().replace('\\', "/"),
            data.join("metadata.db")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .unwrap();
    let cfg = cfg.to_str().unwrap();

    assert_success(run(&["-c", cfg, "migrate", "up"], None));
    let add = assert_success(run(
        &["-c", cfg, "user", "add", "bob"],
        Some("Str0ngPassw0rd!\n"),
    ));
    assert!(add.contains("created user bob"));

    let list = assert_success(run(&["-c", cfg, "user", "list"], None));
    assert!(list.contains("bob"));

    assert_success(run(&["-c", cfg, "user", "set-active", "bob"], None));
    let list = assert_success(run(&["-c", cfg, "user", "list"], None));
    assert!(list.contains("active=false"));

    assert_success(run(
        &["-c", cfg, "user", "set-active", "bob", "--active"],
        None,
    ));
    assert_success(run(
        &["-c", cfg, "user", "passwd", "bob"],
        Some("N3wStr0ngPass!\n"),
    ));
}

#[test]
fn mcp_takes_its_vault_from_the_environment() {
    // E-2: the registry entry for this server configures the vault through
    // `PKV_VAULT_ID` (there is no per-user way to template a CLI flag into a
    // package definition), so the fallback must work in the real binary. A
    // bogus token is fine: reaching the token check proves the vault was
    // resolved, while the old behaviour failed earlier with
    // "--vault is required for stdio transport".
    let tmp = tempfile::tempdir().unwrap();
    let (cfg, _db) = write_test_config(&tmp);
    let output = run_with_env(
        &["-c", &cfg, "mcp"],
        &[
            ("PKV_VAULT_ID", "vault-from-env"),
            ("PKV_TOKEN", "pks_bogus"),
        ],
    );
    let stderr = assert_failure(output);
    assert!(
        !stderr.contains("PKV_VAULT_ID is required"),
        "the environment fallback must satisfy the vault requirement: {stderr}"
    );

    // Without either source the message names both.
    let output = run_with_env(&["-c", &cfg, "mcp"], &[("PKV_TOKEN", "pks_bogus")]);
    let stderr = assert_failure(output);
    assert!(
        stderr.contains("--vault or PKV_VAULT_ID is required"),
        "the error must say how to provide a vault: {stderr}"
    );
}

#[test]
fn user_add_rejects_invalid_username() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    std::fs::create_dir_all(&data).unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        format!(
            r#"
[server]
bind_addr = "127.0.0.1:0"
deployment_key = "k_0123456789abcdef0123456789abcdef"

[storage]
data_dir = "{}"
db_path = "{}"

[network]
trusted_proxies = ["127.0.0.1/32"]
"#,
            data.display().to_string().replace('\\', "/"),
            data.join("metadata.db")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .unwrap();
    let cfg = cfg.to_str().unwrap();

    assert_success(run(&["-c", cfg, "migrate", "up"], None));
    let stderr = assert_failure(run(&["-c", cfg, "user", "add", "ab"], None));
    assert!(stderr.contains("invalid_username"), "{stderr}");
}

#[test]
fn user_add_rejects_invalid_username_before_loading_config() {
    let stderr = assert_failure(run(
        &[
            "-c",
            "missing-config-that-should-not-be-read.toml",
            "user",
            "add",
            "ab",
        ],
        None,
    ));
    assert!(stderr.contains("invalid_username"), "{stderr}");
    assert!(
        !stderr.contains("missing-config-that-should-not-be-read.toml"),
        "{stderr}"
    );
}

#[test]
fn user_add_rejects_weak_password() {
    let tmp = tempfile::tempdir().unwrap();
    let (cfg, _db) = write_test_config(&tmp);
    assert_success(run(&["-c", &cfg, "migrate", "up"], None));

    let stderr = assert_failure(run(&["-c", &cfg, "user", "add", "bob"], Some("passw0rd\n")));

    assert!(stderr.contains("at least 12 characters"), "{stderr}");
    let list = assert_success(run(&["-c", &cfg, "user", "list"], None));
    assert!(!list.contains("bob"), "{list}");
}

#[test]
fn user_passwd_rejects_weak_password() {
    let tmp = tempfile::tempdir().unwrap();
    let (cfg, _db) = write_test_config(&tmp);
    assert_success(run(&["-c", &cfg, "migrate", "up"], None));
    assert_success(run(
        &["-c", &cfg, "user", "add", "bob"],
        Some(STRONG_PASSWORD),
    ));

    let stderr = assert_failure(run(
        &["-c", &cfg, "user", "passwd", "bob"],
        Some("weakpass1\n"),
    ));

    assert!(stderr.contains("at least 12 characters"), "{stderr}");
}

#[test]
fn user_passwd_revokes_existing_tokens() {
    use pkv_sync_server::auth::token;
    use pkv_sync_server::db::pool;
    use pkv_sync_server::db::repos::{
        NewToken, SqliteTokenRepo, SqliteUserRepo, TokenRepo, UserRepo,
    };

    let tmp = tempfile::tempdir().unwrap();
    let (cfg, db_path) = write_test_config(&tmp);
    assert_success(run(&["-c", &cfg, "migrate", "up"], None));
    assert_success(run(
        &["-c", &cfg, "user", "add", "bob"],
        Some(STRONG_PASSWORD),
    ));

    let rt = tokio::runtime::Runtime::new().unwrap();
    let issued_token_hash = rt.block_on(async {
        let pool = pool::connect(std::path::Path::new(&db_path)).await.unwrap();
        let users = SqliteUserRepo::new(pool.clone());
        let tokens = SqliteTokenRepo::new(pool);
        let user = users.find_by_username("bob").await.unwrap().unwrap();
        let raw = token::generate();
        let hash = token::hash(&raw);
        tokens
            .create(NewToken {
                user_id: &user.id,
                token_hash: &hash,
                device_id: "cli-test-device",
                device_name: "CLI test device",
            })
            .await
            .unwrap();
        hash
    });

    assert_success(run(
        &["-c", &cfg, "user", "passwd", "bob"],
        Some(NEW_STRONG_PASSWORD),
    ));

    rt.block_on(async {
        let pool = pool::connect(std::path::Path::new(&db_path)).await.unwrap();
        let tokens = SqliteTokenRepo::new(pool);
        assert!(
            tokens
                .find_by_hash(&issued_token_hash)
                .await
                .unwrap()
                .is_none(),
            "an existing device token must be rejected after a CLI password reset"
        );
    });
}
