use std::collections::BTreeSet;

use crate::ChunkCoord;

/// Determines which chunks should be loaded or simulated (streaming stub).
pub trait ChunkInterestProvider: Send + Sync {
    fn desired_chunks(&self) -> BTreeSet<ChunkCoord>;
}

/// Returns every chunk in a fixed world extent (vertical-slice default).
pub struct FullExtentInterestProvider {
    pub extent: [i32; 3],
}

impl ChunkInterestProvider for FullExtentInterestProvider {
    fn desired_chunks(&self) -> BTreeSet<ChunkCoord> {
        let [ex, ey, ez] = self.extent;
        let half_x = ex / 2;
        let half_y = ey / 2;
        let half_z = ez / 2;
        let mut out = BTreeSet::new();
        for x in -half_x..(-half_x + ex) {
            for y in -half_y..(-half_y + ey) {
                for z in -half_z..(-half_z + ez) {
                    out.insert(ChunkCoord::new(x, y, z));
                }
            }
        }
        out
    }
}

/// Simulation LOD stub for distant terrain abstraction.
pub trait SimulationLodProvider: Send + Sync {
    fn detail_radius_chunks(&self) -> i32 {
        8
    }
}

pub struct DefaultSimulationLod;

impl SimulationLodProvider for DefaultSimulationLod {}
