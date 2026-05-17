use globset::{Glob, GlobSet, GlobSetBuilder};

pub struct EffectiveExcludes {
    set: GlobSet,
}

impl EffectiveExcludes {
    pub fn compile(extras: &[String]) -> Result<Self, globset::Error> {
        let mut builder = GlobSetBuilder::new();
        for raw in extras {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            builder.add(Glob::new(trimmed)?);
        }
        Ok(Self {
            set: builder.build()?,
        })
    }

    pub fn is_excluded(&self, path: &str) -> bool {
        self.set.is_match(path)
    }
}
