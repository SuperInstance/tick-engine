//! tick-engine: Temporal coordination with Tick, TickSchedule, Tempo, Swing.
//! BPM-adaptive timing and T-minus events.

use std::fmt;

/// A single tick event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tick {
    pub count: u64,
    pub timestamp_ms: u64,
}

impl Tick {
    pub fn new(count: u64, timestamp_ms: u64) -> Self {
        Self { count, timestamp_ms }
    }

    pub fn next(&self, interval_ms: u64) -> Self {
        Self {
            count: self.count + 1,
            timestamp_ms: self.timestamp_ms + interval_ms,
        }
    }

    pub fn duration_since(&self, earlier: &Tick) -> i64 {
        self.timestamp_ms as i64 - earlier.timestamp_ms as i64
    }
}

/// A schedule of ticks with optional T-minus events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickSchedule {
    pub start_ms: u64,
    pub interval_ms: u64,
    pub total_ticks: Option<u64>,
    pub t_minus_events: Vec<TMinusEvent>,
}

impl TickSchedule {
    pub fn new(start_ms: u64, interval_ms: u64) -> Self {
        Self {
            start_ms,
            interval_ms,
            total_ticks: None,
            t_minus_events: Vec::new(),
        }
    }

    pub fn with_total_ticks(mut self, total: u64) -> Self {
        self.total_ticks = Some(total);
        self
    }

    pub fn add_t_minus(&mut self, label: impl Into<String>, at_tick: u64) {
        self.t_minus_events.push(TMinusEvent {
            label: label.into(),
            at_tick,
            triggered: false,
        });
    }

    /// Generate ticks up to a given timestamp.
    pub fn ticks_up_to(&self, now_ms: u64) -> Vec<Tick> {
        let mut ticks = Vec::new();
        let mut count = 0u64;
        let mut t = self.start_ms;
        while t <= now_ms {
            if let Some(max) = self.total_ticks {
                if count >= max {
                    break;
                }
            }
            ticks.push(Tick::new(count, t));
            count += 1;
            t += self.interval_ms;
        }
        ticks
    }

    /// Check for T-minus events that should fire at the current tick.
    pub fn check_t_minus(&mut self, tick: &Tick) -> Vec<String> {
        let mut fired = Vec::new();
        for event in &mut self.t_minus_events {
            if !event.triggered && tick.count >= event.at_tick {
                event.triggered = true;
                fired.push(event.label.clone());
            }
        }
        fired
    }

    pub fn reset_t_minus(&mut self) {
        for event in &mut self.t_minus_events {
            event.triggered = false;
        }
    }

    pub fn estimated_end_ms(&self) -> Option<u64> {
        self.total_ticks.map(|n| self.start_ms + (n - 1) * self.interval_ms)
    }
}

/// A T-minus event attached to a schedule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TMinusEvent {
    pub label: String,
    pub at_tick: u64,
    pub triggered: bool,
}

/// Tempo in beats per minute, with adaptive multiplier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tempo {
    pub bpm: f32,
    pub multiplier: f32,
}

impl Tempo {
    pub fn new(bpm: f32) -> Self {
        Self {
            bpm: bpm.max(0.0),
            multiplier: 1.0,
        }
    }

    pub fn with_multiplier(mut self, m: f32) -> Self {
        self.multiplier = m.max(0.0);
        self
    }

    /// Effective BPM.
    pub fn effective_bpm(&self) -> f32 {
        self.bpm * self.multiplier
    }

    /// Milliseconds per beat.
    pub fn ms_per_beat(&self) -> f32 {
        if self.effective_bpm() <= 0.0 {
            f32::INFINITY
        } else {
            60000.0 / self.effective_bpm()
        }
    }

    /// Adapt tempo toward a target BPM by a factor.
    pub fn adapt_toward(&mut self, target_bpm: f32, factor: f32) {
        let delta = target_bpm - self.bpm;
        self.bpm += delta * factor.clamp(0.0, 1.0);
        if self.bpm < 0.0 {
            self.bpm = 0.0;
        }
    }

    pub fn double_time(&mut self) {
        self.multiplier *= 2.0;
    }

    pub fn half_time(&mut self) {
        self.multiplier *= 0.5;
    }
}

impl fmt::Display for Tempo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1} BPM (x{:.2})", self.effective_bpm(), self.multiplier)
    }
}

/// Swing timing offset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Swing {
    pub ratio: f32,
    pub max_offset_ms: f32,
}

impl Default for Swing {
    fn default() -> Self {
        Self {
            ratio: 0.0,
            max_offset_ms: 0.0,
        }
    }
}

impl Swing {
    pub fn new(ratio: f32, max_offset_ms: f32) -> Self {
        Self {
            ratio: ratio.clamp(0.0, 1.0),
            max_offset_ms: max_offset_ms.max(0.0),
        }
    }

    /// Compute offset for a given subdivision.
    pub fn offset_ms(&self, subdivision: u64) -> f32 {
        if subdivision % 2 == 0 {
            0.0
        } else {
            self.ratio * self.max_offset_ms
        }
    }

    pub fn set_ratio(&mut self, ratio: f32) {
        self.ratio = ratio.clamp(0.0, 1.0);
    }

    pub fn set_max_offset_ms(&mut self, ms: f32) {
        self.max_offset_ms = ms.max(0.0);
    }
}

/// A metronome that produces tick schedules from tempo.
#[derive(Debug, Clone)]
pub struct Metronome {
    pub tempo: Tempo,
    pub swing: Swing,
    pub current_tick: u64,
}

impl Metronome {
    pub fn new(tempo: Tempo) -> Self {
        Self {
            tempo,
            swing: Swing::default(),
            current_tick: 0,
        }
    }

    pub fn with_swing(mut self, swing: Swing) -> Self {
        self.swing = swing;
        self
    }

    /// Advance one tick, returning the expected interval in ms.
    pub fn advance(&mut self) -> f32 {
        let base = self.tempo.ms_per_beat();
        let offset = self.swing.offset_ms(self.current_tick);
        self.current_tick += 1;
        base + offset
    }

    pub fn reset(&mut self) {
        self.current_tick = 0;
    }

    pub fn ticks_per_minute(&self) -> f32 {
        self.tempo.effective_bpm()
    }
}

/// A timing coordinator that aligns multiple schedules.
#[derive(Debug, Clone, Default)]
pub struct TimingCoordinator {
    pub schedules: Vec<TickSchedule>,
}

impl TimingCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_schedule(&mut self, schedule: TickSchedule) {
        self.schedules.push(schedule);
    }

    pub fn next_common_tick(&self) -> Option<u64> {
        if self.schedules.is_empty() {
            return None;
        }
        let max_start = self.schedules.iter().map(|s| s.start_ms).max()?;
        let gcd_interval = self
            .schedules
            .iter()
            .map(|s| s.interval_ms)
            .reduce(gcd)?;
        Some(max_start + gcd_interval)
    }

    pub fn schedule_count(&self) -> usize {
        self.schedules.len()
    }
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_new() {
        let t = Tick::new(0, 1000);
        assert_eq!(t.count, 0);
        assert_eq!(t.timestamp_ms, 1000);
    }

    #[test]
    fn test_tick_next() {
        let t = Tick::new(0, 1000);
        let n = t.next(250);
        assert_eq!(n.count, 1);
        assert_eq!(n.timestamp_ms, 1250);
    }

    #[test]
    fn test_tick_duration_since() {
        let a = Tick::new(0, 1000);
        let b = Tick::new(1, 1250);
        assert_eq!(b.duration_since(&a), 250);
        assert_eq!(a.duration_since(&b), -250);
    }

    #[test]
    fn test_schedule_ticks_up_to() {
        let s = TickSchedule::new(0, 100).with_total_ticks(5);
        let ticks = s.ticks_up_to(500);
        assert_eq!(ticks.len(), 5);
        assert_eq!(ticks[4].timestamp_ms, 400);
    }

    #[test]
    fn test_schedule_ticks_respects_total() {
        let s = TickSchedule::new(0, 100).with_total_ticks(3);
        let ticks = s.ticks_up_to(1000);
        assert_eq!(ticks.len(), 3);
    }

    #[test]
    fn test_t_minus_event() {
        let mut s = TickSchedule::new(0, 100);
        s.add_t_minus("launch", 2);
        let fired = s.check_t_minus(&Tick::new(2, 200));
        assert_eq!(fired, vec!["launch"]);
    }

    #[test]
    fn test_t_minus_only_once() {
        let mut s = TickSchedule::new(0, 100);
        s.add_t_minus("launch", 1);
        s.check_t_minus(&Tick::new(1, 100));
        let fired = s.check_t_minus(&Tick::new(2, 200));
        assert!(fired.is_empty());
    }

    #[test]
    fn test_t_minus_reset() {
        let mut s = TickSchedule::new(0, 100);
        s.add_t_minus("launch", 1);
        s.check_t_minus(&Tick::new(1, 100));
        s.reset_t_minus();
        let fired = s.check_t_minus(&Tick::new(1, 100));
        assert_eq!(fired, vec!["launch"]);
    }

    #[test]
    fn test_schedule_estimated_end() {
        let s = TickSchedule::new(0, 100).with_total_ticks(10);
        assert_eq!(s.estimated_end_ms(), Some(900));
    }

    #[test]
    fn test_tempo_new() {
        let t = Tempo::new(120.0);
        assert_eq!(t.bpm, 120.0);
        assert_eq!(t.multiplier, 1.0);
    }

    #[test]
    fn test_tempo_ms_per_beat() {
        let t = Tempo::new(120.0);
        assert!((t.ms_per_beat() - 500.0).abs() < 1e-3);
    }

    #[test]
    fn test_tempo_effective_bpm() {
        let t = Tempo::new(120.0).with_multiplier(2.0);
        assert!((t.effective_bpm() - 240.0).abs() < 1e-3);
    }

    #[test]
    fn test_tempo_adapt_toward() {
        let mut t = Tempo::new(100.0);
        t.adapt_toward(120.0, 0.5);
        assert!((t.bpm - 110.0).abs() < 1e-3);
    }

    #[test]
    fn test_tempo_double_time() {
        let mut t = Tempo::new(120.0);
        t.double_time();
        assert!((t.effective_bpm() - 240.0).abs() < 1e-3);
    }

    #[test]
    fn test_tempo_half_time() {
        let mut t = Tempo::new(120.0);
        t.half_time();
        assert!((t.effective_bpm() - 60.0).abs() < 1e-3);
    }

    #[test]
    fn test_tempo_display() {
        let t = Tempo::new(120.0).with_multiplier(1.5);
        let s = format!("{}", t);
        assert!(s.contains("180.0"));
    }

    #[test]
    fn test_swing_offset() {
        let s = Swing::new(0.5, 20.0);
        assert_eq!(s.offset_ms(0), 0.0);
        assert!((s.offset_ms(1) - 10.0).abs() < 1e-5);
        assert_eq!(s.offset_ms(2), 0.0);
    }

    #[test]
    fn test_swing_clamps() {
        let mut s = Swing::new(0.5, 20.0);
        s.set_ratio(1.5);
        assert_eq!(s.ratio, 1.0);
        s.set_max_offset_ms(-5.0);
        assert_eq!(s.max_offset_ms, 0.0);
    }

    #[test]
    fn test_metronome_advance() {
        let tempo = Tempo::new(120.0);
        let mut m = Metronome::new(tempo);
        let interval = m.advance();
        assert!((interval - 500.0).abs() < 1e-3);
        assert_eq!(m.current_tick, 1);
    }

    #[test]
    fn test_metronome_with_swing() {
        let tempo = Tempo::new(120.0);
        let swing = Swing::new(0.5, 20.0);
        let mut m = Metronome::new(tempo).with_swing(swing);
        let i0 = m.advance(); // even subdivision
        assert!((i0 - 500.0).abs() < 1e-3);
        let i1 = m.advance(); // odd subdivision
        assert!((i1 - 510.0).abs() < 1e-3);
    }

    #[test]
    fn test_metronome_reset() {
        let mut m = Metronome::new(Tempo::new(120.0));
        m.advance();
        m.reset();
        assert_eq!(m.current_tick, 0);
    }

    #[test]
    fn test_timing_coordinator() {
        let mut tc = TimingCoordinator::new();
        tc.add_schedule(TickSchedule::new(0, 100));
        tc.add_schedule(TickSchedule::new(0, 200));
        assert_eq!(tc.schedule_count(), 2);
        assert_eq!(tc.next_common_tick(), Some(100));
    }

    #[test]
    fn test_timing_coordinator_empty() {
        let tc = TimingCoordinator::new();
        assert!(tc.next_common_tick().is_none());
    }

    #[test]
    fn test_tempo_zero_bpm() {
        let t = Tempo::new(0.0);
        assert!(t.ms_per_beat().is_infinite());
    }

    #[test]
    fn test_schedule_no_limit() {
        let s = TickSchedule::new(0, 100);
        let ticks = s.ticks_up_to(300);
        assert_eq!(ticks.len(), 4);
    }
}
