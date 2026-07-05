// crates/procedural_textures/src/texture_graph.rs
//! Composable texture graph compiler and executor.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use blake3::Hasher;
use serde::{Deserialize, Serialize};

use crate::curves::{ColorStop, parse_hex_color, remap, sample_color_ramp, smoothstep};
use crate::error::TextureGenerationError;
use crate::maps::{
    GeneratedPbrMaps, encode_albedo_rgba8_dithered, encode_emissive_rgba8_dithered,
    encode_height_u8_dithered, encode_scalar_u8_dithered, pack_ormh,
};
use crate::noise::SeamlessNoise;
use crate::normal::normals_from_height_field;
use crate::seam::{DEFAULT_SEAM_TOLERANCE, assert_seamless};

pub const GENERATOR_VERSION: u32 = 2;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TextureGraphDefinition {
    pub nodes: BTreeMap<String, GraphNodeDefinition>,
    pub outputs: BTreeMap<String, GraphOutputDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GraphNodeDefinition {
    Constant {
        value: f32,
    },
    Fbm {
        frequency: f32,
        octaves: u32,
        persistence: f32,
        lacunarity: f32,
        #[serde(default)]
        seed: u32,
    },
    RidgedNoise {
        frequency: f32,
        octaves: u32,
        #[serde(default)]
        seed: u32,
    },
    VoronoiDistance {
        frequency: f32,
        #[serde(default)]
        jitter: f32,
        #[serde(default)]
        seed: u32,
    },
    Add {
        #[serde(default)]
        inputs: Vec<WeightedInput>,
    },
    Subtract {
        a: String,
        b: String,
    },
    Multiply {
        a: String,
        b: String,
    },
    Min {
        a: String,
        b: String,
    },
    Max {
        a: String,
        b: String,
    },
    Clamp {
        input: String,
        min: f32,
        max: f32,
    },
    Remap {
        input: String,
        from: [f32; 2],
        to: [f32; 2],
    },
    SmoothStep {
        input: String,
        edge0: f32,
        edge1: f32,
    },
    #[serde(rename = "slope_filter")]
    SlopeFilter {
        input: String,
        lower: f32,
        upper: f32,
    },
    Invert {
        input: String,
    },
    Power {
        input: String,
        exponent: f32,
    },
    ColorRamp {
        input: String,
        stops: Vec<ColorStopYaml>,
    },
    DomainWarp {
        input: String,
        warp_source: String,
        strength: f32,
    },
    Cavity {
        input: String,
        #[serde(default = "default_cavity_radius")]
        radius: u32,
    },
}

fn default_cavity_radius() -> u32 {
    2
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WeightedInput {
    pub source: String,
    pub weight: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ColorStopYaml {
    pub position: f32,
    pub color: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum GraphOutputDefinition {
    NodeRef(String),
    Typed {
        #[serde(rename = "type")]
        kind: String,
        source: String,
        #[serde(default)]
        strength: f32,
        #[serde(default)]
        constant: Option<f32>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TextureGraphRecipe {
    pub seed: u32,
    pub normal_strength: f32,
    pub roughness: f32,
    pub metallic: f32,
    pub graph: TextureGraphDefinition,
    pub seam_tolerance: f32,
}

impl TextureGraphRecipe {
    pub fn from_yaml_value(
        value: &serde_yaml::Value,
        seed: u32,
    ) -> Result<Self, TextureGenerationError> {
        let graph: TextureGraphDefinition = serde_yaml::from_value(value.clone())
            .map_err(|e| TextureGenerationError::InvalidConfig(format!("texture graph: {e}")))?;
        validate_graph(&graph)?;
        Ok(Self {
            seed,
            normal_strength: 3.0,
            roughness: 0.85,
            metallic: 0.0,
            graph,
            seam_tolerance: DEFAULT_SEAM_TOLERANCE,
        })
    }

    pub fn fingerprint(&self) -> [u8; 32] {
        let json = serde_json::to_string(&(GENERATOR_VERSION, self)).unwrap_or_default();
        *Hasher::new().update(json.as_bytes()).finalize().as_bytes()
    }

    pub fn generate(
        &self,
        width: u32,
        height: u32,
    ) -> Result<GeneratedPbrMaps, TextureGenerationError> {
        let mut executor = GraphExecutor::new(&self.graph, self.seed, width, height)?;
        let maps = executor.execute(self)?;
        assert_seamless(&maps, self.seam_tolerance)
            .map_err(|e| TextureGenerationError::InvalidConfig(e))?;
        Ok(maps)
    }
}

fn validate_graph(graph: &TextureGraphDefinition) -> Result<(), TextureGenerationError> {
    if graph.nodes.is_empty() {
        return Err(TextureGenerationError::InvalidConfig(
            "texture graph has no nodes".to_owned(),
        ));
    }
    for (name, node) in &graph.nodes {
        validate_node_refs(name, node, &graph.nodes)?;
    }
    detect_cycles(graph)?;
    Ok(())
}

fn detect_cycles(graph: &TextureGraphDefinition) -> Result<(), TextureGenerationError> {
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for name in graph.nodes.keys() {
        visit_node(name, graph, &mut visiting, &mut visited)?;
    }
    Ok(())
}

fn visit_node(
    name: &str,
    graph: &TextureGraphDefinition,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) -> Result<(), TextureGenerationError> {
    if visited.contains(name) {
        return Ok(());
    }
    if !visiting.insert(name.to_owned()) {
        return Err(TextureGenerationError::InvalidConfig(format!(
            "texture graph cycle detected at node `{name}`"
        )));
    }
    if let Some(node) = graph.nodes.get(name) {
        for dep in node_dependencies(node) {
            visit_node(&dep, graph, visiting, visited)?;
        }
    }
    visiting.remove(name);
    visited.insert(name.to_owned());
    Ok(())
}

fn node_dependencies(node: &GraphNodeDefinition) -> Vec<String> {
    match node {
        GraphNodeDefinition::Subtract { a, b }
        | GraphNodeDefinition::Multiply { a, b }
        | GraphNodeDefinition::Min { a, b }
        | GraphNodeDefinition::Max { a, b } => vec![a.clone(), b.clone()],
        GraphNodeDefinition::Clamp { input, .. }
        | GraphNodeDefinition::Remap { input, .. }
        | GraphNodeDefinition::SmoothStep { input, .. }
        | GraphNodeDefinition::SlopeFilter { input, .. }
        | GraphNodeDefinition::Invert { input }
        | GraphNodeDefinition::Power { input, .. }
        | GraphNodeDefinition::ColorRamp { input, .. }
        | GraphNodeDefinition::Cavity { input, .. } => vec![input.clone()],
        GraphNodeDefinition::DomainWarp {
            input, warp_source, ..
        } => vec![input.clone(), warp_source.clone()],
        GraphNodeDefinition::Add { inputs } => inputs.iter().map(|i| i.source.clone()).collect(),
        _ => Vec::new(),
    }
}

fn validate_node_refs(
    _name: &str,
    node: &GraphNodeDefinition,
    nodes: &BTreeMap<String, GraphNodeDefinition>,
) -> Result<(), TextureGenerationError> {
    let check = |ref_name: &str| -> Result<(), TextureGenerationError> {
        if !nodes.contains_key(ref_name) {
            return Err(TextureGenerationError::InvalidConfig(format!(
                "graph node references unknown input `{ref_name}`"
            )));
        }
        Ok(())
    };
    match node {
        GraphNodeDefinition::Subtract { a, b }
        | GraphNodeDefinition::Multiply { a, b }
        | GraphNodeDefinition::Min { a, b }
        | GraphNodeDefinition::Max { a, b } => {
            check(a)?;
            check(b)?;
        }
        GraphNodeDefinition::Clamp { input, .. }
        | GraphNodeDefinition::Remap { input, .. }
        | GraphNodeDefinition::SmoothStep { input, .. }
        | GraphNodeDefinition::SlopeFilter { input, .. }
        | GraphNodeDefinition::Invert { input }
        | GraphNodeDefinition::Power { input, .. }
        | GraphNodeDefinition::ColorRamp { input, .. }
        | GraphNodeDefinition::Cavity { input, .. } => check(input)?,
        GraphNodeDefinition::DomainWarp {
            input, warp_source, ..
        } => {
            check(input)?;
            check(warp_source)?;
        }
        GraphNodeDefinition::Add { inputs } => {
            for inp in inputs {
                check(&inp.source)?;
            }
        }
        _ => {}
    }
    Ok(())
}

struct GraphExecutor {
    graph: TextureGraphDefinition,
    seed: u32,
    width: u32,
    height: u32,
    scalar_cache: BTreeMap<String, Arc<[f32]>>,
    color_cache: BTreeMap<String, Vec<[f32; 3]>>,
}

impl GraphExecutor {
    fn new(
        graph: &TextureGraphDefinition,
        seed: u32,
        width: u32,
        height: u32,
    ) -> Result<Self, TextureGenerationError> {
        validate_graph(graph)?;
        Ok(Self {
            graph: graph.clone(),
            seed,
            width,
            height,
            scalar_cache: BTreeMap::new(),
            color_cache: BTreeMap::new(),
        })
    }

    fn pixel_count(&self) -> usize {
        self.width as usize * self.height as usize
    }

    fn execute(
        &mut self,
        recipe: &TextureGraphRecipe,
    ) -> Result<GeneratedPbrMaps, TextureGenerationError> {
        let mut height = vec![0.5f32; self.pixel_count()];
        let mut base_color = vec![[0.5, 0.5, 0.5]; self.pixel_count()];
        let mut roughness = vec![recipe.roughness; self.pixel_count()];
        let mut metallic = vec![recipe.metallic; self.pixel_count()];
        let mut ao = vec![1.0f32; self.pixel_count()];
        let mut emissive = vec![0.0f32; self.pixel_count()];

        for (name, output) in &self.graph.outputs.clone() {
            match name.as_str() {
                "height" => {
                    if let Some(values) = self.resolve_scalar_output(output)? {
                        height = values;
                    }
                }
                "base_color" => {
                    if let Some(values) = self.resolve_color_output(output)? {
                        base_color = values;
                    }
                }
                "roughness" => {
                    if let Some(values) = self.resolve_scalar_output(output)? {
                        roughness = values;
                    }
                }
                "metallic" => {
                    if let Some(values) = self.resolve_scalar_output(output)? {
                        metallic = values;
                    }
                }
                "occlusion" => {
                    if let Some(values) = self.resolve_scalar_output(output)? {
                        ao = values;
                    }
                }
                "emissive" => {
                    if let Some(values) = self.resolve_scalar_output(output)? {
                        emissive = values;
                    }
                }
                _ => {}
            }
        }

        // All channels are dithered on quantization so smooth procedural
        // gradients don't band into 8-bit contour steps. The dither is
        // sub-tolerance and periodic, so the maps stay seamless (see `maps`).
        let albedo_rgba8 = encode_albedo_rgba8_dithered(&base_color, self.width);

        let normal_rgba8 =
            normals_from_height_field(self.width, self.height, &height, recipe.normal_strength);

        let height_u8 = encode_height_u8_dithered(&height, 0.0, 1.0, self.width);
        let roughness_u8 = encode_scalar_u8_dithered(&roughness, self.width);
        let metallic_u8 = encode_scalar_u8_dithered(&metallic, self.width);
        let ao_u8 = encode_scalar_u8_dithered(&ao, self.width);
        let ormh_rgba8 = pack_ormh(&ao_u8, &roughness_u8, &metallic_u8, &height_u8);
        let emissive_rgba8 = encode_emissive_rgba8_dithered(&emissive, self.width);

        Ok(GeneratedPbrMaps {
            width: self.width,
            height: self.height,
            albedo_rgba8,
            normal_rgba8,
            ormh_rgba8,
            emissive_rgba8: Some(emissive_rgba8),
            mip_level_count: 1,
        })
    }

    fn resolve_scalar_output(
        &mut self,
        output: &GraphOutputDefinition,
    ) -> Result<Option<Vec<f32>>, TextureGenerationError> {
        match output {
            GraphOutputDefinition::NodeRef(name) => Ok(Some(self.eval_scalar_node(name)?.to_vec())),
            GraphOutputDefinition::Typed { kind, source, .. } if kind == "normal_from_height" => {
                let _ = self.eval_scalar_node(source)?;
                Ok(None)
            }
            GraphOutputDefinition::Typed {
                kind,
                source,
                constant,
                ..
            } if kind == "constant" => {
                let v = constant.unwrap_or(0.0);
                Ok(Some(vec![v; self.pixel_count()]))
            }
            GraphOutputDefinition::Typed { source, .. } => {
                Ok(Some(self.eval_scalar_node(source)?.to_vec()))
            }
        }
    }

    fn resolve_color_output(
        &mut self,
        output: &GraphOutputDefinition,
    ) -> Result<Option<Vec<[f32; 3]>>, TextureGenerationError> {
        match output {
            GraphOutputDefinition::NodeRef(name) => {
                if let Some(cached) = self.color_cache.get(name) {
                    return Ok(Some(cached.clone()));
                }
                let node = self.graph.nodes.get(name).cloned().ok_or_else(|| {
                    TextureGenerationError::InvalidConfig(format!("unknown color node `{name}`"))
                })?;
                if let GraphNodeDefinition::ColorRamp { input, stops } = node {
                    let scalar = self.eval_scalar_node(&input)?;
                    let stops = parse_color_stops(&stops);
                    let colors: Vec<[f32; 3]> = scalar
                        .iter()
                        .map(|t| sample_color_ramp(&stops, *t))
                        .collect();
                    self.color_cache.insert(name.clone(), colors.clone());
                    return Ok(Some(colors));
                }
                Err(TextureGenerationError::InvalidConfig(format!(
                    "output `{name}` is not a color node"
                )))
            }
            GraphOutputDefinition::Typed { kind, source, .. } if kind == "color_ramp" => {
                let node = self.graph.nodes.get(source).cloned();
                if let Some(GraphNodeDefinition::ColorRamp { input, stops }) = node {
                    let scalar = self.eval_scalar_node(&input)?;
                    let stops = parse_color_stops(&stops);
                    let colors: Vec<[f32; 3]> = scalar
                        .iter()
                        .map(|t| sample_color_ramp(&stops, *t))
                        .collect();
                    Ok(Some(colors))
                } else {
                    let scalar = self.eval_scalar_node(source)?;
                    Ok(Some(
                        scalar.iter().map(|t| [t.clamp(0.0, 1.0); 3]).collect(),
                    ))
                }
            }
            _ => Ok(None),
        }
    }

    fn eval_scalar_node(&mut self, name: &str) -> Result<Arc<[f32]>, TextureGenerationError> {
        if let Some(cached) = self.scalar_cache.get(name) {
            return Ok(Arc::clone(cached));
        }
        let node = self.graph.nodes.get(name).cloned().ok_or_else(|| {
            TextureGenerationError::InvalidConfig(format!("unknown graph node `{name}`"))
        })?;
        let values = self.eval_node(&node)?;
        let cached = Arc::from(values.into_boxed_slice());
        self.scalar_cache
            .insert(name.to_string(), Arc::clone(&cached));
        Ok(cached)
    }

    fn eval_node(
        &mut self,
        node: &GraphNodeDefinition,
    ) -> Result<Vec<f32>, TextureGenerationError> {
        let count = self.pixel_count();
        match node {
            GraphNodeDefinition::Constant { value } => Ok(vec![*value; count]),
            GraphNodeDefinition::Fbm {
                frequency,
                octaves,
                persistence,
                lacunarity,
                seed,
            } => {
                let noise = SeamlessNoise::new(self.seed.wrapping_add(*seed));
                Ok(sample_fbm(
                    &noise,
                    self.width,
                    self.height,
                    *frequency,
                    *octaves,
                    *persistence,
                    *lacunarity,
                ))
            }
            GraphNodeDefinition::RidgedNoise {
                frequency,
                octaves,
                seed,
            } => {
                let noise = SeamlessNoise::new(self.seed.wrapping_add(*seed));
                Ok(sample_ridged(
                    &noise,
                    self.width,
                    self.height,
                    *frequency,
                    *octaves,
                ))
            }
            GraphNodeDefinition::VoronoiDistance {
                frequency,
                jitter,
                seed,
            } => Ok(sample_voronoi(
                self.width,
                self.height,
                *frequency,
                *jitter,
                self.seed.wrapping_add(*seed),
            )),
            GraphNodeDefinition::Add { inputs } => {
                let mut out = vec![0.0f32; count];
                for inp in inputs {
                    let src = self.eval_scalar_node(&inp.source)?;
                    for (o, s) in out.iter_mut().zip(src.iter()) {
                        *o += s * inp.weight;
                    }
                }
                Ok(out)
            }
            GraphNodeDefinition::Subtract { a, b } => {
                let va = self.eval_scalar_node(a)?;
                let vb = self.eval_scalar_node(b)?;
                Ok(va.iter().zip(vb.iter()).map(|(x, y)| x - y).collect())
            }
            GraphNodeDefinition::Multiply { a, b } => {
                let va = self.eval_scalar_node(a)?;
                let vb = self.eval_scalar_node(b)?;
                Ok(va.iter().zip(vb.iter()).map(|(x, y)| x * y).collect())
            }
            GraphNodeDefinition::Min { a, b } => {
                let va = self.eval_scalar_node(a)?;
                let vb = self.eval_scalar_node(b)?;
                Ok(va.iter().zip(vb.iter()).map(|(x, y)| x.min(*y)).collect())
            }
            GraphNodeDefinition::Max { a, b } => {
                let va = self.eval_scalar_node(a)?;
                let vb = self.eval_scalar_node(b)?;
                Ok(va.iter().zip(vb.iter()).map(|(x, y)| x.max(*y)).collect())
            }
            GraphNodeDefinition::Clamp { input, min, max } => {
                let src = self.eval_scalar_node(input)?;
                Ok(src.iter().map(|v| v.clamp(*min, *max)).collect())
            }
            GraphNodeDefinition::Remap { input, from, to } => {
                let src = self.eval_scalar_node(input)?;
                Ok(src
                    .iter()
                    .map(|v| remap(*v, from[0], from[1], to[0], to[1]))
                    .collect())
            }
            GraphNodeDefinition::SmoothStep {
                input,
                edge0,
                edge1,
            } => {
                let src = self.eval_scalar_node(input)?;
                Ok(src.iter().map(|v| smoothstep(*edge0, *edge1, *v)).collect())
            }
            GraphNodeDefinition::SlopeFilter {
                input,
                lower,
                upper,
            } => {
                let src = self.eval_scalar_node(input)?;
                Ok(slope_from_height(
                    &src,
                    self.width,
                    self.height,
                    *lower,
                    *upper,
                ))
            }
            GraphNodeDefinition::Invert { input } => {
                let src = self.eval_scalar_node(input)?;
                Ok(src.iter().map(|v| 1.0 - v).collect())
            }
            GraphNodeDefinition::Power { input, exponent } => {
                let src = self.eval_scalar_node(input)?;
                Ok(src.iter().map(|v| v.powf(*exponent)).collect())
            }
            GraphNodeDefinition::ColorRamp { input, .. } => {
                Ok(self.eval_scalar_node(input)?.to_vec())
            }
            GraphNodeDefinition::DomainWarp {
                input,
                warp_source,
                strength,
            } => {
                let warp = self.eval_scalar_node(warp_source)?;
                sample_domain_warped(self, input, &warp, *strength)
            }
            GraphNodeDefinition::Cavity { input, radius } => {
                let src = self.eval_scalar_node(input)?;
                Ok(apply_cavity(&src, self.width, self.height, *radius))
            }
        }
    }
}

fn parse_color_stops(stops: &[ColorStopYaml]) -> Vec<ColorStop> {
    stops
        .iter()
        .filter_map(|stop| {
            parse_hex_color(&stop.color).map(|color| ColorStop {
                position: stop.position,
                color,
            })
        })
        .collect()
}

fn sample_fbm(
    noise: &SeamlessNoise,
    width: u32,
    height: u32,
    frequency: f32,
    octaves: u32,
    persistence: f32,
    lacunarity: f32,
) -> Vec<f32> {
    let count = width as usize * height as usize;
    let mut out = vec![0.0f32; count];
    for y in 0..height {
        for x in 0..width {
            let u = x as f32 / width as f32;
            let v = y as f32 / height as f32;
            let mut amp = 1.0;
            let mut freq = frequency;
            let mut sum = 0.0;
            let mut norm = 0.0;
            for _ in 0..octaves.max(1) {
                sum += noise.sample(u * freq, v * freq) * amp;
                norm += amp;
                amp *= persistence;
                freq *= lacunarity;
            }
            out[y as usize * width as usize + x as usize] =
                (sum / norm.max(f32::EPSILON)).clamp(0.0, 1.0);
        }
    }
    out
}

fn sample_ridged(
    noise: &SeamlessNoise,
    width: u32,
    height: u32,
    frequency: f32,
    octaves: u32,
) -> Vec<f32> {
    let count = width as usize * height as usize;
    let mut out = vec![0.0f32; count];
    for y in 0..height {
        for x in 0..width {
            let u = x as f32 / width as f32;
            let v = y as f32 / height as f32;
            let mut amp = 0.5;
            let mut freq = frequency;
            let mut sum = 0.0;
            let mut weight = 1.0;
            for _ in 0..octaves.max(1) {
                let n = noise.sample(u * freq, v * freq);
                let signal = 1.0 - (n * 2.0 - 1.0).abs();
                sum += signal * amp * weight;
                weight = signal.clamp(0.0, 1.0);
                amp *= 0.5;
                freq *= 2.0;
            }
            out[y as usize * width as usize + x as usize] = sum.clamp(0.0, 1.0);
        }
    }
    out
}

fn sample_voronoi(width: u32, height: u32, frequency: f32, jitter: f32, seed: u32) -> Vec<f32> {
    let count = width as usize * height as usize;
    let mut out = vec![0.0f32; count];
    let cells = frequency.max(1.0) as i32;
    for y in 0..height {
        for x in 0..width {
            let u = x as f32 / width as f32 * cells as f32;
            let v = y as f32 / height as f32 * cells as f32;
            let mut min_dist = f32::MAX;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let cx = u.floor() as i32 + dx;
                    let cy = v.floor() as i32 + dy;
                    let hash = hash_cell(cx, cy, seed, cells);
                    let px = cx as f32 + hash.0 * jitter + 0.5 * (1.0 - jitter);
                    let py = cy as f32 + hash.1 * jitter + 0.5 * (1.0 - jitter);
                    let dist = ((u - px).powi(2) + (v - py).powi(2)).sqrt();
                    min_dist = min_dist.min(dist);
                }
            }
            out[y as usize * width as usize + x as usize] =
                (1.0 - min_dist * frequency / cells as f32).clamp(0.0, 1.0);
        }
    }
    out
}

fn hash_cell(x: i32, y: i32, seed: u32, cells: i32) -> (f32, f32) {
    let cx = x.rem_euclid(cells.max(1)) as u32;
    let cy = y.rem_euclid(cells.max(1)) as u32;
    let mut h = seed
        .wrapping_mul(374761393)
        .wrapping_add(cx)
        .wrapping_mul(668265263)
        .wrapping_add(cy);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    let hx = (h & 0xFFFF) as f32 / 65535.0;
    let hy = ((h >> 16) & 0xFFFF) as f32 / 65535.0;
    (hx, hy)
}

fn slope_from_height(
    height: &[f32],
    width: u32,
    height_px: u32,
    lower: f32,
    upper: f32,
) -> Vec<f32> {
    let w = width as usize;
    let h = height_px as usize;
    let mut out = vec![0.0f32; height.len()];
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let left = height[y * w + x.saturating_sub(1)];
            let right = height[y * w + (x + 1).min(w - 1)];
            let up = height[y.saturating_sub(1) * w + x];
            let down = height[(y + 1).min(h - 1) * w + x];
            let slope = ((right - left).abs() + (down - up).abs()) * 0.5;
            out[idx] = smoothstep(lower, upper, slope);
        }
    }
    out
}

/// Toroidal bilinear sample of a tileable scalar field at normalized coords.
///
/// Coordinates wrap on both axes, so warped lookups near the texture edge stay
/// seamless. Samples sit at integer texel positions; the field is smooth enough
/// that the half-texel bias is immaterial.
fn sample_bilinear_toroidal(field: &[f32], width: u32, height: u32, u: f32, v: f32) -> f32 {
    let w = width as usize;
    let h = height as usize;
    if w == 0 || h == 0 {
        return 0.0;
    }
    let fx = u.rem_euclid(1.0) * width as f32;
    let fy = v.rem_euclid(1.0) * height as f32;
    let x0 = (fx.floor() as usize) % w;
    let y0 = (fy.floor() as usize) % h;
    let x1 = (x0 + 1) % w;
    let y1 = (y0 + 1) % h;
    let tx = fx - fx.floor();
    let ty = fy - fy.floor();

    let s00 = field[y0 * w + x0];
    let s10 = field[y0 * w + x1];
    let s01 = field[y1 * w + x0];
    let s11 = field[y1 * w + x1];
    let top = s00 + (s10 - s00) * tx;
    let bottom = s01 + (s11 - s01) * tx;
    top + (bottom - top) * ty
}

/// Domain warp: resample the input field at coordinates displaced by the warp
/// field.
///
/// This evaluates the input node's field **once** (O(N), and cached) and then
/// looks it up at warped coordinates via seamless bilinear sampling. It works
/// for any input node — not just raw noise — and replaces the previous version
/// that recomputed the entire input field per pixel (O(N²)) while discarding
/// the warped coordinates entirely. The two warp components are decorrelated by
/// reading the warp field at the pixel and at a toroidal half-shift, instead of
/// the old adjacent-pixel read that made the displacement near-diagonal.
fn sample_domain_warped(
    executor: &mut GraphExecutor,
    input: &str,
    warp: &[f32],
    strength: f32,
) -> Result<Vec<f32>, TextureGenerationError> {
    let width = executor.width;
    let height = executor.height;
    let count = executor.pixel_count();
    let src = executor.eval_scalar_node(input)?;

    if warp.len() != count || src.len() != count {
        return Err(TextureGenerationError::InvalidConfig(
            "domain warp field size mismatch".to_owned(),
        ));
    }

    let w = width as usize;
    let h = height as usize;
    let mut out = vec![0.0f32; count];
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let u = x as f32 / width as f32;
            let v = y as f32 / height as f32;
            // Second warp component comes from a half-texture toroidal shift so
            // horizontal and vertical displacement aren't the same field value.
            let shifted = ((y + h / 2) % h) * w + ((x + w / 2) % w);
            let wu = u + (warp[idx] - 0.5) * strength * 0.15;
            let wv = v + (warp[shifted] - 0.5) * strength * 0.15;
            out[idx] = sample_bilinear_toroidal(&src, width, height, wu, wv);
        }
    }
    Ok(out)
}

fn apply_cavity(src: &[f32], width: u32, height: u32, radius: u32) -> Vec<f32> {
    let r = radius.max(1) as i32;
    let w = width as i32;
    let h = height as i32;
    src.iter()
        .enumerate()
        .map(|(idx, &center)| {
            let x = (idx as i32) % w;
            let y = (idx as i32) / w;
            let mut min_neighbor = center;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = (x + dx).rem_euclid(w) as usize;
                    let ny = (y + dy).rem_euclid(h) as usize;
                    let nidx = ny * w as usize + nx;
                    min_neighbor = min_neighbor.min(src[nidx]);
                }
            }
            (center - min_neighbor).clamp(0.0, 1.0)
        })
        .collect()
}

pub fn texture_graph_from_yaml_value(
    value: &serde_yaml::Value,
    seed: u32,
) -> Result<TextureGraphRecipe, TextureGenerationError> {
    TextureGraphRecipe::from_yaml_value(value, seed)
}
