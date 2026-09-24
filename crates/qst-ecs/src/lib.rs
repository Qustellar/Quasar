pub use bevy_ecs::prelude::*;
pub use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule, ScheduleLabel};
pub use bevy_ecs::system::ScheduleSystem;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ScheduleLabel)]
pub struct FixedUpdate;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ScheduleLabel)]
pub struct VariableUpdate;

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct SimulationStep {
    pub index: u64,
    pub delta_seconds: f32,
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct RenderInterpolation {
    pub alpha: f32,
}

pub fn make_fixed_schedule() -> Schedule {
    Schedule::new(FixedUpdate)
}

pub fn make_variable_schedule() -> Schedule {
    Schedule::new(VariableUpdate)
}
