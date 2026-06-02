use pkv_sync_server::auth::{password, token, AuthenticatedUser};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewToken, NewUser, RuntimeConfigRepo, TokenRepo, UserRepo};
use pkv_sync_server::service::{sync, vault, AppState};
use pkv_sync_server::storage::git::{Git2VaultStore, GitVaultStore, StoredFile};

async fn setup() -> (AppState, AuthenticatedUser, String, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let db = pool::connect(&tmp.path().join("metadata.db"))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    let state = AppState::new(db, tmp.path().to_path_buf(), "test".into(), true)
        .await
        .unwrap();
    let user = state
        .users
        .create(NewUser {
            username: "alice".into(),
            password_hash: password::hash("passw0rd!!").unwrap(),
            is_admin: false,
        })
        .await
        .unwrap();
    let token_row = state
        .tokens
        .create(NewToken {
            user_id: &user.id,
            token_hash: &token::hash(&token::generate()),
            device_id: "device-auto-merge",
            device_name: "auto merge",
        })
        .await
        .unwrap();
    let auth = AuthenticatedUser {
        user_id: user.id.clone(),
        username: user.username,
        is_admin: false,
        token_id: token_row.id,
        device_id: token_row.device_id,
    };
    let vault = vault::create_vault(&state, &user.id, "main").await.unwrap();
    (state, auth, vault.id, tmp)
}

async fn read_text(state: &AppState, vault_id: &str, path: &str) -> String {
    let git = Git2VaultStore::new(state.default_vault_root());
    match git.read_file(vault_id, path, None).await.unwrap().unwrap() {
        StoredFile::Text { bytes } => String::from_utf8(bytes).unwrap(),
        other => panic!("expected text file, got {other:?}"),
    }
}

#[tokio::test]
async fn two_devices_disjoint_changes_auto_merge_clean() {
    let (state, user, vault_id, _tmp) = setup().await;
    let base = sync::push(
        &state,
        &user,
        &vault_id,
        None,
        None,
        sync::PushReq {
            device_name: Some("base".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "alpha\nbeta\ngamma\n".into(),
            }],
        },
    )
    .await
    .unwrap();

    let _device_a = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-a".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "ALPHA\nbeta\ngamma\n".into(),
            }],
        },
    )
    .await
    .unwrap();

    let device_b = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-b".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "alpha\nbeta\nGAMMA\n".into(),
            }],
        },
    )
    .await
    .unwrap();

    assert_eq!(device_b.files_changed, 1);
    assert_eq!(
        read_text(&state, &vault_id, "note.md").await,
        "ALPHA\nbeta\nGAMMA\n"
    );
    let pulled = sync::pull(&state, &user.user_id, &vault_id, Some(&base.new_commit))
        .await
        .unwrap();
    assert!(pulled.modified.iter().any(|file| file.path == "note.md"
        && file.content_inline.as_deref() == Some("ALPHA\nbeta\nGAMMA\n")));
    assert!(!pulled
        .added
        .iter()
        .chain(pulled.modified.iter())
        .any(|file| file.path.contains(".conflict-")));
}

#[tokio::test]
async fn two_devices_same_line_change_produces_marker_file() {
    let (state, user, vault_id, _tmp) = setup().await;
    let base = sync::push(
        &state,
        &user,
        &vault_id,
        None,
        None,
        sync::PushReq {
            device_name: Some("base".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "alpha\n".into(),
            }],
        },
    )
    .await
    .unwrap();
    let _device_a = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-a".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "ALPHA\n".into(),
            }],
        },
    )
    .await
    .unwrap();

    let device_b = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-b".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "AlPhA\n".into(),
            }],
        },
    )
    .await
    .unwrap();

    assert_eq!(device_b.files_changed, 1);
    assert_eq!(read_text(&state, &vault_id, "note.md").await, "ALPHA\n");
    let pulled = sync::pull(&state, &user.user_id, &vault_id, Some(&base.new_commit))
        .await
        .unwrap();
    let conflict = pulled
        .added
        .iter()
        .chain(pulled.modified.iter())
        .find(|file| file.path.contains(".conflict-"))
        .expect("expected marker conflict file");
    let marked = conflict.content_inline.as_deref().unwrap();
    assert!(marked.contains("<<<<<<< local"));
    assert!(marked.contains("======="));
    assert!(marked.contains(">>>>>>> remote"));
}

#[tokio::test]
async fn auto_merge_conflict_file_respects_exclude_globs() {
    let (state, user, vault_id, _tmp) = setup().await;
    state
        .runtime_cfg_repo
        .set_extra_exclude_globs(vec!["*.conflict-*.md".into()], None)
        .await
        .unwrap();
    state
        .runtime_cfg
        .replace(state.runtime_cfg_repo.load().await.unwrap())
        .await;

    let base = sync::push(
        &state,
        &user,
        &vault_id,
        None,
        None,
        sync::PushReq {
            device_name: Some("base".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "alpha\n".into(),
            }],
        },
    )
    .await
    .unwrap();
    let _device_a = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-a".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "ALPHA\n".into(),
            }],
        },
    )
    .await
    .unwrap();

    let err = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-b".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "AlPhA\n".into(),
            }],
        },
    )
    .await
    .unwrap_err();

    assert_eq!(err.code, "path_excluded");
    assert_eq!(read_text(&state, &vault_id, "note.md").await, "ALPHA\n");
}

#[tokio::test]
async fn auto_merge_disabled_falls_back_to_head_mismatch() {
    let (state, user, vault_id, _tmp) = setup().await;
    state
        .runtime_cfg_repo
        .set_enable_auto_merge(false, None)
        .await
        .unwrap();
    state
        .runtime_cfg
        .replace(state.runtime_cfg_repo.load().await.unwrap())
        .await;
    let base = sync::push(
        &state,
        &user,
        &vault_id,
        None,
        None,
        sync::PushReq {
            device_name: Some("base".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "alpha\nbeta\ngamma\n".into(),
            }],
        },
    )
    .await
    .unwrap();
    let _device_a = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-a".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "ALPHA\nbeta\ngamma\n".into(),
            }],
        },
    )
    .await
    .unwrap();

    let err = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-b".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "alpha\nbeta\nGAMMA\n".into(),
            }],
        },
    )
    .await
    .unwrap_err();

    assert_eq!(err.code, "head_mismatch");
}
