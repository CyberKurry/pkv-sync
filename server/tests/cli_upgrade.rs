use pkv_sync_server::cli::upgrade::{
    asset_name_for, container_guidance, new_binary_path_for, parse_sha256sums,
    prepare_download_target, render_dry_run, target_triple, ContainerProbe, UpgradePlan,
};
use std::path::Path;

#[test]
fn parses_sha256sums_for_release_assets() {
    let hash = "a".repeat(64);
    let sums = format!(
        "{hash}  ./pkvsyncd-x86_64-unknown-linux-gnu\n{}  plugin/pkv-sync-plugin.zip\n",
        "b".repeat(64)
    );

    assert_eq!(
        parse_sha256sums(&sums, "pkvsyncd-x86_64-unknown-linux-gnu").as_deref(),
        Some(hash.as_str())
    );
}

#[test]
fn rejects_invalid_checksum_lines() {
    let sums = "not-a-hash  pkvsyncd-x86_64-unknown-linux-gnu\n";

    assert!(parse_sha256sums(sums, "pkvsyncd-x86_64-unknown-linux-gnu").is_none());
}

#[test]
fn target_asset_matches_current_platform() {
    let triple = target_triple();
    let asset = asset_name_for(triple).expect("current test platform should have an asset");

    assert!(asset.starts_with("pkvsyncd-"));
    assert!(
        asset.contains(triple),
        "asset {asset} should include target triple {triple}"
    );
}

#[test]
fn side_by_side_path_uses_new_suffix_next_to_current_binary() {
    let path = new_binary_path_for(Path::new("/usr/local/bin/pkvsyncd"));

    assert_eq!(path, Path::new("/usr/local/bin/pkvsyncd.new"));
}

#[cfg(windows)]
#[test]
fn side_by_side_path_keeps_windows_executable_extension() {
    let path = new_binary_path_for(Path::new("C:\\PKVSync\\pkvsyncd.exe"));

    assert_eq!(path, Path::new("C:\\PKVSync\\pkvsyncd.new.exe"));
}

#[test]
fn docker_or_kubernetes_probe_prints_container_guidance() {
    let probe = ContainerProbe {
        docker_env_exists: false,
        kubernetes_service_host: Some("10.0.0.1".into()),
    };

    let guidance = container_guidance(&probe);

    assert!(guidance.contains("Docker"));
    assert!(guidance.contains("docker pull ghcr.io/cyberkurry/pkv-sync"));
    assert!(guidance.contains("restart"));
}

#[test]
fn dry_run_renders_target_path_without_download_claim() {
    let plan = UpgradePlan {
        version: "0.9.1".into(),
        asset_name: "pkvsyncd-x86_64-unknown-linux-gnu".into(),
        target_path: Path::new("/usr/local/bin/pkvsyncd.new").to_path_buf(),
        release_url: "https://github.com/cyberkurry/pkv-sync/releases/tag/v0.9.1".into(),
    };

    let rendered = render_dry_run(&plan);

    assert!(rendered.contains("dry run"));
    assert!(rendered.contains("pkvsyncd-x86_64-unknown-linux-gnu"));
    assert!(rendered.contains("/usr/local/bin/pkvsyncd.new"));
    assert!(rendered.contains("NEXT STEPS"));
    assert!(!rendered.contains("Downloaded"));
}

#[test]
fn rejects_target_when_parent_directory_is_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("missing").join("pkvsyncd.new");

    let err = prepare_download_target(&target).unwrap_err();

    assert!(err.to_string().contains("does not exist"), "{err}");
}

#[cfg(unix)]
#[test]
fn rejects_target_when_parent_directory_is_not_writable() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o555)).unwrap();

    let err = prepare_download_target(&tmp.path().join("pkvsyncd.new")).unwrap_err();

    std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(err.to_string().contains("not writable"), "{err}");
}
