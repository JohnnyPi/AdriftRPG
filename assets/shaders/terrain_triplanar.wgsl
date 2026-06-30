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

fn terrain_props(idx: u32) -> vec4<f32> {
    switch idx {
        case 0u: { return params.props0; }
        case 1u: { return params.props1; }
        case 2u: { return params.props2; }
        case 3u: { return params.props3; }
        default: { return params.props4; }
    }
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    out.position = position_world_to_clip(vertex.position);
    out.world_position = vec4<f32>(vertex.position, 1.0);
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

fn triplanar_weights(normal: vec3<f32>) -> vec3<f32> {
    let blending = abs(normal);
    let b = blending / (blending.x + blending.y + blending.z + 0.0001);
    return b * b;
}

fn sample_color(id: u32, p: vec3<f32>, normal: vec3<f32>, scale: f32) -> vec3<f32> {
    let w = triplanar_weights(normal);
    let base = terrain_color(id).rgb;
    let n = p * scale;
    let tint = vec3<f32>(
        sin(n.x) * 0.04 + cos(n.z) * 0.03,
        sin(n.y) * 0.02,
        cos(n.x + n.z) * 0.03,
    );
    return base + tint * w;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
#ifdef VERTEX_UVS_B
    let ids = vec4<f32>(in.uv.x, in.uv.y, in.uv_b.x, in.uv_b.y);
#else
    let ids = vec4<f32>(in.uv.x, in.uv.y, 0.0, 0.0);
#endif
    let weights = in.color;
    let normal = normalize(in.world_normal);
    let p = in.world_position.xyz;

    var color = vec3<f32>(0.0);
    var rough = 0.85;
    for (var i = 0u; i < 4u; i = i + 1u) {
        let id = u32(ids[i] + 0.5);
        let w = weights[i];
        if w > 0.001 {
            let idx = min(id, 4u);
            let props = terrain_props(idx);
            color += sample_color(idx, p, normal, props.x) * w;
            rough += props.y * w;
        }
    }

    let slope = 1.0 - abs(normal.y);
    color = mix(color, params.color2.rgb, slope * 0.35);

    return vec4<f32>(color, 1.0);
}
