#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
}

struct WaterParams {
    shallow_color: vec4<f32>,
    deep_color: vec4<f32>,
    wave: vec4<f32>,
    animation: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> params: WaterParams;

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let wave = sin((vertex.position.x + vertex.position.z) * 0.15 + params.animation.x * params.wave.y)
        * params.wave.z;
    var pos = vertex.position;
    pos.y += wave;
    out.position = position_world_to_clip(pos);
    out.world_position = vec4<f32>(pos, 1.0);
    out.world_normal = vec3<f32>(0.0, 1.0, 0.0);
#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = in.world_position.xyz;
    let depth = clamp((params.wave.x - world_pos.y + 1.5) * 0.35, 0.0, 1.0);
    let color = mix(params.shallow_color.rgb, params.deep_color.rgb, depth);
    let view = normalize(-world_pos);
    let fresnel = pow(1.0 - max(dot(view, in.world_normal), 0.0), 3.0);
    let foam = smoothstep(0.0, 0.15, sin(world_pos.x * 0.5 + params.animation.x) * 0.5 + 0.5) * 0.08;
    return vec4<f32>(color + fresnel * 0.15 + foam, params.wave.w);
}
