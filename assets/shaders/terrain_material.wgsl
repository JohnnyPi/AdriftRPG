#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
    pbr_types::{PbrInput, pbr_input_new},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing, calculate_view, prepare_world_normal},
    mesh_view_bindings::view,
}

struct TerrainSettings {
    triplanar_sharpness: f32,
    global_texture_scale: f32,
    normal_strength: f32,
    height_blend_strength: f32,
    layer_count: u32,
    debug_mode: u32,
    _padding: vec2<f32>,
}

struct TerrainLayerScales {
    scales0: vec4<f32>,
    scales1: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var albedo_array: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var albedo_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var normal_array: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var normal_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var ormh_array: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var ormh_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var<uniform> settings: TerrainSettings;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var<uniform> layer_scales: TerrainLayerScales;

fn layer_scale(layer: u32) -> f32 {
    if layer < 4u {
        return layer_scales.scales0[layer];
    }
    return layer_scales.scales1[layer - 4u];
}

fn triplanar_weights(n: vec3<f32>) -> vec3<f32> {
    let an = abs(n);
    let w = pow(an, vec3<f32>(settings.triplanar_sharpness));
    let sum = w.x + w.y + w.z;
    if sum < 0.0001 {
        return vec3<f32>(0.0, 1.0, 0.0);
    }
    return w / sum;
}

fn sample_triplanar_albedo(layer: u32, world_pos: vec3<f32>, world_normal: vec3<f32>) -> vec3<f32> {
    let scale = settings.global_texture_scale / max(layer_scale(layer), 0.01);
    let blend = triplanar_weights(world_normal);
    let uv_x = world_pos.zy * scale;
    let uv_y = world_pos.xz * scale;
    let uv_z = world_pos.xy * scale;
    let cx = textureSampleLevel(albedo_array, albedo_sampler, uv_x, i32(layer), 0.0).rgb;
    let cy = textureSampleLevel(albedo_array, albedo_sampler, uv_y, i32(layer), 0.0).rgb;
    let cz = textureSampleLevel(albedo_array, albedo_sampler, uv_z, i32(layer), 0.0).rgb;
    return cx * blend.x + cy * blend.y + cz * blend.z;
}

fn sample_triplanar_ormh(layer: u32, world_pos: vec3<f32>, world_normal: vec3<f32>) -> vec4<f32> {
    let scale = settings.global_texture_scale / max(layer_scale(layer), 0.01);
    let blend = triplanar_weights(world_normal);
    let uv_x = world_pos.zy * scale;
    let uv_y = world_pos.xz * scale;
    let uv_z = world_pos.xy * scale;
    let cx = textureSampleLevel(ormh_array, ormh_sampler, uv_x, i32(layer), 0.0);
    let cy = textureSampleLevel(ormh_array, ormh_sampler, uv_y, i32(layer), 0.0);
    let cz = textureSampleLevel(ormh_array, ormh_sampler, uv_z, i32(layer), 0.0);
    return cx * blend.x + cy * blend.y + cz * blend.z;
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world_position =
        mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    out.position = position_world_to_clip(world_position.xyz);
    out.world_position = world_position;
#ifdef VERTEX_NORMALS
    out.world_normal =
        mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#else
    out.world_normal = vec3<f32>(0.0, 1.0, 0.0);
#endif
#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif
#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif
    return out;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> @location(0) vec4<f32> {
    let world_pos = in.world_position.xyz;
    let world_normal = normalize(in.world_normal);

#ifdef VERTEX_UVS_B
    let ids = vec4<f32>(in.uv.x, in.uv.y, in.uv_b.x, in.uv_b.y);
#else
#ifdef VERTEX_UVS_A
    let ids = vec4<f32>(in.uv.x, in.uv.y, 0.0, 0.0);
#else
    let ids = vec4<f32>(0.0, 0.0, 0.0, 0.0);
#endif
#endif
#ifdef VERTEX_COLORS
    let weights = in.color;
#else
    let weights = vec4<f32>(1.0, 0.0, 0.0, 0.0);
#endif

    if settings.debug_mode == 2u {
        return vec4<f32>(0.34, 0.52, 0.28, 1.0);
    }

    var albedo = vec3<f32>(0.0);
    var roughness = 0.85;
    var metallic = 0.0;

    for (var i = 0u; i < 4u; i = i + 1u) {
        let layer = u32(ids[i] + 0.5);
        let w = weights[i];
        if w > 0.001 && layer < settings.layer_count {
            albedo += sample_triplanar_albedo(layer, world_pos, world_normal) * w;
            let ormh = sample_triplanar_ormh(layer, world_pos, world_normal);
            roughness += ormh.g * w;
            metallic += ormh.b * w;
        }
    }

    let weight_sum = weights.x + weights.y + weights.z + weights.w;
    if weight_sum < 0.001 {
        albedo = sample_triplanar_albedo(0u, world_pos, world_normal);
    }

    if settings.debug_mode == 1u {
        let layer = u32(ids.x + 0.5);
        let hue = f32(layer) / max(f32(settings.layer_count), 1.0);
        return vec4<f32>(hue, 0.5, 1.0 - hue, 1.0);
    }

    var pbr_input: PbrInput = pbr_input_new();
    pbr_input.is_orthographic = view.clip_from_view[3].w == 1.0;
    pbr_input.V = calculate_view(in.world_position, pbr_input.is_orthographic);
    pbr_input.frag_coord = in.position;
    pbr_input.world_position = in.world_position;
    let prepared_normal = prepare_world_normal(world_normal, false, is_front);
    pbr_input.world_normal = prepared_normal;
    pbr_input.N = normalize(prepared_normal);
    pbr_input.material.base_color = vec4<f32>(albedo, 1.0);
    pbr_input.material.perceptual_roughness = clamp(roughness, 0.04, 1.0);
    pbr_input.material.metallic = clamp(metallic, 0.0, 1.0);

    let lit = apply_pbr_lighting(pbr_input);
    return main_pass_post_lighting_processing(pbr_input, lit);
}
