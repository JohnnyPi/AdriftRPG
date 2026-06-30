use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::{ChunkCoord, MaterialId, TerrainSample, WorldSample};

/// A single density/material override at a world sample corner.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DensityDelta {
    pub world_x: i32,
    pub world_y: i32,
    pub world_z: i32,
    pub density: f32,
    pub material: MaterialId,
}

/// Serializable edit commands for terrain modification.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TerrainEditCommand {
    SubtractSphere {
        center: [f32; 3],
        radius_m: f32,
    },
    AddSphere {
        center: [f32; 3],
        radius_m: f32,
    },
    PaintMaterial {
        center: [f32; 3],
        radius_m: f32,
        material: MaterialId,
    },
}

/// Per-chunk list of sample overrides (for persistence stubs).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChunkDelta {
    pub coord: ChunkCoord,
    pub edits: Vec<DensityDelta>,
}

/// Runtime overlay applied on top of procedural density sampling.
#[derive(Clone, Debug, Default)]
pub struct TerrainEditStore {
    /// World-sample corner → overridden terrain sample.
    overrides: HashMap<(i32, i32, i32), TerrainSample>,
}

impl TerrainEditStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }

    pub fn sample_override(&self, wx: i32, wy: i32, wz: i32) -> Option<TerrainSample> {
        self.overrides.get(&(wx, wy, wz)).copied()
    }

    pub fn apply_command(
        &mut self,
        command: &TerrainEditCommand,
        procedural_density: impl Fn(i32, i32, i32) -> f32,
        procedural_material: impl Fn(i32, i32, i32, f32) -> MaterialId,
    ) -> BTreeSet<ChunkCoord> {
        match command {
            TerrainEditCommand::SubtractSphere { center, radius_m } => {
                self.apply_sphere(*center, *radius_m, true, &procedural_density, &procedural_material)
            }
            TerrainEditCommand::AddSphere { center, radius_m } => {
                self.apply_sphere(*center, *radius_m, false, &procedural_density, &procedural_material)
            }
            TerrainEditCommand::PaintMaterial {
                center,
                radius_m,
                material,
            } => self.apply_paint(*center, *radius_m, *material, &procedural_density),
        }
    }

    fn apply_sphere(
        &mut self,
        center: [f32; 3],
        radius_m: f32,
        subtract: bool,
        procedural_density: &impl Fn(i32, i32, i32) -> f32,
        procedural_material: &impl Fn(i32, i32, i32, f32) -> MaterialId,
    ) -> BTreeSet<ChunkCoord> {
        let center = Vec3::from_array(center);
        let min = center - Vec3::splat(radius_m + 1.0);
        let max = center + Vec3::splat(radius_m + 1.0);
        let mut affected = BTreeSet::new();

        for wx in min.x.floor() as i32..=max.x.ceil() as i32 {
            for wy in min.y.floor() as i32..=max.y.ceil() as i32 {
                for wz in min.z.floor() as i32..=max.z.ceil() as i32 {
                    let world = WorldSample::new(wx, wy, wz);
                    let pos = Vec3 {
                        x: wx as f32,
                        y: wy as f32,
                        z: wz as f32,
                    };
                    let dist = pos.distance(center);
                    if dist > radius_m {
                        continue;
                    }
                    let t = 1.0 - dist / radius_m;
                    let strength = t * t;
                    let base = self
                        .overrides
                        .get(&(wx, wy, wz))
                        .map(|s| s.density)
                        .unwrap_or_else(|| procedural_density(wx, wy, wz));
                    let density = if subtract {
                        base + strength * 2.5
                    } else {
                        base - strength * 2.5
                    };
                    let material = procedural_material(wx, wy, wz, density);
                    self.overrides
                        .insert((wx, wy, wz), TerrainSample { density, material });
                    affected.insert(world.chunk_coord());
                }
            }
        }
        expand_neighbor_chunks(&mut affected);
        affected
    }

    fn apply_paint(
        &mut self,
        center: [f32; 3],
        radius_m: f32,
        material: MaterialId,
        procedural_density: &impl Fn(i32, i32, i32) -> f32,
    ) -> BTreeSet<ChunkCoord> {
        let center = Vec3::from_array(center);
        let min = center - Vec3::splat(radius_m + 1.0);
        let max = center + Vec3::splat(radius_m + 1.0);
        let mut affected = BTreeSet::new();

        for wx in min.x.floor() as i32..=max.x.ceil() as i32 {
            for wy in min.y.floor() as i32..=max.y.ceil() as i32 {
                for wz in min.z.floor() as i32..=max.z.ceil() as i32 {
                    let pos = Vec3 {
                        x: wx as f32,
                        y: wy as f32,
                        z: wz as f32,
                    };
                    if pos.distance(center) > radius_m {
                        continue;
                    }
                    let density = self
                        .overrides
                        .get(&(wx, wy, wz))
                        .map(|s| s.density)
                        .unwrap_or_else(|| procedural_density(wx, wy, wz));
                    if density > 0.0 {
                        continue;
                    }
                    self.overrides.insert(
                        (wx, wy, wz),
                        TerrainSample {
                            density,
                            material,
                        },
                    );
                    affected.insert(WorldSample::new(wx, wy, wz).chunk_coord());
                }
            }
        }
        expand_neighbor_chunks(&mut affected);
        affected
    }

    pub fn chunk_deltas(&self) -> Vec<ChunkDelta> {
        let mut by_chunk: BTreeMap<ChunkCoord, Vec<DensityDelta>> = BTreeMap::new();
        for ((wx, wy, wz), sample) in &self.overrides {
            let coord = WorldSample::new(*wx, *wy, *wz).chunk_coord();
            by_chunk.entry(coord).or_default().push(DensityDelta {
                world_x: *wx,
                world_y: *wy,
                world_z: *wz,
                density: sample.density,
                material: sample.material,
            });
        }
        by_chunk
            .into_iter()
            .map(|(coord, edits)| ChunkDelta { coord, edits })
            .collect()
    }
}

fn expand_neighbor_chunks(chunks: &mut BTreeSet<ChunkCoord>) {
    let base: Vec<_> = chunks.iter().copied().collect();
    for coord in base {
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    chunks.insert(ChunkCoord::new(
                        coord.x + dx,
                        coord.y + dy,
                        coord.z + dz,
                    ));
                }
            }
        }
    }
}

/// Lightweight vec3 for voxel_core (no glam dependency).
#[derive(Clone, Copy, Debug)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    fn from_array(a: [f32; 3]) -> Self {
        Self {
            x: a[0],
            y: a[1],
            z: a[2],
        }
    }

    fn splat(v: f32) -> Self {
        Self { x: v, y: v, z: v }
    }

    fn distance(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CHUNK_CELLS;

    #[test]
    fn subtract_sphere_marks_neighbor_chunks() {
        let mut store = TerrainEditStore::new();
        let proc = |_x, _y, _z| -1.0f32;
        let mat = |_x, _y, _z, _d| MaterialId(0);
        let center = [CHUNK_CELLS as f32 - 0.5, 8.0, CHUNK_CELLS as f32 - 0.5];
        let affected = store.apply_command(
            &TerrainEditCommand::SubtractSphere {
                center,
                radius_m: 2.0,
            },
            proc,
            mat,
        );
        assert!(affected.contains(&ChunkCoord::new(0, 0, 0)));
        assert!(affected.contains(&ChunkCoord::new(1, 0, 1)));
    }

    #[test]
    fn shared_face_samples_match_after_edit() {
        let mut store = TerrainEditStore::new();
        let proc = |_x, _y, _z| -1.0f32;
        let mat = |_x, _y, _z, _d| MaterialId(0);
        let center = [CHUNK_CELLS as f32, 8.0, 8.0];
        store.apply_command(
            &TerrainEditCommand::SubtractSphere {
                center,
                radius_m: 3.0,
            },
            proc,
            mat,
        );
        let face_x = CHUNK_CELLS as i32;
        let sample = store
            .sample_override(face_x, 8, 8)
            .expect("face sample edited");
        assert!(sample.density > 0.0, "subtract should push toward air");
    }
}
