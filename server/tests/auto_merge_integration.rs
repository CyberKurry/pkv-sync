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
async fn auto_merge_conflict_file_bypasses_user_exclude_globs() {
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

    let git = state.git_store();
    let tree = git
        .list_tree(&vault_id, Some(&device_b.new_commit))
        .await
        .unwrap();
    let conflict_path = tree
        .iter()
        .find(|entry| entry.path.contains(".conflict-"))
        .map(|entry| entry.path.as_str())
        .expect("expected conflict sidecar to be preserved in git tree");
    let marked = read_text(&state, &vault_id, conflict_path).await;
    assert!(marked.contains("<<<<<<< local"));
    assert!(marked.contains("======="));
    assert!(marked.contains(">>>>>>> remote"));
    assert!(marked.contains("AlPhA"));
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

// --- Task 4: Text merge outcomes threading ---

#[tokio::test]
async fn text_merge_nonoverlapping_edits_reports_merged_outcome() {
    // THE dogfood scenario: base "one\ntwo\n", local edits line 1, remote edits
    // line 2 → merged content contains BOTH edits, outcome merged, NO conflict
    // files anywhere in the tree.
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
                content: "one\ntwo\n".into(),
            }],
        },
    )
    .await
    .unwrap();

    // Device A edits line 1.
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
                content: "ONE\ntwo\n".into(),
            }],
        },
    )
    .await
    .unwrap();

    // Device B edits line 2 with stale If-Match → triggers auto-merge.
    let merged = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-b".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "one\nTWO\n".into(),
            }],
        },
    )
    .await
    .unwrap();

    assert_eq!(merged.files_changed, 1);
    let outcomes = merged
        .merge_outcomes
        .as_ref()
        .expect("merge_outcomes should be present for stale push");
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].path, "note.md");
    assert_eq!(outcomes[0].outcome, sync::MergeOutcomeKind::Merged);
    assert!(outcomes[0].conflict_path.is_none());

    // Committed content must contain BOTH edits.
    assert_eq!(read_text(&state, &vault_id, "note.md").await, "ONE\nTWO\n");

    // No conflict sidecar files anywhere in the tree.
    let git = pkv_sync_server::storage::git::Git2VaultStore::new(state.default_vault_root());
    let tree = git
        .list_tree(&vault_id, Some(&merged.new_commit))
        .await
        .unwrap();
    assert!(
        !tree.iter().any(|e| e.path.contains(".conflict-")),
        "expected no conflict sidecar files, found: {:?}",
        tree.iter()
            .filter(|e| e.path.contains(".conflict-"))
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn text_merge_same_line_overlap_reports_conflict_outcome() {
    // Same-line overlap → outcome conflict + conflict_path pointing at the marker
    // file; original path holds remote content.
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

    let merged = sync::push(
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

    assert_eq!(merged.files_changed, 1);
    let outcomes = merged
        .merge_outcomes
        .as_ref()
        .expect("merge_outcomes should be present");
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].path, "note.md");
    assert_eq!(outcomes[0].outcome, sync::MergeOutcomeKind::Conflict);
    let conflict_path = outcomes[0]
        .conflict_path
        .as_ref()
        .expect("conflict should have conflict_path");
    assert!(conflict_path.contains(".conflict-"));

    // Original path holds remote (device-a) content.
    assert_eq!(read_text(&state, &vault_id, "note.md").await, "ALPHA\n");

    // Conflict marker file must exist and contain conflict markers.
    let marked = read_text(&state, &vault_id, conflict_path).await;
    assert!(marked.contains("<<<<<<< local"));
    assert!(marked.contains("======="));
    assert!(marked.contains(">>>>>>> remote"));
}

#[tokio::test]
async fn fast_path_push_omits_merge_outcomes_from_json() {
    // Fast path (fresh If-Match) → merge_outcomes ABSENT (None), guarding the
    // verbatim contract.
    let (state, user, vault_id, _tmp) = setup().await;
    let resp = sync::push(
        &state,
        &user,
        &vault_id,
        None,
        Some("idem-fast-path-no-merge-outcomes"),
        sync::PushReq {
            device_name: Some("test".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "hello".into(),
            }],
        },
    )
    .await
    .unwrap();
    assert_eq!(resp.files_changed, 1);

    // When merge_outcomes is None, the JSON body must NOT contain the key.
    let raw_json = serde_json::to_string(&resp).unwrap();
    assert!(
        !raw_json.contains("merge_outcomes"),
        "fast-path push should not include merge_outcomes in JSON, got: {raw_json}"
    );

    // The PushResp struct must still round-trip through serialization.
    let deserialized: sync::PushResp = serde_json::from_str(&raw_json).unwrap();
    assert_eq!(deserialized.new_commit, resp.new_commit);
    assert_eq!(deserialized.files_changed, resp.files_changed);
    assert!(deserialized.merge_outcomes.is_none());
}

#[test]
fn push_resp_deserializes_without_merge_outcomes_for_old_cached_entries() {
    // Simulates reading an old idempotency cache entry that was written
    // before merge_outcomes was added to PushResp.
    let old_json = r#"{"new_commit":"abc123","files_changed":2}"#;
    let resp: sync::PushResp = serde_json::from_str(old_json).unwrap();
    assert_eq!(resp.new_commit, "abc123");
    assert_eq!(resp.files_changed, 2);
    assert!(resp.merge_outcomes.is_none());
}

#[tokio::test]
async fn stale_push_idempotent_replay_returns_cached_merge_outcomes() {
    // Idempotent replay of a merged push returns the cached response INCLUDING
    // its merge_outcomes.
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
                content: "one\ntwo\n".into(),
            }],
        },
    )
    .await
    .unwrap();

    // Device A moves head forward.
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
                content: "ONE\ntwo\n".into(),
            }],
        },
    )
    .await
    .unwrap();

    // Device B makes a stale push with an idempotency key → triggers merge.
    fn stale_req() -> sync::PushReq {
        sync::PushReq {
            device_name: Some("device-b".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "one\nTWO\n".into(),
            }],
        }
    }
    let first = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        Some("idem-merge-replay"),
        stale_req(),
    )
    .await
    .unwrap();

    // The first push should have merge_outcomes.
    let first_outcomes = first
        .merge_outcomes
        .as_ref()
        .expect("first push should have merge_outcomes");
    assert_eq!(first_outcomes.len(), 1);
    assert_eq!(first_outcomes[0].outcome, sync::MergeOutcomeKind::Merged);

    // Idempotent replay should return the SAME response, including merge_outcomes.
    let replay = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        Some("idem-merge-replay"),
        stale_req(),
    )
    .await
    .unwrap();

    assert_eq!(replay.new_commit, first.new_commit);
    let replay_outcomes = replay
        .merge_outcomes
        .as_ref()
        .expect("replay should have merge_outcomes");
    assert_eq!(replay_outcomes.len(), first_outcomes.len());
    assert_eq!(replay_outcomes[0].path, first_outcomes[0].path);
    assert_eq!(replay_outcomes[0].outcome, first_outcomes[0].outcome);
}

// --- Task 2: Merge delete changes on stale push ---

#[tokio::test]
async fn merge_delete_untouched_by_remote_outcome_clean() {
    let (state, user, vault_id, _tmp) = setup().await;
    let base = sync::push(
        &state,
        &user,
        &vault_id,
        None,
        None,
        sync::PushReq {
            device_name: Some("base".into()),
            changes: vec![
                sync::PushChange::Text {
                    path: "gone.md".into(),
                    content: "delete me".into(),
                },
                sync::PushChange::Text {
                    path: "other.md".into(),
                    content: "keep".into(),
                },
            ],
        },
    )
    .await
    .unwrap();

    let _device_b = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-b".into()),
            changes: vec![sync::PushChange::Text {
                path: "other.md".into(),
                content: "changed by B".into(),
            }],
        },
    )
    .await
    .unwrap();

    let merged = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-a".into()),
            changes: vec![sync::PushChange::Delete {
                path: "gone.md".into(),
            }],
        },
    )
    .await
    .unwrap();

    assert_eq!(merged.files_changed, 1);
    let outcomes = merged
        .merge_outcomes
        .expect("merge_outcomes should be present");
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].path, "gone.md");
    assert_eq!(outcomes[0].outcome, sync::MergeOutcomeKind::Clean);
    assert!(outcomes[0].conflict_path.is_none());

    let git = pkv_sync_server::storage::git::Git2VaultStore::new(state.default_vault_root());
    let file = git.read_file(&vault_id, "gone.md", None).await.unwrap();
    assert!(file.is_none(), "gone.md should have been deleted");
    assert_eq!(
        read_text(&state, &vault_id, "other.md").await,
        "changed by B"
    );
}

#[tokio::test]
async fn merge_delete_modified_by_remote_outcome_conflict() {
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
                path: "kept.md".into(),
                content: "original".into(),
            }],
        },
    )
    .await
    .unwrap();

    let _device_b = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-b".into()),
            changes: vec![sync::PushChange::Text {
                path: "kept.md".into(),
                content: "remote update".into(),
            }],
        },
    )
    .await
    .unwrap();

    let merged = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-a".into()),
            changes: vec![sync::PushChange::Delete {
                path: "kept.md".into(),
            }],
        },
    )
    .await
    .unwrap();

    assert_eq!(merged.files_changed, 0);
    let outcomes = merged
        .merge_outcomes
        .expect("merge_outcomes should be present");
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].path, "kept.md");
    assert_eq!(outcomes[0].outcome, sync::MergeOutcomeKind::Conflict);
    assert!(outcomes[0].conflict_path.is_none());

    assert_eq!(
        read_text(&state, &vault_id, "kept.md").await,
        "remote update"
    );
}

#[tokio::test]
async fn merge_delete_already_deleted_by_remote_outcome_clean() {
    let (state, user, vault_id, _tmp) = setup().await;
    let base = sync::push(
        &state,
        &user,
        &vault_id,
        None,
        None,
        sync::PushReq {
            device_name: Some("base".into()),
            changes: vec![
                sync::PushChange::Text {
                    path: "doomed.md".into(),
                    content: "soon gone".into(),
                },
                sync::PushChange::Text {
                    path: "survivor.md".into(),
                    content: "stays".into(),
                },
            ],
        },
    )
    .await
    .unwrap();

    let _device_b = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-b".into()),
            changes: vec![sync::PushChange::Delete {
                path: "doomed.md".into(),
            }],
        },
    )
    .await
    .unwrap();

    let merged = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-a".into()),
            changes: vec![sync::PushChange::Delete {
                path: "doomed.md".into(),
            }],
        },
    )
    .await
    .unwrap();

    assert_eq!(merged.files_changed, 0);
    let outcomes = merged
        .merge_outcomes
        .expect("merge_outcomes should be present");
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].path, "doomed.md");
    assert_eq!(outcomes[0].outcome, sync::MergeOutcomeKind::Clean);
    assert!(outcomes[0].conflict_path.is_none());

    let git = pkv_sync_server::storage::git::Git2VaultStore::new(state.default_vault_root());
    let file = git.read_file(&vault_id, "doomed.md", None).await.unwrap();
    assert!(file.is_none(), "doomed.md should remain deleted");
}

// --- Task 3: Merge blob changes on stale push ---

fn sha256_hex(data: &[u8]) -> String {
    use pkv_sync_server::storage::blob::LocalFsBlobStore;
    LocalFsBlobStore::sha256(&bytes::Bytes::from(data.to_vec()))
}

#[tokio::test]
async fn merge_blob_upsert_untouched_by_remote_outcome_clean() {
    let (state, user, vault_id, _tmp) = setup().await;
    let base = sync::push(
        &state,
        &user,
        &vault_id,
        None,
        None,
        sync::PushReq {
            device_name: Some("base".into()),
            changes: vec![
                sync::PushChange::Text {
                    path: "note.md".into(),
                    content: "keep".into(),
                },
                sync::PushChange::Text {
                    path: "other.md".into(),
                    content: "other".into(),
                },
            ],
        },
    )
    .await
    .unwrap();

    let _device_b = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-b".into()),
            changes: vec![sync::PushChange::Text {
                path: "other.md".into(),
                content: "changed by B".into(),
            }],
        },
    )
    .await
    .unwrap();

    let blob_data = bytes::Bytes::from_static(b"image data here");
    let blob_size = blob_data.len() as u64;
    let blob_hash = sha256_hex(&blob_data);
    sync::upload_blob(&state, &user.user_id, &vault_id, &blob_hash, blob_data)
        .await
        .unwrap();

    let merged = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-a".into()),
            changes: vec![sync::PushChange::Blob {
                path: "photo.png".into(),
                blob_hash: blob_hash.clone(),
                size: blob_size,
                mime: Some("image/png".into()),
            }],
        },
    )
    .await
    .unwrap();

    assert_eq!(merged.files_changed, 1);
    let outcomes = merged
        .merge_outcomes
        .expect("merge_outcomes should be present");
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].path, "photo.png");
    assert_eq!(outcomes[0].outcome, sync::MergeOutcomeKind::Clean);
    assert!(outcomes[0].conflict_path.is_none());

    let git = pkv_sync_server::storage::git::Git2VaultStore::new(state.default_vault_root());
    let file = git
        .read_file(&vault_id, "photo.png", None)
        .await
        .unwrap()
        .expect("photo.png should exist");
    match file {
        pkv_sync_server::storage::git::StoredFile::BlobPointer { hash, .. } => {
            assert_eq!(hash, blob_hash);
        }
        other => panic!("expected blob pointer, got {other:?}"),
    }
    assert_eq!(
        read_text(&state, &vault_id, "other.md").await,
        "changed by B"
    );
}

#[tokio::test]
async fn merge_blob_upsert_conflict_remote_modified_same_path() {
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
                content: "original".into(),
            }],
        },
    )
    .await
    .unwrap();

    let _device_b = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-b".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "remote modified".into(),
            }],
        },
    )
    .await
    .unwrap();

    let blob_data = bytes::Bytes::from_static(b"image data");
    let blob_hash = sha256_hex(&blob_data);
    sync::upload_blob(&state, &user.user_id, &vault_id, &blob_hash, blob_data)
        .await
        .unwrap();

    let merged = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-a".into()),
            changes: vec![sync::PushChange::Blob {
                path: "note.md".into(),
                blob_hash: blob_hash.clone(),
                size: 10,
                mime: Some("image/png".into()),
            }],
        },
    )
    .await
    .unwrap();

    assert_eq!(merged.files_changed, 1);
    let outcomes = merged
        .merge_outcomes
        .expect("merge_outcomes should be present");
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].path, "note.md");
    assert_eq!(outcomes[0].outcome, sync::MergeOutcomeKind::Conflict);
    let conflict_path = outcomes[0]
        .conflict_path
        .as_ref()
        .expect("conflict should have a conflict_path");
    assert!(conflict_path.contains(".conflict-"));

    assert_eq!(
        read_text(&state, &vault_id, "note.md").await,
        "remote modified"
    );

    let git = pkv_sync_server::storage::git::Git2VaultStore::new(state.default_vault_root());
    let conflict_file = git
        .read_file(&vault_id, conflict_path, None)
        .await
        .unwrap()
        .expect("conflict sidecar should exist");
    match conflict_file {
        pkv_sync_server::storage::git::StoredFile::BlobPointer { hash, .. } => {
            assert_eq!(hash, blob_hash);
        }
        other => panic!("expected blob pointer in conflict sidecar, got {other:?}"),
    }
}

#[tokio::test]
async fn merge_mixed_batch_text_delete_blob_one_commit() {
    let (state, user, vault_id, _tmp) = setup().await;
    let base = sync::push(
        &state,
        &user,
        &vault_id,
        None,
        None,
        sync::PushReq {
            device_name: Some("base".into()),
            changes: vec![
                sync::PushChange::Text {
                    path: "note.md".into(),
                    content: "alpha\nbeta\n".into(),
                },
                sync::PushChange::Text {
                    path: "gone.md".into(),
                    content: "delete me".into(),
                },
                sync::PushChange::Text {
                    path: "target.md".into(),
                    content: "target content".into(),
                },
            ],
        },
    )
    .await
    .unwrap();

    let _device_b = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-b".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "ALPHA\nbeta\n".into(),
            }],
        },
    )
    .await
    .unwrap();

    let blob_data = bytes::Bytes::from_static(b"blob content");
    let blob_hash = sha256_hex(&blob_data);
    sync::upload_blob(&state, &user.user_id, &vault_id, &blob_hash, blob_data)
        .await
        .unwrap();

    let merged = sync::push(
        &state,
        &user,
        &vault_id,
        Some(&base.new_commit),
        None,
        sync::PushReq {
            device_name: Some("device-a".into()),
            changes: vec![
                sync::PushChange::Text {
                    path: "note.md".into(),
                    content: "alpha\nGAMMA\n".into(),
                },
                sync::PushChange::Delete {
                    path: "gone.md".into(),
                },
                sync::PushChange::Blob {
                    path: "new_img.png".into(),
                    blob_hash: blob_hash.clone(),
                    size: 12,
                    mime: Some("image/png".into()),
                },
            ],
        },
    )
    .await
    .unwrap();

    assert!(merged.files_changed >= 1);
    let outcomes = merged
        .merge_outcomes
        .expect("merge_outcomes should be present");
    assert_eq!(outcomes.len(), 3);

    // 1. Text: note.md merged (clean because non-overlapping edits)
    assert_eq!(outcomes[0].path, "note.md");
    assert_eq!(outcomes[0].outcome, sync::MergeOutcomeKind::Merged);

    // 2. Delete: gone.md clean (remote didn't touch it)
    assert_eq!(outcomes[1].path, "gone.md");
    assert_eq!(outcomes[1].outcome, sync::MergeOutcomeKind::Clean);

    // 3. Blob: new_img.png clean (remote didn't touch it)
    assert_eq!(outcomes[2].path, "new_img.png");
    assert_eq!(outcomes[2].outcome, sync::MergeOutcomeKind::Clean);

    assert_eq!(
        read_text(&state, &vault_id, "note.md").await,
        "ALPHA\nGAMMA\n"
    );
    let git = pkv_sync_server::storage::git::Git2VaultStore::new(state.default_vault_root());
    assert!(git
        .read_file(&vault_id, "gone.md", None)
        .await
        .unwrap()
        .is_none());
    let file = git
        .read_file(&vault_id, "new_img.png", None)
        .await
        .unwrap()
        .expect("new_img.png should exist");
    match file {
        pkv_sync_server::storage::git::StoredFile::BlobPointer { hash, .. } => {
            assert_eq!(hash, blob_hash);
        }
        other => panic!("expected blob pointer, got {other:?}"),
    }
}

// --- Task 5: Guards ---

#[tokio::test]
async fn stale_push_with_bogus_if_match_returns_head_mismatch_not_internal_error() {
    // Stale push whose If-Match is a completely unknown commit hash (simulates
    // vault rollback where the old commit is truly gone) → 409 head_mismatch,
    // not a 500.
    let (state, user, vault_id, _tmp) = setup().await;
    // Create one commit so the vault has a head.
    let _base = sync::push(
        &state,
        &user,
        &vault_id,
        None,
        None,
        sync::PushReq {
            device_name: Some("base".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "original".into(),
            }],
        },
    )
    .await
    .unwrap();

    // Push with a completely bogus If-Match (40 hex chars, not a real commit).
    // This simulates the "unreachable commit" scenario.
    let bogus_commit = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
    let result = sync::push(
        &state,
        &user,
        &vault_id,
        Some(bogus_commit),
        None,
        sync::PushReq {
            device_name: Some("device-b".into()),
            changes: vec![sync::PushChange::Text {
                path: "other.md".into(),
                content: "new file".into(),
            }],
        },
    )
    .await;

    // Must NOT be a 500 Internal Server Error. It should either be a 409
    // Conflict (head_mismatch) or succeed via auto-merge if the bogus commit
    // gracefully falls through. The critical invariant: no 500.
    match result {
        Ok(resp) => {
            // Auto-merge may succeed; that's fine.
            assert!(!resp.new_commit.is_empty());
        }
        Err(err) => {
            // Must be CONFLICT, not INTERNAL_SERVER_ERROR.
            assert_eq!(err.status, axum::http::StatusCode::CONFLICT);
            assert_eq!(err.code, "head_mismatch");
        }
    }
}

#[tokio::test]
async fn auto_merge_disabled_never_produces_merge_outcomes() {
    // enable_auto_merge=false → stale push 409s; merge_outcomes never present.
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

    // Device B tries a stale push with non-overlapping edits.
    // With auto_merge disabled, this should be a plain 409.
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

    assert_eq!(err.status, axum::http::StatusCode::CONFLICT);
    assert_eq!(err.code, "head_mismatch");
}
