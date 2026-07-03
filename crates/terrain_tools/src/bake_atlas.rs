// crates/terrain_tools/src/bake_atlas.rs
use std::path::{Path, PathBuf};

use clap::Parser;
use game_data::load_registry_from_directory;
use shared::StableId;
use terrain_generation::{
    build_island_atlas, island_params_from_compiled, validate_island_world_budget,
    write_baked_atlas,
};

#[derive(Parser, Debug)]
#[command(name = "bake-atlas")]
struct Args {
    /// World stable id (e.g. world.island_testbed).
    #[arg(short, long)]
    world: String,

    /// Assets root directory.
    #[arg(short, long, default_value = "assets")]
    assets: PathBuf,

    /// Output directory for the baked atlas archive.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Override seed (defaults to world seed).
    #[arg(long)]
    seed: Option<u64>,
}

fn default_output(assets: &Path, world_id: &str, seed: u64) -> PathBuf {
    let slug = world_id.strip_prefix("world.").unwrap_or(world_id);
    assets.join(format!("terrain/baked/{slug}.seed{seed}.atlas"))
}

fn main() {
    let args = Args::parse();
    let registry = load_registry_from_directory(&args.assets).expect("load registry");
    let world_id = StableId::new(&args.world);
    let world = registry.world_by_id(&world_id).expect("world");
    let water = registry.water.get(&world.water).expect("water");
    let island_gen = registry
        .island_generation_for_world(world)
        .expect("island_gen required for atlas bake");
    let seed = args.seed.unwrap_or(world.seed);

    let mut merged = island_gen.clone();
    merged.seed = seed;

    let budget = validate_island_world_budget(&merged, world, water.sea_level_m);
    if !budget.is_empty() {
        eprintln!("island/world budget validation failed:");
        for msg in &budget {
            eprintln!("  - {msg}");
        }
        std::process::exit(1);
    }

    let params = island_params_from_compiled(&merged, world, seed, water.sea_level_m);
    let atlas = build_island_atlas(&params);

    let output = args
        .output
        .unwrap_or_else(|| default_output(&args.assets, world.id.as_str(), seed));

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).expect("create output parent");
    }

    let manifest =
        write_baked_atlas(&output, &atlas, world.id.as_str()).expect("write baked atlas");

    println!("Baked atlas for {} (seed {})", world.id.as_str(), seed);
    println!("  output: {}", output.display());
    println!("  content_hash: {}", manifest.content_hash);
    println!("  validation_passed: {}", manifest.validation_passed);
    if !manifest.validation_messages.is_empty() {
        println!("  validation_messages:");
        for msg in &manifest.validation_messages {
            println!("    - {msg}");
        }
    }
    for (name, meta) in &manifest.fields {
        println!("  field {name}: {}x{} sha256={}", meta.width, meta.height, &meta.sha256[..16]);
    }
}
