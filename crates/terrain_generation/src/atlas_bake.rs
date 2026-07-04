//! Serialize / deserialize baked island atlases for golden-reference regression.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::field2d::Field2D;
use crate::island_atlas::{BiomeWeights, IslandAtlas};
use crate::resolution::GenerationResolution;
use crate::water_body::{RiverControlPoint, RiverSpline};

pub const ATLAS_BAKE_SCHEMA_VERSION: u32 = 1;
pub const MANIFEST_FILENAME: &str = "manifest.yaml";
pub const RIVER_GRAPH_FILENAME: &str = "river_graph.json";

const ZSTD_LEVEL: i32 = 3;

/// Scalar raster fields stored as compressed little-endian `f32` blobs.
pub const SCALAR_FIELD_NAMES: &[&str] = &[
    "elevation_regional",
    "elevation_local",
    "bathymetry",
    "island_mask",
    "slope",
    "coast_distance",
    "filled_elevation",
    "flow_accumulation",
    "river_mask",
    "wetness",
    "sediment",
    "cliff_mask",
    "beach_mask",
    "soil_depth",
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ResolutionManifest {
    pub world_control_m: f32,
    pub regional_m: f32,
    pub local_m: f32,
    pub voxel_m: f32,
}

impl From<GenerationResolution> for ResolutionManifest {
    fn from(r: GenerationResolution) -> Self {
        Self {
            world_control_m: r.world_control_m,
            regional_m: r.regional_m,
            local_m: r.local_m,
            voxel_m: r.voxel_m,
        }
    }
}

impl From<ResolutionManifest> for GenerationResolution {
    fn from(r: ResolutionManifest) -> Self {
        Self {
            world_control_m: r.world_control_m,
            regional_m: r.regional_m,
            local_m: r.local_m,
            voxel_m: r.voxel_m,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FieldTierMeta {
    pub width: u32,
    pub height: u32,
    pub origin: [f32; 2],
    pub spacing_m: f32,
    pub sha256: String,
    pub blob: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AtlasBakeManifest {
    pub schema_version: u32,
    pub world_id: String,
    pub seed: u64,
    pub sea_level_m: f32,
    pub voxel_amplitude_m: f32,
    pub origin: [f32; 2],
    pub resolution: ResolutionManifest,
    pub validation_passed: bool,
    pub validation_messages: Vec<String>,
    pub content_hash: String,
    pub fields: BTreeMap<String, FieldTierMeta>,
    pub has_river_graph: bool,
}

#[derive(Debug)]
pub enum AtlasBakeError {
    Io(io::Error),
    Parse(String),
    SchemaMismatch { expected: u32, found: u32 },
    WorldMismatch { expected: String, found: String },
    SeedMismatch { expected: u64, found: u64 },
    HashMismatch { field: String },
    MissingField(String),
}

impl From<io::Error> for AtlasBakeError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl std::fmt::Display for AtlasBakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Parse(msg) => write!(f, "{msg}"),
            Self::SchemaMismatch { expected, found } => {
                write!(
                    f,
                    "atlas schema version mismatch: expected {expected}, found {found}"
                )
            }
            Self::WorldMismatch { expected, found } => {
                write!(
                    f,
                    "atlas world mismatch: expected {expected}, found {found}"
                )
            }
            Self::SeedMismatch { expected, found } => {
                write!(f, "atlas seed mismatch: expected {expected}, found {found}")
            }
            Self::HashMismatch { field } => write!(f, "atlas field hash mismatch: {field}"),
            Self::MissingField(name) => write!(f, "atlas missing field: {name}"),
        }
    }
}

impl std::error::Error for AtlasBakeError {}

#[derive(Serialize, Deserialize)]
struct RiverGraphFile {
    points: Vec<RiverControlPointFile>,
}

#[derive(Serialize, Deserialize)]
struct RiverControlPointFile {
    position_xz: [f32; 2],
    bed_elevation: f32,
    water_elevation: f32,
    width: f32,
    depth: f32,
    discharge: f32,
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn f32_bytes(field: &Field2D<f32>) -> Vec<u8> {
    let mut out = Vec::with_capacity(field.samples.len() * 4);
    for sample in &field.samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

fn biome_bytes(field: &Field2D<BiomeWeights>) -> Vec<u8> {
    let mut out = Vec::with_capacity(field.samples.len() * 5 * 4);
    for b in &field.samples {
        for channel in [
            b.rainforest,
            b.grassland,
            b.volcanic_rock,
            b.beach,
            b.wetland,
        ] {
            out.extend_from_slice(&channel.to_le_bytes());
        }
    }
    out
}

fn u8_bytes(field: &Field2D<u8>) -> Vec<u8> {
    field.samples.clone()
}

fn write_compressed_blob(dir: &Path, blob_name: &str, raw: &[u8]) -> io::Result<()> {
    let compressed = zstd::encode_all(raw, ZSTD_LEVEL)?;
    fs::write(dir.join(blob_name), compressed)
}

fn read_compressed_blob(dir: &Path, blob_name: &str) -> Result<Vec<u8>, AtlasBakeError> {
    let path = dir.join(blob_name);
    let compressed = fs::read(&path)
        .map_err(|_| AtlasBakeError::MissingField(format!("{} ({})", blob_name, path.display())))?;
    zstd::decode_all(&compressed[..])
        .map_err(|e| AtlasBakeError::Parse(format!("zstd decode {blob_name}: {e}")))
}

fn write_field_meta(
    dir: &Path,
    name: &str,
    field: &Field2D<f32>,
    fields: &mut BTreeMap<String, FieldTierMeta>,
) -> io::Result<()> {
    let raw = f32_bytes(field);
    let blob = format!("{name}.f32.zst");
    write_compressed_blob(dir, &blob, &raw)?;
    fields.insert(
        name.to_string(),
        FieldTierMeta {
            width: field.width,
            height: field.height,
            origin: field.origin,
            spacing_m: field.spacing,
            sha256: sha256_hex(&raw),
            blob,
        },
    );
    Ok(())
}

fn load_f32_field(dir: &Path, meta: &FieldTierMeta) -> Result<Field2D<f32>, AtlasBakeError> {
    let raw = read_compressed_blob(dir, &meta.blob)?;
    if sha256_hex(&raw) != meta.sha256 {
        return Err(AtlasBakeError::HashMismatch {
            field: meta.blob.clone(),
        });
    }
    let expected = (meta.width as usize) * (meta.height as usize);
    if raw.len() != expected * 4 {
        return Err(AtlasBakeError::Parse(format!(
            "{} byte length {} != {} samples",
            meta.blob,
            raw.len(),
            expected
        )));
    }
    let mut samples = Vec::with_capacity(expected);
    for chunk in raw.chunks_exact(4) {
        samples.push(f32::from_le_bytes(chunk.try_into().unwrap()));
    }
    Ok(Field2D {
        width: meta.width,
        height: meta.height,
        origin: meta.origin,
        spacing: meta.spacing_m,
        samples,
    })
}

fn write_biome_field(
    dir: &Path,
    field: &Field2D<BiomeWeights>,
    fields: &mut BTreeMap<String, FieldTierMeta>,
) -> io::Result<()> {
    let name = "biome_weights";
    let raw = biome_bytes(field);
    let blob = format!("{name}.f32.zst");
    write_compressed_blob(dir, &blob, &raw)?;
    fields.insert(
        name.to_string(),
        FieldTierMeta {
            width: field.width,
            height: field.height,
            origin: field.origin,
            spacing_m: field.spacing,
            sha256: sha256_hex(&raw),
            blob,
        },
    );
    Ok(())
}

fn load_biome_field(
    dir: &Path,
    meta: &FieldTierMeta,
) -> Result<Field2D<BiomeWeights>, AtlasBakeError> {
    let raw = read_compressed_blob(dir, &meta.blob)?;
    if sha256_hex(&raw) != meta.sha256 {
        return Err(AtlasBakeError::HashMismatch {
            field: meta.blob.clone(),
        });
    }
    let cell_count = (meta.width as usize) * (meta.height as usize);
    if raw.len() != cell_count * 5 * 4 {
        return Err(AtlasBakeError::Parse(format!(
            "biome_weights byte length mismatch"
        )));
    }
    let mut samples = Vec::with_capacity(cell_count);
    for cell in raw.chunks_exact(5 * 4) {
        let read_f32 =
            |offset: usize| f32::from_le_bytes(cell[offset..offset + 4].try_into().unwrap());
        samples.push(BiomeWeights {
            rainforest: read_f32(0),
            grassland: read_f32(4),
            volcanic_rock: read_f32(8),
            beach: read_f32(12),
            wetland: read_f32(16),
        });
    }
    Ok(Field2D {
        width: meta.width,
        height: meta.height,
        origin: meta.origin,
        spacing: meta.spacing_m,
        samples,
    })
}

fn write_u8_field(
    dir: &Path,
    name: &str,
    field: &Field2D<u8>,
    fields: &mut BTreeMap<String, FieldTierMeta>,
) -> io::Result<()> {
    let raw = u8_bytes(field);
    let blob = format!("{name}.u8.zst");
    write_compressed_blob(dir, &blob, &raw)?;
    fields.insert(
        name.to_string(),
        FieldTierMeta {
            width: field.width,
            height: field.height,
            origin: field.origin,
            spacing_m: field.spacing,
            sha256: sha256_hex(&raw),
            blob,
        },
    );
    Ok(())
}

fn load_u8_field(dir: &Path, meta: &FieldTierMeta) -> Result<Field2D<u8>, AtlasBakeError> {
    let raw = read_compressed_blob(dir, &meta.blob)?;
    if sha256_hex(&raw) != meta.sha256 {
        return Err(AtlasBakeError::HashMismatch {
            field: meta.blob.clone(),
        });
    }
    let expected = (meta.width as usize) * (meta.height as usize);
    if raw.len() != expected {
        return Err(AtlasBakeError::Parse(format!(
            "{} u8 length mismatch",
            meta.blob
        )));
    }
    Ok(Field2D {
        width: meta.width,
        height: meta.height,
        origin: meta.origin,
        spacing: meta.spacing_m,
        samples: raw,
    })
}

fn river_to_file(river: &RiverSpline) -> RiverGraphFile {
    RiverGraphFile {
        points: river
            .points
            .iter()
            .map(|p| RiverControlPointFile {
                position_xz: p.position_xz,
                bed_elevation: p.bed_elevation,
                water_elevation: p.water_elevation,
                width: p.width,
                depth: p.depth,
                discharge: p.discharge,
            })
            .collect(),
    }
}

fn river_from_file(file: RiverGraphFile) -> RiverSpline {
    RiverSpline {
        points: file
            .points
            .into_iter()
            .map(|p| RiverControlPoint {
                position_xz: p.position_xz,
                bed_elevation: p.bed_elevation,
                water_elevation: p.water_elevation,
                width: p.width,
                depth: p.depth,
                discharge: p.discharge,
            })
            .collect(),
    }
}

/// Fingerprint of all raster samples (for regression tests).
pub fn atlas_content_hash(atlas: &IslandAtlas) -> String {
    let mut hasher = Sha256::new();
    hasher.update(atlas.seed.to_le_bytes());
    hasher.update(atlas.sea_level_m.to_le_bytes());
    for name in SCALAR_FIELD_NAMES {
        let field = scalar_field_ref(atlas, name);
        hasher.update(name.as_bytes());
        for sample in &field.samples {
            hasher.update(sample.to_le_bytes());
        }
    }
    hasher.update(b"biome_weights");
    for b in &atlas.biome_weights.samples {
        for channel in [
            b.rainforest,
            b.grassland,
            b.volcanic_rock,
            b.beach,
            b.wetland,
        ] {
            hasher.update(channel.to_le_bytes());
        }
    }
    hasher.update(b"flow_direction");
    hasher.update(&atlas.flow_direction.samples);
    hex::encode(hasher.finalize())
}

fn scalar_field_ref<'a>(atlas: &'a IslandAtlas, name: &str) -> &'a Field2D<f32> {
    match name {
        "elevation_regional" => &atlas.elevation_regional,
        "elevation_local" => &atlas.elevation_local,
        "bathymetry" => &atlas.bathymetry,
        "island_mask" => &atlas.island_mask,
        "slope" => &atlas.slope,
        "coast_distance" => &atlas.coast_distance,
        "filled_elevation" => &atlas.filled_elevation,
        "flow_accumulation" => &atlas.flow_accumulation,
        "river_mask" => &atlas.river_mask,
        "wetness" => &atlas.wetness,
        "sediment" => &atlas.sediment,
        "cliff_mask" => &atlas.cliff_mask,
        "beach_mask" => &atlas.beach_mask,
        "soil_depth" => &atlas.soil_depth,
        other => panic!("unknown scalar field {other}"),
    }
}

/// Write a baked atlas directory (`manifest.yaml` + compressed field blobs).
pub fn write_baked_atlas(
    dir: &Path,
    atlas: &IslandAtlas,
    world_id: &str,
) -> Result<AtlasBakeManifest, AtlasBakeError> {
    fs::create_dir_all(dir)?;
    let mut fields = BTreeMap::new();
    for name in SCALAR_FIELD_NAMES {
        write_field_meta(dir, name, scalar_field_ref(atlas, name), &mut fields)?;
    }
    write_biome_field(dir, &atlas.biome_weights, &mut fields)?;
    write_u8_field(dir, "flow_direction", &atlas.flow_direction, &mut fields)?;

    let has_river_graph = atlas.river_graph.is_some();
    if let Some(ref river) = atlas.river_graph {
        let json = serde_json::to_string_pretty(&river_to_file(river))
            .map_err(|e| AtlasBakeError::Parse(e.to_string()))?;
        fs::write(dir.join(RIVER_GRAPH_FILENAME), json)?;
    }

    let manifest = AtlasBakeManifest {
        schema_version: ATLAS_BAKE_SCHEMA_VERSION,
        world_id: world_id.to_string(),
        seed: atlas.seed,
        sea_level_m: atlas.sea_level_m,
        voxel_amplitude_m: atlas.voxel_amplitude_m,
        origin: atlas.origin,
        resolution: atlas.resolution.into(),
        validation_passed: atlas.validation_passed,
        validation_messages: atlas.validation_messages.clone(),
        content_hash: atlas_content_hash(atlas),
        fields,
        has_river_graph,
    };

    let yaml =
        serde_yaml::to_string(&manifest).map_err(|e| AtlasBakeError::Parse(e.to_string()))?;
    fs::write(dir.join(MANIFEST_FILENAME), yaml)?;
    Ok(manifest)
}

/// Load a baked atlas from `dir` (directory containing `manifest.yaml`).
pub fn load_baked_atlas(
    dir: &Path,
    expected_world_id: Option<&str>,
    expected_seed: Option<u64>,
) -> Result<IslandAtlas, AtlasBakeError> {
    let manifest_text = fs::read_to_string(dir.join(MANIFEST_FILENAME))?;
    let manifest: AtlasBakeManifest =
        serde_yaml::from_str(&manifest_text).map_err(|e| AtlasBakeError::Parse(e.to_string()))?;
    if manifest.schema_version != ATLAS_BAKE_SCHEMA_VERSION {
        return Err(AtlasBakeError::SchemaMismatch {
            expected: ATLAS_BAKE_SCHEMA_VERSION,
            found: manifest.schema_version,
        });
    }
    if let Some(expected) = expected_world_id {
        if manifest.world_id != expected {
            return Err(AtlasBakeError::WorldMismatch {
                expected: expected.to_string(),
                found: manifest.world_id,
            });
        }
    }
    if let Some(expected) = expected_seed {
        if manifest.seed != expected {
            return Err(AtlasBakeError::SeedMismatch {
                expected,
                found: manifest.seed,
            });
        }
    }

    let load_scalar = |name: &str| -> Result<Field2D<f32>, AtlasBakeError> {
        let meta = manifest
            .fields
            .get(name)
            .ok_or_else(|| AtlasBakeError::MissingField(name.to_string()))?;
        load_f32_field(dir, meta)
    };

    let elevation_regional = load_scalar("elevation_regional")?;
    let elevation_local = load_scalar("elevation_local")?;
    let bathymetry = load_scalar("bathymetry")?;
    let island_mask = load_scalar("island_mask")?;
    let slope = load_scalar("slope")?;
    let coast_distance = load_scalar("coast_distance")?;
    let filled_elevation = load_scalar("filled_elevation")?;
    let flow_accumulation = load_scalar("flow_accumulation")?;
    let river_mask = load_scalar("river_mask")?;
    let wetness = load_scalar("wetness")?;
    let sediment = load_scalar("sediment")?;
    let cliff_mask = load_scalar("cliff_mask")?;
    let beach_mask = load_scalar("beach_mask")?;
    let soil_depth = load_scalar("soil_depth")?;

    let biome_meta = manifest
        .fields
        .get("biome_weights")
        .ok_or_else(|| AtlasBakeError::MissingField("biome_weights".to_string()))?;
    let biome_weights = load_biome_field(dir, biome_meta)?;

    let flow_meta = manifest
        .fields
        .get("flow_direction")
        .ok_or_else(|| AtlasBakeError::MissingField("flow_direction".to_string()))?;
    let flow_direction = load_u8_field(dir, flow_meta)?;

    let river_graph = if manifest.has_river_graph {
        let text = fs::read_to_string(dir.join(RIVER_GRAPH_FILENAME))?;
        let file: RiverGraphFile =
            serde_json::from_str(&text).map_err(|e| AtlasBakeError::Parse(e.to_string()))?;
        Some(river_from_file(file))
    } else {
        None
    };

    let atlas = IslandAtlas {
        resolution: manifest.resolution.into(),
        seed: manifest.seed,
        sea_level_m: manifest.sea_level_m,
        voxel_amplitude_m: manifest.voxel_amplitude_m,
        origin: manifest.origin,
        elevation_regional,
        elevation_local,
        bathymetry,
        island_mask,
        slope,
        coast_distance,
        filled_elevation,
        flow_direction,
        flow_accumulation,
        river_mask,
        wetness,
        sediment,
        cliff_mask,
        beach_mask,
        soil_depth,
        biome_weights,
        river_graph,
        validation_passed: manifest.validation_passed,
        validation_messages: manifest.validation_messages,
    };

    let loaded_hash = atlas_content_hash(&atlas);
    if loaded_hash != manifest.content_hash {
        return Err(AtlasBakeError::Parse(format!(
            "content hash mismatch: manifest {} loaded {}",
            manifest.content_hash, loaded_hash
        )));
    }

    Ok(atlas)
}

/// Resolve a baked-atlas path relative to the assets root.
pub fn resolve_baked_atlas_path(assets_root: &Path, relative: &str) -> PathBuf {
    let trimmed = relative.trim().trim_end_matches(".atlas");
    let with_suffix = if trimmed.ends_with(".atlas") {
        trimmed.to_string()
    } else {
        format!("{trimmed}.atlas")
    };
    assets_root.join(with_suffix)
}

/// Load baked atlas when `relative_path` is set on the world definition.
pub fn try_load_baked_atlas(
    assets_root: &Path,
    relative_path: &str,
    world_id: &str,
    seed: u64,
) -> Result<IslandAtlas, AtlasBakeError> {
    let dir = resolve_baked_atlas_path(assets_root, relative_path);
    load_baked_atlas(&dir, Some(world_id), Some(seed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field2d::Field2D;

    fn tiny_atlas() -> IslandAtlas {
        let spacing = 4.0;
        let origin = [-20.0, -20.0];
        let mut elev = Field2D::new(5, 5, origin, spacing);
        elev.for_each_world(|_, _, v| *v = 12.0);
        IslandAtlas {
            resolution: GenerationResolution::for_extent(80.0),
            seed: 42,
            sea_level_m: 2.0,
            voxel_amplitude_m: 0.5,
            origin,
            elevation_regional: elev.clone(),
            elevation_local: Field2D::new(5, 5, origin, spacing),
            bathymetry: elev.clone(),
            island_mask: elev.clone(),
            slope: elev.clone(),
            coast_distance: elev.clone(),
            filled_elevation: elev.clone(),
            flow_direction: Field2D::new(5, 5, origin, spacing),
            flow_accumulation: elev.clone(),
            river_mask: elev.clone(),
            wetness: elev.clone(),
            sediment: elev.clone(),
            cliff_mask: elev.clone(),
            beach_mask: elev.clone(),
            soil_depth: elev.clone(),
            biome_weights: Field2D::new(5, 5, origin, spacing),
            river_graph: None,
            validation_passed: true,
            validation_messages: Vec::new(),
        }
    }

    #[test]
    fn round_trip_preserves_content_hash() {
        let dir = std::env::temp_dir().join("rpg_adrift_atlas_bake_test");
        let _ = fs::remove_dir_all(&dir);
        let atlas = tiny_atlas();
        let manifest = write_baked_atlas(&dir, &atlas, "world.test").expect("write");
        let loaded = load_baked_atlas(&dir, Some("world.test"), Some(42)).expect("load");
        assert_eq!(manifest.content_hash, atlas_content_hash(&loaded));
        assert_eq!(loaded.seed, 42);
        let _ = fs::remove_dir_all(&dir);
    }
}
