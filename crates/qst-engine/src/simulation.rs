use super::*;

impl EngineApp {
    pub fn update_animations(&mut self, delta_seconds: f32) {
        let start = Instant::now();
        let clips = &self.animation_clips;
        let entity_ids = self
            .scene_entities
            .iter()
            .enumerate()
            .map(|(index, &entity)| (entity, self.scene.entities[index].id))
            .collect::<std::collections::HashMap<_, _>>();
        let mut query = self
            .world
            .query::<(Entity, &mut AnimationPlayer, &mut LocalTransform)>();
        for (entity, mut player, mut local) in query.iter_mut(&mut self.world) {
            if !player.playing {
                continue;
            }
            let Some(name) = player.clip.clone() else {
                continue;
            };
            let Some(clip) = clips.get(&name) else {
                continue;
            };
            player.time += delta_seconds.max(0.0) * player.speed;
            if clip.duration > 0.0 && player.time >= clip.duration {
                if player.looping {
                    player.time %= clip.duration;
                } else {
                    player.time = clip.duration;
                    player.playing = false;
                    player.finished = true;
                }
            }
            let mut state = local.0;
            for channel in &clip.channels {
                if channel.target.is_valid()
                    && entity_ids.get(&entity).copied() != Some(channel.target)
                {
                    continue;
                }
                let Some(sampler) = clip.samplers.get(channel.sampler) else {
                    continue;
                };
                let value = sampler.sample(player.time);
                let property = &channel.property;
                match property {
                    AnimationProperty::Translation => {
                        state.translation = glam::Vec3::from_array([value[0], value[1], value[2]])
                    }
                    AnimationProperty::Rotation => {
                        state.rotation =
                            glam::Quat::from_xyzw(value[0], value[1], value[2], value[3])
                                .normalize()
                    }
                    AnimationProperty::Scale => {
                        state.scale = glam::Vec3::from_array([value[0], value[1], value[2]])
                    }
                    AnimationProperty::Joint(_) => {}
                }
            }
            local.0 = state;
        }
        self.diagnostics.animation_update_seconds = start.elapsed().as_secs_f32();
    }

    pub fn render_snapshot(&mut self) -> RenderSnapshot {
        let mut snapshot = RenderSnapshot::default();
        let alpha = self.fixed_time.alpha();
        let mut palette_indices = HashMap::<(String, [u32; 16]), u32>::new();
        let mut meshes = self.world.query::<(
            Entity,
            &Transform,
            Option<&WorldTransform>,
            Option<&PreviousWorldTransform>,
            &MeshRenderer,
            Option<&SkinBinding>,
        )>();
        for (entity, transform, world_transform, previous_world, mesh, skin_binding) in
            meshes.iter(&self.world)
        {
            let (Some(&mesh_handle), Some(&material_handle)) = (
                self.mesh_names.get(&mesh.mesh),
                self.material_names.get(&mesh.material),
            ) else {
                continue;
            };
            let instance_transform = match (previous_world, world_transform) {
                (Some(previous), Some(world)) => previous.0.lerp(world.0, alpha),
                _ => transform.interpolated(alpha),
            };
            let skin_key = skin_binding
                .and_then(|binding| binding.skin.as_ref())
                .map(|name| {
                    (
                        name.clone(),
                        transform_matrix(instance_transform)
                            .to_cols_array()
                            .map(f32::to_bits),
                    )
                });
            let skin_index = if let Some(index) = skin_key
                .as_ref()
                .and_then(|key| palette_indices.get(key))
                .copied()
            {
                Some(index)
            } else {
                let skin = skin_binding
                    .and_then(|binding| binding.skin.as_deref())
                    .and_then(|name| self.scene.skins.iter().find(|skin| skin.name == name))
                    .and_then(|skin| {
                        let instance_inverse = transform_matrix(instance_transform).inverse();
                        if !instance_inverse.is_finite() {
                            return None;
                        }
                        let matrices = skin
                            .joints
                            .iter()
                            .enumerate()
                            .map(|(joint_index, joint_id)| {
                                let scene_index = self.scene.index_of(*joint_id)?;
                                let joint_entity = *self.scene_entities.get(scene_index)?;
                                let joint_state = self
                                    .world
                                    .get::<WorldTransform>(joint_entity)
                                    .zip(self.world.get::<PreviousWorldTransform>(joint_entity))
                                    .map(|(current, previous)| previous.0.lerp(current.0, alpha))
                                    .or_else(|| {
                                        self.world
                                            .get::<WorldTransform>(joint_entity)
                                            .map(|current| current.0)
                                    })?;
                                let inverse_bind = skin
                                    .inverse_bind_matrices
                                    .get(joint_index)
                                    .map(glam::Mat4::from_cols_array_2d)
                                    .unwrap_or(glam::Mat4::IDENTITY);
                                let matrix =
                                    instance_inverse * transform_matrix(joint_state) * inverse_bind;
                                matrix.is_finite().then_some(matrix)
                            })
                            .collect::<Option<Vec<_>>>()?;
                        Some(RenderSkin {
                            matrices: Arc::from(matrices.into_boxed_slice()),
                        })
                    });
                skin.map(|skin| {
                    let index = snapshot.skins.len() as u32;
                    snapshot.skins.push(skin);
                    if let Some(key) = skin_key {
                        palette_indices.insert(key, index);
                    }
                    index
                })
            };
            if skin_binding
                .and_then(|binding| binding.skin.as_ref())
                .is_some()
                && skin_index.is_none()
            {
                snapshot.skin_fallbacks += 1;
            }
            snapshot.instances.push(RenderInstance {
                entity: entity.to_bits(),
                transform: instance_transform,
                bounds: self
                    .world
                    .get::<RenderBounds>(entity)
                    .map(|bounds| {
                        [
                            bounds.center[0],
                            bounds.center[1],
                            bounds.center[2],
                            bounds.radius,
                        ]
                    })
                    .unwrap_or([0.0, 0.0, 0.0, 1.0]),
                mesh: mesh_handle,
                material: material_handle,
                skin: skin_index,
            });
        }
        self.diagnostics.render_instances = snapshot.instances.len();
        let mut cameras = self.world.query::<(
            &Transform,
            Option<&WorldTransform>,
            Option<&PreviousWorldTransform>,
            &Camera,
        )>();
        if let Some((transform, world_transform, previous_world, camera)) =
            cameras.iter(&self.world).next()
        {
            snapshot.camera = Some(RenderCamera {
                transform: match (previous_world, world_transform) {
                    (Some(previous), Some(world)) => previous.0.lerp(world.0, alpha),
                    _ => transform.interpolated(alpha),
                },
                fov_y_radians: camera.fov_y_radians,
                near: camera.near,
                far: camera.far,
            });
        }
        let mut lights = self.world.query::<(
            &Transform,
            Option<&WorldTransform>,
            Option<&PreviousWorldTransform>,
            &DirectionalLight,
        )>();
        for (transform, world_transform, previous_world, light) in lights.iter(&self.world) {
            let direction = match (previous_world, world_transform) {
                (Some(previous), Some(world)) => previous.0.lerp(world.0, alpha),
                _ => transform.interpolated(alpha),
            }
            .rotation
                * glam::Vec3::from_array(light.direction);
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
        self.diagnostics.fixed_steps = steps;
        self.diagnostics.dropped_fixed_steps += self.fixed_time.dropped_steps as u64;
        let start = Instant::now();
        for _ in 0..steps {
            let mut transforms = self.world.query::<&mut Transform>();
            for mut transform in transforms.iter_mut(&mut self.world) {
                transform.previous = transform.current;
            }
            let mut previous = self
                .world
                .query::<(&WorldTransform, &mut PreviousWorldTransform)>();
            for (world, mut old) in previous.iter_mut(&mut self.world) {
                old.0 = world.0;
            }
            self.update_animations(self.fixed_time.step_seconds);
            self.fixed_schedule.run(&mut self.world);
            self.propagate_scene_hierarchy(false);
            self.physics_schedule.run(&mut self.world);
            self.physics.step(&mut self.world);
            self.sync_dynamic_physics_locals();
            self.propagate_scene_hierarchy(false);
            self.post_update_schedule.run(&mut self.world);
            let mut step = self.world.resource_mut::<SimulationStep>();
            step.index += 1;
        }
        self.diagnostics.fixed_update_seconds = start.elapsed().as_secs_f32();
    }

    pub fn drain_collisions(&mut self) -> impl Iterator<Item = PhysicsCollision> + '_ {
        self.physics.drain_events()
    }

    #[cfg(feature = "editor")]
    pub(crate) fn apply_editor_change(&mut self, entity_id: EntityId) {
        let Some(&index) = self.scene_index_by_id.get(&entity_id) else {
            return;
        };
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
            self.scene.entities[index].local_transform = source_transform;
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
