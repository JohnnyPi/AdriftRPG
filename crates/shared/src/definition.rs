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
        if self.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(crate::DataError::UnsupportedSchemaVersion {
                id: self.id.clone(),
                found: self.schema_version,
                expected: SUPPORTED_SCHEMA_VERSION,
            });
        }
        Ok(())
    }
}
