// crates/procedural_textures/src/maps.rs
//! PBR map container plus float→`u8` channel encoders.
//!
//! Smooth procedural gradients band badly when quantized straight to 8 bits:
//! a slowly varying channel crosses successive `1/255` thresholds as visible
//! contour lines. The dithered encoders here spread that rounding decision over
//! an ordered (Bayer) pattern so the band boundary dissolves into a fine dot
//! pattern that reads as a continuous gradient.
//!
//! The dither amplitude is at most one quantization step (`≈ 1/255 ≈ 0.004`),
//! well under [`crate::seam::DEFAULT_SEAM_TOLERANCE`] (`0.02`), and the Bayer
//! matrix is periodic, so tiling and determinism are preserved.

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

// --- Ordered dithering -----------------------------------------------------

/// Canonical 8×8 Bayer threshold matrix (values `0..=63`).
#[rustfmt::skip]
const BAYER_8X8: [[u8; 8]; 8] = [
    [ 0, 32,  8, 40,  2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44,  4, 36, 14, 46,  6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [ 3, 35, 11, 43,  1, 33,  9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47,  7, 39, 13, 45,  5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

/// Ordered-dither offset for pixel `(x, y)`, in the half-open range
/// `[-0.5, 0.5)`. Added to a value already scaled to `0..255` before rounding,
/// this makes the probability of rounding up equal the value's fractional part
/// — proper ordered dithering with a one-LSB amplitude.
#[inline]
fn ordered_dither_offset(x: usize, y: usize) -> f32 {
    let threshold = (BAYER_8X8[y & 7][x & 7] as f32 + 0.5) / 64.0; // (0, 1)
    threshold - 0.5
}

/// Quantize a unit value to `u8` with ordered dithering at pixel `(x, y)`.
#[inline]
pub fn quantize_unit_dithered(value: f32, x: usize, y: usize) -> u8 {
    let scaled = value.clamp(0.0, 1.0) * 255.0 + ordered_dither_offset(x, y);
    scaled.round().clamp(0.0, 255.0) as u8
}

// --- sRGB encoding ---------------------------------------------------------

/// Linear → sRGB transfer (IEC 61966-2-1), clamped to `[0, 1]`.
#[inline]
pub fn linear_to_srgb(linear: f32) -> f32 {
    let c = linear.clamp(0.0, 1.0);
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Per-value linear→sRGB `u8` (rounded, no dithering). Retained for callers
/// that re-encode individual pixels, e.g. post-hoc tinting.
pub fn linear_to_srgb_u8(linear: f32) -> u8 {
    (linear_to_srgb(linear) * 255.0).round() as u8
}

/// Linear→sRGB `u8` with ordered dithering at pixel `(x, y)`.
#[inline]
pub fn srgb_u8_dithered(linear: f32, x: usize, y: usize) -> u8 {
    quantize_unit_dithered(linear_to_srgb(linear), x, y)
}

// --- Whole-image dithered encoders -----------------------------------------

/// Encode linear RGB triplets to dithered sRGB RGBA8 (alpha forced to 255).
///
/// `width` is required to recover each sample's `(x, y)` for the dither
/// pattern. The same offset is applied to all three channels of a pixel, which
/// keeps the dither achromatic.
pub fn encode_albedo_rgba8_dithered(linear_rgb: &[[f32; 3]], width: u32) -> Vec<u8> {
    let w = (width as usize).max(1);
    let mut out = Vec::with_capacity(linear_rgb.len() * 4);
    for (index, rgb) in linear_rgb.iter().enumerate() {
        let x = index % w;
        let y = index / w;
        out.push(srgb_u8_dithered(rgb[0], x, y));
        out.push(srgb_u8_dithered(rgb[1], x, y));
        out.push(srgb_u8_dithered(rgb[2], x, y));
        out.push(255);
    }
    out
}

/// Encode a linear scalar channel (roughness, metallic, AO, …) to dithered
/// single-channel `u8`. Replaces the old truncating `(v * 255.0) as u8`, which
/// both banded and biased values darker.
pub fn encode_scalar_u8_dithered(values: &[f32], width: u32) -> Vec<u8> {
    let w = (width as usize).max(1);
    values
        .iter()
        .enumerate()
        .map(|(index, &v)| quantize_unit_dithered(v, index % w, index / w))
        .collect()
}

/// Encode a grayscale emissive channel to dithered RGBA8 (`v,v,v,255`).
pub fn encode_emissive_rgba8_dithered(values: &[f32], width: u32) -> Vec<u8> {
    let w = (width as usize).max(1);
    let mut out = Vec::with_capacity(values.len() * 4);
    for (index, &v) in values.iter().enumerate() {
        let q = quantize_unit_dithered(v, index % w, index / w);
        out.extend_from_slice(&[q, q, q, 255]);
    }
    out
}

// --- Height packing --------------------------------------------------------

/// Normalize and encode a height field to `u8` (rounded, no dithering).
/// Retained for callers/tests that want the exact non-dithered mapping.
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

/// Normalize and encode a height field to `u8` with ordered dithering.
///
/// Dithering the height reduces stepped parallax and banding in normals
/// derived from the packed height at sample time.
pub fn encode_height_u8_dithered(
    height: &[f32],
    min_value: f32,
    max_value: f32,
    width: u32,
) -> Vec<u8> {
    let range = (max_value - min_value).max(f32::EPSILON);
    let w = (width as usize).max(1);
    height
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let normalized = (*value - min_value) / range;
            quantize_unit_dithered(normalized, index % w, index / w)
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
        output.extend_from_slice(&[ao[index], roughness[index], metallic[index], height[index]]);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dither_offset_is_centered_and_bounded() {
        let mut sum = 0.0f32;
        for y in 0..8 {
            for x in 0..8 {
                let d = ordered_dither_offset(x, y);
                assert!((-0.5..0.5).contains(&d), "offset {d} out of range");
                sum += d;
            }
        }
        // The 64 Bayer thresholds are symmetric about 0.5, so offsets sum to 0.
        assert!(sum.abs() < 1e-4, "dither must be unbiased, got sum {sum}");
    }

    #[test]
    fn dither_preserves_mean_over_a_tile() {
        // A constant channel must average back to its input across an 8×8 tile,
        // i.e. dithering adds no DC bias — that is what removes banding without
        // shifting brightness.
        for &v in &[0.13f32, 0.5, 0.502, 0.9, 0.999] {
            let mut total = 0u32;
            for y in 0..8 {
                for x in 0..8 {
                    total += quantize_unit_dithered(v, x, y) as u32;
                }
            }
            let mean = total as f32 / 64.0;
            let expected = v * 255.0;
            assert!(
                (mean - expected).abs() < 0.75,
                "mean {mean} should track {expected} for v={v}"
            );
        }
    }

    #[test]
    fn dithered_value_never_deviates_more_than_one_step() {
        for &v in &[0.0f32, 0.25, 0.4, 0.6, 1.0] {
            let naive = (v.clamp(0.0, 1.0) * 255.0).round() as i32;
            for y in 0..8 {
                for x in 0..8 {
                    let d = quantize_unit_dithered(v, x, y) as i32;
                    assert!((d - naive).abs() <= 1, "v={v} deviated {} steps", d - naive);
                }
            }
        }
    }

    #[test]
    fn albedo_encoder_emits_rgba_with_opaque_alpha() {
        let pixels = vec![[0.2, 0.4, 0.6]; 16];
        let encoded = encode_albedo_rgba8_dithered(&pixels, 4);
        assert_eq!(encoded.len(), 16 * 4);
        assert!(encoded.chunks_exact(4).all(|px| px[3] == 255));
    }

    #[test]
    fn constant_extremes_are_stable() {
        // Fully black / white constants must not dither into off-by-one noise.
        assert!((0..64).all(|i| srgb_u8_dithered(0.0, i % 8, i / 8) == 0));
        assert!((0..64).all(|i| srgb_u8_dithered(1.0, i % 8, i / 8) == 255));
    }
}