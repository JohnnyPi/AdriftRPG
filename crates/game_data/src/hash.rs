use sha2::{Digest, Sha256};

use crate::registry::ConfigRegistry;

pub fn registry_hash(registry: &ConfigRegistry) -> String {
    let mut hasher = Sha256::new();
    hasher.update(registry.canonical_bytes());
    hex::encode(hasher.finalize())
}
