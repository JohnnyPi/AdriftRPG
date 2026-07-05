//! Seam metrics between overlapping regional windows.

use crate::fields::scalar::ScalarField;

#[derive(Clone, Debug, Default)]
pub struct SeamMetrics {
    pub max_abs_diff: f32,
    pub mean_abs_diff: f32,
    pub sample_count: u32,
}

pub fn compute_seam_metrics(residual: &ScalarField) -> SeamMetrics {
    let w = residual.descriptor.width;
    let h = residual.descriptor.height;
    if w < 2 || h < 2 {
        return SeamMetrics::default();
    }

    let mut max_diff = 0.0f32;
    let mut sum = 0.0f32;
    let mut count = 0u32;

    for z in 0..h {
        for x in 0..w - 1 {
            let d = (residual.get(x, z) - residual.get(x + 1, z)).abs();
            max_diff = max_diff.max(d);
            sum += d;
            count += 1;
        }
    }
    for z in 0..h - 1 {
        for x in 0..w {
            let d = (residual.get(x, z) - residual.get(x, z + 1)).abs();
            max_diff = max_diff.max(d);
            sum += d;
            count += 1;
        }
    }

    SeamMetrics {
        max_abs_diff: max_diff,
        mean_abs_diff: if count > 0 { sum / count as f32 } else { 0.0 },
        sample_count: count,
    }
}
