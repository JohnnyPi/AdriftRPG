//! Generator version and deterministic seed derivation.

use serde::{Deserialize, Serialize};

use super::coordinates::TileCoord;

/// Monotonic generator version included in manifests and seed derivation.
pub const GENERATOR_VERSION: GeneratorVersion = GeneratorVersion(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GeneratorVersion(pub u32);

/// Derive a stable local seed from world seed, namespace, optional tile, and local id.
pub fn derive_seed(
    world_seed: u64,
    namespace: &str,
    coordinate: Option<TileCoord>,
    local_id: u64,
) -> u64 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&world_seed.to_le_bytes());
    hasher.update(&GENERATOR_VERSION.0.to_le_bytes());
    hasher.update(namespace.as_bytes());
    if let Some(coord) = coordinate {
        hasher.update(&coord.x.to_le_bytes());
        hasher.update(&coord.z.to_le_bytes());
    }
    hasher.update(&local_id.to_le_bytes());
    let bytes = hasher.finalize();
    u64::from_le_bytes(bytes.as_bytes()[0..8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_is_stable() {
        let a = derive_seed(42, "boundary", None, 0);
        let b = derive_seed(42, "boundary", None, 0);
        assert_eq!(a, b);
    }

    #[test]
    fn seed_differs_by_namespace() {
        let a = derive_seed(42, "boundary", None, 0);
        let b = derive_seed(42, "island", None, 0);
        assert_ne!(a, b);
    }

    #[test]
    fn seed_differs_by_tile() {
        let a = derive_seed(42, "regional", Some(TileCoord { x: 0, z: 0 }), 0);
        let b = derive_seed(42, "regional", Some(TileCoord { x: 1, z: 0 }), 0);
        assert_ne!(a, b);
    }
}
