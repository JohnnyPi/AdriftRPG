// crates/terrain_tools/src/validate_catalog.rs
use std::path::PathBuf;

use clap::Parser;
use game_data::load_registry_from_directory;

#[derive(Parser, Debug)]
#[command(name = "validate-catalog")]
struct Args {
    #[arg(short, long, default_value = "assets")]
    assets: PathBuf,
}

fn main() {
    let args = Args::parse();
    let assets = args.assets.canonicalize().expect("assets path");
    let registry = load_registry_from_directory(&assets).expect("load registry");

    println!(
        "Registry hash {} — {} palettes, {} surface registries",
        registry.hash,
        registry.materials.len(),
        registry.surface_registries.len()
    );

    for (id, reg) in &registry.surface_registries {
        println!(
            "  {id}: {} textures, {} surfaces, {} overlays",
            reg.textures.len(),
            reg.surfaces.len(),
            reg.overlays.len()
        );
        for tex in &reg.textures {
            if tex.generator.is_none() && tex.graph.is_none() {
                eprintln!("ERROR: texture `{}` has no generator or graph", tex.id);
                std::process::exit(1);
            }
        }
    }

    for (id, palette) in &registry.materials {
        for key in &palette.layer_order {
            if game_data::is_deprecated_overlay_material(key.as_str()) {
                eprintln!(
                    "WARN: palette `{id}` uses deprecated overlay material `{key}` — prefer base surface + wetness overlay"
                );
            }
        }
    }

    println!("Catalog validation OK");
}
