// crates/procedural_textures/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum TextureGenerationError {
    #[error("texture dimensions must be non-zero")]
    ZeroDimension,

    #[error("texture dimensions exceed configured limit")]
    DimensionsTooLarge,

    #[error("generated buffer had invalid length")]
    InvalidBufferLength,

    #[error("invalid generator configuration: {0}")]
    InvalidConfig(String),
}
