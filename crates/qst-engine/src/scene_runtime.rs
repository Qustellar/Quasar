use super::*;

impl EngineApp {
    pub fn load_scene(&mut self, path: impl AsRef<Path>) -> EngineResult<&mut Self> {
        let scene_load_start = Instant::now();
        let path = std::fs::canonicalize(path)?;
        let mut scene = SceneAsset::load_ron(&path)?;
        let imported = if let Some(source) = scene.source_gltf.as_ref() {
            let source = if source.is_absolute() {
                source.clone()
            } else {
                path.parent().unwrap_or(Path::new(".")).join(source)
            };
            let source = std::fs::canonicalize(source)?;
            let import_start = Instant::now();
            let (imported, cache_hit) = import_gltf_cached(&source)?;
            self.diagnostics.asset_import_seconds = import_start.elapsed().as_secs_f32();
            self.diagnostics.cache_load_seconds = self.diagnostics.asset_import_seconds;
            if cache_hit {
                self.diagnostics.asset_cache_hits += 1;
            } else {
                self.diagnostics.asset_cache_misses += 1;
            }
            scene.source_gltf = Some(source);
            Some(imported)
        } else {
            None
        };
        self.validate_scene_assets(&scene, imported.as_ref())?;
        self.set_scene(scene)?;
        if let Some(imported) = imported {
            self.register_gltf_assets(imported.meshes);
            if let Some(source) = self.scene.source_gltf.clone() {
                self.assets.watch_path(&source)?;
                self.gltf_path = Some(source);
            }
        }
        self.assets.watch_path(&path)?;
        self.scene_path = Some(path);
        self.saved_scene_contents = None;
        self.diagnostics.scene_load_seconds = scene_load_start.elapsed().as_secs_f32();
        Ok(self)
    }

    pub fn save_scene(&mut self, path: impl AsRef<Path>) -> EngineResult<&mut Self> {
        let mut scene = self.scene.clone();
        if let Some(source) = scene.source_gltf.as_ref()
            && let Ok(parent) =
                std::fs::canonicalize(path.as_ref().parent().unwrap_or(Path::new(".")))
            && let Ok(relative) = source.strip_prefix(parent)
        {
            scene.source_gltf = Some(relative.to_path_buf());
        }
        scene.save_ron(&path)?;
        let path = std::fs::canonicalize(path)?;
        self.assets.watch_path(&path)?;
        self.saved_scene_contents = Some((path.clone(), std::fs::read(&path)?));
        self.scene_path = Some(path);
        Ok(self)
    }

    pub fn set_scene(&mut self, scene: SceneAsset) -> EngineResult<&mut Self> {
        let mut scene = scene;
        for entity in &mut scene.entities {
            if entity.local_transform == TransformState::identity()
                && entity.transform != TransformState::identity()
            {
                entity.local_transform = entity.transform;
            }
            entity.transform = entity.local_transform;
        }
        let hierarchy_order = scene.hierarchy_order()?;
        self.fixed_time.accumulator_seconds = 0.0;
        self.last_frame = Instant::now();
        #[cfg(feature = "editor")]
        if let Some(editor) = &mut self.editor {
            editor.plugin.selected_entity = None;
        }
        self.clear_gltf_assets();
        self.gltf_path = None;
        self.scene_path = None;
        self.saved_scene_contents = None;
        self.scene = scene;
        self.hierarchy_order = hierarchy_order;
        self.scene_index_by_id = self
            .scene
            .entities
            .iter()
            .enumerate()
            .map(|(index, entity)| (entity.id, index))
            .collect();
        self.authored_transforms = self
            .scene
            .entities
            .iter()
            .map(|entity| entity.transform)
            .collect();
        self.last_scene_transforms = self.authored_transforms.clone();
        self.world = World::new();
        self.world.insert_resource(SimulationStep {
            index: 0,
            delta_seconds: self.fixed_time.step_seconds,
        });
        self.world.insert_resource(InputState::default());
        self.physics = PhysicsWorld::new(PhysicsConfig::default());
        self.scene_entities = self.scene.instantiate(&mut self.world);
        for (index, entity) in self.scene_entities.iter().copied().enumerate() {
            let state = self.scene.entities[index].local_transform;
            self.world.entity_mut(entity).insert((
                LocalTransform(state),
                WorldTransform(state),
                PreviousWorldTransform(state),
            ));
            if let Some(parent) = self.scene.entities[index].parent {
                self.world.entity_mut(entity).insert(Parent(parent));
            }
            if let Some(audio) = self.scene.entities[index].audio_source.clone() {
                self.world.entity_mut(entity).insert(audio);
            }
            if let Some(animation) = self.scene.entities[index].animation_player.clone() {
                self.world.entity_mut(entity).insert(animation);
            }
        }
        self.physics.sync_from_scene(&mut self.world);
        Ok(self)
    }

    pub(crate) fn clear_gltf_assets(&mut self) {
        for name in std::mem::take(&mut self.gltf_mesh_names) {
            if let Some(handle) = self.mesh_names.remove(&name) {
                self.mesh_assets.remove(handle);
                self.assets.invalidate::<MeshAsset>(&name);
                if let Some(renderer) = &mut self.renderer {
                    renderer.remove_mesh(handle);
                }
            }
            if let Some(handle) = self.material_names.remove(&name) {
                self.material_assets.remove(handle);
                self.assets.invalidate::<MaterialAsset>(&name);
                if let Some(renderer) = &mut self.renderer {
                    renderer.remove_material(handle);
                }
            }
        }
    }

    pub(crate) fn validate_scene_assets(
        &self,
        scene: &SceneAsset,
        imported: Option<&GltfImport>,
    ) -> EngineResult<()> {
        for entity in &scene.entities {
            let Some(mesh) = &entity.mesh else {
                continue;
            };
            let available = |name: &str, registered: bool| {
                imported.is_some_and(|source| source.meshes.iter().any(|asset| asset.name == name))
                    || (registered && !self.gltf_mesh_names.iter().any(|old| old == name))
            };
            if !available(&mesh.mesh, self.mesh_names.contains_key(&mesh.mesh)) {
                return Err(EngineError::AssetNotFound(mesh.mesh.clone()));
            }
            if !available(
                &mesh.material,
                self.material_names.contains_key(&mesh.material),
            ) {
                return Err(EngineError::AssetNotFound(mesh.material.clone()));
            }
        }
        Ok(())
    }

    pub fn import_gltf(&mut self, path: impl AsRef<Path>) -> EngineResult<&mut Self> {
        let path = std::fs::canonicalize(path)?;
        let (mut imported, cache_hit) = import_gltf_cached(&path)?;
        if cache_hit {
            self.diagnostics.asset_cache_hits += 1;
        } else {
            self.diagnostics.asset_cache_misses += 1;
        }
        imported.scene.source_gltf = Some(path.clone());
        self.apply_gltf_import(imported)?;
        self.assets.watch_path(&path)?;
        self.gltf_path = Some(path);
        Ok(self)
    }

    pub(crate) fn apply_gltf_import(&mut self, imported: GltfImport) -> EngineResult<()> {
        let animations = imported.animations.clone();
        self.set_scene(imported.scene)?;
        self.register_gltf_assets(imported.meshes);
        for animation in animations {
            self.register_animation_clip(animation);
        }
        Ok(())
    }

    pub(crate) fn register_gltf_assets(&mut self, meshes: Vec<ImportedMesh>) {
        for mesh in meshes {
            let vertices = mesh
                .positions
                .into_iter()
                .zip(mesh.normals)
                .map(|(position, normal)| MeshVertex { position, normal })
                .collect();
            self.register_mesh(
                mesh.name.clone(),
                MeshAsset {
                    vertices,
                    indices: mesh.indices,
                },
            );
            self.register_material(mesh.name.clone(), MaterialAsset::from_color(mesh.color));
            self.gltf_mesh_names.push(mesh.name);
        }
    }

    pub fn register_mesh(&mut self, name: impl Into<String>, mesh: MeshAsset) -> Handle<MeshAsset> {
        let name = name.into();
        if let Some(old) = self.mesh_names.remove(&name) {
            self.mesh_assets.remove(old);
            if let Some(renderer) = &mut self.renderer {
                renderer.remove_mesh(old);
            }
            self.assets.invalidate::<MeshAsset>(&name);
        }
        let handle = self.assets.load::<MeshAsset>(&name);
        self.mesh_assets.insert(handle, mesh);
        let _ = self.assets.mark_loaded(handle);
        self.mesh_names.insert(name, handle);
        if let (Some(renderer), Some(mesh)) = (&mut self.renderer, self.mesh_assets.get(handle)) {
            renderer.upload_mesh(handle, &mesh);
        }
        handle
    }

    pub fn register_material(
        &mut self,
        name: impl Into<String>,
        material: MaterialAsset,
    ) -> Handle<MaterialAsset> {
        let name = name.into();
        if let Some(old) = self.material_names.remove(&name) {
            self.material_assets.remove(old);
            if let Some(renderer) = &mut self.renderer {
                renderer.remove_material(old);
            }
            self.assets.invalidate::<MaterialAsset>(&name);
        }
        let handle = self.assets.load::<MaterialAsset>(&name);
        self.material_assets.insert(handle, material);
        let _ = self.assets.mark_loaded(handle);
        self.material_names.insert(name, handle);
        if let Some(renderer) = &mut self.renderer {
            renderer.upload_material(handle, material);
        }
        handle
    }

    pub fn register_texture(
        &mut self,
        name: impl Into<String>,
        texture: TextureAsset,
    ) -> Handle<TextureAsset> {
        let name = name.into();
        if let Some(old) = self.texture_names.remove(&name) {
            self.texture_assets.remove(old);
            self.assets.invalidate::<TextureAsset>(&name);
        }
        let handle = self.assets.load::<TextureAsset>(&name);
        self.texture_assets.insert(handle, texture);
        let _ = self.assets.mark_loaded(handle);
        self.texture_names.insert(name, handle);
        handle
    }

    pub(crate) fn propagate_scene_hierarchy(&mut self, snap_interpolation: bool) {
        for &index in &self.hierarchy_order {
            let Some(parent_id) = self.scene.entities[index].parent else {
                continue;
            };
            let Some(&parent_index) = self.scene_index_by_id.get(&parent_id) else {
                continue;
            };
            let Some(parent) = self
                .world
                .get::<Transform>(self.scene_entities[parent_index])
                .map(|transform| transform.current)
            else {
                continue;
            };
            let previous_parent = self.last_scene_transforms[parent_index];
            if parent == previous_parent {
                continue;
            }
            let previous_matrix = transform_matrix(previous_parent);
            if previous_matrix.determinant().abs() < 1e-6 {
                tracing::warn!(parent_index, "cannot propagate singular parent transform");
                continue;
            }
            let delta = transform_matrix(parent) * previous_matrix.inverse();
            let entity = self.scene_entities[index];
            let Some(current) = self
                .world
                .get::<Transform>(entity)
                .map(|transform| transform.current)
            else {
                continue;
            };
            let Some(next) = transformed_state(delta, current) else {
                continue;
            };
            if let Some(mut transform) = self.world.get_mut::<Transform>(entity) {
                if snap_interpolation {
                    transform.previous = next;
                }
                transform.current = next;
            }
            if let Some(mut world_transform) = self.world.get_mut::<WorldTransform>(entity) {
                world_transform.0 = next;
            }
            if snap_interpolation
                && let Some(mut previous_world) =
                    self.world.get_mut::<PreviousWorldTransform>(entity)
            {
                previous_world.0 = next;
            }
            self.physics.set_entity_pose(entity, next);
            if let Some(mut collider) = self.world.get_mut::<BoxCollider>(entity)
                && let Some(extents) =
                    scaled_half_extents(collider.half_extents, current.scale, next.scale)
            {
                collider.half_extents = extents;
                self.physics.set_collider_half_extents(entity, extents);
            }
        }
        for (index, &entity) in self.scene_entities.iter().enumerate() {
            if let Some(transform) = self.world.get::<Transform>(entity) {
                let current = transform.current;
                self.last_scene_transforms[index] = current;
                if let Some(mut world_transform) = self.world.get_mut::<WorldTransform>(entity) {
                    world_transform.0 = current;
                }
            }
        }
    }

    #[cfg(feature = "editor")]
    pub(crate) fn move_authored_descendants(
        &mut self,
        index: usize,
        previous: TransformState,
        current: TransformState,
    ) {
        self.authored_transforms[index] = current;
        let previous_matrix = transform_matrix(previous);
        if previous_matrix.determinant().abs() < 1e-6 {
            return;
        }
        let delta = transform_matrix(current) * previous_matrix.inverse();
        let mut affected = vec![false; self.scene.entities.len()];
        affected[index] = true;
        for &child_index in &self.hierarchy_order {
            let Some(parent_id) = self.scene.entities[child_index].parent else {
                continue;
            };
            let Some(&parent_index) = self.scene_index_by_id.get(&parent_id) else {
                continue;
            };
            if !affected[parent_index] {
                continue;
            }
            let previous_child = self.scene.entities[child_index].transform;
            if let Some(next) = transformed_state(delta, previous_child) {
                self.scene.entities[child_index].transform = next;
                self.scene.entities[child_index].local_transform = next;
                self.authored_transforms[child_index] = next;
                if let Some(collider) = self.scene.entities[child_index].collider.as_mut()
                    && let Some(extents) =
                        scaled_half_extents(collider.half_extents, previous_child.scale, next.scale)
                {
                    collider.half_extents = extents;
                }
                affected[child_index] = true;
            }
        }
    }
}
