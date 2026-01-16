#![allow(dead_code)]

/// Tap-based tempo estimator.
///
/// Typical usage:
///
/// ```
/// let mut tapper = TapTempo::new(4, 2.0);
/// if let Some(bpm) = tapper.add_tap(timestamp_secs) {
///     println!("Detected tempo: {bpm}");
/// }
/// ```
#[derive(Debug, Clone)]
pub struct TapTempo {
    taps_needed: usize,
    reset_gap: f64,
    taps: Vec<f64>,
}

impl TapTempo {
    /// Create a new tap-tempo helper.
    ///
    /// * `taps_needed` – number of taps required before a BPM is produced.
    /// * `reset_gap` – if the time between consecutive taps exceeds this many seconds,
    ///   the tap history is cleared.
    pub fn new(taps_needed: usize, reset_gap: f64) -> Self {
        assert!(
            taps_needed >= 2,
            "at least two taps are required to compute BPM"
        );
        assert!(reset_gap >= 0.0, "reset gap must be non-negative");

        Self {
            taps_needed,
            reset_gap,
            taps: Vec::with_capacity(taps_needed),
        }
    }

    /// Register a tap at the supplied timestamp (seconds).
    ///
    /// Returns `Some(bpm)` when enough taps have been collected to estimate the tempo,
    /// otherwise returns `None`.
    pub fn add_tap(&mut self, timestamp_sec: f64) -> Option<f64> {
        if let Some(&last) = self.taps.last() {
            if timestamp_sec - last > self.reset_gap {
                self.taps.clear();
            }
        }

        self.taps.push(timestamp_sec);

        if self.taps.len() < self.taps_needed {
            return None;
        }

        let mut sum = 0.0;
        for window in self.taps.windows(2) {
            sum += window[1] - window[0];
        }

        let avg_interval = sum / (self.taps.len() - 1) as f64;
        self.taps.clear();

        if avg_interval > 0.0 {
            Some(60.0 / avg_interval)
        } else {
            None
        }
    }

    /// Reset the tap history explicitly.
    pub fn reset(&mut self) {
        self.taps.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::TapTempo;

    #[test]
    fn computes_expected_bpm() {
        let mut tapper = TapTempo::new(4, 2.0);

        let taps = [0.0, 0.5, 1.0, 1.5]; // 120 BPM
        let mut bpm = None;
        for &t in &taps {
            bpm = tapper.add_tap(t);
        }

        assert!(bpm.is_some());
        assert!((bpm.unwrap() - 120.0).abs() < 1e-6);
    }

    #[test]
    fn resets_after_gap() {
        let mut tapper = TapTempo::new(3, 1.0);

        assert!(tapper.add_tap(0.0).is_none());
        assert!(tapper.add_tap(0.3).is_none());
        // big gap -> reset
        assert!(tapper.add_tap(2.0).is_none());
        assert!(tapper.add_tap(2.2).is_none());
        assert!(tapper.add_tap(2.4).is_some());
    }
}
