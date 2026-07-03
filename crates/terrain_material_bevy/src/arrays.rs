// crates/terrain_material_bevy/src/arrays.rs
use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDataOrder, TextureDimension, TextureFormat, TextureUsages,
};
use procedural_textures::CpuTextureArrays;

#[derive(Clone)]
pub struct TerrainArrayHandles {
    pub albedo: Handle<Image>,
    pub normal: Handle<Image>,
    pub ormh: Handle<Image>,
}

/// How texels are averaged when building the mip chain.
///
/// The procedural layers are full of texel-frequency noise; uploading them
/// with a single mip level (the `Image::new` default) leaves the sampler
/// minifying that noise with no prefiltered levels, which renders as
/// salt-and-pepper "static" on any surface more than a few meters away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MipFilter {
    /// sRGB-encoded color: average in linear space, re-encode.
    SrgbColor,
    /// Tangent-space normals in RGB: average as vectors, renormalize.
    NormalMap,
    /// Linear channel data (occlusion/roughness/metallic/height).
    Linear,
}

fn srgb_to_linear(v: u8) -> f32 {
    let c = v as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let e = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (e * 255.0 + 0.5) as u8
}

fn mip_level_count(width: u32, height: u32) -> u32 {
    32 - width.max(height).max(1).leading_zeros()
}

fn mip_dims(width: u32, height: u32, level: u32) -> (u32, u32) {
    ((width >> level).max(1), (height >> level).max(1))
}

/// Total bytes of a full RGBA8 mip chain for one layer.
fn layer_chain_bytes(width: u32, height: u32) -> usize {
    let mips = mip_level_count(width, height);
    (0..mips)
        .map(|m| {
            let (w, h) = mip_dims(width, height, m);
            w as usize * h as usize * 4
        })
        .sum()
}

/// Box-downsample one RGBA8 mip into the next level.
fn downsample_rgba8(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
    filter: MipFilter,
) {
    debug_assert_eq!(src.len(), (src_w * src_h * 4) as usize);
    debug_assert_eq!(dst.len(), (dst_w * dst_h * 4) as usize);

    let texel = |x: u32, y: u32| -> [u8; 4] {
        let x = x.min(src_w - 1);
        let y = y.min(src_h - 1);
        let i = ((y * src_w + x) * 4) as usize;
        [src[i], src[i + 1], src[i + 2], src[i + 3]]
    };

    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let sx = dx * 2;
            let sy = dy * 2;
            let quad = [
                texel(sx, sy),
                texel(sx + 1, sy),
                texel(sx, sy + 1),
                texel(sx + 1, sy + 1),
            ];
            let out_index = ((dy * dst_w + dx) * 4) as usize;
            let out = &mut dst[out_index..out_index + 4];
            match filter {
                MipFilter::SrgbColor => {
                    for ch in 0..3 {
                        let sum: f32 = quad.iter().map(|t| srgb_to_linear(t[ch])).sum();
                        out[ch] = linear_to_srgb(sum * 0.25);
                    }
                    let a: u32 = quad.iter().map(|t| t[3] as u32).sum();
                    out[3] = (a / 4) as u8;
                }
                MipFilter::NormalMap => {
                    let mut n = [0.0f32; 3];
                    for t in &quad {
                        for ch in 0..3 {
                            n[ch] += t[ch] as f32 / 255.0 * 2.0 - 1.0;
                        }
                    }
                    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                    let n = if len > 1e-5 {
                        [n[0] / len, n[1] / len, n[2] / len]
                    } else {
                        [0.0, 0.0, 1.0]
                    };
                    for ch in 0..3 {
                        out[ch] = ((n[ch] * 0.5 + 0.5) * 255.0 + 0.5) as u8;
                    }
                    let a: u32 = quad.iter().map(|t| t[3] as u32).sum();
                    out[3] = (a / 4) as u8;
                }
                MipFilter::Linear => {
                    for ch in 0..4 {
                        let sum: u32 = quad.iter().map(|t| t[ch] as u32).sum();
                        out[ch] = (sum / 4) as u8;
                    }
                }
            }
        }
    }
}

/// Build a full mip chain for an RGBA8 array texture.
///
/// Input is tightly packed mip-0 layers (`width * height * 4` bytes per
/// layer). Output is **layer-major** — all mips of layer 0, then all mips of
/// layer 1, and so on — matching `TextureDataOrder::LayerMajor`, which is what
/// Bevy passes to `create_texture_with_data`.
pub fn build_layer_major_mip_chain(
    width: u32,
    height: u32,
    layers: u32,
    mip0: &[u8],
    filter: MipFilter,
) -> (Vec<u8>, u32) {
    let layer_mip0_bytes = width as usize * height as usize * 4;
    assert_eq!(mip0.len(), layer_mip0_bytes * layers as usize);

    let mips = mip_level_count(width, height);
    let chain_bytes = layer_chain_bytes(width, height);
    let mut data = Vec::with_capacity(chain_bytes * layers as usize);

    for layer in 0..layers as usize {
        let base = &mip0[layer * layer_mip0_bytes..(layer + 1) * layer_mip0_bytes];
        data.extend_from_slice(base);
        let mut prev = base.to_vec();
        let (mut pw, mut ph) = (width, height);
        for level in 1..mips {
            let (w, h) = mip_dims(width, height, level);
            let mut next = vec![0u8; (w * h * 4) as usize];
            downsample_rgba8(&prev, pw, ph, &mut next, w, h, filter);
            data.extend_from_slice(&next);
            prev = next;
            pw = w;
            ph = h;
        }
    }
    (data, mips)
}

/// Single-mip array image (placeholders and any caller that manages its own
/// filtering). Prefer [`create_mipmapped_array_image`] for real content.
pub fn create_array_image(
    width: u32,
    height: u32,
    layers: u32,
    data: Vec<u8>,
    format: TextureFormat,
) -> Image {
    let expected = width as usize * height as usize * layers as usize * 4;
    assert_eq!(data.len(), expected);

    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: layers,
        },
        TextureDimension::D2,
        data,
        format,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );

    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image.sampler = terrain_array_sampler();
    image
}

/// Array image with a CPU-baked mip chain, laid out layer-major.
pub fn create_mipmapped_array_image(
    width: u32,
    height: u32,
    layers: u32,
    mip0: &[u8],
    format: TextureFormat,
    filter: MipFilter,
) -> Image {
    let (data, mips) = build_layer_major_mip_chain(width, height, layers, mip0, filter);

    // `Image::new` asserts the data length matches mip 0 exactly, so build
    // the image from the base level first, then attach the full chain.
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: layers,
        },
        TextureDimension::D2,
        mip0.to_vec(),
        format,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    image.data = Some(data);
    image.data_order = TextureDataOrder::LayerMajor;
    image.texture_descriptor.mip_level_count = mips;
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image.sampler = terrain_array_sampler();
    image
}

fn terrain_array_sampler() -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 4,
        ..Default::default()
    })
}

pub fn create_placeholder_array_images(
    images: &mut Assets<Image>,
    layers: u32,
) -> TerrainArrayHandles {
    let width = 4u32;
    let height = 4u32;
    let count = (width * height * layers * 4) as usize;

    let mut albedo = vec![0u8; count];
    for chunk in albedo.chunks_mut(4) {
        chunk[0] = 86;
        chunk[1] = 132;
        chunk[2] = 70;
        chunk[3] = 255;
    }

    let mut normal = vec![0u8; count];
    for chunk in normal.chunks_mut(4) {
        chunk[0] = 128;
        chunk[1] = 128;
        chunk[2] = 255;
        chunk[3] = 255;
    }

    let mut ormh = vec![0u8; count];
    for chunk in ormh.chunks_mut(4) {
        chunk[0] = 255;
        chunk[1] = 217;
        chunk[2] = 0;
        chunk[3] = 255;
    }

    TerrainArrayHandles {
        albedo: images.add(create_array_image(
            width,
            height,
            layers,
            albedo,
            TextureFormat::Rgba8UnormSrgb,
        )),
        normal: images.add(create_array_image(
            width,
            height,
            layers,
            normal,
            TextureFormat::Rgba8Unorm,
        )),
        ormh: images.add(create_array_image(
            width,
            height,
            layers,
            ormh,
            TextureFormat::Rgba8Unorm,
        )),
    }
}

pub fn upload_texture_arrays(
    cpu: &CpuTextureArrays,
    images: &mut Assets<Image>,
) -> TerrainArrayHandles {
    let albedo = create_mipmapped_array_image(
        cpu.width,
        cpu.height,
        cpu.layers,
        &cpu.albedo,
        TextureFormat::Rgba8UnormSrgb,
        MipFilter::SrgbColor,
    );
    let normal = create_mipmapped_array_image(
        cpu.width,
        cpu.height,
        cpu.layers,
        &cpu.normal,
        TextureFormat::Rgba8Unorm,
        MipFilter::NormalMap,
    );
    let ormh = create_mipmapped_array_image(
        cpu.width,
        cpu.height,
        cpu.layers,
        &cpu.ormh,
        TextureFormat::Rgba8Unorm,
        MipFilter::Linear,
    );

    TerrainArrayHandles {
        albedo: images.add(albedo),
        normal: images.add(normal),
        ormh: images.add(ormh),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mip_count_covers_full_chain() {
        assert_eq!(mip_level_count(256, 256), 9);
        assert_eq!(mip_level_count(4, 4), 3);
        assert_eq!(mip_level_count(1, 1), 1);
        assert_eq!(mip_level_count(256, 64), 9);
    }

    #[test]
    fn chain_is_layer_major_and_sized_exactly() {
        let w = 4u32;
        let h = 4u32;
        let layers = 2u32;
        let layer_bytes = (w * h * 4) as usize;
        let mut mip0 = vec![0u8; layer_bytes * layers as usize];
        // Layer 0 solid 40, layer 1 solid 200 (linear data).
        mip0[..layer_bytes].fill(40);
        mip0[layer_bytes..].fill(200);

        let (data, mips) = build_layer_major_mip_chain(w, h, layers, &mip0, MipFilter::Linear);
        assert_eq!(mips, 3);
        let chain = layer_chain_bytes(w, h); // 4x4 + 2x2 + 1x1 = 21 texels * 4
        assert_eq!(chain, 21 * 4);
        assert_eq!(data.len(), chain * layers as usize);

        // Every byte of layer 0's chain averages back to 40; layer 1 to 200.
        assert!(data[..chain].iter().all(|&b| b == 40));
        assert!(data[chain..].iter().all(|&b| b == 200));
    }

    #[test]
    fn srgb_flat_color_survives_downsampling() {
        let w = 4u32;
        let h = 4u32;
        let mut mip0 = vec![0u8; (w * h * 4) as usize];
        for chunk in mip0.chunks_mut(4) {
            chunk.copy_from_slice(&[86, 132, 70, 255]);
        }
        let (data, mips) = build_layer_major_mip_chain(w, h, 1, &mip0, MipFilter::SrgbColor);
        assert_eq!(mips, 3);
        // Last mip is the final 4 bytes; flat color must round-trip within 1.
        let tail = &data[data.len() - 4..];
        for (got, want) in tail.iter().zip([86u8, 132, 70, 255]) {
            assert!((*got as i16 - want as i16).abs() <= 1, "got {got}, want {want}");
        }
    }

    #[test]
    fn normal_mips_stay_unit_length() {
        let w = 2u32;
        let h = 2u32;
        // Four different tilted normals; the average must be renormalized.
        let texels: [[u8; 4]; 4] = [
            [255, 128, 128, 255],
            [0, 128, 128, 255],
            [128, 255, 128, 255],
            [128, 128, 255, 255],
        ];
        let mut mip0 = Vec::new();
        for t in texels {
            mip0.extend_from_slice(&t);
        }
        let (data, mips) = build_layer_major_mip_chain(w, h, 1, &mip0, MipFilter::NormalMap);
        assert_eq!(mips, 2);
        let n = &data[data.len() - 4..];
        let v = [
            n[0] as f32 / 255.0 * 2.0 - 1.0,
            n[1] as f32 / 255.0 * 2.0 - 1.0,
            n[2] as f32 / 255.0 * 2.0 - 1.0,
        ];
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        assert!((len - 1.0).abs() < 0.02, "mip normal length {len}");
    }
}