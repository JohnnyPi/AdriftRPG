That pattern looks much more like a **biome-selection or UV-coordinate artifact** than a terrain-generation problem.

The terrain geometry itself appears coherent, but the cream-colored biome/material is forming thin, maze-like bands across nearly every surface. The most likely cause is that you are feeding a rapidly varying value—often raw noise, elevation, slope, or UV coordinates—through hard biome thresholds.

## Most likely cause: narrow threshold bands

You may have logic resembling:

```rust
if biome_value < 0.45 {
    grass
} else if biome_value < 0.50 {
    sand
} else {
    grass
}
```

or WGSL resembling:

```wgsl
if biome_noise > 0.48 && biome_noise < 0.52 {
    color = sand_color;
}
```

A narrow range cut from continuous noise naturally creates contour-like lines. Those lines follow every rise, dip, and noise fluctuation, producing exactly this “topographic maze” appearance.

For example, this creates bands:

```rust
let biome = noise.get([world_x, world_z]);

let color = if biome > 0.45 && biome < 0.55 {
    SAND
} else {
    GRASS
};
```

Instead, classify broad biome regions:

```rust
let biome = noise.get([world_x * 0.002, world_z * 0.002]);

let color = if biome < -0.25 {
    DESERT
} else if biome < 0.20 {
    GRASSLAND
} else if biome < 0.55 {
    FOREST
} else {
    RAINFOREST
};
```

The important differences are:

* much lower noise frequency;
* broad threshold ranges;
* each range maps to a complete biome;
* no narrow “special band” between two similar regions.

## Second likely cause: using height contours as biome input

If your biome decision includes something like this:

```rust
let band = (height * 10.0).fract();
```

or:

```wgsl
let biome = fract(world_position.y * frequency);
```

you will get repeated elevation stripes.

Likewise, this produces contour lines:

```rust
let quantized_height = (height / 2.0).floor() as i32;

if quantized_height % 2 == 0 {
    grass
} else {
    sand
}
```

Elevation should influence biome suitability gradually, not directly alternate the surface material.

A better approach is:

```rust
let temperature =
    latitude_temperature
    - elevation * elevation_cooling;

let moisture =
    moisture_noise
    - rain_shadow
    + coast_humidity;

let biome = classify_biome(temperature, moisture, elevation);
```

Use elevation mainly for things such as:

* beach eligibility;
* alpine zones;
* snow line;
* swamp lowlands;
* temperature reduction.

Do not use elevation as the primary repeating biome pattern.

## Third likely cause: biome noise sampled at voxel or vertex scale

If your world coordinates are passed directly into noise:

```rust
let biome_noise = noise.get([world_x, world_z]);
```

the frequency is probably far too high.

For large biome regions, use a scale measured in hundreds or thousands of world units:

```rust
const BIOME_SCALE: f64 = 0.0015;

let biome_noise = noise.get([
    world_x as f64 * BIOME_SCALE,
    world_z as f64 * BIOME_SCALE,
]);
```

With eight-unit voxels, I would begin around:

```rust
const BIOME_SCALE: f64 = 1.0 / 1024.0;
```

Then adjust from there.

Terrain detail noise and biome noise should be separate:

```rust
let continental = continental_noise.get([
    x * 0.0005,
    z * 0.0005,
]);

let biome_region = biome_noise.get([
    x * 0.001,
    z * 0.001,
]);

let hills = hill_noise.get([
    x * 0.008,
    z * 0.008,
]);

let surface_detail = detail_noise.get([
    x * 0.04,
    z * 0.04,
]);
```

Do not reuse the high-frequency terrain-detail field for biome identity.

## Fourth possibility: bad UV generation

If those cream regions are an actual texture rather than biome colors, inspect your UVs.

Procedural terrain commonly breaks when every quad or triangle gets UVs such as:

```rust
[0.0, 0.0]
[1.0, 0.0]
[1.0, 1.0]
[0.0, 1.0]
```

but vertices are shared incorrectly, or when UVs are generated from the wrong axes.

Bevy expects valid mesh UV attributes for texture sampling, and its UV origin convention starts at the top-left. ([Docs.rs][1])

For a horizontal terrain surface, world-space planar UVs would look like:

```rust
let u = world_x / texture_world_size;
let v = world_z / texture_world_size;

uvs.push([u, v]);
```

However, planar XZ UVs will stretch badly across cliffs. For voxel terrain with slopes, caves, and overhangs, use **triplanar mapping** rather than conventional terrain UVs.

Conceptually:

```wgsl
let weights = pow(abs(world_normal), vec3<f32>(4.0));
let normalized_weights = weights / dot(weights, vec3<f32>(1.0));

let sample_x = textureSample(
    terrain_texture,
    terrain_sampler,
    world_position.yz * texture_scale,
);

let sample_y = textureSample(
    terrain_texture,
    terrain_sampler,
    world_position.xz * texture_scale,
);

let sample_z = textureSample(
    terrain_texture,
    terrain_sampler,
    world_position.xy * texture_scale,
);

let color =
    sample_x * normalized_weights.x +
    sample_y * normalized_weights.y +
    sample_z * normalized_weights.z;
```

That keeps:

* top surfaces mapped in XZ;
* east/west cliffs mapped in YZ;
* north/south cliffs mapped in XY;
* transitions blended by the surface normal.

Bevy supports custom materials and custom WGSL shaders for this kind of terrain treatment. Its official material example passes mesh UV data into a custom shader, while an extended material can preserve the built-in PBR pipeline and modify its output. ([Bevy][2])

## Fifth possibility: interpolated vertex biome IDs

A subtle but common issue is storing biome identity as a floating-point vertex value:

```rust
biome_ids.push(0.0); // grass
biome_ids.push(1.0); // sand
```

The rasterizer interpolates ordinary vertex attributes across triangles. A triangle whose vertices have different IDs can produce all values between `0.0` and `1.0`.

Then a shader like:

```wgsl
if biome_id > 0.45 && biome_id < 0.55 {
    return sand_color;
}
```

creates thin interior bands across triangles.

Use one of these instead:

### Give the whole triangle one biome

Duplicate boundary vertices so each triangle has a consistent biome attribute.

### Mark the shader attribute as flat

Where supported by your custom shader pipeline:

```wgsl
@location(7) @interpolate(flat)
biome_id: u32,
```

### Use blend weights intentionally

Store weights such as:

```text
grass_weight
sand_weight
rock_weight
mud_weight
```

and blend textures:

```wgsl
let total = max(
    grass_weight + sand_weight + rock_weight,
    0.0001,
);

let weights = vec3<f32>(
    grass_weight,
    sand_weight,
    rock_weight,
) / total;
```

A biome ID should not accidentally behave like a smoothly interpolated color channel.

## Fastest diagnostic sequence

Temporarily remove textures and render one constant color:

```rust
StandardMaterial {
    base_color: Color::srgb(0.2, 0.7, 0.25),
    ..default()
}
```

If the lines disappear, the geometry is fine and the problem is in UVs, textures, or the shader.

Then render only the biome scalar as grayscale:

```wgsl
return vec4<f32>(
    biome_value,
    biome_value,
    biome_value,
    1.0,
);
```

You want to see large cloudy regions. If you see dense contour lines, the biome field is too high-frequency or derived from height.

Next, render discrete debug colors:

```wgsl
if biome_value < 0.25 {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}

if biome_value < 0.50 {
    return vec4<f32>(0.0, 1.0, 0.0, 1.0);
}

if biome_value < 0.75 {
    return vec4<f32>(0.0, 0.0, 1.0, 1.0);
}

return vec4<f32>(1.0, 1.0, 0.0, 1.0);
```

Finally, render the normals:

```wgsl
return vec4<f32>(
    world_normal * 0.5 + 0.5,
    1.0,
);
```

This will reveal whether the cream areas are actually related to slope or face orientation.

## Recommended biome architecture

For your island generator, generate biome data as low-resolution climate fields rather than assigning biomes independently per voxel.

```rust
pub struct ClimateSample {
    pub elevation: f32,
    pub temperature: f32,
    pub moisture: f32,
    pub continentalness: f32,
    pub ruggedness: f32,
    pub coast_distance: f32,
}
```

Then classify:

```rust
fn classify_biome(sample: ClimateSample) -> Biome {
    if sample.elevation < BEACH_MAX_HEIGHT
        && sample.coast_distance < BEACH_DISTANCE
    {
        return Biome::Beach;
    }

    if sample.elevation > ALPINE_HEIGHT {
        return Biome::Alpine;
    }

    match (sample.temperature, sample.moisture) {
        (t, _) if t < 0.20 => Biome::ColdHighland,
        (t, m) if t > 0.70 && m > 0.70 => Biome::Rainforest,
        (t, m) if t > 0.65 && m < 0.25 => Biome::DryScrub,
        (_, m) if m > 0.75 => Biome::Wetland,
        (_, m) if m > 0.45 => Biome::Forest,
        _ => Biome::Grassland,
    }
}
```

Sample the climate map using X/Z world position, then let height and slope choose the **surface treatment inside the biome**:

```rust
let surface = if slope > 0.75 {
    SurfaceMaterial::ExposedRock
} else if near_water {
    SurfaceMaterial::WetSoil
} else {
    biome.default_surface()
};
```

That distinction is important:

```text
Biome = regional ecology
Surface material = local rendering/material
```

A rainforest biome can still contain rock cliffs, mud, riverbanks, and exposed soil without turning each local material change into a new biome.

Based on the screenshot, I would inspect these three lines of code first:

1. where biome noise coordinates are scaled;
2. where biome thresholds are applied;
3. where biome/material values are written to mesh vertices or shader attributes.

The exact faulty line will probably be in one of those areas.

[1]: https://docs.rs/bevy/latest/bevy/mesh/struct.Mesh.html?utm_source=chatgpt.com "Mesh in bevy::mesh - Rust"
[2]: https://bevy.org/examples/shaders/shader-material/?utm_source=chatgpt.com "Material"
