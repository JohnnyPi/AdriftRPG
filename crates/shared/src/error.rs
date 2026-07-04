// crates/shared/src/error.rs
use crate::StableId;

#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error("failed to read `{path}`: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse `{path}`: {message}")]
    Parse { path: String, message: String },

    #[error("definition `{id}` uses unsupported schema version {found}; expected {expected}")]
    UnsupportedSchemaVersion {
        id: StableId,
        found: u32,
        expected: u32,
    },

    #[error(
        "duplicate definition id `{id}` (first seen in `{first_path}`, also in `{duplicate_path}`)"
    )]
    DuplicateId {
        id: StableId,
        first_path: String,
        duplicate_path: String,
    },

    #[error("unknown reference `{reference}` in `{context}`")]
    UnknownReference {
        reference: StableId,
        context: String,
    },

    #[error("invalid value in `{context}`: {message}")]
    InvalidValue { context: String, message: String },

    #[error("registry validation failed with {count} error(s):\n{details}")]
    ValidationFailed { count: usize, details: String },
}

pub type DataResult<T> = Result<T, DataError>;
