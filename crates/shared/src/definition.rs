// crates/shared/src/definition.rs
use serde::{Deserialize, Serialize};

pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// Header present on every YAML definition file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefinitionHeader {
    pub schema_version: u32,
    pub id: crate::StableId,
}

impl DefinitionHeader {
    pub fn validate(&self) -> crate::DataResult<()> {
        self.validate_schema(&[SUPPORTED_SCHEMA_VERSION])
    }

    pub fn validate_schema(&self, allowed: &[u32]) -> crate::DataResult<()> {
        if !allowed.contains(&self.schema_version) {
            return Err(crate::DataError::UnsupportedSchemaVersion {
                id: self.id.clone(),
                found: self.schema_version,
                expected: *allowed.first().expect("allowed schema versions"),
            });
        }
        Ok(())
    }
}
