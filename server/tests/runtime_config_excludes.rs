use pkv_sync_server::service::exclude::{
    EffectiveExcludes, ExcludeError, MAX_GLOB_PATTERNS, MAX_GLOB_PATTERN_LEN, MAX_GLOB_TOTAL_LEN,
};

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

#[test]
fn rejects_too_many_glob_patterns() {
    let globs = vec!["*.tmp".to_string(); MAX_GLOB_PATTERNS + 1];

    let err = match EffectiveExcludes::compile(&globs) {
        Ok(_) => panic!("expected too many patterns error"),
        Err(err) => err,
    };

    assert!(matches!(err, ExcludeError::TooManyPatterns { .. }));
}

#[test]
fn rejects_overlong_glob_patterns() {
    let globs = vec![format!("{}*", "a".repeat(MAX_GLOB_PATTERN_LEN + 1))];

    let err = match EffectiveExcludes::compile(&globs) {
        Ok(_) => panic!("expected overlong pattern error"),
        Err(err) => err,
    };

    assert!(matches!(err, ExcludeError::PatternTooLong { .. }));
}

#[test]
fn rejects_overlong_glob_lists() {
    let pattern_len = (MAX_GLOB_TOTAL_LEN / MAX_GLOB_PATTERNS) + 1;
    let globs = vec!["a".repeat(pattern_len); MAX_GLOB_PATTERNS];

    let err = match EffectiveExcludes::compile(&globs) {
        Ok(_) => panic!("expected overlong glob list error"),
        Err(err) => err,
    };

    assert!(matches!(err, ExcludeError::TotalTooLong { .. }));
}
