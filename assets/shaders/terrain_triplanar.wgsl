#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
}

struct TerrainParams {
    color0: vec4<f32>,
    color1: vec4<f32>,
    color2: vec4<f32>,
    color3: vec4<f32>,
    color4: vec4<f32>,
    props0: vec4<f32>,
    props1: vec4<f32>,
    props2: vec4<f32>,
    props3: vec4<f32>,
    props4: vec4<f32>,
    debug: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> params: TerrainParams;

fn terrain_color(idx: u32) -> vec4<f32> {
    switch idx {
        case 0u: { return params.color0; }
        case 1u: { return params.color1; }
        case 2u: { return params.color2; }
        case 3u: { return params.color3; }
        default: { return params.color4; }
    }
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world_position =
        mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    out.position = position_world_to_clip(world_position.xyz);
    out.world_position = world_position;
    out.world_normal = vertex.normal;
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

fn sample_color(id: u32) -> vec3<f32> {
    return terrain_color(id).rgb;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
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

    var color = vec3<f32>(0.0);
    for (var i = 0u; i < 4u; i = i + 1u) {
        let id = u32(ids[i] + 0.5);
        let w = weights[i];
        if w > 0.001 {
            color += sample_color(min(id, 4u)) * w;
        }
    }

    let weight_sum = weights.x + weights.y + weights.z + weights.w;
    if weight_sum < 0.001 {
        let id = u32(ids.x + 0.5);
        color = terrain_color(min(id, 4u)).rgb;
    }

    let debug_mode = u32(params.debug.x + 0.5);
    if debug_mode == 1u {
        return vec4<f32>(0.2, 0.7, 0.25, 1.0);
    }

    return vec4<f32>(color, 1.0);
}
