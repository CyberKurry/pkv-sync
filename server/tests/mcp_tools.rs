use bytes::Bytes;
use pkv_sync_server::db::repos::{BlobRefRepo, NewUser, UserRepo, VaultRepo};
use pkv_sync_server::mcp::tools::{
    list_files, read_file, search, ListFilesInput, ReadFileInput, SearchInput,
};
use pkv_sync_server::service::AppState;
use pkv_sync_server::storage::blob::{BlobStore, LocalFsBlobStore};
use pkv_sync_server::storage::git::{FileChange, Git2VaultStore, GitVaultStore, StoredFile};

async fn test_state() -> (AppState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let p = pkv_sync_server::db::pool::connect(&tmp.path().join("test.db"))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&p).await.unwrap();
    let state = AppState::new(p, tmp.path().to_path_buf(), "test".into(), true)
        .await
        .unwrap();
    (state, tmp)
}

async fn create_user(state: &AppState, username: &str) -> String {
    state
        .users
        .create(NewUser {
            username: username.into(),
            password_hash: "hash".into(),
            is_admin: false,
        })
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn list_files_enforces_vault_ownership() {
    let (state, _tmp) = test_state().await;
    let owner = create_user(&state, "owner").await;
    let intruder = create_user(&state, "intruder").await;
    let vault = state.vaults.create(&owner, "main").await.unwrap();
    let git = Git2VaultStore::new(state.default_vault_root());
    git.commit_changes(
        &vault.id,
        None,
        &[FileChange::Upsert {
            path: "note.md".into(),
            file: StoredFile::Text {
                bytes: b"secret".to_vec(),
            },
        }],
        "seed",
    )
    .await
    .unwrap();

    let err = list_files(
        &state,
        &intruder,
        ListFilesInput {
            vault_id: vault.id,
            at: None,
        },
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("vault not found"));
}

#[tokio::test]
async fn read_file_returns_text_and_expands_text_blob_pointer() {
    let (state, _tmp) = test_state().await;
    let user_id = create_user(&state, "reader").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();
    let blob = LocalFsBlobStore::new(state.default_blob_root());
    let blob_bytes = Bytes::from_static(b"expanded blob text");
    let blob_hash = LocalFsBlobStore::sha256(&blob_bytes);
    blob.put_verified(&blob_hash, blob_bytes).await.unwrap();

    let git = Git2VaultStore::new(state.default_vault_root());
    let commit = git
        .commit_changes(
            &vault.id,
            None,
            &[
                FileChange::Upsert {
                    path: "plain.md".into(),
                    file: StoredFile::Text {
                        bytes: b"hello utf8".to_vec(),
                    },
                },
                FileChange::Upsert {
                    path: "blob.txt".into(),
                    file: StoredFile::BlobPointer {
                        hash: blob_hash.clone(),
                        size: 18,
                        mime: Some("text/plain".into()),
                    },
                },
            ],
            "seed",
        )
        .await
        .unwrap();
    state
        .blob_refs
        .add_refs(&vault.id, &commit, std::slice::from_ref(&blob_hash))
        .await
        .unwrap();

    let plain = read_file(
        &state,
        &user_id,
        ReadFileInput {
            vault_id: vault.id.clone(),
            path: "plain.md".into(),
        },
    )
    .await
    .unwrap();
    let expanded = read_file(
        &state,
        &user_id,
        ReadFileInput {
            vault_id: vault.id,
            path: "blob.txt".into(),
        },
    )
    .await
    .unwrap();

    assert!(!plain.is_binary);
    assert_eq!(plain.encoding.as_deref(), Some("utf-8"));
    assert_eq!(plain.content, "hello utf8");
    assert!(!expanded.is_binary);
    assert_eq!(expanded.mime.as_deref(), Some("text/plain"));
    assert_eq!(expanded.encoding.as_deref(), Some("utf-8"));
    assert_eq!(expanded.content, "expanded blob text");
}

#[tokio::test]
async fn read_file_normalizes_mcp_paths_before_git_lookup() {
    let (state, _tmp) = test_state().await;
    let user_id = create_user(&state, "mcp-path-reader").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();
    let git = Git2VaultStore::new(state.default_vault_root());
    git.commit_changes(
        &vault.id,
        None,
        &[FileChange::Upsert {
            path: "folder/note.md".into(),
            file: StoredFile::Text {
                bytes: b"normalized".to_vec(),
            },
        }],
        "seed",
    )
    .await
    .unwrap();

    let output = read_file(
        &state,
        &user_id,
        ReadFileInput {
            vault_id: vault.id,
            path: "folder\\note.md".into(),
        },
    )
    .await
    .unwrap();

    assert_eq!(output.path, "folder/note.md");
    assert_eq!(output.content, "normalized");
}

#[tokio::test]
async fn read_file_rejects_mcp_parent_traversal_paths() {
    let (state, _tmp) = test_state().await;
    let user_id = create_user(&state, "mcp-path-reject").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();

    let err = read_file(
        &state,
        &user_id,
        ReadFileInput {
            vault_id: vault.id,
            path: "../secret.md".into(),
        },
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("invalid_path"), "{err}");
}

#[tokio::test]
async fn search_finds_case_insensitive_text_matches_and_skips_binary_and_blob_content() {
    let (state, _tmp) = test_state().await;
    let user_id = create_user(&state, "searcher").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();
    let blob = LocalFsBlobStore::new(state.default_blob_root());
    let blob_bytes = Bytes::from_static(b"Needle hidden in blob");
    let blob_hash = LocalFsBlobStore::sha256(&blob_bytes);
    blob.put_verified(&blob_hash, blob_bytes).await.unwrap();

    let git = Git2VaultStore::new(state.default_vault_root());
    git.commit_changes(
        &vault.id,
        None,
        &[
            FileChange::Upsert {
                path: "notes/a.md".into(),
                file: StoredFile::Text {
                    bytes: b"first line\nFind the NEEDLE here\nlast line".to_vec(),
                },
            },
            FileChange::Upsert {
                path: "raw.bin".into(),
                file: StoredFile::Text {
                    bytes: b"needle in binary extension".to_vec(),
                },
            },
            FileChange::Upsert {
                path: "attachment.txt".into(),
                file: StoredFile::BlobPointer {
                    hash: blob_hash,
                    size: 21,
                    mime: Some("text/plain".into()),
                },
            },
        ],
        "seed",
    )
    .await
    .unwrap();

    let output = search(
        &state,
        &user_id,
        SearchInput {
            vault_id: vault.id,
            query: "needle".into(),
            at: None,
            limit: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(output.matches.len(), 1);
    assert_eq!(output.matches[0].path, "notes/a.md");
    assert_eq!(output.matches[0].line_number, 2);
    assert_eq!(output.matches[0].line, "Find the NEEDLE here");
    assert_eq!(output.matches[0].snippet, "Find the NEEDLE here");
}

#[tokio::test]
async fn search_rejects_trees_over_safe_file_limit() {
    let (state, _tmp) = test_state().await;
    let user_id = create_user(&state, "large").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();
    let git = Git2VaultStore::new(state.default_vault_root());
    let changes = (0..=5000)
        .map(|i| FileChange::Upsert {
            path: format!("notes/{i}.md"),
            file: StoredFile::Text {
                bytes: b"needle".to_vec(),
            },
        })
        .collect::<Vec<_>>();
    git.commit_changes(&vault.id, None, &changes, "large")
        .await
        .unwrap();

    let err = search(
        &state,
        &user_id,
        SearchInput {
            vault_id: vault.id,
            query: "needle".into(),
            at: None,
            limit: None,
        },
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("too many files"));
}
