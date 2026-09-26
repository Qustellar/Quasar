use std::collections::HashMap;

use qst_ecs::{Entity, World};
use qst_scene::{BoxCollider, Transform, WorldTransform};
use rapier3d::prelude::*;

fn physics_rotation(rotation: qst_core::glam::Quat) -> rapier3d::math::Rotation {
    rapier3d::math::Rotation::from_xyzw(rotation.x, rotation.y, rotation.z, rotation.w)
}

#[derive(Clone, Copy, Debug)]
pub struct PhysicsConfig {
    pub gravity: [f32; 3],
    pub step_seconds: f32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            gravity: [0.0, -9.81, 0.0],
            step_seconds: 1.0 / 60.0,
        }
    }
}

pub struct PhysicsWorld {
    config: PhysicsConfig,
    pipeline: PhysicsPipeline,
    integration: IntegrationParameters,
    islands: IslandManager,
    broad_phase: BroadPhaseBvh,
    narrow_phase: NarrowPhase,
    bodies: RigidBodySet,
    colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    ccd: CCDSolver,
    entity_bodies: HashMap<Entity, RigidBodyHandle>,
    body_entities: HashMap<RigidBodyHandle, Entity>,
    collector: ChannelEventCollector,
    collision_rx: std::sync::mpsc::Receiver<CollisionEvent>,
    events: Vec<PhysicsCollision>,
}

#[derive(Clone, Copy, Debug)]
pub struct PhysicsCollision {
    pub a: Entity,
    pub b: Entity,
    pub started: bool,
}

impl PhysicsWorld {
    pub fn new(config: PhysicsConfig) -> Self {
        let (collision_tx, collision_rx) = std::sync::mpsc::channel();
        let (force_tx, _) = std::sync::mpsc::channel();
        let integration = IntegrationParameters {
            dt: config.step_seconds,
            ..Default::default()
        };
        Self {
            config,
            pipeline: PhysicsPipeline::new(),
            integration,
            islands: IslandManager::new(),
            broad_phase: BroadPhaseBvh::new(),
            narrow_phase: NarrowPhase::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            ccd: CCDSolver::new(),
            entity_bodies: HashMap::new(),
            body_entities: HashMap::new(),
            collector: ChannelEventCollector::new(collision_tx, force_tx),
            collision_rx,
            events: Vec::new(),
        }
    }

    pub fn sync_from_scene(&mut self, world: &mut World) {
        let mut query = world.query::<(Entity, &Transform, &BoxCollider)>();
        for (entity, transform, collider) in query.iter(world) {
            if self.entity_bodies.contains_key(&entity) {
                continue;
            }
            let p = transform.current.translation;
            let builder = if collider.dynamic {
                RigidBodyBuilder::dynamic()
            } else {
                RigidBodyBuilder::fixed()
            };
            let body = self
                .bodies
                .insert(builder.translation(vector![p.x, p.y, p.z].into()).build());
            if let Some(rigid_body) = self.bodies.get_mut(body) {
                rigid_body.set_rotation(physics_rotation(transform.current.rotation), false);
            }
            let h = collider.half_extents;
            self.colliders.insert_with_parent(
                ColliderBuilder::cuboid(h[0], h[1], h[2])
                    .active_events(ActiveEvents::COLLISION_EVENTS)
                    .build(),
                body,
                &mut self.bodies,
            );
            self.entity_bodies.insert(entity, body);
            self.body_entities.insert(body, entity);
        }
    }

    pub fn step(&mut self, world: &mut World) {
        self.pipeline.step(
            vector![
                self.config.gravity[0],
                self.config.gravity[1],
                self.config.gravity[2]
            ]
            .into(),
            &self.integration,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.ccd,
            &(),
            &self.collector,
        );
        while let Ok(event) = self.collision_rx.try_recv() {
            let (a, b, started) = match event {
                CollisionEvent::Started(a, b, _) => (a, b, true),
                CollisionEvent::Stopped(a, b, _) => (a, b, false),
            };
            let entities = self
                .colliders
                .get(a)
                .and_then(|c| c.parent())
                .and_then(|h| self.body_entities.get(&h).copied())
                .zip(
                    self.colliders
                        .get(b)
                        .and_then(|c| c.parent())
                        .and_then(|h| self.body_entities.get(&h).copied()),
                );
            if let Some((a, b)) = entities {
                self.events.push(PhysicsCollision { a, b, started });
            }
        }
        for (&entity, &handle) in &self.entity_bodies {
            let mut synced = None;
            if let (Some(body), Some(mut transform)) =
                (self.bodies.get(handle), world.get_mut::<Transform>(entity))
            {
                let t = body.translation();
                transform.current.translation = qst_core::glam::Vec3::new(t.x, t.y, t.z);
                let r = body.rotation();
                transform.current.rotation = qst_core::glam::Quat::from_xyzw(r.x, r.y, r.z, r.w);
                synced = Some(transform.current);
            }
            if let Some(state) = synced
                && let Some(mut world_transform) = world.get_mut::<WorldTransform>(entity)
            {
                world_transform.0 = state;
            }
        }
    }

    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }

    pub fn drain_events(&mut self) -> impl Iterator<Item = PhysicsCollision> + '_ {
        self.events.drain(..)
    }

    pub fn set_entity_translation(&mut self, entity: Entity, translation: qst_core::glam::Vec3) {
        if let Some(handle) = self.entity_bodies.get(&entity).copied()
            && let Some(body) = self.bodies.get_mut(handle)
        {
            body.set_translation(
                rapier3d::math::Vec3::new(translation.x, translation.y, translation.z),
                true,
            );
        }
    }

    pub fn set_entity_pose(&mut self, entity: Entity, pose: qst_core::TransformState) {
        if let Some(handle) = self.entity_bodies.get(&entity).copied()
            && let Some(body) = self.bodies.get_mut(handle)
        {
            body.set_translation(
                rapier3d::math::Vec3::new(
                    pose.translation.x,
                    pose.translation.y,
                    pose.translation.z,
                ),
                true,
            );
            body.set_rotation(physics_rotation(pose.rotation), true);
        }
    }

    pub fn set_collider_half_extents(&mut self, entity: Entity, half_extents: [f32; 3]) {
        let Some(body_handle) = self.entity_bodies.get(&entity).copied() else {
            return;
        };
        let collider_handle = self
            .bodies
            .get(body_handle)
            .and_then(|body| body.colliders().first().copied());
        if let Some(collider) = collider_handle.and_then(|handle| self.colliders.get_mut(handle)) {
            collider.set_shape(SharedShape::cuboid(
                half_extents[0],
                half_extents[1],
                half_extents[2],
            ));
        }
    }
}

#[cfg(test)]
mod tests;
