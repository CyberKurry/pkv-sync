use pkv_sync_server::service::merge::{three_way_merge, three_way_merge_bytes, MergeOutcome};

#[test]
fn clean_merge_disjoint_changes() {
    let base = "alpha\nbeta\ngamma\n";
    let local = "ALPHA\nbeta\ngamma\n";
    let remote = "alpha\nbeta\nGAMMA\n";

    let outcome = three_way_merge(base, local, remote);

    match outcome {
        MergeOutcome::Clean(merged) => assert_eq!(merged, "ALPHA\nbeta\nGAMMA\n"),
        other => panic!("expected Clean, got {other:?}"),
    }
}

#[test]
fn conflicting_change_on_same_line() {
    let base = "alpha\n";
    let local = "ALPHA\n";
    let remote = "AlPhA\n";

    let outcome = three_way_merge(base, local, remote);

    match outcome {
        MergeOutcome::Conflicted(marked) => {
            assert!(marked.contains("<<<<<<< local"));
            assert!(marked.contains("ALPHA"));
            assert!(marked.contains("======="));
            assert!(marked.contains("AlPhA"));
            assert!(marked.contains(">>>>>>> remote"));
        }
        other => panic!("expected Conflicted, got {other:?}"),
    }
}

#[test]
fn local_unchanged_remote_modified_takes_remote() {
    let base = "alpha\nbeta\n";
    let local = "alpha\nbeta\n";
    let remote = "alpha\nBETA\n";

    let outcome = three_way_merge(base, local, remote);

    assert!(matches!(
        outcome,
        MergeOutcome::Clean(ref s) if s == "alpha\nBETA\n"
    ));
}

#[test]
fn remote_deleted_local_modified_is_conflict() {
    let base = "alpha\nbeta\n";
    let local = "ALPHA\nbeta\n";
    let remote = "";

    let outcome = three_way_merge(base, local, remote);

    assert!(matches!(outcome, MergeOutcome::Conflicted(_)));
}

#[test]
fn no_base_means_conflict_when_both_added() {
    let local = "from local\n";
    let remote = "from remote\n";

    let outcome = three_way_merge("", local, remote);

    assert!(matches!(outcome, MergeOutcome::Conflicted(_)));
}

#[test]
fn binary_content_returns_binary_outcome() {
    let base = b"hello\0world".to_vec();
    let local = b"hello\0WORLD".to_vec();
    let remote = b"HELLO\0world".to_vec();

    let outcome = three_way_merge_bytes(&base, &local, &remote);

    assert!(matches!(outcome, MergeOutcome::Binary));
}

#[test]
fn merge_oversized_file_falls_back_to_binary() {
    let base = "same\n".repeat(100_001);
    let local = base.replacen("same", "local", 1);
    let remote = base.replacen("same", "remote", 1);

    let outcome = three_way_merge(&base, &local, &remote);

    assert!(matches!(outcome, MergeOutcome::Binary));
}
