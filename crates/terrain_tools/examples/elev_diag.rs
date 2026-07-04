//! Offline elevation probe for the default island atlas (run with `cargo run -p terrain_tools --example elev_diag`).
use terrain_generation::{IslandGenParams, RecipeDensitySource, TerrainRecipe, build_island_atlas};

fn main() {
    let params = IslandGenParams::default();
    let atlas = build_island_atlas(&params);
    let mut max_comp = f32::MIN;
    let mut max_reg = f32::MIN;
    let mut min_comp = f32::MAX;
    let mut land_cells = 0u32;
    for z in 0..atlas.height() {
        for x in 0..atlas.width() {
            let wx = atlas.origin[0] + x as f32 * atlas.spacing_m();
            let wz = atlas.origin[1] + z as f32 * atlas.spacing_m();
            let mask = atlas.island_mask.sample_bilinear(wx, wz);
            let comp = atlas.composed_land_elevation_at(wx, wz);
            let reg = atlas.elevation_regional.sample_bilinear(wx, wz);
            if mask > 0.5 {
                land_cells += 1;
                max_comp = max_comp.max(comp);
                max_reg = max_reg.max(reg);
                min_comp = min_comp.min(comp);
            }
        }
    }
    let recipe = TerrainRecipe {
        seed: params.seed,
        sea_level: 0.0,
        spawn_x: 70.0,
        spawn_z: 160.0,
        coord_offset: [128.0, 0.0, 128.0],
        ops: vec![],
    };
    let source = RecipeDensitySource::new(recipe).with_atlas(atlas, 3.5);
    let peak = source.terrain_surface_height_at(0.0, 0.0);
    let spawn_h = source.terrain_surface_height_at(-58.0, 32.0);
    println!(
        "land_cells={land_cells} max_comp={max_comp:.2} max_reg={max_reg:.2} min_comp={min_comp:.2}"
    );
    println!("peak_center={peak:.2} spawn_h={spawn_h:.2}");
    println!(
        "resolution regional={} local={}",
        params.resolution.regional_m, params.resolution.local_m
    );
}
