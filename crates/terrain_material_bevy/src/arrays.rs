use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use procedural_textures::CpuTextureArrays;

#[derive(Clone)]
pub struct TerrainArrayHandles {
    pub albedo: Handle<Image>,
    pub normal: Handle<Image>,
    pub ormh: Handle<Image>,
}

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
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..Default::default()
    });

    image
}

pub fn create_placeholder_array_images(images: &mut Assets<Image>, layers: u32) -> TerrainArrayHandles {
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

pub fn upload_texture_arrays(cpu: &CpuTextureArrays, images: &mut Assets<Image>) -> TerrainArrayHandles {
    let albedo = create_array_image(
        cpu.width,
        cpu.height,
        cpu.layers,
        cpu.albedo.clone(),
        TextureFormat::Rgba8UnormSrgb,
    );
    let normal = create_array_image(
        cpu.width,
        cpu.height,
        cpu.layers,
        cpu.normal.clone(),
        TextureFormat::Rgba8Unorm,
    );
    let ormh = create_array_image(
        cpu.width,
        cpu.height,
        cpu.layers,
        cpu.ormh.clone(),
        TextureFormat::Rgba8Unorm,
    );

    TerrainArrayHandles {
        albedo: images.add(albedo),
        normal: images.add(normal),
        ormh: images.add(ormh),
    }
}
