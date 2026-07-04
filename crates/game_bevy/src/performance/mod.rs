// crates/game_bevy/src/performance/mod.rs
//! Release-build FPS validation (VS1 §30, VS2 performance acceptance).

use std::collections::VecDeque;

use bevy::prelude::*;
use tracing::info;

const TARGET_FPS: f32 = 60.0;
const DEFAULT_BENCHMARK_SECS: f32 = 30.0;

#[derive(Resource, Debug)]
pub struct PerformanceTracker {
    pub frame_times_ms: VecDeque<f32>,
    pub capacity: usize,
    pub benchmark_secs: Option<f32>,
    pub benchmark_started: bool,
    pub finished: bool,
}

impl Default for PerformanceTracker {
    fn default() -> Self {
        Self {
            frame_times_ms: VecDeque::new(),
            capacity: 7200,
            benchmark_secs: benchmark_duration_from_env(),
            benchmark_started: false,
            finished: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PerformanceReport {
    pub sample_count: usize,
    pub avg_fps: f32,
    pub min_fps: f32,
    pub one_percent_low_fps: f32,
    pub avg_frame_ms: f32,
    pub max_frame_ms: f32,
    pub target_fps: f32,
    pub meets_target: bool,
}

impl PerformanceReport {
    pub fn from_samples(samples: &[f32], target_fps: f32) -> Self {
        if samples.is_empty() {
            return Self {
                sample_count: 0,
                avg_fps: 0.0,
                min_fps: 0.0,
                one_percent_low_fps: 0.0,
                avg_frame_ms: 0.0,
                max_frame_ms: 0.0,
                target_fps,
                meets_target: false,
            };
        }

        let mut sorted = samples.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let avg_ms: f32 = samples.iter().sum::<f32>() / samples.len() as f32;
        let max_ms = *sorted.last().unwrap_or(&0.0);
        let min_ms = *sorted.first().unwrap_or(&0.0);
        let _ = min_ms;
        let p99_index =
            ((samples.len() as f32 * 0.99) as usize).min(sorted.len().saturating_sub(1));
        let p99_ms = sorted[p99_index];

        let avg_fps = 1000.0 / avg_ms.max(0.001);
        let one_percent_low_fps = 1000.0 / p99_ms.max(0.001);

        Self {
            sample_count: samples.len(),
            avg_fps,
            min_fps: 1000.0 / max_ms.max(0.001),
            one_percent_low_fps,
            avg_frame_ms: avg_ms,
            max_frame_ms: max_ms,
            target_fps,
            meets_target: one_percent_low_fps >= target_fps,
        }
    }

    pub fn format_summary(&self) -> String {
        format!(
            "FPS validation ({}/{} samples)\n\
             Average: {:.1} FPS ({:.2} ms)\n\
             1% low: {:.1} FPS (p99 {:.2} ms)\n\
             Min: {:.1} FPS (max {:.2} ms)\n\
             Target: {:.0} FPS @ 2560x1440 — {}",
            self.sample_count,
            self.sample_count,
            self.avg_fps,
            self.avg_frame_ms,
            self.one_percent_low_fps,
            1000.0 / self.one_percent_low_fps.max(0.001),
            self.min_fps,
            self.max_frame_ms,
            self.target_fps,
            if self.meets_target { "PASS" } else { "REVIEW" }
        )
    }
}

pub struct PerformanceValidationPlugin;

impl Plugin for PerformanceValidationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PerformanceTracker>()
            .add_systems(Update, (record_frame_time, finalize_benchmark).chain());
    }
}

fn record_frame_time(time: Res<Time>, mut tracker: ResMut<PerformanceTracker>) {
    if tracker.finished {
        return;
    }
    if tracker.benchmark_secs.is_some() && !tracker.benchmark_started {
        tracker.benchmark_started = true;
    }

    let ms = time.delta_secs() * 1000.0;
    if tracker.frame_times_ms.len() >= tracker.capacity {
        tracker.frame_times_ms.pop_front();
    }
    tracker.frame_times_ms.push_back(ms);
}

fn finalize_benchmark(time: Res<Time>, mut tracker: ResMut<PerformanceTracker>) {
    let Some(duration) = tracker.benchmark_secs else {
        return;
    };
    if tracker.finished || !tracker.benchmark_started {
        return;
    }
    if time.elapsed_secs() < duration {
        return;
    }

    tracker.finished = true;
    let samples: Vec<f32> = tracker.frame_times_ms.iter().copied().collect();
    let report = PerformanceReport::from_samples(&samples, TARGET_FPS);
    info!(target: "fps_validation", "{}", report.format_summary());
    eprintln!("{}", report.format_summary());

    if std::env::var("RPG_ADRIFT_FPS_EXIT")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(true)
    {
        std::process::exit(if report.meets_target { 0 } else { 2 });
    }
}

fn benchmark_duration_from_env() -> Option<f32> {
    std::env::var("RPG_ADRIFT_FPS_BENCHMARK")
        .ok()
        .and_then(|raw| raw.parse::<f32>().ok())
        .map(|secs| secs.max(5.0))
        .or_else(|| {
            std::env::var("RPG_ADRIFT_FPS_BENCHMARK")
                .ok()
                .filter(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .map(|_| DEFAULT_BENCHMARK_SECS)
        })
}

pub fn terrain_pipeline_within_budget(density_ms: f32, mesh_ms: f32, chunk_count: usize) -> bool {
    let density_per_chunk = density_ms / chunk_count.max(1) as f32;
    let mesh_per_chunk = mesh_ms / chunk_count.max(1) as f32;
    density_per_chunk <= 2.5 && mesh_per_chunk <= 4.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_flags_sub_60_one_percent_low() {
        let samples: Vec<f32> = (0..100)
            .map(|i| if i == 99 { 25.0 } else { 16.0 })
            .collect();
        let report = PerformanceReport::from_samples(&samples, 60.0);
        assert!(report.avg_fps > 60.0);
        assert!(!report.meets_target);
    }

    #[test]
    fn steady_60fps_passes_validation() {
        let samples = vec![16.5; 120];
        let report = PerformanceReport::from_samples(&samples, 60.0);
        assert!(report.meets_target);
    }

    #[test]
    fn terrain_pipeline_budget_thresholds() {
        assert!(terrain_pipeline_within_budget(25.0, 40.0, 10));
        assert!(!terrain_pipeline_within_budget(30.0, 50.0, 10));
    }
}
