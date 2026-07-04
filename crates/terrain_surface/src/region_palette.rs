// crates/terrain_surface/src/region_palette.rs
//! Material-region palette selection shared across neighboring chunks.
//!
//! Region palettes sit between the world [`MaterialLayerRegistry`](crate::MaterialLayerRegistry)
//! and per-chunk [`ChunkSlotPalette`](crate::chunk_palette::ChunkSlotPalette).

use std::collections::BTreeMap;

use crate::chunk_palette::{CHUNK_LOCAL_SLOT_COUNT, ChunkSlotPalette};
use crate::material_id::MaterialKey;

pub const DEFAULT_REGION_CHUNKS: u32 = 4;
pub const MAX_REGION_SURFACES: usize = CHUNK_LOCAL_SLOT_COUNT;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MaterialRegionCoord {
    pub x: i32,
    pub z: i32,
}

impl MaterialRegionCoord {
    pub fn from_chunk(chunk_x: i32, chunk_z: i32, region_chunks: u32) -> Self {
        let size = region_chunks.max(1) as i32;
        Self {
            x: chunk_x.div_euclid(size),
            z: chunk_z.div_euclid(size),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SurfaceCoverage {
    counts: BTreeMap<MaterialKey, f32>,
}

impl SurfaceCoverage {
    pub fn record(&mut self, material: MaterialKey, weight: f32) {
        *self.counts.entry(material).or_insert(0.0) += weight.max(0.0);
    }

    pub fn top_materials(&self, limit: usize) -> Vec<MaterialKey> {
        let mut ranked: Vec<_> = self.counts.iter().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked
            .into_iter()
            .take(limit)
            .map(|(key, _)| key.clone())
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct MaterialRegionPalette {
    pub region: MaterialRegionCoord,
    pub surfaces: Vec<MaterialKey>,
    pub global_layers: Vec<u32>,
}

impl MaterialRegionPalette {
    pub fn build(
        region: MaterialRegionCoord,
        coverage: &SurfaceCoverage,
        layer_lookup: &BTreeMap<MaterialKey, u32>,
        fallback: MaterialKey,
    ) -> Self {
        let mut surfaces = coverage.top_materials(MAX_REGION_SURFACES);
        if surfaces.is_empty() {
            surfaces.push(fallback.clone());
        }
        if !surfaces.iter().any(|m| m == &fallback) && surfaces.len() < MAX_REGION_SURFACES {
            surfaces.push(fallback);
        }
        let global_layers: Vec<u32> = surfaces
            .iter()
            .filter_map(|key| layer_lookup.get(key).copied())
            .collect();
        Self {
            region,
            surfaces,
            global_layers,
        }
    }

    pub fn chunk_slot_palette(&self) -> ChunkSlotPalette {
        let mut remapper = crate::chunk_palette::ChunkSlotRemapper::new();
        for &global in &self.global_layers {
            remapper.allocate_global(global);
        }
        remapper.finish()
    }
}

pub struct MaterialRegionPaletteCache {
    region_chunks: u32,
    palettes: BTreeMap<MaterialRegionCoord, MaterialRegionPalette>,
}

impl MaterialRegionPaletteCache {
    pub fn new(region_chunks: u32) -> Self {
        Self {
            region_chunks: region_chunks.max(1),
            palettes: BTreeMap::new(),
        }
    }

    pub fn region_chunks(&self) -> u32 {
        self.region_chunks
    }

    pub fn region_for_chunk(&self, chunk_x: i32, chunk_z: i32) -> MaterialRegionCoord {
        MaterialRegionCoord::from_chunk(chunk_x, chunk_z, self.region_chunks)
    }

    pub fn get(&self, region: &MaterialRegionCoord) -> Option<&MaterialRegionPalette> {
        self.palettes.get(region)
    }

    pub fn get_or_insert_with(
        &mut self,
        region: MaterialRegionCoord,
        build: impl FnOnce() -> MaterialRegionPalette,
    ) -> &MaterialRegionPalette {
        self.palettes.entry(region).or_insert_with(build)
    }
}
