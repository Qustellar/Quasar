use super::*;

impl EngineApp {
    pub fn render_snapshot(&mut self) -> RenderSnapshot {
        let mut snapshot = RenderSnapshot::default();
        let alpha = self.fixed_time.alpha();
        let mut meshes = self.world.query::<(Entity, &Transform, &MeshRenderer)>();
        for (entity, transform, mesh) in meshes.iter(&self.world) {
            let (Some(&mesh_handle), Some(&material_handle)) = (
                self.mesh_names.get(&mesh.mesh),
                self.material_names.get(&mesh.material),
            ) else {
                continue;
            };
            snapshot.instances.push(RenderInstance {
                entity: entity.to_bits(),
                transform: transform.interpolated(alpha),
                mesh: mesh_handle,
                material: material_handle,
            });
        }
        let mut cameras = self.world.query::<(&Transform, &Camera)>();
        if let Some((transform, camera)) = cameras.iter(&self.world).next() {
            snapshot.camera = Some(RenderCamera {
                transform: transform.interpolated(alpha),
                fov_y_radians: camera.fov_y_radians,
                near: camera.near,
                far: camera.far,
            });
        }
        let mut lights = self.world.query::<(&Transform, &DirectionalLight)>();
        for (transform, light) in lights.iter(&self.world) {
            let direction =
                transform.interpolated(alpha).rotation * glam::Vec3::from_array(light.direction);
            snapshot.lights.push(RenderLight {
                direction: direction.to_array(),
                color: light.color,
                intensity: light.intensity,
            });
        }
        snapshot
    }

    pub fn update_fixed(&mut self, delta: std::time::Duration) {
        let _span = tracing::info_span!("fixed_update").entered();
        let steps = self.fixed_time.push(delta);
        let start = Instant::now();
        for _ in 0..steps {
            let mut transforms = self.world.query::<&mut Transform>();
            for mut transform in transforms.iter_mut(&mut self.world) {
                transform.previous = transform.current;
            }
            self.fixed_schedule.run(&mut self.world);
            self.propagate_scene_hierarchy(false);
            self.physics.step(&mut self.world);
            self.propagate_scene_hierarchy(false);
            let mut step = self.world.resource_mut::<SimulationStep>();
            step.index += 1;
        }
        self.diagnostics.fixed_update_seconds = start.elapsed().as_secs_f32();
    }

    pub fn drain_collisions(&mut self) -> impl Iterator<Item = PhysicsCollision> + '_ {
        self.physics.drain_events()
    }

    #[cfg(feature = "editor")]
    pub(crate) fn apply_editor_change(&mut self, index: usize) {
        let (Some(&entity), Some(source)) = (
            self.scene_entities.get(index),
            self.scene.entities.get(index),
        ) else {
            return;
        };
        let source_transform = source.transform;
        let camera = source.camera;
        let light = source.light;
        let previous_authored = self.authored_transforms[index];
        if previous_authored != source_transform {
            if let Some(collider) = self.scene.entities[index].collider.as_mut()
                && let Some(extents) = scaled_half_extents(
                    collider.half_extents,
                    previous_authored.scale,
                    source_transform.scale,
                )
            {
                collider.half_extents = extents;
            }
            if let Some(mut transform) = self.world.get_mut::<Transform>(entity) {
                transform.previous = source_transform;
                transform.current = source_transform;
            }
            self.physics.set_entity_pose(entity, source_transform);
            self.propagate_scene_hierarchy(true);
            self.move_authored_descendants(index, previous_authored, source_transform);
        }
        if let Some(camera) = camera
            && let Some(mut current) = self.world.get_mut::<Camera>(entity)
        {
            *current = camera;
        }
        if let Some(light) = light
            && let Some(mut current) = self.world.get_mut::<DirectionalLight>(entity)
        {
            *current = light;
        }
        if let Some(collider) = self.scene.entities[index].collider {
            if let Some(mut current) = self.world.get_mut::<BoxCollider>(entity) {
                *current = collider;
            }
            self.physics
                .set_collider_half_extents(entity, collider.half_extents);
        }
    }
}
