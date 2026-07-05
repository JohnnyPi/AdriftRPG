//! Spatial metadata for aligned 2D fields.

use serde::{Deserialize, Serialize};

use crate::contract::coordinates::{CellSizeMeters, WorldXZ};

use super::key::FieldKey;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum SampleLayout {
    CellCenter,
    CellCorner,
    VertexGrid,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum FieldValueKind {
    Scalar,
    Mask,
    Categorical,
    Vector,
    Index,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FieldDescriptor {
    pub key: FieldKey,
    pub origin_world: WorldXZ,
    pub cell_size_m: f64,
    pub width: u32,
    pub height: u32,
    pub sample_layout: SampleLayout,
    pub value_kind: FieldValueKind,
}

impl FieldDescriptor {
    pub fn new(
        key: FieldKey,
        origin_world: WorldXZ,
        cell_size_m: f64,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            key,
            origin_world,
            cell_size_m: cell_size_m,
            width,
            height,
            sample_layout: SampleLayout::CellCenter,
            value_kind: FieldValueKind::Scalar,
        }
    }

    pub fn with_kind(mut self, kind: FieldValueKind) -> Self {
        self.value_kind = kind;
        self
    }

    pub fn origin_x(&self) -> f64 {
        self.origin_world.x()
    }

    pub fn origin_z(&self) -> f64 {
        self.origin_world.z()
    }

    pub fn cell_size(&self) -> CellSizeMeters {
        CellSizeMeters(self.cell_size_m)
    }

    pub fn extent_x_m(&self) -> f64 {
        self.width as f64 * self.cell_size_m
    }

    pub fn extent_z_m(&self) -> f64 {
        self.height as f64 * self.cell_size_m
    }
}
