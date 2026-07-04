// crates/terrain_surface/src/chunk_palette.rs
use std::collections::BTreeMap;

pub const CHUNK_LOCAL_SLOT_COUNT: usize = 8;
pub const UNUSED_SLOT: u32 = u32::MAX;

/// Per-chunk mapping from local vertex slot (0..7) to global texture-array layer.
///
/// See crate-level docs for where this sits in the palette hierarchy.
#[derive(Clone, Copy, Debug)]
pub struct ChunkSlotPalette {
    local_to_global: [u32; CHUNK_LOCAL_SLOT_COUNT],
    slot_count: u8,
}

impl Default for ChunkSlotPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl ChunkSlotPalette {
    pub fn new() -> Self {
        Self {
            local_to_global: [UNUSED_SLOT; CHUNK_LOCAL_SLOT_COUNT],
            slot_count: 0,
        }
    }

    pub fn slot_count(&self) -> u8 {
        self.slot_count
    }

    pub fn local_to_global(&self) -> &[u32; CHUNK_LOCAL_SLOT_COUNT] {
        &self.local_to_global
    }

    pub fn global_for_local(&self, local: u8) -> Option<u32> {
        let idx = local as usize;
        if idx >= CHUNK_LOCAL_SLOT_COUNT {
            return None;
        }
        let global = self.local_to_global[idx];
        if global == UNUSED_SLOT {
            None
        } else {
            Some(global)
        }
    }
}

/// Assigns global texture layers to chunk-local slots during meshing.
#[derive(Clone, Debug, Default)]
pub struct ChunkSlotRemapper {
    global_to_local: BTreeMap<u32, u8>,
    slot_weights: [f32; CHUNK_LOCAL_SLOT_COUNT],
    palette: ChunkSlotPalette,
}

impl ChunkSlotRemapper {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allocate_global(&mut self, global_layer: u32, weight: f32) -> u8 {
        if let Some(&local) = self.global_to_local.get(&global_layer) {
            self.slot_weights[local as usize] += weight.max(0.0);
            return local;
        }
        let local = self.palette.slot_count;
        if local as usize >= CHUNK_LOCAL_SLOT_COUNT {
            let best = self.slot_weights[..self.palette.slot_count as usize]
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(idx, _)| idx as u8)
                .unwrap_or(0);
            self.slot_weights[best as usize] += weight.max(0.0);
            return best;
        }
        self.palette.local_to_global[local as usize] = global_layer;
        self.palette.slot_count = local.saturating_add(1);
        self.global_to_local.insert(global_layer, local);
        self.slot_weights[local as usize] = weight.max(0.0);
        local
    }

    pub fn palette_snapshot(&self) -> ChunkSlotPalette {
        self.palette
    }

    pub fn finish(self) -> ChunkSlotPalette {
        self.palette
    }
}
