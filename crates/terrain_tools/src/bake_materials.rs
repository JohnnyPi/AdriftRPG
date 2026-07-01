use std::path::PathBuf;

use clap::Parser;
use procedural_textures::{
    build_cpu_arrays, document_fingerprint, ProceduralMaterialsDocument,
};

#[derive(Parser, Debug)]
#[command(name = "bake-materials")]
struct Args {
    #[arg(short, long, default_value = "assets/procedural/terrain/procedural_island.yaml")]
    input: PathBuf,

    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn cache_path_for(fingerprint: [u8; 32]) -> PathBuf {
    PathBuf::from("target/terrain_material_cache").join(format!("{}.bin", hex::encode(fingerprint)))
}

fn main() {
    let args = Args::parse();
    let text = std::fs::read_to_string(&args.input).expect("read input yaml");
    let text = procedural_textures::strip_utf8_bom(&text);
    let doc: ProceduralMaterialsDocument = serde_yaml::from_str(text).expect("parse yaml");
    let fingerprint = document_fingerprint(&doc);
    let arrays = build_cpu_arrays(&doc.materials).expect("build arrays");

    let cache_path = args.output.unwrap_or_else(|| cache_path_for(fingerprint));
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent).expect("create cache dir");
    }
    let bytes = bincode::serialize(&arrays).expect("serialize cache");
    std::fs::write(&cache_path, bytes).expect("write cache");

    println!(
        "Baked {} layers at {}x{} -> {}",
        arrays.layers,
        arrays.width,
        arrays.height,
        cache_path.display()
    );
    println!("Recipe fingerprint: {}", hex::encode(fingerprint));
}
