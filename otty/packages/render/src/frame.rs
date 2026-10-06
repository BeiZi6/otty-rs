//! Frame pacing.
//!
//! `max-fps` is the only render knob the shipped product has, and the schema
//! annotates it "macOS." with the reason: on a ProMotion display a terminal that
//! redraws on every byte of output costs CPU for frames nobody can see. `0`
//! means "follow the display", which is the default.
//!
//! The pacer is a *budget*, not a timer: it answers "may I draw now", and a
//! stream of output that arrives faster than the budget is coalesced rather than
//! queued. That distinction is the whole point — a queue would make the terminal
//! show stale frames in order, while coalescing shows the current one.

use std::time::{Duration, Instant};

/// The value of `max-fps`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MaxFps {
    /// `0` — draw as fast as the display presents.
    #[default]
    Display,
    /// A cap, in frames per second.
    Capped(u32),
}

impl MaxFps {
    /// Read the config value.
    ///
    /// Any number above zero is a cap; anything else — including a negative
    /// number, which the key's own parser would have rejected earlier — means
    /// "follow the display" rather than "draw zero frames".
    pub fn from_config(value: i64) -> Self {
        if value > 0 {
            Self::Capped(value as u32)
        } else {
            Self::Display
        }
    }

    /// The minimum interval between frames, or `None` when uncapped.
    pub fn interval(self) -> Option<Duration> {
        match self {
            Self::Display => None,
            Self::Capped(0) => None,
            Self::Capped(fps) => Some(Duration::from_nanos(1_000_000_000 / fps as u64)),
        }
    }
}

/// What the pacer has seen, for the About panel and a bug report.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FrameStats {
    /// Frames actually presented.
    pub presented: u64,
    /// Redraws that the budget swallowed because a frame was already due.
    pub coalesced: u64,
    /// The longest gap between two frames, in milliseconds.
    pub worst_gap_ms: u64,
}

impl FrameStats {
    /// The fraction of redraw requests that were coalesced, 0.0–1.0.
    pub fn coalesced_fraction(&self) -> f32 {
        let total = self.presented + self.coalesced;
        if total == 0 {
            0.0
        } else {
            self.coalesced as f32 / total as f32
        }
    }
}

/// Decides whether a frame may be drawn now.
#[derive(Debug)]
pub struct FramePacer {
    max_fps: MaxFps,
    last: Option<Instant>,
    stats: FrameStats,
}

impl FramePacer {
    pub fn new(max_fps: MaxFps) -> Self {
        Self { max_fps, last: None, stats: FrameStats::default() }
    }

    pub fn with_config_value(value: i64) -> Self {
        Self::new(MaxFps::from_config(value))
    }

    pub fn max_fps(&self) -> MaxFps {
        self.max_fps
    }

    pub fn stats(&self) -> FrameStats {
        self.stats
    }

    /// Change the cap at runtime, which is what `config set max-fps` does on a
    /// live window. The last frame's timestamp is kept, so a change takes effect
    /// from the next frame rather than resetting the budget.
    pub fn set_max_fps(&mut self, max_fps: MaxFps) {
        self.max_fps = max_fps;
    }

    /// Whether a frame may be presented at `now`.
    ///
    /// A refused frame is counted as coalesced rather than dropped, because
    /// "requested twice, drawn once" is exactly what coalescing means and is the
    /// number worth showing a user who wonders why raising the cap helped.
    pub fn allow(&mut self, now: Instant) -> bool {
        let Some(interval) = self.max_fps.interval() else {
            self.record(now);
            return true;
        };
        match self.last {
            Some(last) if now.duration_since(last) < interval => {
                self.stats.coalesced += 1;
                false
            }
            _ => {
                self.record(now);
                true
            }
        }
    }

    fn record(&mut self, now: Instant) {
        if let Some(last) = self.last {
            let gap = now.duration_since(last).as_millis() as u64;
            self.stats.worst_gap_ms = self.stats.worst_gap_ms.max(gap);
        }
        self.last = Some(now);
        self.stats.presented += 1;
    }

    /// How long to wait before the next frame may be drawn.
    ///
    /// `None` when a frame may be drawn now, so a caller can pass it straight to
    /// a wait instead of computing a zero-length sleep.
    pub fn delay_until_next(&self, now: Instant) -> Option<Duration> {
        let interval = self.max_fps.interval()?;
        let last = self.last?;
        let elapsed = now.duration_since(last);
        (elapsed < interval).then(|| interval - elapsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_means_follow_the_display() {
        assert_eq!(MaxFps::from_config(0), MaxFps::Display);
        assert_eq!(MaxFps::from_config(0).interval(), None);
        assert_eq!(MaxFps::default(), MaxFps::Display);
    }

    #[test]
    fn a_positive_value_is_a_cap_and_a_negative_one_is_not_a_zero_frame_cap() {
        assert_eq!(MaxFps::from_config(60), MaxFps::Capped(60));
        assert_eq!(MaxFps::from_config(60).interval(), Some(Duration::from_nanos(16_666_666)));
        // A negative value must not mean "draw nothing"; the display default is
        // the only reading that cannot freeze the terminal.
        assert_eq!(MaxFps::from_config(-5), MaxFps::Display);
    }

    #[test]
    fn an_uncapped_pacer_allows_every_frame() {
        let mut p = FramePacer::new(MaxFps::Display);
        let t0 = Instant::now();
        for i in 0..5 {
            assert!(p.allow(t0 + Duration::from_micros(i)));
        }
        assert_eq!(p.stats().presented, 5);
        assert_eq!(p.stats().coalesced, 0);
        assert_eq!(p.delay_until_next(t0), None);
    }

    #[test]
    fn the_first_frame_is_never_refused() {
        let mut p = FramePacer::with_config_value(60);
        assert!(p.allow(Instant::now()));
        assert_eq!(p.stats().presented, 1);
    }

    #[test]
    fn frames_inside_the_budget_are_coalesced_not_dropped() {
        let mut p = FramePacer::with_config_value(60);
        let t0 = Instant::now();
        assert!(p.allow(t0));
        // 5 ms later is inside a 16.6 ms budget.
        assert!(!p.allow(t0 + Duration::from_millis(5)));
        assert!(!p.allow(t0 + Duration::from_millis(10)));
        // Past the budget, a frame goes out.
        assert!(p.allow(t0 + Duration::from_millis(20)));
        assert_eq!(p.stats().presented, 2);
        assert_eq!(p.stats().coalesced, 2);
        // Two of four requests were swallowed.
        assert!((p.stats().coalesced_fraction() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn the_delay_reaches_exactly_the_budget_boundary() {
        let mut p = FramePacer::with_config_value(60);
        let t0 = Instant::now();
        assert!(p.allow(t0));
        let delay = p.delay_until_next(t0 + Duration::from_millis(6)).unwrap();
        assert_eq!(delay, Duration::from_nanos(16_666_666) - Duration::from_millis(6));
        // At the boundary there is nothing to wait for.
        assert_eq!(p.delay_until_next(t0 + Duration::from_nanos(16_666_666)), None);
    }

    #[test]
    fn the_worst_gap_is_recorded_from_presented_frames_only() {
        let mut p = FramePacer::with_config_value(1); // one frame a second
        let t0 = Instant::now();
        assert!(p.allow(t0));
        // 1.5 s later: past the budget, so this frame goes out (gap 1500 ms).
        assert!(p.allow(t0 + Duration::from_millis(1500)));
        // 100 ms after that is *inside* the new budget, so it is swallowed. A
        // refused frame must not contribute to the gap statistics — otherwise a
        // busy stream of output would make the worst gap look tiny.
        assert!(!p.allow(t0 + Duration::from_millis(1600)));
        // 1.1 s after the second frame: out it goes (gap 1100 ms).
        assert!(p.allow(t0 + Duration::from_millis(2600)));
        assert_eq!(p.stats().presented, 3);
        assert_eq!(p.stats().coalesced, 1);
        assert_eq!(p.stats().worst_gap_ms, 1500);
    }

    #[test]
    fn changing_the_cap_does_not_reset_the_budget() {
        let mut p = FramePacer::with_config_value(60);
        let t0 = Instant::now();
        assert!(p.allow(t0));
        p.set_max_fps(MaxFps::Display);
        assert_eq!(p.max_fps(), MaxFps::Display);
        // Uncapped now, so the very next request goes out.
        assert!(p.allow(t0 + Duration::from_micros(1)));

        let mut p = FramePacer::with_config_value(0);
        let t0 = Instant::now();
        assert!(p.allow(t0));
        p.set_max_fps(MaxFps::Capped(60));
        // Capping after the fact still applies to the next request.
        assert!(!p.allow(t0 + Duration::from_millis(1)));
    }

    #[test]
    fn a_coalescing_ratio_of_nothing_is_zero_not_a_division_error() {
        assert_eq!(FrameStats::default().coalesced_fraction(), 0.0);
    }
}
