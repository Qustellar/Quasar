pub use bevy_ecs::prelude::*;
pub use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule, ScheduleLabel};
pub use bevy_ecs::system::ScheduleSystem;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ScheduleLabel)]
pub struct FixedUpdate;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ScheduleLabel)]
pub struct VariableUpdate;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ScheduleLabel)]
pub struct Startup;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ScheduleLabel)]
pub struct PreUpdate;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ScheduleLabel)]
pub struct Physics;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ScheduleLabel)]
pub struct PostUpdate;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ScheduleLabel)]
pub struct RenderExtract;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ScheduleLabel)]
pub struct Render;

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

pub fn make_startup_schedule() -> Schedule {
    Schedule::new(Startup)
}
pub fn make_pre_update_schedule() -> Schedule {
    Schedule::new(PreUpdate)
}
pub fn make_physics_schedule() -> Schedule {
    Schedule::new(Physics)
}
pub fn make_post_update_schedule() -> Schedule {
    Schedule::new(PostUpdate)
}
pub fn make_render_extract_schedule() -> Schedule {
    Schedule::new(RenderExtract)
}
pub fn make_render_schedule() -> Schedule {
    Schedule::new(Render)
}
