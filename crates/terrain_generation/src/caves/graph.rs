//! Cave graph data structures stored on the world atlas.

use serde::{Deserialize, Serialize};

use crate::contract::coordinates::WorldPosition;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CaveFamily {
    LavaTube,
    Limestone,
    SeaCave,
    Fracture,
    Talus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CaveNodeKind {
    Entrance,
    Chamber,
    Junction,
    Shaft,
    Pool,
    Squeeze,
    Terminus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaveNode {
    pub kind: CaveNodeKind,
    pub position: WorldPosition,
    pub radius_m: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct WallNoiseParams {
    pub frequency: f32,
    pub amplitude_m: f32,
}

impl Default for WallNoiseParams {
    fn default() -> Self {
        Self {
            frequency: 0.35,
            amplitude_m: 0.4,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaveEdge {
    pub from: usize,
    pub to: usize,
    pub radius_m: f32,
    pub noise: WallNoiseParams,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaveSystem {
    pub id: String,
    pub family: CaveFamily,
    pub nodes: Vec<CaveNode>,
    pub edges: Vec<CaveEdge>,
    pub entrance_world: [f64; 3],
    pub overhang_enabled: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CaveGraphRegistry {
    pub systems: Vec<CaveSystem>,
}

impl CaveGraphRegistry {
    pub fn system_count(&self) -> usize {
        self.systems.len()
    }

    pub fn traversable_system_count(&self) -> usize {
        self.systems
            .iter()
            .filter(|s| s.nodes.len() >= 2 && !s.edges.is_empty())
            .count()
    }
}
