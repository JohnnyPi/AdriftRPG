#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::view

struct StarParams {
    density: f32,
    sun_elevation_deg: f32,
    _pad0: f32,
    _pad1: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> params: StarParams;

fn hash33(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn star_field(view_dir: vec3<f32>, density: f32) -> f32 {
    let uv = view_dir * 800.0;
    let cell = floor(uv);
    let f = fract(uv);
    var brightness = 0.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let g = cell + vec2<f32>(f32(x), f32(y));
            let h = hash33(vec3<f32>(g.x, g.y, 17.0));
            if h > density {
                continue;
            }
            let star = vec2<f32>(h, fract(h * 7.13));
            let d = length(f - vec2<f32>(f32(x), f32(y)) - star);
            brightness = brightness + smoothstep(0.035, 0.0, d) * (0.4 + h * 0.6);
        }
    }
    return brightness;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let view_dir = normalize(in.world_position.xyz - view.world_position.xyz);
    let star_fade = clamp((-params.sun_elevation_deg - 2.0) / 10.0, 0.0, 1.0);
    let stars = star_field(view_dir, params.density) * star_fade;
    return vec4<f32>(vec3<f32>(stars), stars * 0.95);
}
