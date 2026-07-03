// assets/shaders/clouds.wgsl — Level-1 animated cloud shell (SkyLightingGuide §15).

#import bevy_pbr::forward_io::VertexOutput

struct CloudParams {
    coverage: f32,
    wind: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>,
    horizon_color: vec4<f32>,
    shell: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> params: CloudParams;

fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    for (var i = 0; i < 4; i = i + 1) {
        value = value + amplitude * noise2(p * frequency);
        frequency = frequency * 2.03;
        amplitude = amplitude * 0.5;
    }
    return value;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = in.world_position.xyz;
    let wind_dir = params.wind.xy;
    let wind_speed = params.wind.z;
    let time = params.wind.w;

    let uv = world_pos.xz * 0.00035 + wind_dir * (time * wind_speed);

    var density = fbm(uv);
    density = density * 0.65 + fbm(uv * 2.1 + vec2<f32>(4.1, 1.7)) * 0.35;
    density = smoothstep(0.38, 0.72, density);

    let coverage = clamp(params.coverage, 0.0, 1.0);
    density = density * coverage;

    let n = normalize(in.world_normal);
    let sun_dir = normalize(params.sun_dir.xyz);
    let sun_lit = clamp(dot(n, sun_dir) * 0.5 + 0.5, 0.0, 1.0);

    let base = mix(params.horizon_color.rgb, vec3<f32>(1.0, 0.99, 0.96), sun_lit * 0.55);
    let shaded = mix(base * 0.72, base, sun_lit);
    let rgb = mix(shaded, params.sun_color.rgb, sun_lit * 0.18);

    let shell_y = params.shell.x;
    let fade = smoothstep(shell_y - 80.0, shell_y + 120.0, world_pos.y);
    let alpha = density * (1.0 - fade * 0.85);

    if (alpha < 0.02) {
        discard;
    }

    return vec4<f32>(rgb, alpha);
}
