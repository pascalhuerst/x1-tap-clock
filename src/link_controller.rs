use ableton_link::{Clock, Link, SessionState};

/// Simple wrapper around the `ableton_link` crate that provides a convenient,
/// ergonomic Rust API for tempo and transport control.
///
/// The struct manages enabling Link, start/stop sync, and exposes helpers
/// for setting tempo / playing state from application threads.
pub struct LinkController {
    link: Link,
}

impl LinkController {
    /// Create a new Ableton Link controller with the provided initial tempo (in BPM).
    pub fn new(initial_bpm: f64) -> Self {
        let mut link = Link::new(initial_bpm);
        link.enable_start_stop_sync(true);
        link.enable(true);
        Self { link }
    }

    /// Access the underlying link clock (in microseconds).
    pub fn clock(&self) -> Clock {
        self.link.clock()
    }

    /// Set the transport tempo (in BPM) at the current clock time.
    pub fn set_tempo(&mut self, bpm: f64) {
        let now = self.link.clock().micros();
        let mut state_opt = None;
        self.link
            .with_app_session_state(|state| state_opt = Some(state));
        if let Some(mut state) = state_opt {
            state.set_tempo(bpm, now);
            self.link.commit_app_session_state(state);
        }
    }

    /// Toggle the Link playing state at the current clock time.
    pub fn set_playing(&mut self, playing: bool) {
        let now = self.link.clock().micros();
        let mut state_opt = None;
        self.link
            .with_app_session_state(|state| state_opt = Some(state));
        if let Some(mut state) = state_opt {
            state.set_is_playing(playing, now);
            self.link.commit_app_session_state(state);
        }
    }

    /// Atomically set both tempo and playing state.
    #[allow(dead_code)]
    pub fn set_tempo_and_playing(&mut self, bpm: f64, playing: bool) {
        let now = self.link.clock().micros();
        let mut state_opt = None;
        self.link
            .with_app_session_state(|state| state_opt = Some(state));
        if let Some(mut state) = state_opt {
            state.set_tempo(bpm, now);
            state.set_is_playing(playing, now);
            self.link.commit_app_session_state(state);
        }
    }

    /// Inspect the current session state via a closure.
    pub fn with_session_state<F>(&self, mut f: F)
    where
        F: FnMut(SessionState),
    {
        let mut state_opt = None;
        self.link
            .with_app_session_state(|state| state_opt = Some(state));
        if let Some(state) = state_opt {
            f(state);
        }
    }
}
