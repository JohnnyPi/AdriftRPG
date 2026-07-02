// crates/terrain_surface/src/material_id.rs
use std::fmt;

/// Stable string key for a terrain material (matches game_data palette keys).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MaterialKey(pub String);

impl MaterialKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MaterialKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for MaterialKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for MaterialKey {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}
