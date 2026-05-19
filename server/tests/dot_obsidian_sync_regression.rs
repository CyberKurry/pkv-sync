use pkv_sync_server::auth::{password, token};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewToken, NewUser, RuntimeConfigRepo, TokenRepo, UserRepo};
use pkv_sync_server::service::sync::{pull, push, PushChange, PushReq};
use pkv_sync_server::service::{vault, vault_settings, AppState};
use pkv_sync_server::storage::git::{FileChange, Git2VaultStore, GitVaultStore, StoredFile};

async fn setup() -> (
    AppState,
    pkv_sync_server::auth::AuthenticatedUser,
    String,
    tempfile::TempDir,
) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("metadata.db");
    let p = pool::connect(&db_path).await.unwrap();
    pool::migrate_up(&p).await.unwrap();
    let state = AppState::new(p, tmp.path().to_path_buf(), "test".into(), true)
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
    let raw = token::generate();
    let token_row = state
        .tokens
        .create(NewToken {
            user_id: &user.id,
            token_hash: &token::hash(&raw),
            device_id: "device-dot-obsidian",
            device_name: "dot",
        })
        .await
        .unwrap();
    let auth_user = pkv_sync_server::auth::AuthenticatedUser {
        user_id: user.id.clone(),
        username: user.username,
        is_admin: false,
        token_id: token_row.id,
        device_id: token_row.device_id,
    };
    let vault = vault::create_vault(&state, &user.id, "main").await.unwrap();
    (state, auth_user, vault.id, tmp)
}

async fn save_allowlist(state: &AppState, vault_id: &str, globs: Vec<&str>) {
    vault_settings::save(
        state,
        vault_id,
        &vault_settings::VaultSettings {
            extra_sync_globs: globs.into_iter().map(String::from).collect(),
        },
    )
    .await
    .unwrap();
}

fn text(path: &str, content: &str) -> PushChange {
    PushChange::Text {
        path: path.to_string(),
        content: content.to_string(),
    }
}

fn delete(path: &str) -> PushChange {
    PushChange::Delete {
        path: path.to_string(),
    }
}

async fn push_changes(
    state: &AppState,
    user: &pkv_sync_server::auth::AuthenticatedUser,
    vault_id: &str,
    parent: Option<&str>,
    changes: Vec<PushChange>,
) -> Result<pkv_sync_server::service::sync::PushResp, pkv_sync_server::api::error::ApiError> {
    push(
        state,
        user,
        vault_id,
        parent,
        None,
        PushReq {
            device_name: Some("test".into()),
            changes,
        },
    )
    .await
}

#[tokio::test]
async fn hidden_paths_require_allowlist_and_allow_nested_deep_paths() {
    let (state, user, vault_id, _tmp) = setup().await;
    save_allowlist(&state, &vault_id, vec![]).await;

    let rejected = push_changes(
        &state,
        &user,
        &vault_id,
        None,
        vec![text(".claude/agents/agent.json", "{}")],
    )
    .await
    .unwrap_err();
    assert_eq!(rejected.status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(rejected.code, "path_excluded");

    save_allowlist(
        &state,
        &vault_id,
        vec![".obsidian/themes/**", ".claude/agents/**"],
    )
    .await;
    let first = push_changes(
        &state,
        &user,
        &vault_id,
        None,
        vec![
            text(".obsidian/themes/deep/nested/theme.css", "body{}"),
            text(".obsidian/themes/.draft/theme.css", "body{color:red}"),
            text(".claude/agents/.cache/file.json", "{}"),
        ],
    )
    .await
    .unwrap();
    assert_eq!(first.files_changed, 3);

    let second = push_changes(
        &state,
        &user,
        &vault_id,
        Some(&first.new_commit),
        vec![
            delete(".obsidian/themes/deep/nested/theme.css"),
            delete(".claude/agents/.cache/file.json"),
            text(
                ".claude/agents/renamed/.cache/file.json",
                "{\"renamed\":true}",
            ),
        ],
    )
    .await
    .unwrap();
    assert_eq!(second.files_changed, 3);

    let pulled = pull(&state, &user.user_id, &vault_id, Some(&first.new_commit))
        .await
        .unwrap();
    assert_eq!(
        pulled
            .added
            .iter()
            .map(|f| f.path.as_str())
            .collect::<Vec<_>>(),
        vec![".claude/agents/renamed/.cache/file.json"]
    );
    assert_eq!(
        pulled.deleted,
        vec![
            ".claude/agents/.cache/file.json",
            ".obsidian/themes/deep/nested/theme.css"
        ]
    );
}

#[tokio::test]
async fn starter_allowlist_accepts_selected_obsidian_settings_but_hard_excludes_win() {
    let (state, user, vault_id, _tmp) = setup().await;

    let accepted = push_changes(
        &state,
        &user,
        &vault_id,
        None,
        vec![
            text(".obsidian/hotkeys.json", "{}"),
            text(".obsidian/app.json", "{}"),
            text(".obsidian/appearance.json", "{}"),
            text(".obsidian/community-plugins.json", "[]"),
            text(".obsidian/core-plugins.json", "[]"),
            text(".obsidian/snippets/theme.css", "body{}"),
            text(".obsidian/themes/custom/theme.css", "body{}"),
        ],
    )
    .await
    .unwrap();
    assert_eq!(accepted.files_changed, 7);

    save_allowlist(&state, &vault_id, vec![".obsidian/**"]).await;
    let rejected = push_changes(
        &state,
        &user,
        &vault_id,
        Some(&accepted.new_commit),
        vec![text(".obsidian/workspace.json", "{}")],
    )
    .await
    .unwrap_err();
    assert_eq!(rejected.status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(rejected.code, "path_excluded");
}

#[tokio::test]
async fn normal_paths_obey_user_excludes_after_hidden_allowlist_check() {
    let (state, user, vault_id, _tmp) = setup().await;
    state
        .runtime_cfg_repo
        .set_extra_exclude_globs(vec!["drafts/**".into(), "*.json".into()], None)
        .await
        .unwrap();
    state
        .runtime_cfg
        .replace(state.runtime_cfg_repo.load().await.unwrap())
        .await;
    save_allowlist(&state, &vault_id, vec![".claude/agents/**"]).await;

    let normal_rejected = push_changes(
        &state,
        &user,
        &vault_id,
        None,
        vec![text("drafts/today.md", "draft")],
    )
    .await
    .unwrap_err();
    assert_eq!(normal_rejected.code, "path_excluded");

    let hidden_rejected = push_changes(
        &state,
        &user,
        &vault_id,
        None,
        vec![text(".claude/agents/drafts/agent.json", "{}")],
    )
    .await
    .unwrap_err();
    assert_eq!(hidden_rejected.code, "path_excluded");
}

#[tokio::test]
async fn pull_silently_skips_filtered_added_modified_and_deleted_paths() {
    let (state, user, vault_id, _tmp) = setup().await;
    save_allowlist(&state, &vault_id, vec![]).await;
    state
        .runtime_cfg_repo
        .set_extra_exclude_globs(vec!["scratch/**".into()], None)
        .await
        .unwrap();
    state
        .runtime_cfg
        .replace(state.runtime_cfg_repo.load().await.unwrap())
        .await;

    let git = Git2VaultStore::new(state.default_vault_root());
    let first = git
        .commit_changes(
            &vault_id,
            None,
            &[
                FileChange::Upsert {
                    path: "notes/keep.md".into(),
                    file: StoredFile::Text {
                        bytes: b"keep".to_vec(),
                    },
                },
                FileChange::Upsert {
                    path: ".obsidian/workspace.json".into(),
                    file: StoredFile::Text {
                        bytes: b"{}".to_vec(),
                    },
                },
                FileChange::Upsert {
                    path: "scratch/tmp.md".into(),
                    file: StoredFile::Text {
                        bytes: b"tmp".to_vec(),
                    },
                },
            ],
            "seed filtered paths",
        )
        .await
        .unwrap();

    let initial = pull(&state, &user.user_id, &vault_id, None).await.unwrap();
    assert_eq!(
        initial
            .added
            .iter()
            .map(|f| f.path.as_str())
            .collect::<Vec<_>>(),
        vec!["notes/keep.md"]
    );

    let second = git
        .commit_changes(
            &vault_id,
            Some(&first),
            &[
                FileChange::Upsert {
                    path: "notes/keep.md".into(),
                    file: StoredFile::Text {
                        bytes: b"keep v2".to_vec(),
                    },
                },
                FileChange::Upsert {
                    path: ".obsidian/workspace.json".into(),
                    file: StoredFile::Text {
                        bytes: b"{\"open\":true}".to_vec(),
                    },
                },
                FileChange::Delete {
                    path: "scratch/tmp.md".into(),
                },
            ],
            "update filtered paths",
        )
        .await
        .unwrap();

    let delta = pull(&state, &user.user_id, &vault_id, Some(&first))
        .await
        .unwrap();
    assert_eq!(delta.to.as_deref(), Some(second.as_str()));
    assert_eq!(
        delta
            .modified
            .iter()
            .map(|f| f.path.as_str())
            .collect::<Vec<_>>(),
        vec!["notes/keep.md"]
    );
    assert!(delta.added.is_empty());
    assert!(delta.deleted.is_empty());
}
