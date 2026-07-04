// assets/shaders/water.wgsl
//
// Basic water surface for WaterMaterial (crates/game_bevy/src/water/mod.rs).
//
// Uniform packing (must match WaterParams in mod.rs):
//   shallow_color: rgb = shallow tint, a = transparency
//   deep_color:    rgb = deep tint
//   wave:          x = surface elevation (m), y = wave_speed,
//                  z = wave_amplitude, w = transparency
//   animation:     x = elapsed time (s), y = foam_strength, z = wave_count tier
//
// This is intentionally self-contained (no depth prepass required): apparent
// depth is approximated from the view angle, so the plane reads shallow near
// grazing sight lines at the shore and deep when looking straight down.
// If a DepthPrepass camera is added later, this is the file to upgrade with
// prepass_depth-based true water-column depth.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::view

struct WaterParams {
    shallow_color: vec4<f32>,
    deep_color: vec4<f32>,
    wave: vec4<f32>,
    animation: vec4<f32>,
};

struct WaterLightingParams {
    sun_dir: vec4<f32>,
    sky_tint: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> params: WaterParams;

@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var<uniform> lighting: WaterLightingParams;

// Sum of a few directional gerstner-lite sine waves; returns height and
// analytic XZ gradient so the normal is stable (no per-pixel noise "static").
fn wave_field(p: vec2<f32>, t: f32, speed: f32, amplitude: f32) -> vec3<f32> {
    var height = 0.0;
    var grad = vec2<f32>(0.0, 0.0);

    // direction, frequency (rad/m), relative amplitude
    let dirs = array<vec2<f32>, 4>(
        normalize(vec2<f32>( 1.0,  0.30)),
        normalize(vec2<f32>(-0.55, 1.0)),
        normalize(vec2<f32>( 0.20, -1.0)),
        normalize(vec2<f32>(-1.0, -0.45)),
    );
    let freqs = array<f32, 4>(0.35, 0.55, 0.90, 1.60);
    let amps  = array<f32, 4>(0.50, 0.28, 0.15, 0.07);
    let speeds = array<f32, 4>(1.00, 1.35, 1.80, 2.40);

    let wave_count = max(u32(params.animation.z), 1u);

    for (var i = 0u; i < wave_count; i = i + 1u) {
        let d = dirs[i];
        let f = freqs[i];
        let a = amps[i] * amplitude;
        let phase = dot(d, p) * f + t * speeds[i] * speed;
        height = height + a * sin(phase);
        grad = grad + d * (a * f * cos(phase));
    }
    return vec3<f32>(height, grad.x, grad.y);
}

// Normalize that never returns NaN: a zero-length input (e.g. an unset sun
// direction before lighting is synced) falls back instead of poisoning the
// pixel, which would otherwise render black.
fn safe_normalize(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let len = length(v);
    if (len > 1e-5) {
        return v / len;
    }
    return fallback;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = in.world_position.xyz;
    let t = params.animation.x;
    let wave_speed = max(params.wave.y, 0.0);
    let wave_amplitude = max(params.wave.z, 0.0);
    let transparency = clamp(params.wave.w, 0.0, 1.0);

    // Animated surface normal from the analytic wave gradient, blended over
    // the mesh normal so river ribbons (non-horizontal) still behave.
    let w = wave_field(world_pos.xz, t, wave_speed, wave_amplitude);
    let wave_normal = normalize(vec3<f32>(-w.y, 1.0, -w.z));
    let base_normal = safe_normalize(in.world_normal, vec3<f32>(0.0, 1.0, 0.0));
    let n = safe_normalize(mix(base_normal, wave_normal, 0.85), base_normal);

    let view_dir = safe_normalize(view.world_position.xyz - world_pos, vec3<f32>(0.0, 1.0, 0.0));
    let n_dot_v = clamp(dot(n, view_dir), 0.0, 1.0);

    // Schlick fresnel, F0 ~ 0.02 for water.
    let fresnel = 0.02 + 0.98 * pow(1.0 - n_dot_v, 5.0);

    // Apparent-depth proxy: grazing angles read shallow, top-down reads deep;
    // wave crests pull slightly toward the shallow tint for visible motion.
    let crest = clamp(w.x / max(wave_amplitude, 1e-3) * 0.5 + 0.5, 0.0, 1.0);
    let depth_factor = clamp(n_dot_v * 0.9 + 0.15 - crest * 0.18, 0.0, 1.0);
    var water_rgb = mix(params.shallow_color.rgb, params.deep_color.rgb, depth_factor);

    // Sky-ish reflection tint at grazing angles (from celestial fog inscattering).
    // Floor the reflected color to a dim fraction of the water's own tint: at
    // grazing angles fresnel approaches 1, so an unset or nighttime sky_tint
    // (which can be pure black before lighting is synced) would otherwise
    // reflect the surface to black. The floor keeps a plausible dim reflection
    // while still showing the real sky when it's brighter than the floor.
    let sky_tint = lighting.sky_tint.xyz;
    let reflection = max(sky_tint, water_rgb * 0.35);
    water_rgb = mix(water_rgb, reflection, fresnel * 0.85);

    // Sun glint aligned with the live directional sun.
    let sun_dir = safe_normalize(lighting.sun_dir.xyz, vec3<f32>(0.0, 1.0, 0.0));
    let refl = reflect(-view_dir, n);
    let glint = pow(clamp(dot(refl, sun_dir), 0.0, 1.0), 220.0);
    water_rgb = water_rgb + vec3<f32>(1.0, 0.98, 0.92) * glint * 0.8;

    // More opaque when looking across the surface or at deep color; the
    // authored transparency sets the floor seen when looking straight down.
    let alpha = clamp(mix(transparency, 1.0, max(fresnel, depth_factor * 0.35)), 0.0, 1.0);

    // Shoreline foam driven by shallow depth proxy and YAML foam_strength (animation.y).
    let foam_noise = fract(sin(dot(world_pos.xz, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    let foam = pow(1.0 - depth_factor, 2.5) * params.animation.y * foam_noise;
    water_rgb = mix(water_rgb, vec3<f32>(0.95, 0.98, 1.0), foam * 0.55);

    return vec4<f32>(max(water_rgb, vec3<f32>(0.0)), alpha);
}
