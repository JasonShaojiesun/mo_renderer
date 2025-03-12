use bevy_ecs::prelude::*;
use std::time::{Duration, Instant};

/// Frame tracking service
///
/// Calculates FPS and tracks delta time between renderings
#[derive(Resource)]
pub struct Timer {
    first: Option<Instant>,
    current: Option<Instant>,
    counter_start: Instant,
    counter: u32,
    fps: Option<f32>,
    delta: Duration,
    time: Duration,
}

impl Timer {
    /// Constructs service instance
    pub fn new() -> Self {
        Self {
            first: None,
            current: None,
            counter_start: Instant::now(),
            counter: 0,
            fps: None,
            delta: Duration::from_secs(0),
            time: Duration::from_secs(0),
        }
    }

    pub fn next(&mut self) {
        let now = Instant::now();
        if let Some(first) = self.first {
            self.time = now - first;
        } else {
            self.first = Some(now);
            self.counter_start = now;
        }

        if let Some(current) = self.current {
            self.delta = now - current;
        }
        self.current = Some(now);

        let duration = now - self.counter_start;
        if duration > Duration::from_secs(1) {
            self.fps = Some(self.counter as f32 / duration.as_secs_f32());
            self.counter = 0;
            self.counter_start = now;
        }

        self.counter += 1;
    }

    /// Returns [`Duration`] from application start
    pub fn time(&self) -> Duration {
        self.time
    }

    /// Returns FPS
    pub fn fps(&self) -> f32 {
        self.fps.unwrap_or_else(|| {
            let duration = Instant::now() - self.counter_start;
            self.counter as f32 / duration.as_secs_f32()
        })
    }

    /// Returns [`Duration`] from previous rendering
    pub fn delta(&self) -> Duration {
        self.delta
    }

    /// Returns [`Duration`] from previous rendering as `f32`
    pub fn delta_time(&self) -> f32 {
        self.delta.as_secs_f32()
    }

    /// This system update the global resource timer, it should be added to the runtime schedule.
    pub fn update_timer(mut timer: ResMut<Timer>) {
        timer.next();
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}
