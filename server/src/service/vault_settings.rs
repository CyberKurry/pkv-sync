use crate::db::repos::VaultSettingsRepo;
use crate::service::AppState;

const EXTRA_SYNC_GLOBS_KEY: &str = "extra_sync_globs";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VaultSettings {
    pub extra_sync_globs: Vec<String>,
}

pub fn starter_extra_sync_globs() -> Vec<String> {
    [
        ".obsidian/themes/**",
        ".obsidian/snippets/**",
        ".obsidian/hotkeys.json",
        ".obsidian/app.json",
        ".obsidian/appearance.json",
        ".obsidian/community-plugins.json",
        ".obsidian/core-plugins.json",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

pub async fn load(state: &AppState, vault_id: &str) -> Result<VaultSettings, sqlx::Error> {
    let raw = state.vault_settings.load_for_vault(vault_id).await?;
    let extra_sync_globs = raw
        .get(EXTRA_SYNC_GLOBS_KEY)
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .unwrap_or_default();
    Ok(VaultSettings { extra_sync_globs })
}

pub async fn save(
    state: &AppState,
    vault_id: &str,
    settings: &VaultSettings,
) -> Result<(), sqlx::Error> {
    let extra_sync_globs =
        serde_json::to_string(&settings.extra_sync_globs).expect("string vector serializes");
    state
        .vault_settings
        .set(vault_id, EXTRA_SYNC_GLOBS_KEY, &extra_sync_globs)
        .await?;
    state.invalidate_vault_path_filter_cache(vault_id);
    Ok(())
}

#[cfg(test)]
pub async fn load_raw_for_tests(
    state: &AppState,
    vault_id: &str,
) -> Result<std::collections::HashMap<String, String>, sqlx::Error> {
    state.vault_settings.load_for_vault(vault_id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool;
    use crate::db::repos::{NewUser, UserRepo, VaultRepo};

    async fn state_and_vault() -> (AppState, String, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let p = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        let state = AppState::new(p, tmp.path().to_path_buf(), "test".into(), true)
            .await
            .unwrap();
        let user = state
            .users
            .create(NewUser {
                username: "cyberkurry".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        let vault = state.vaults.create(&user.id, "main").await.unwrap();
        (state, vault.id, tmp)
    }

    #[tokio::test]
    async fn load_defaults_to_empty_for_vault_without_settings() {
        let (state, vault_id, _tmp) = state_and_vault().await;

        assert_eq!(
            load(&state, &vault_id).await.unwrap(),
            VaultSettings::default()
        );
    }

    #[tokio::test]
    async fn save_invalidates_cached_vault_path_filter() {
        let (state, vault_id, _tmp) = state_and_vault().await;
        let filter = crate::service::exclude::SyncPathFilter::compile(&[], &[]).unwrap();
        state.cache_vault_path_filter(&vault_id, &[], filter);
        assert!(state
            .cached_vault_path_filter(&vault_id, &[], std::time::Duration::from_secs(300))
            .is_some());

        save(
            &state,
            &vault_id,
            &VaultSettings {
                extra_sync_globs: vec!["notes/**".into()],
            },
        )
        .await
        .unwrap();

        assert!(state
            .cached_vault_path_filter(&vault_id, &[], std::time::Duration::from_secs(300))
            .is_none());
    }
}
