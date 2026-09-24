use super::*;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct FixedTime {
    pub step_seconds: f32,
    pub accumulator_seconds: f32,
    pub max_steps_per_frame: u32,
}

impl Default for FixedTime {
    fn default() -> Self {
        Self {
            step_seconds: 1.0 / 60.0,
            accumulator_seconds: 0.0,
            max_steps_per_frame: 4,
        }
    }
}

impl FixedTime {
    pub fn push(&mut self, delta: Duration) -> u32 {
        self.accumulator_seconds += delta.as_secs_f32().min(0.25);
        let mut steps = 0;
        while self.accumulator_seconds >= self.step_seconds && steps < self.max_steps_per_frame {
            self.accumulator_seconds -= self.step_seconds;
            steps += 1;
        }
        if steps == self.max_steps_per_frame && self.accumulator_seconds >= self.step_seconds {
            self.accumulator_seconds = 0.0;
        }
        steps
    }

    pub fn alpha(&self) -> f32 {
        (self.accumulator_seconds / self.step_seconds).clamp(0.0, 1.0)
    }
}
