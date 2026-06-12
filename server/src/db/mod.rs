pub mod pool;
pub mod repos;

pub const SQLITE_SAFE_BIND_LIMIT: usize = 900;

#[cfg(test)]
mod tests {
    #[test]
    fn sqlite_safe_bind_limit_is_not_redefined_in_hot_paths() {
        for (path, source) in [
            ("db/repos/blob_ref.rs", include_str!("repos/blob_ref.rs")),
            (
                "db/repos/blob_upload.rs",
                include_str!("repos/blob_upload.rs"),
            ),
            (
                "service/sync/push.rs",
                include_str!("../service/sync/push.rs"),
            ),
        ] {
            assert!(
                !source.contains("const SQLITE_SAFE_BIND_LIMIT: usize = 900;"),
                "{path} should import the shared db::SQLITE_SAFE_BIND_LIMIT"
            );
        }
    }
}
