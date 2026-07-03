#import bevy_pbr::{
    mesh_view_bindings::view,
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
}

struct SkyParams {
    zenith: vec4<f32>,
    horizon: vec4<f32>,
    sun_dir: vec4<f32>,
    /// `.x` = mie strength, `.y` = sun elevation (deg), `.z` = star opacity, `.w` = sun disc radius
    mie: vec4<f32>,
    /// moon direction xyz, angular radius in `.w`
    moon: vec4<f32>,
    /// `.x` = moon brightness, `.y` = moon phase, `.z` = cloud opacity, `.w` = night mix
    celestial: vec4<f32>,
    /// `.x/.y` = cloud scroll, `.z` = cloud speed factor, `.w` = cloud altitude
    clouds: vec4<f32>,
    night_zenith: vec4<f32>,
    night_horizon: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> sky: SkyParams;

fn hash21(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn star_field(view_dir: vec3<f32>, density: f32) -> f32 {
    let uv = vec2<f32>(atan2(view_dir.z, view_dir.x), asin(clamp(view_dir.y, -1.0, 1.0)));
    let cell = floor(uv * vec2<f32>(180.0, 90.0));
    let rnd = hash21(cell);
    if rnd > density {
        return 0.0;
    }
    let sparkle = pow(rnd, 12.0);
    return sparkle * smoothstep(0.02, 0.18, view_dir.y);
}

fn cloud_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
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
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let view_dir = normalize(in.world_position.xyz - view.world_position);
    let up = max(view_dir.y, 0.0);
    let horizon_mix = pow(1.0 - up, 2.0);

    let night_mix = sky.celestial.w;
    let day_zenith = mix(sky.zenith.rgb, sky.night_zenith.rgb, night_mix);
    let day_horizon = mix(sky.horizon.rgb, sky.night_horizon.rgb, night_mix);
    var color = mix(day_zenith, day_horizon, horizon_mix);

    let sun_dot = max(dot(view_dir, normalize(sky.sun_dir.xyz)), 0.0);
    let sun_disc = smoothstep(1.0 - sky.mie.w, 1.0, sun_dot);
    let mie_glow = pow(sun_dot, 8.0) * sky.mie.x * 0.35;
    color += vec3<f32>(1.0, 0.95, 0.85) * (sun_disc + mie_glow) * (1.0 - night_mix * 0.85);

    let moon_dot = max(dot(view_dir, normalize(sky.moon.xyz)), 0.0);
    let moon_disc = smoothstep(1.0 - sky.moon.w, 1.0, moon_dot);
    let moon_tint = vec3<f32>(0.82, 0.86, 0.95);
    color += moon_tint * moon_disc * sky.celestial.x * sky.celestial.y;

    let star_fade = sky.mie.z * (1.0 - smoothstep(-6.0, 12.0, sky.mie.y));
    color += vec3<f32>(1.0) * star_field(view_dir, 0.55) * star_fade;

    let cloud_uv = vec2<f32>(view_dir.x, view_dir.z) * 0.35 + sky.clouds.xy;
    let cloud_sample = cloud_noise(cloud_uv * 3.0) * cloud_noise(cloud_uv * 7.0 + 4.2);
    let cloud_mask = smoothstep(sky.clouds.w, sky.clouds.w + 0.35, up) * cloud_sample;
    let cloud_color = mix(day_horizon, vec3<f32>(1.0), 0.35);
    color = mix(color, cloud_color, cloud_mask * sky.celestial.z);

    return vec4<f32>(color, 1.0);
}
