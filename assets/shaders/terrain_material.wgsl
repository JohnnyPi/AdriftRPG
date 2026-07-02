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
    count: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
    scales: array<vec4<f32>, 16>,
}

struct ChunkSlotPaletteUniform {
    local_to_global: array<vec4<u32>, 2>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var albedo_array: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var albedo_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var normal_array: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var normal_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var ormh_array: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var ormh_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var<uniform> settings: TerrainSettings;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var<uniform> layer_scales: TerrainLayerScales;
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var<uniform> chunk_slots: ChunkSlotPaletteUniform;

fn layer_scale(layer: u32) -> f32 {
    if layer >= layer_scales.count {
        return 1.0;
    }
    let chunk = layer / 4u;
    let component = layer % 4u;
    let row = layer_scales.scales[chunk];
    if component == 0u {
        return row.x;
    } else if component == 1u {
        return row.y;
    } else if component == 2u {
        return row.z;
    }
    return row.w;
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

fn sample_triplanar_albedo(
    layer: u32,
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    ddx_p: vec3<f32>,
    ddy_p: vec3<f32>,
) -> vec3<f32> {
    let scale = settings.global_texture_scale / max(layer_scale(layer), 0.01);
    let blend = triplanar_weights(world_normal);
    let uv_x = world_pos.zy * scale;
    let uv_y = world_pos.xz * scale;
    let uv_z = world_pos.xy * scale;
    let ddx_x = ddx_p.zy * scale;
    let ddy_x = ddy_p.zy * scale;
    let ddx_y = ddx_p.xz * scale;
    let ddy_y = ddy_p.xz * scale;
    let ddx_z = ddx_p.xy * scale;
    let ddy_z = ddy_p.xy * scale;
    let cx = textureSampleGrad(albedo_array, albedo_sampler, uv_x, i32(layer), ddx_x, ddy_x).rgb;
    let cy = textureSampleGrad(albedo_array, albedo_sampler, uv_y, i32(layer), ddx_y, ddy_y).rgb;
    let cz = textureSampleGrad(albedo_array, albedo_sampler, uv_z, i32(layer), ddx_z, ddy_z).rgb;
    return cx * blend.x + cy * blend.y + cz * blend.z;
}

fn sample_triplanar_ormh(
    layer: u32,
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    ddx_p: vec3<f32>,
    ddy_p: vec3<f32>,
) -> vec4<f32> {
    let scale = settings.global_texture_scale / max(layer_scale(layer), 0.01);
    let blend = triplanar_weights(world_normal);
    let uv_x = world_pos.zy * scale;
    let uv_y = world_pos.xz * scale;
    let uv_z = world_pos.xy * scale;
    let ddx_x = ddx_p.zy * scale;
    let ddy_x = ddy_p.zy * scale;
    let ddx_y = ddx_p.xz * scale;
    let ddy_y = ddy_p.xz * scale;
    let ddx_z = ddx_p.xy * scale;
    let ddy_z = ddy_p.xy * scale;
    let cx = textureSampleGrad(ormh_array, ormh_sampler, uv_x, i32(layer), ddx_x, ddy_x);
    let cy = textureSampleGrad(ormh_array, ormh_sampler, uv_y, i32(layer), ddx_y, ddy_y);
    let cz = textureSampleGrad(ormh_array, ormh_sampler, uv_z, i32(layer), ddx_z, ddy_z);
    return cx * blend.x + cy * blend.y + cz * blend.z;
}

fn decode_normal(sample: vec4<f32>) -> vec3<f32> {
    return normalize(sample.xyz * 2.0 - vec3<f32>(1.0, 1.0, 1.0));
}

fn sample_triplanar_normal(
    layer: u32,
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    ddx_p: vec3<f32>,
    ddy_p: vec3<f32>,
) -> vec3<f32> {
    let scale = settings.global_texture_scale / max(layer_scale(layer), 0.01);
    let blend = triplanar_weights(world_normal);
    let uv_x = world_pos.zy * scale;
    let uv_y = world_pos.xz * scale;
    let uv_z = world_pos.xy * scale;
    let ddx_x = ddx_p.zy * scale;
    let ddy_x = ddy_p.zy * scale;
    let ddx_y = ddx_p.xz * scale;
    let ddy_y = ddy_p.xz * scale;
    let ddx_z = ddx_p.xy * scale;
    let ddy_z = ddy_p.xy * scale;

    let sx = decode_normal(textureSampleGrad(normal_array, normal_sampler, uv_x, i32(layer), ddx_x, ddy_x));
    let sy = decode_normal(textureSampleGrad(normal_array, normal_sampler, uv_y, i32(layer), ddx_y, ddy_y));
    let sz = decode_normal(textureSampleGrad(normal_array, normal_sampler, uv_z, i32(layer), ddx_z, ddy_z));

    var nx = normalize(vec3<f32>(sx.z, sx.y, sx.x));
    var ny = normalize(vec3<f32>(sy.x, sy.z, sy.y));
    var nz = normalize(vec3<f32>(sz.x, sz.y, sz.z));

    nx.x *= select(-1.0, 1.0, world_normal.x >= 0.0);
    ny.y *= select(-1.0, 1.0, world_normal.y >= 0.0);
    nz.z *= select(-1.0, 1.0, world_normal.z >= 0.0);

    return normalize(nx * blend.x + ny * blend.y + nz * blend.z);
}

fn global_layer_for_local(local: u32) -> u32 {
    if local >= 8u {
        return 0u;
    }
    let chunk = local / 4u;
    let component = local % 4u;
    let row = chunk_slots.local_to_global[chunk];
    var mapped: u32;
    if component == 0u {
        mapped = row.x;
    } else if component == 1u {
        mapped = row.y;
    } else if component == 2u {
        mapped = row.z;
    } else {
        mapped = row.w;
    }
    if mapped >= 4294967295u {
        return 0u;
    }
    return mapped;
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

#ifdef VERTEX_COLORS
    let weights = in.color;
#else
    let weights = vec4<f32>(1.0, 0.0, 0.0, 0.0);
#endif
#ifdef VERTEX_UVS_A
    let idx01 = in.uv;
#else
    let idx01 = vec2<f32>(0.0, 0.0);
#endif
#ifdef VERTEX_UVS_B
    let idx23 = in.uv_b;
#else
    let idx23 = vec2<f32>(0.0, 0.0);
#endif

    if settings.debug_mode == 2u {
        return vec4<f32>(1.0, 0.0, 1.0, 1.0);
    }

    let ddx_p = dpdx(world_pos);
    let ddy_p = dpdy(world_pos);

    var albedo = vec3<f32>(0.0);
    var roughness = 0.0;
    var metallic = 0.0;
    var occlusion = 0.0;
    var detail_normal = vec3<f32>(0.0, 0.0, 0.0);

    var local_indices = array<u32, 4>(
        u32(idx01.x),
        u32(idx01.y),
        u32(idx23.x),
        u32(idx23.y),
    );
    var blend_weights = array<f32, 4>(weights.x, weights.y, weights.z, weights.w);

    var height_weights = array<f32, 4>(0.0, 0.0, 0.0, 0.0);
    var height_sum = 0.0;
    let height_k = settings.height_blend_strength;

    for (var i = 0u; i < 4u; i = i + 1u) {
        let w = blend_weights[i];
        if w <= 0.001 {
            continue;
        }
        let global_layer = global_layer_for_local(local_indices[i]);
        if global_layer >= settings.layer_count {
            continue;
        }
        let ormh = sample_triplanar_ormh(global_layer, world_pos, world_normal, ddx_p, ddy_p);
        let h = max(ormh.a, 0.01);
        let hw = w * pow(h, height_k);
        height_weights[i] = hw;
        height_sum += hw;
    }

    if height_sum > 0.001 {
        for (var i = 0u; i < 4u; i = i + 1u) {
            blend_weights[i] = height_weights[i] / height_sum;
        }
    }

    var weight_sum = 0.0;
    for (var i = 0u; i < 4u; i = i + 1u) {
        let w = blend_weights[i];
        if w <= 0.001 {
            continue;
        }
        let global_layer = global_layer_for_local(local_indices[i]);
        if global_layer >= settings.layer_count {
            continue;
        }
        albedo += sample_triplanar_albedo(global_layer, world_pos, world_normal, ddx_p, ddy_p) * w;
        detail_normal += sample_triplanar_normal(global_layer, world_pos, world_normal, ddx_p, ddy_p) * w;
        let ormh = sample_triplanar_ormh(global_layer, world_pos, world_normal, ddx_p, ddy_p);
        occlusion += ormh.r * w;
        roughness += ormh.g * w;
        metallic += ormh.b * w;
        weight_sum += w;
    }

    if weight_sum < 0.001 {
        albedo = sample_triplanar_albedo(0u, world_pos, world_normal, ddx_p, ddy_p);
        detail_normal = sample_triplanar_normal(0u, world_pos, world_normal, ddx_p, ddy_p);
        let ormh = sample_triplanar_ormh(0u, world_pos, world_normal, ddx_p, ddy_p);
        occlusion = ormh.r;
        roughness = 0.85;
        weight_sum = 1.0;
    } else {
        albedo /= weight_sum;
        roughness /= weight_sum;
        metallic /= weight_sum;
        occlusion /= weight_sum;
        detail_normal = normalize(detail_normal / weight_sum);
    }

    if settings.debug_mode == 1u {
        var dominant = 0u;
        var dominant_w = 0.0;
        for (var i = 0u; i < 4u; i = i + 1u) {
            if blend_weights[i] > dominant_w {
                dominant_w = blend_weights[i];
                dominant = global_layer_for_local(local_indices[i]);
            }
        }
        let hue = f32(dominant) / max(f32(settings.layer_count), 1.0);
        return vec4<f32>(hue, 0.5, 1.0 - hue, 1.0);
    }

    var pbr_input: PbrInput = pbr_input_new();
    pbr_input.is_orthographic = view.clip_from_view[3].w == 1.0;
    pbr_input.V = calculate_view(in.world_position, pbr_input.is_orthographic);
    pbr_input.frag_coord = in.position;
    pbr_input.world_position = in.world_position;
    let normal_mix = clamp(settings.normal_strength * 0.75, 0.0, 1.0);
    let shading_normal = normalize(mix(world_normal, detail_normal, normal_mix));
    let prepared_normal = prepare_world_normal(shading_normal, false, is_front);
    pbr_input.world_normal = prepared_normal;
    pbr_input.N = normalize(prepared_normal);
    pbr_input.material.base_color = vec4<f32>(albedo, 1.0);
    pbr_input.material.perceptual_roughness = clamp(roughness, 0.04, 1.0);
    pbr_input.material.metallic = clamp(metallic, 0.0, 1.0);
    pbr_input.diffuse_occlusion = vec3<f32>(occlusion);

    let lit = apply_pbr_lighting(pbr_input);
    return main_pass_post_lighting_processing(pbr_input, lit);
}
