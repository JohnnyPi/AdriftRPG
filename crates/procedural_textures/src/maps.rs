// crates/procedural_textures/src/maps.rs
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GeneratedPbrMaps {
    pub width: u32,
    pub height: u32,
    /// RGBA8 sRGB albedo.
    pub albedo_rgba8: Vec<u8>,
    /// RGBA8 tangent-space normal (R=X, G=Y, B=Z, A=255).
    pub normal_rgba8: Vec<u8>,
    /// RGBA8 packed ORM + height (R=AO, G=roughness, B=metallic, A=height).
    pub ormh_rgba8: Vec<u8>,
    pub emissive_rgba8: Option<Vec<u8>>,
    pub mip_level_count: u32,
}

pub fn encode_height_u8(height: &[f32], min_value: f32, max_value: f32) -> Vec<u8> {
    let range = (max_value - min_value).max(f32::EPSILON);
    height
        .iter()
        .map(|value| {
            let normalized = ((*value - min_value) / range).clamp(0.0, 1.0);
            (normalized * 255.0).round() as u8
        })
        .collect()
}

pub fn pack_ormh(ao: &[u8], roughness: &[u8], metallic: &[u8], height: &[u8]) -> Vec<u8> {
    let count = ao.len();
    assert_eq!(roughness.len(), count);
    assert_eq!(metallic.len(), count);
    assert_eq!(height.len(), count);

    let mut output = Vec::with_capacity(count * 4);
    for index in 0..count {
        output.extend_from_slice(&[
            ao[index],
            roughness[index],
            metallic[index],
            height[index],
        ]);
    }
    output
}

pub fn linear_to_srgb_u8(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let srgb = if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (srgb * 255.0).round() as u8
}
