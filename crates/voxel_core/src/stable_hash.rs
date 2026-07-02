// crates/voxel_core/src/stable_hash.rs
//! Stable FNV-1a hashing for cross-toolchain regression comparisons.

pub const FNV_OFFSET: u64 = 0xcbf29ce484222325;
pub const FNV_PRIME: u64 = 0x100000001b3;

pub fn fnv1a_update(mut hash: u64, bytes: impl AsRef<[u8]>) -> u64 {
    for byte in bytes.as_ref() {
        hash = (hash ^ *byte as u64).wrapping_mul(FNV_PRIME);
    }
    hash
}

pub fn fnv1a_hash(bytes: impl AsRef<[u8]>) -> u64 {
    fnv1a_update(FNV_OFFSET, bytes)
}

/// Quantize density to ~1 mm so bit-level float noise does not change identity.
pub fn quantize_density_mm(density: f32) -> u32 {
    (density * 1024.0).round() as i32 as u32
}
