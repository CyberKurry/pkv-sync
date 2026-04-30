use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn wait_with_timeout(mut child: std::process::Child) -> Output {
    let start = Instant::now();
    loop {
        match child.try_wait().unwrap() {
            Some(_) => return child.wait_with_output().unwrap(),
            None if start.elapsed() > Duration::from_secs(5) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("pkvsyncd command timed out");
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
    wait_with_timeout(child)
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
deployment_key = "k_cliusersmoke"

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
        Some("passw0rd!!\n"),
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
        Some("newpass1234\n"),
    ));
}
