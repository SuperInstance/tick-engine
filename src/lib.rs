/// A single tick in the temporal clock.
#[derive(Debug, Clone)]
pub struct Tick {
    pub id: u64,
    pub timestamp: f64,
    pub delta: f64,
}

impl Tick {
    pub fn new(id: u64, timestamp: f64) -> Self {
        Self {
            id,
            timestamp,
            delta: 0.0,
        }
    }
}

/// A repeating tick schedule driven by BPM with optional swing.
#[derive(Debug, Clone)]
pub struct TickSchedule {
    pub bpm: f64,
    pub swing: f64,
    pub next_tick: u64,
}

impl TickSchedule {
    pub fn new(bpm: f64, swing: f64) -> Self {
        Self {
            bpm,
            swing: swing.clamp(0.0, 1.0),
            next_tick: 0,
        }
    }

    /// Base interval between beats in seconds.
    pub fn tick_interval(&self) -> f64 {
        60.0 / self.bpm
    }

    /// Swing offset applied to off-beat ticks.
    /// On-beats (even tick ids) get no offset.
    /// Off-beats get `interval * swing * 0.33` added.
    pub fn swing_offset(&self, tick_id: u64) -> f64 {
        if tick_id % 2 == 1 {
            self.tick_interval() * self.swing * 0.33
        } else {
            0.0
        }
    }

    /// Advance to the next tick, returning it with computed timing.
    pub fn next_tick(&mut self) -> Tick {
        let id = self.next_tick;
        let interval = self.tick_interval();
        let swing = self.swing_offset(id);
        let delta = interval + swing;
        let timestamp = id as f64 * interval + swing;

        self.next_tick += 1;

        Tick {
            id,
            timestamp,
            delta,
        }
    }
}

/// Adaptive tempo with energy-based BPM adjustment.
#[derive(Debug, Clone)]
pub struct Tempo {
    pub bpm: f64,
    pub min_bpm: f64,
    pub max_bpm: f64,
}

impl Tempo {
    pub fn new(bpm: f64) -> Self {
        Self {
            bpm,
            min_bpm: 30.0,
            max_bpm: 300.0,
        }
    }

    /// Adjust BPM based on energy level (0.0–1.0).
    /// High energy → faster, low energy → slower.
    pub fn adapt(&mut self, energy: f64) {
        let energy = energy.clamp(0.0, 1.0);
        // Map energy 0→-20% change, energy 1→+20% change
        let factor = 1.0 + (energy - 0.5) * 0.4;
        self.bpm = self.bpm * factor;
        self.clamp();
    }

    /// Keep BPM within min/max bounds.
    pub fn clamp(&mut self) {
        self.bpm = self.bpm.clamp(self.min_bpm, self.max_bpm);
    }
}

/// A countdown event scheduled N ticks in the future.
#[derive(Debug, Clone)]
pub struct TMinusEvent {
    pub ticks_until: u64,
    pub action: String,
    pub priority: f64,
}

impl TMinusEvent {
    pub fn new(ticks_until: u64, action: String, priority: f64) -> Self {
        Self {
            ticks_until,
            action,
            priority,
        }
    }

    /// Decrement the tick counter.
    pub fn tick(&mut self) {
        if self.ticks_until > 0 {
            self.ticks_until -= 1;
        }
    }

    /// Is this event ready to fire?
    pub fn is_ready(&self) -> bool {
        self.ticks_until == 0
    }

    /// Compare priority with another event.
    /// Returns `Ordering` based on priority (higher = sooner).
    pub fn compare_priority(&self, other: &TMinusEvent) -> std::cmp::Ordering {
        self.priority.partial_cmp(&other.priority).unwrap_or(std::cmp::Ordering::Equal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_tick() {
        let t = Tick::new(1, 0.5);
        assert_eq!(t.id, 1);
        assert!((t.timestamp - 0.5).abs() < f64::EPSILON);
        assert!((t.delta).abs() < f64::EPSILON);
    }

    #[test]
    fn test_tick_interval_120_bpm() {
        let s = TickSchedule::new(120.0, 0.0);
        assert!((s.tick_interval() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_tick_interval_60_bpm() {
        let s = TickSchedule::new(60.0, 0.0);
        assert!((s.tick_interval() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_swing_offset_zero() {
        let s = TickSchedule::new(120.0, 0.0);
        // Even tick
        assert!((s.swing_offset(0)).abs() < 1e-9);
        // Odd tick — swing is 0 so still no offset
        assert!((s.swing_offset(1)).abs() < 1e-9);
    }

    #[test]
    fn test_swing_offset_max() {
        let s = TickSchedule::new(120.0, 1.0);
        let expected = 0.5 * 1.0 * 0.33; // interval * swing * 0.33
        assert!((s.swing_offset(1) - expected).abs() < 1e-9);
    }

    #[test]
    fn test_swing_only_offbeats() {
        let s = TickSchedule::new(120.0, 1.0);
        // Even ticks (on-beats) should have zero swing
        assert!((s.swing_offset(0)).abs() < 1e-9);
        assert!((s.swing_offset(2)).abs() < 1e-9);
        assert!((s.swing_offset(4)).abs() < 1e-9);
        // Odd ticks (off-beats) should have swing
        assert!(s.swing_offset(1) > 0.0);
        assert!(s.swing_offset(3) > 0.0);
    }

    #[test]
    fn test_next_tick_advances() {
        let mut s = TickSchedule::new(120.0, 0.0);
        let t0 = s.next_tick();
        assert_eq!(t0.id, 0);
        let t1 = s.next_tick();
        assert_eq!(t1.id, 1);
        assert_eq!(s.next_tick, 2);
    }

    #[test]
    fn test_tempo_adapt_high_energy() {
        let mut t = Tempo::new(120.0);
        t.adapt(1.0);
        assert!(t.bpm > 120.0, "high energy should increase bpm, got {}", t.bpm);
    }

    #[test]
    fn test_tempo_adapt_low_energy() {
        let mut t = Tempo::new(120.0);
        t.adapt(0.0);
        assert!(t.bpm < 120.0, "low energy should decrease bpm, got {}", t.bpm);
    }

    #[test]
    fn test_tempo_clamp() {
        let mut t = Tempo::new(120.0);
        t.bpm = 500.0;
        t.clamp();
        assert!((t.bpm - 300.0).abs() < 1e-9);
        t.bpm = 5.0;
        t.clamp();
        assert!((t.bpm - 30.0).abs() < 1e-9);
    }

    #[test]
    fn test_tminus_decrements() {
        let mut e = TMinusEvent::new(3, "fire".into(), 1.0);
        e.tick();
        assert_eq!(e.ticks_until, 2);
        e.tick();
        assert_eq!(e.ticks_until, 1);
        e.tick();
        assert_eq!(e.ticks_until, 0);
    }

    #[test]
    fn test_tminus_is_ready() {
        let mut e = TMinusEvent::new(1, "fire".into(), 1.0);
        assert!(!e.is_ready());
        e.tick();
        assert!(e.is_ready());
    }

    #[test]
    fn test_priority_comparison() {
        let a = TMinusEvent::new(1, "a".into(), 0.9);
        let b = TMinusEvent::new(1, "b".into(), 0.5);
        assert_eq!(a.compare_priority(&b), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_multiple_ticks_accumulate_time() {
        let mut s = TickSchedule::new(60.0, 0.0);
        let mut total = 0.0;
        for _ in 0..4 {
            let t = s.next_tick();
            total += t.delta;
        }
        // 4 ticks at 60 bpm = 4.0 seconds
        assert!((total - 4.0).abs() < 1e-9);
    }

    #[test]
    fn test_extreme_bpm() {
        let mut s = TickSchedule::new(300.0, 0.0);
        let t = s.next_tick();
        assert!((t.delta - 0.2).abs() < 1e-9);

        let mut s2 = TickSchedule::new(30.0, 0.0);
        let t2 = s2.next_tick();
        assert!((t2.delta - 2.0).abs() < 1e-9);
    }
}
