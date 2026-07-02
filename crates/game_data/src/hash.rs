// crates/game_data/src/hash.rs
use sha2::{Digest, Sha256};

use crate::registry::ConfigRegistry;

pub fn registry_hash(registry: &ConfigRegistry) -> String {
    let bytes = serde_json::to_vec(registry).expect("registry canonical serialization");
    hex::encode(Sha256::digest(bytes))
}
