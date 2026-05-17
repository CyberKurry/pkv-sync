use pkv_sync_server::service::exclude::EffectiveExcludes;

#[test]
fn empty_excludes_match_nothing() {
    let excludes = EffectiveExcludes::compile(&[]).unwrap();
    assert!(!excludes.is_excluded("foo.md"));
    assert!(!excludes.is_excluded("foo.tmp"));
}

#[test]
fn star_tmp_matches_tmp_files() {
    let excludes = EffectiveExcludes::compile(&["*.tmp".into()]).unwrap();
    assert!(excludes.is_excluded("foo.tmp"));
    assert!(!excludes.is_excluded("foo.md"));
}

#[test]
fn glob_star_star_build() {
    let excludes = EffectiveExcludes::compile(&["build/**".into()]).unwrap();
    assert!(excludes.is_excluded("build/x/y.js"));
    assert!(!excludes.is_excluded("src/build.md"));
}

#[test]
fn invalid_glob_returns_error() {
    assert!(EffectiveExcludes::compile(&["[invalid".into()]).is_err());
}

#[test]
fn empty_strings_are_skipped() {
    let excludes = EffectiveExcludes::compile(&["".into(), "  ".into(), "*.log".into()]).unwrap();
    assert!(excludes.is_excluded("debug.log"));
    assert!(!excludes.is_excluded("debug.md"));
}
