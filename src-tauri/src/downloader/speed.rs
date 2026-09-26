use std::collections::VecDeque;
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(5);

/// Download speed averaged over a rolling window, so the UI shows a stable
/// number instead of jittery instantaneous values.
#[derive(Debug, Default)]
pub struct SpeedMeter {
    samples: VecDeque<(Instant, u64)>,
}

impl SpeedMeter {
    pub fn record(&mut self, at: Instant, total_bytes: u64) {
        // A restart from zero (e.g. the server dropped range support) invalidates history.
        if self.samples.back().is_some_and(|&(_, last)| total_bytes < last) {
            self.samples.clear();
        }
        self.samples.push_back((at, total_bytes));
        while self.samples.len() > 2 && self.samples.front().is_some_and(|&(t, _)| at.duration_since(t) > WINDOW) {
            self.samples.pop_front();
        }
    }

    /// Bytes per second.
    pub fn speed(&self) -> f64 {
        match (self.samples.front(), self.samples.back()) {
            (Some(&(t0, b0)), Some(&(t1, b1))) if t1 > t0 => (b1 - b0) as f64 / (t1 - t0).as_secs_f64(),
            _ => 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn averages_over_window() {
        let mut m = SpeedMeter::default();
        let t0 = Instant::now();
        m.record(t0, 0);
        assert_eq!(m.speed(), 0.0);
        m.record(t0 + Duration::from_secs(1), 1000);
        m.record(t0 + Duration::from_secs(2), 1000); // stall
        assert!((m.speed() - 500.0).abs() < 1e-6);
        // Old samples fall out of the window.
        m.record(t0 + Duration::from_secs(10), 11_000);
        m.record(t0 + Duration::from_secs(11), 12_000);
        assert!((m.speed() - 1000.0).abs() < 1e-6);
    }

    #[test]
    fn restart_clears_history() {
        let mut m = SpeedMeter::default();
        let t0 = Instant::now();
        m.record(t0, 5000);
        m.record(t0 + Duration::from_secs(1), 10);
        assert_eq!(m.speed(), 0.0);
    }
}
