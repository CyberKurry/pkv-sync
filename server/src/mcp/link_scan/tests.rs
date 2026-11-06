//! Tests for MEL-07: fixed-commit scanning, budgets, completeness.

use crate::db::repos::UserRepo;
use crate::mcp::link_scan::{target_index, visit_snapshot, ScanLimits, VaultSnapshot, VisitFlow};
use crate::service::exclude::{EffectiveExcludes, SyncPathFilter};
use crate::storage::git::GitVaultStore;

async fn state_user_vault() -> (crate::service::AppState, String, String, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let p = crate::db::pool::connect_memory().await.unwrap();
    sqlx::migrate!("./migrations").run(&p).await.unwrap();
    let state = crate::service::AppState::new(p, tmp.path().to_path_buf(), "t".into(), true)
        .await
        .unwrap();
    let user = state
        .users
        .create(crate::db::repos::NewUser {
            username: "u".into(),
            password_hash: crate::auth::password::hash("passw0rd!!").unwrap(),
            is_admin: false,
        })
        .await
        .unwrap();
    let vault = crate::service::vault::create_vault(&state, &user.id, "main")
        .await
        .unwrap();
    (state, user.id, vault.id, tmp)
}

fn plain_filter() -> SyncPathFilter {
    SyncPathFilter::new(
        EffectiveExcludes::compile(&[]).unwrap(),
        EffectiveExcludes::compile(&[]).unwrap(),
    )
}

#[tokio::test]
async fn mel_scan_budget_visits_all_files_sorted_and_reports_complete() {
    let (state, _uid, vid, _tmp) = state_user_vault().await;
    let git = state.git_store();
    let head = git
        .commit_changes(
            &vid,
            None,
            &[
                crate::storage::git::FileChange::upsert_text("b.md", b"[[a]]".to_vec()),
                crate::storage::git::FileChange::upsert_text("a.md", b"[[b]]".to_vec()),
            ],
            "seed",
        )
        .await
        .unwrap();

    let snapshot = VaultSnapshot {
        at: head.clone(),
        filter: plain_filter(),
        max_file_size: 1024 * 1024,
    };
    let limits = ScanLimits::default();
    let mut seen: Vec<String> = Vec::new();
    let integrity = visit_snapshot(&git, &vid, &snapshot, &limits, |file| {
        seen.push(file.path.to_string());
        assert!(file.scan.reasons.is_empty());
        VisitFlow::Continue
    })
    .await
    .unwrap();
    assert!(
        integrity.complete,
        "no reason to be incomplete: {:?}",
        integrity.reasons
    );
    assert_eq!(seen, ["a.md", "b.md"], "sorted tree order");
}

#[tokio::test]
async fn mel_scan_budget_byte_cap_stops_before_materializing_oversized() {
    let (state, _uid, vid, _tmp) = state_user_vault().await;
    let git = state.git_store();
    let head = git
        .commit_changes(
            &vid,
            None,
            &[crate::storage::git::FileChange::upsert_text(
                "big.md",
                vec![b'x'; 2048],
            )],
            "seed",
        )
        .await
        .unwrap();

    let snapshot = VaultSnapshot {
        at: head,
        filter: plain_filter(),
        max_file_size: 1024 * 1024,
    };
    let limits = ScanLimits {
        max_source_bytes: 1024, // big.md is 2048 — must not be read
        ..ScanLimits::default()
    };
    let mut visited = 0usize;
    let integrity = visit_snapshot(&git, &vid, &snapshot, &limits, |_| {
        visited += 1;
        VisitFlow::Continue
    })
    .await
    .unwrap();
    assert!(!integrity.complete);
    assert!(integrity.reasons.contains(&"byte_cap"));
    assert_eq!(visited, 0, "oversized entry never materialized");
}

#[tokio::test]
async fn mel_scan_budget_file_cap_marks_incomplete_and_guards_basenames() {
    let (state, _uid, vid, _tmp) = state_user_vault().await;
    let git = state.git_store();
    let changes: Vec<_> = (0..4)
        .map(|i| {
            crate::storage::git::FileChange::upsert_text(format!("f{i}.md"), b"[[x]]".to_vec())
        })
        .collect();
    let head = git
        .commit_changes(&vid, None, &changes, "seed")
        .await
        .unwrap();

    let snapshot = VaultSnapshot {
        at: head.clone(),
        filter: plain_filter(),
        max_file_size: 1024 * 1024,
    };
    // Index cap below the file count: integrity incomplete.
    let tree = git.list_tree(&vid, Some(&head)).await.unwrap();
    let limits = ScanLimits {
        max_files: 2,
        ..ScanLimits::default()
    };
    let (_index, integrity) = target_index(&git, &snapshot, &tree, &limits).await;
    assert!(!integrity.complete);
    assert!(integrity.reasons.contains(&"file_cap"));
}

#[tokio::test]
async fn mel_scan_budget_link_cap_propagates_from_document_scan() {
    let (state, _uid, vid, _tmp) = state_user_vault().await;
    let git = state.git_store();
    let head = git
        .commit_changes(
            &vid,
            None,
            &[crate::storage::git::FileChange::upsert_text(
                "spam.md",
                b"[[a]] [[b]] [[c]]".to_vec(),
            )],
            "seed",
        )
        .await
        .unwrap();
    let snapshot = VaultSnapshot {
        at: head,
        filter: plain_filter(),
        max_file_size: 1024 * 1024,
    };
    let limits = ScanLimits {
        max_links_per_file: 2,
        ..ScanLimits::default()
    };
    let integrity = visit_snapshot(&git, &vid, &snapshot, &limits, |_| VisitFlow::Continue)
        .await
        .unwrap();
    assert!(!integrity.complete);
    assert!(integrity.reasons.contains(&"link_cap"));
}

#[tokio::test]
async fn mel_scan_budget_visitor_stop_keeps_scan_complete() {
    let (state, _uid, vid, _tmp) = state_user_vault().await;
    let git = state.git_store();
    let head = git
        .commit_changes(
            &vid,
            None,
            &[
                crate::storage::git::FileChange::upsert_text("a.md", b"x".to_vec()),
                crate::storage::git::FileChange::upsert_text("b.md", b"y".to_vec()),
            ],
            "seed",
        )
        .await
        .unwrap();
    let snapshot = VaultSnapshot {
        at: head,
        filter: plain_filter(),
        max_file_size: 1024 * 1024,
    };
    let mut seen = 0;
    let integrity = visit_snapshot(&git, &vid, &snapshot, &ScanLimits::default(), |_| {
        seen += 1;
        VisitFlow::Stop
    })
    .await
    .unwrap();
    assert_eq!(seen, 1, "visitor stop halts immediately");
    assert!(
        integrity.complete,
        "caller stop is not scan incompleteness: {:?}",
        integrity.reasons
    );
}

#[tokio::test]
async fn mel_scan_budget_hidden_paths_never_become_sources() {
    let (state, _uid, vid, _tmp) = state_user_vault().await;
    let git = state.git_store();
    let head = git
        .commit_changes(
            &vid,
            None,
            &[
                crate::storage::git::FileChange::upsert_text("a.md", b"[[b]]".to_vec()),
                crate::storage::git::FileChange::upsert_text(".hidden/b.md", b"x".to_vec()),
            ],
            "seed",
        )
        .await
        .unwrap();
    let snapshot = VaultSnapshot {
        at: head,
        filter: plain_filter(),
        max_file_size: 1024 * 1024,
    };
    let mut seen: Vec<String> = Vec::new();
    visit_snapshot(&git, &vid, &snapshot, &ScanLimits::default(), |f| {
        seen.push(f.path.to_string());
        VisitFlow::Continue
    })
    .await
    .unwrap();
    assert_eq!(seen, ["a.md"]);
}
