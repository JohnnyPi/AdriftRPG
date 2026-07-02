// crates/game_bevy/src/data/watcher.rs
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use bevy::prelude::*;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::data::{debounce_duration, is_yaml_path};

#[derive(Resource)]
pub struct YamlWatcher {
    inner: Arc<Mutex<InnerState>>,
    _watcher: RecommendedWatcher,
}

struct InnerState {
    pending: bool,
    last_event: Option<Instant>,
}

impl YamlWatcher {
    pub fn new(assets_root: &Path) -> Self {
        let inner = Arc::new(Mutex::new(InnerState {
            pending: false,
            last_event: None,
        }));

        let callback_state = Arc::clone(&inner);
        let mut watcher = notify::recommended_watcher(
            move |result: Result<Event, notify::Error>| {
                let Ok(event) = result else {
                    return;
                };

                if !matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                ) {
                    return;
                }

                if !event.paths.iter().any(|path| is_yaml_path(path)) {
                    return;
                }

                if let Ok(mut state) = callback_state.lock() {
                    state.pending = true;
                    state.last_event = Some(Instant::now());
                }
            },
        )
        .expect("yaml watcher");

        watcher
            .watch(assets_root, RecursiveMode::Recursive)
            .expect("watch assets directory");

        Self {
            inner,
            _watcher: watcher,
        }
    }

    pub fn drain_pending(&self) -> bool {
        let mut state = self.inner.lock().expect("watcher state");
        if !state.pending {
            return false;
        }

        let Some(last_event) = state.last_event else {
            state.pending = false;
            return false;
        };

        if last_event.elapsed() < debounce_duration() {
            return false;
        }

        state.pending = false;
        state.last_event = None;
        true
    }
}
