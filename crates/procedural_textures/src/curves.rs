// crates/procedural_textures/src/curves.rs
//! Shared curve and ramp utilities for texture graphs and classifiers.

pub use shared::math::{remap, smoothstep};

#[derive(Clone, Debug)]
pub struct ColorStop {
    pub position: f32,
    pub color: [f32; 3],
}

pub fn sample_color_ramp(stops: &[ColorStop], t: f32) -> [f32; 3] {
    if stops.is_empty() {
        return [0.5, 0.5, 0.5];
    }
    if stops.len() == 1 {
        return stops[0].color;
    }
    let t = t.clamp(0.0, 1.0);
    for window in stops.windows(2) {
        let a = &window[0];
        let b = &window[1];
        if t <= b.position || window.len() == 2 && t >= a.position && t <= b.position {
            let span = (b.position - a.position).max(f32::EPSILON);
            let local = ((t - a.position) / span).clamp(0.0, 1.0);
            return [
                a.color[0] + (b.color[0] - a.color[0]) * local,
                a.color[1] + (b.color[1] - a.color[1]) * local,
                a.color[2] + (b.color[2] - a.color[2]) * local,
            ];
        }
    }
    stops.last().map(|s| s.color).unwrap_or([0.5, 0.5, 0.5])
}

pub fn parse_hex_color(hex: &str) -> Option<[f32; 3]> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
    Some([r, g, b])
}

#[cfg(test)]
mod tests {
    use super::smoothstep;

    #[test]
    fn smoothstep_boundaries() {
        assert_eq!(smoothstep(0.0, 1.0, -0.5), 0.0);
        assert_eq!(smoothstep(0.0, 1.0, 1.5), 1.0);
        assert!((smoothstep(0.0, 1.0, 0.5) - 0.5).abs() < f32::EPSILON);
    }
}
