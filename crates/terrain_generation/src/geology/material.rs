//! Bedrock material identifiers.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum BedrockId {
    #[default]
    Ocean = 0,
    Basalt = 1,
    Tuff = 2,
    Ash = 3,
    WeatheredBasalt = 4,
    Limestone = 5,
}

impl BedrockId {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Ocean,
            1 => Self::Basalt,
            2 => Self::Tuff,
            3 => Self::Ash,
            4 => Self::WeatheredBasalt,
            5 => Self::Limestone,
            _ => Self::Basalt,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BedrockProperties {
    pub hardness: f32,
    pub erodibility: f32,
    pub permeability: f32,
}

impl BedrockId {
    pub fn default_properties(self) -> BedrockProperties {
        match self {
            Self::Ocean => BedrockProperties {
                hardness: 0.0,
                erodibility: 0.0,
                permeability: 1.0,
            },
            Self::Basalt => BedrockProperties {
                hardness: 0.85,
                erodibility: 0.25,
                permeability: 0.05,
            },
            Self::Tuff => BedrockProperties {
                hardness: 0.45,
                erodibility: 0.65,
                permeability: 0.15,
            },
            Self::Ash => BedrockProperties {
                hardness: 0.25,
                erodibility: 0.80,
                permeability: 0.25,
            },
            Self::WeatheredBasalt => BedrockProperties {
                hardness: 0.35,
                erodibility: 0.70,
                permeability: 0.20,
            },
            Self::Limestone => BedrockProperties {
                hardness: 0.40,
                erodibility: 0.55,
                permeability: 0.45,
            },
        }
    }
}

impl From<BedrockId> for u8 {
    fn from(value: BedrockId) -> Self {
        value as u8
    }
}
