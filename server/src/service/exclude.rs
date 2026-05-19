use globset::{Glob, GlobSet, GlobSetBuilder};
use std::sync::OnceLock;

const HARD_EXCLUDE_GLOBS: &[&str] = &[
    ".obsidian/workspace.json",
    ".obsidian/workspace-mobile.json",
    ".obsidian/workspaces.json",
    ".obsidian/cache/**",
    ".git/**",
    ".trash/**",
    ".conflict-*",
    "*.lock",
    "*.tmp",
];

pub struct EffectiveExcludes {
    set: GlobSet,
    has_patterns: bool,
}

impl EffectiveExcludes {
    pub fn compile(extras: &[String]) -> Result<Self, globset::Error> {
        let mut builder = GlobSetBuilder::new();
        let mut has_patterns = false;
        for raw in extras {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            builder.add(Glob::new(trimmed)?);
            has_patterns = true;
        }
        Ok(Self {
            set: builder.build()?,
            has_patterns,
        })
    }

    pub fn is_excluded(&self, path: &str) -> bool {
        self.set.is_match(path)
    }

    pub fn is_empty(&self) -> bool {
        !self.has_patterns
    }
}

pub struct SyncPathFilter {
    user_excludes: EffectiveExcludes,
    vault_allowlist: EffectiveExcludes,
}

impl SyncPathFilter {
    pub fn new(user_excludes: EffectiveExcludes, vault_allowlist: EffectiveExcludes) -> Self {
        Self {
            user_excludes,
            vault_allowlist,
        }
    }

    pub fn compile(
        user_exclude_globs: &[String],
        vault_allowlist_globs: &[String],
    ) -> Result<Self, globset::Error> {
        Ok(Self {
            user_excludes: EffectiveExcludes::compile(user_exclude_globs)?,
            vault_allowlist: EffectiveExcludes::compile(vault_allowlist_globs)?,
        })
    }

    pub fn path_accepts(&self, path: &str) -> bool {
        if is_hard_excluded(path) {
            return false;
        }
        if is_hidden_path(path)
            && (self.vault_allowlist.is_empty() || !self.vault_allowlist.is_excluded(path))
        {
            return false;
        }
        !self.user_excludes.is_excluded(path)
    }
}

pub fn is_hidden_path(path: &str) -> bool {
    path.split('/').any(|part| part.starts_with('.'))
}

pub fn is_hard_excluded(path: &str) -> bool {
    static HARD_EXCLUDES: OnceLock<EffectiveExcludes> = OnceLock::new();
    let hard_set = HARD_EXCLUDES.get_or_init(|| {
        let globs = HARD_EXCLUDE_GLOBS
            .iter()
            .map(|glob| (*glob).to_string())
            .collect::<Vec<_>>();
        EffectiveExcludes::compile(&globs).expect("hard excludes compile")
    });
    hard_set.is_excluded(path)
        || path
            .split('/')
            .any(|part| part == ".git" || part == ".trash" || part.starts_with(".conflict-"))
}
