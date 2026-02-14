//! Time management for the game loop.

use std::time::{Duration, Instant};

/// Manages frame timing and delta time calculation.
#[derive(Debug)]
pub struct Time {
    /// Time when the engine started.
    start_time: Instant,
    /// Time of the last frame.
    last_frame: Instant,
    /// Duration of the last frame.
    delta: Duration,
    /// Total elapsed time since start.
    elapsed: Duration,
    /// Frame count since start.
    frame_count: u64,
    /// Fixed timestep for physics (default 60 Hz).
    fixed_timestep: Duration,
    /// Accumulated time for fixed updates.
    accumulator: Duration,
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}

impl Time {
    /// Create a new time manager.
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            start_time: now,
            last_frame: now,
            delta: Duration::ZERO,
            elapsed: Duration::ZERO,
            frame_count: 0,
            fixed_timestep: Duration::from_secs_f64(1.0 / 60.0),
            accumulator: Duration::ZERO,
        }
    }

    /// Update timing at the start of a new frame.
    pub fn update(&mut self) {
        let now = Instant::now();
        self.delta = now - self.last_frame;
        self.last_frame = now;
        self.elapsed = now - self.start_time;
        self.frame_count += 1;
        self.accumulator += self.delta;
    }

    /// Get the delta time in seconds.
    pub fn delta_seconds(&self) -> f32 {
        self.delta.as_secs_f32()
    }

    /// Get the delta time as a Duration.
    pub fn delta(&self) -> Duration {
        self.delta
    }

    /// Get total elapsed time in seconds.
    pub fn elapsed_seconds(&self) -> f32 {
        self.elapsed.as_secs_f32()
    }

    /// Get total elapsed time as Duration.
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Get the current frame count.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Get the fixed timestep in seconds.
    pub fn fixed_timestep_seconds(&self) -> f32 {
        self.fixed_timestep.as_secs_f32()
    }

    /// Check if a fixed update should run and consume the time.
    pub fn should_fixed_update(&mut self) -> bool {
        if self.accumulator >= self.fixed_timestep {
            self.accumulator -= self.fixed_timestep;
            true
        } else {
            false
        }
    }

    /// Get the current FPS (averaged over last frame).
    pub fn fps(&self) -> f32 {
        if self.delta.as_secs_f32() > 0.0 {
            1.0 / self.delta.as_secs_f32()
        } else {
            0.0
        }
    }

    /// Set the fixed timestep rate in Hz.
    pub fn set_fixed_rate(&mut self, hz: f64) {
        self.fixed_timestep = Duration::from_secs_f64(1.0 / hz);
    }
}
