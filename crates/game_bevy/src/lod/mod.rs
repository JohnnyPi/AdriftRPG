//! Unified LOD policy loaded from world YAML.

use bevy::prelude::*;
use game_data::{
    CompiledContentLod, CompiledDistantLod, CompiledRenderDistanceLodTier, CompiledRenderProfile,
    CompiledTerrainLodTier, CompiledWorld, TerrainColliderLodDefinition,
};
use shared::StableId;
use voxel_core::ChunkCoord;

use crate::data::{ConfigRegistryResource, UserSetupPrefs};
use crate::state::AppState;
use crate::terrain::residency::chunk_chebyshev_distance;
use crate::ui::WorldTweaks;
use crate::world::requested_world_id;

pub struct LodPolicyPlugin;

impl Plugin for LodPolicyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LodPolicy>()
            .init_resource::<LodRuntimeState>()
            .add_systems(OnEnter(AppState::Running), init_lod_policy_from_registry)
            .add_systems(
                Update,
                refresh_lod_on_world_change.run_if(in_state(AppState::Running)),
            );
    }
}

fn refresh_lod_on_world_change(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    policy: ResMut<LodPolicy>,
    world_tweaks: ResMut<WorldTweaks>,
    mut last: Local<Option<String>>,
) {
    if last.as_ref() == Some(&prefs.world_id) {
        return;
    }
    *last = Some(prefs.world_id.clone());
    init_lod_policy_from_registry(registry, prefs, policy, world_tweaks);
}

/// Data-driven LOD policy compiled from the active world profile.
#[derive(Resource, Clone, Debug)]
pub struct LodPolicy {
    pub density_radius: i32,
    pub render_radius: i32,
    pub physics_radius: i32,
    pub decoration_radius: i32,
    pub high_detail_radius: i32,
    pub terrain_tiers: Vec<CompiledTerrainLodTier>,
    pub render_profile_id: StableId,
    pub render_distance_lod: Vec<CompiledRenderDistanceLodTier>,
    pub content: CompiledContentLod,
    pub distant: CompiledDistantLod,
    pub prefetch_chunks_ahead: i32,
    pub preload_atlas: bool,
    pub preload_material_arrays: bool,
    pub lod_hysteresis_frames: u32,
}

impl Default for LodPolicy {
    fn default() -> Self {
        let world = game_data::CompiledChunkResidency {
            density_radius: 10,
            render_radius: 7,
            physics_radius: 5,
            decoration_radius: 5,
            high_detail_radius: 4,
        };
        Self::from_world_residency(&world, &CompiledContentLod {
            vegetation_max_distance_m: 80.0,
            grass_lod: [25.0, 70.0, 140.0],
        }, &CompiledDistantLod {
            horizon_skirt: true,
            impostor_start_m: 400.0,
        }, &[], StableId::new("render.terrain_high"), None)
    }
}

impl LodPolicy {
    pub fn from_world(
        world: &CompiledWorld,
        render_profile: Option<&CompiledRenderProfile>,
    ) -> Self {
        Self::from_world_residency(
            &world.residency,
            &world.lod.content,
            &world.lod.distant,
            &world.lod.terrain,
            world.lod.materials.render_profile.clone(),
            render_profile,
        )
    }

    fn from_world_residency(
        residency: &game_data::CompiledChunkResidency,
        content: &CompiledContentLod,
        distant: &CompiledDistantLod,
        terrain: &[CompiledTerrainLodTier],
        render_profile_id: StableId,
        render_profile: Option<&CompiledRenderProfile>,
    ) -> Self {
        let render_distance_lod = render_profile
            .map(|p| p.distance_lod.clone())
            .unwrap_or_default();
        Self {
            density_radius: residency.density_radius,
            render_radius: residency.render_radius,
            physics_radius: residency.physics_radius,
            decoration_radius: residency.decoration_radius,
            high_detail_radius: residency.high_detail_radius,
            terrain_tiers: terrain.to_vec(),
            render_profile_id,
            render_distance_lod,
            content: content.clone(),
            distant: distant.clone(),
            prefetch_chunks_ahead: 2,
            preload_atlas: true,
            preload_material_arrays: true,
            lod_hysteresis_frames: 8,
        }
    }

    pub fn apply_to_world_tweaks(&self, tweaks: &mut WorldTweaks) {
        tweaks.density_radius = self.density_radius;
        tweaks.render_radius = self.render_radius;
        tweaks.physics_radius = self.physics_radius;
        tweaks.decoration_radius = self.decoration_radius;
        tweaks.high_detail_radius = self.high_detail_radius;
    }
}

/// Per-chunk LOD transition hysteresis state.
#[derive(Resource, Default, Debug)]
pub struct LodRuntimeState {
    pub chunk_tiers: std::collections::HashMap<ChunkCoord, ChunkLodState>,
}

#[derive(Clone, Debug)]
pub struct ChunkLodState {
    pub tier: u8,
    pub mesh_resolution_scale: f32,
    pub collider: TerrainColliderLodDefinition,
    pub pending_tier: Option<u8>,
    pub hysteresis: u32,
}

pub fn terrain_lod_for_distance(
    policy: &LodPolicy,
    center: ChunkCoord,
    coord: ChunkCoord,
) -> (u8, f32, TerrainColliderLodDefinition) {
    let dist = chunk_chebyshev_distance(center, coord);
    for (i, tier) in policy.terrain_tiers.iter().enumerate() {
        if dist <= tier.max_distance_chunks {
            return (
                i as u8,
                tier.mesh_resolution_scale,
                tier.collider,
            );
        }
    }
    let last = policy.terrain_tiers.last();
    (
        policy.terrain_tiers.len().saturating_sub(1) as u8,
        last.map(|t| t.mesh_resolution_scale).unwrap_or(0.25),
        last.map(|t| t.collider).unwrap_or(TerrainColliderLodDefinition::None),
    )
}

pub fn terrain_lod_with_hysteresis(
    policy: &LodPolicy,
    runtime: &mut LodRuntimeState,
    center: ChunkCoord,
    coord: ChunkCoord,
) -> (u8, f32, TerrainColliderLodDefinition) {
    let (target_tier, scale, collider) = terrain_lod_for_distance(policy, center, coord);
    let state = runtime
        .chunk_tiers
        .entry(coord)
        .or_insert_with(|| ChunkLodState {
            tier: target_tier,
            mesh_resolution_scale: scale,
            collider,
            pending_tier: None,
            hysteresis: 0,
        });

    if target_tier == state.tier {
        state.pending_tier = None;
        state.hysteresis = 0;
        state.mesh_resolution_scale = scale;
        state.collider = collider;
        return (state.tier, state.mesh_resolution_scale, state.collider);
    }

    if state.pending_tier != Some(target_tier) {
        state.pending_tier = Some(target_tier);
        state.hysteresis = 0;
    } else {
        state.hysteresis += 1;
    }

    if state.hysteresis >= policy.lod_hysteresis_frames {
        state.tier = target_tier;
        state.mesh_resolution_scale = scale;
        state.collider = collider;
        state.pending_tier = None;
        state.hysteresis = 0;
    }

    (state.tier, state.mesh_resolution_scale, state.collider)
}

pub fn mesh_cell_stride(mesh_resolution_scale: f32) -> u32 {
    if mesh_resolution_scale >= 0.99 {
        1
    } else if mesh_resolution_scale >= 0.49 {
        2
    } else {
        4
    }
}

pub fn render_lod_tier_for_distance(
    policy: &LodPolicy,
    distance_m: f32,
) -> Option<&CompiledRenderDistanceLodTier> {
    policy
        .render_distance_lod
        .iter()
        .find(|t| distance_m <= t.maximum_distance_m)
        .or_else(|| policy.render_distance_lod.last())
}

pub fn grass_lod_band(policy: &LodPolicy, distance_m: f32) -> u8 {
    let [near, mid, far] = policy.content.grass_lod;
    if distance_m <= near {
        0
    } else if distance_m <= mid {
        1
    } else if distance_m <= far {
        2
    } else {
        3
    }
}

fn init_lod_policy_from_registry(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    mut policy: ResMut<LodPolicy>,
    mut world_tweaks: ResMut<WorldTweaks>,
) {
    let world_id = requested_world_id(&prefs);
    let Ok(world) = registry.0.effective_world(Some(&world_id)) else {
        return;
    };
    let render_profile = registry.0.effective_render_profile(world);
    *policy = LodPolicy::from_world(world, render_profile);
    policy.prefetch_chunks_ahead = world.staging.prefetch_chunks_ahead;
    policy.preload_atlas = world.staging.preload_atlas;
    policy.preload_material_arrays = world.staging.preload_material_arrays;
    policy.apply_to_world_tweaks(&mut world_tweaks);
}
