//! Field debug export utilities.

use std::path::Path;

use crate::fields::key::FieldKey;
use crate::world::atlas::WorldAtlas;

pub fn export_all_fields(atlas: &WorldAtlas, dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for key in [
        FieldKey::BoundaryDistance,
        FieldKey::OceanBasin,
        FieldKey::IslandInfluence,
        FieldKey::LandMask,
        FieldKey::BaseElevation,
        FieldKey::Bathymetry,
        FieldKey::CoastDistance,
        FieldKey::RockHardness,
        FieldKey::RegionalResidual,
        FieldKey::FinalElevation,
        FieldKey::Temperature,
        FieldKey::Rainfall,
        FieldKey::Humidity,
        FieldKey::WindExposure,
        FieldKey::FlowAccumulation,
        FieldKey::RiverMask,
        FieldKey::ErodedElevation,
        FieldKey::SedimentThickness,
        FieldKey::BeachSuitability,
        FieldKey::CliffSuitability,
        FieldKey::ReefSuitability,
        FieldKey::LagoonSuitability,
        FieldKey::MangroveSuitability,
        FieldKey::SoilDepth,
        FieldKey::PrimaryBiome,
        FieldKey::RegolithDepth,
        FieldKey::WeatheringDepth,
        FieldKey::WaveExposureCoastal,
        FieldKey::CoastalElevation,
    ] {
        if let Some(field) = atlas.fields.get_scalar(key) {
            export_scalar_grayscale(&field, &dir.join(format!("{key:?}.pgm")))?;
        }
    }
    Ok(())
}

pub fn export_scalar_grayscale(
    field: &crate::fields::scalar::ScalarField,
    path: &Path,
) -> std::io::Result<()> {
    let (min, max) = field.min_max();
    let range = (max - min).max(1e-6);
    let w = field.descriptor.width;
    let h = field.descriptor.height;
    let out = format!("P5\n{w} {h}\n255\n");
    let header = out.len();
    let mut bytes = vec![0u8; header + (w * h) as usize];
    bytes[..header].copy_from_slice(out.as_bytes());
    for z in 0..h {
        for x in 0..w {
            let v = ((field.get(x, z) - min) / range * 255.0).clamp(0.0, 255.0) as u8;
            bytes[header + (z * w + x) as usize] = v;
        }
    }
    std::fs::write(path, bytes)
}

pub fn field_histogram(field: &crate::fields::scalar::ScalarField, bins: usize) -> Vec<u32> {
    let (min, max) = field.min_max();
    let range = (max - min).max(1e-6);
    let mut hist = vec![0u32; bins];
    for v in &field.values {
        let t = ((*v - min) / range * (bins - 1) as f32).round() as usize;
        hist[t.min(bins - 1)] += 1;
    }
    hist
}
