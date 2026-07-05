//! World compiler errors.

use thiserror::Error;

use super::pass::PassKey;

#[derive(Debug, Error)]
pub enum WorldgenError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("missing prerequisite for pass {pass:?}: {missing}")]
    MissingPrerequisite {
        pass: PassKey,
        missing: &'static str,
    },
    #[error("compile failed in pass {pass:?}: {message}")]
    PassFailed { pass: PassKey, message: String },
    #[error("game data error: {0}")]
    GameData(#[from] game_data::WorldgenValidationError),
}
