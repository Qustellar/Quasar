use super::*;

pub const LEGACY_SCENE_SCHEMA_VERSION: u32 = 1;
pub const SCHEMA_2: u32 = 2;
pub const SCHEMA_3: u32 = 3;
pub const SCENE_SCHEMA_VERSION: u32 = 4;

#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Transform {
    pub previous: TransformState,
    pub current: TransformState,
}

#[derive(Component, Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct LocalTransform(pub TransformState);

impl LocalTransform {
    pub fn identity() -> Self {
        Self(TransformState::identity())
    }
}

#[derive(Component, Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct WorldTransform(pub TransformState);

#[derive(Component, Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PreviousWorldTransform(pub TransformState);

#[derive(Component, Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Parent(pub EntityId);

#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct RenderBounds {
    pub center: [f32; 3],
    pub radius: f32,
}

impl Default for RenderBounds {
    fn default() -> Self {
        Self {
            center: [0.0; 3],
            radius: 1.0,
        }
    }
}

#[derive(Component, Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkinBinding {
    pub skeleton: Option<String>,
    pub skin: Option<String>,
}

#[derive(Component, Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioListener {
    pub active: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrefabReference {
    pub asset: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditorMetadata {
    pub locked: bool,
    pub hidden: bool,
    pub color: Option<[u8; 4]>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetDependency {
    pub path: String,
    pub hash: String,
}

impl Default for Transform {
    fn default() -> Self {
        let identity = TransformState::identity();
        Self {
            previous: identity,
            current: identity,
        }
    }
}

impl Transform {
    pub fn interpolated(&self, alpha: f32) -> TransformState {
        self.previous.lerp(self.current, alpha)
    }
    pub fn from_state(state: TransformState) -> Self {
        Self {
            previous: state,
            current: state,
        }
    }
}

#[derive(Component, Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Camera {
    pub fov_y_radians: f32,
    pub near: f32,
    pub far: f32,
}

#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DirectionalLight {
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            direction: [0.0, -1.0, 0.0],
            color: [1.0; 3],
            intensity: 1.0,
        }
    }
}

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct MeshRenderer {
    pub mesh: String,
    pub material: String,
}

#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BoxCollider {
    pub half_extents: [f32; 3],
    pub dynamic: bool,
}

#[derive(Component, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AudioSource {
    pub clip: Option<String>,
    pub volume: f32,
    pub looping: bool,
    pub autoplay: bool,
}

impl Default for AudioSource {
    fn default() -> Self {
        Self {
            clip: None,
            volume: 1.0,
            looping: false,
            autoplay: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum AnimationInterpolation {
    Linear,
    Step,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum AnimationProperty {
    Translation,
    Rotation,
    Scale,
    Joint(u32),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AnimationSampler {
    pub input: Vec<f32>,
    pub output: Vec<[f32; 4]>,
    pub interpolation: AnimationInterpolation,
}

impl AnimationSampler {
    pub fn sample(&self, time: f32) -> [f32; 4] {
        if self.input.is_empty() || self.output.is_empty() {
            return [0.0; 4];
        }
        let index = self
            .input
            .partition_point(|value| *value <= time)
            .saturating_sub(1)
            .min(self.output.len() - 1);
        if self.interpolation == AnimationInterpolation::Step
            || index + 1 >= self.input.len()
            || index + 1 >= self.output.len()
        {
            return self.output[index];
        }
        let span = (self.input[index + 1] - self.input[index]).max(f32::EPSILON);
        let alpha = ((time - self.input[index]) / span).clamp(0.0, 1.0);
        let a = self.output[index];
        let b = self.output[index + 1];
        [
            a[0] + (b[0] - a[0]) * alpha,
            a[1] + (b[1] - a[1]) * alpha,
            a[2] + (b[2] - a[2]) * alpha,
            a[3] + (b[3] - a[3]) * alpha,
        ]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AnimationChannel {
    #[serde(default)]
    pub target: EntityId,
    pub property: AnimationProperty,
    pub sampler: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AnimationClip {
    pub name: String,
    pub duration: f32,
    pub samplers: Vec<AnimationSampler>,
    pub channels: Vec<AnimationChannel>,
}

impl AnimationClip {
    pub fn sample(&self, time: f32) -> Vec<(AnimationProperty, [f32; 4])> {
        self.channels
            .iter()
            .filter_map(|channel| {
                self.samplers
                    .get(channel.sampler)
                    .map(|sampler| (channel.property.clone(), sampler.sample(time)))
            })
            .collect()
    }
}

#[derive(Component, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AnimationPlayer {
    pub clip: Option<String>,
    pub speed: f32,
    pub looping: bool,
    pub playing: bool,
    pub time: f32,
    #[serde(default)]
    pub finished: bool,
}

impl Default for AnimationPlayer {
    fn default() -> Self {
        Self {
            clip: None,
            speed: 1.0,
            looping: true,
            playing: false,
            time: 0.0,
            finished: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneEntity {
    #[serde(default)]
    pub id: EntityId,
    pub name: String,
    #[serde(default)]
    pub parent: Option<EntityId>,
    #[serde(default)]
    pub local_transform: TransformState,
    /// Compatibility mirror for schema 1/2/3 callers. Schema 4 writes local_transform.
    #[serde(default)]
    pub transform: TransformState,
    pub mesh: Option<MeshRenderer>,
    pub camera: Option<Camera>,
    pub light: Option<DirectionalLight>,
    pub collider: Option<BoxCollider>,
    #[serde(default)]
    pub audio_source: Option<AudioSource>,
    #[serde(default)]
    pub animation_player: Option<AnimationPlayer>,
    #[serde(default)]
    pub render_bounds: Option<RenderBounds>,
    #[serde(default)]
    pub skin: Option<SkinBinding>,
    #[serde(default)]
    pub audio_listener: Option<AudioListener>,
    #[serde(default)]
    pub prefab: Option<PrefabReference>,
    #[serde(default)]
    pub editor: EditorMetadata,
    #[serde(default)]
    pub dependencies: Vec<AssetDependency>,
}

impl Default for SceneEntity {
    fn default() -> Self {
        Self {
            id: EntityId::default(),
            name: "Entity".into(),
            parent: None,
            transform: TransformState::identity(),
            local_transform: TransformState::identity(),
            mesh: None,
            camera: None,
            light: None,
            collider: None,
            audio_source: None,
            animation_player: None,
            render_bounds: None,
            skin: None,
            audio_listener: None,
            prefab: None,
            editor: EditorMetadata::default(),
            dependencies: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneAsset {
    pub schema_version: u32,
    pub name: String,
    #[serde(default)]
    pub source_gltf: Option<std::path::PathBuf>,
    pub entities: Vec<SceneEntity>,
}

impl Default for SceneAsset {
    fn default() -> Self {
        Self {
            schema_version: SCENE_SCHEMA_VERSION,
            name: "Untitled".into(),
            source_gltf: None,
            entities: Vec::new(),
        }
    }
}

impl SceneAsset {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    pub fn push_entity(&mut self, mut entity: SceneEntity) -> EntityId {
        if entity.local_transform == TransformState::identity()
            && entity.transform != TransformState::identity()
        {
            entity.local_transform = entity.transform;
        }
        entity.transform = entity.local_transform;
        let id = self.allocate_entity_id();
        entity.id = id;
        self.entities.push(entity);
        id
    }

    pub fn allocate_entity_id(&self) -> EntityId {
        EntityId::new(
            self.entities
                .iter()
                .map(|entity| entity.id.0)
                .max()
                .unwrap_or(0)
                .saturating_add(1)
                .max(1),
        )
    }

    pub fn index_of(&self, id: EntityId) -> Option<usize> {
        self.entities.iter().position(|entity| entity.id == id)
    }

    pub fn instantiate(&self, world: &mut World) -> Vec<Entity> {
        self.entities
            .iter()
            .map(|entity| {
                let mut commands = world.spawn((
                    Transform::from_state(entity.local_transform),
                    LocalTransform(entity.local_transform),
                    WorldTransform(entity.local_transform),
                    PreviousWorldTransform(entity.local_transform),
                ));
                if let Some(parent) = entity.parent {
                    commands.insert(Parent(parent));
                }
                if let Some(mesh) = entity.mesh.clone() {
                    commands.insert(mesh);
                }
                if let Some(camera) = entity.camera {
                    commands.insert(camera);
                }
                if let Some(light) = entity.light {
                    commands.insert(light);
                }
                if let Some(collider) = entity.collider {
                    commands.insert(collider);
                }
                if let Some(audio) = entity.audio_source.clone() {
                    commands.insert(audio);
                }
                if let Some(animation) = entity.animation_player.clone() {
                    commands.insert(animation);
                }
                if let Some(bounds) = entity.render_bounds {
                    commands.insert(bounds);
                }
                if let Some(skin) = entity.skin.clone() {
                    commands.insert(skin);
                }
                if let Some(listener) = entity.audio_listener {
                    commands.insert(listener);
                }
                commands.id()
            })
            .collect()
    }

    pub fn hierarchy_order(&self) -> EngineResult<Vec<usize>> {
        self.validate_ids()?;
        let indices = self
            .entities
            .iter()
            .enumerate()
            .map(|(index, entity)| (entity.id, index))
            .collect::<std::collections::HashMap<_, _>>();
        let mut children = vec![Vec::new(); self.entities.len()];
        let mut roots = Vec::new();
        for (index, entity) in self.entities.iter().enumerate() {
            if let Some(parent) = entity.parent {
                let Some(&parent_index) = indices.get(&parent) else {
                    return Err(EngineError::Unsupported(format!(
                        "scene entity {} has missing parent {}",
                        entity.id.0, parent.0
                    )));
                };
                children[parent_index].push(index);
            } else {
                roots.push(index);
            }
        }

        let mut order = Vec::with_capacity(self.entities.len());
        let mut stack = roots.into_iter().rev().collect::<Vec<_>>();
        while let Some(index) = stack.pop() {
            order.push(index);
            stack.extend(children[index].iter().rev().copied());
        }
        if order.len() != self.entities.len() {
            return Err(EngineError::Unsupported(
                "scene hierarchy has a cycle".into(),
            ));
        }
        Ok(order)
    }

    pub fn validate_ids(&self) -> EngineResult<()> {
        let mut ids = std::collections::HashSet::with_capacity(self.entities.len());
        for entity in &self.entities {
            if !entity.id.is_valid() {
                return Err(EngineError::Unsupported(format!(
                    "scene entity {} has invalid id 0",
                    entity.name
                )));
            }
            if !ids.insert(entity.id) {
                return Err(EngineError::Unsupported(format!(
                    "duplicate scene entity id {}",
                    entity.id.0
                )));
            }
        }
        Ok(())
    }

    pub fn save_ron(&self, path: impl AsRef<Path>) -> EngineResult<()> {
        if self.schema_version != SCENE_SCHEMA_VERSION {
            return Err(EngineError::Unsupported(format!(
                "scene schema {}",
                self.schema_version
            )));
        }
        self.validate_ids()?;
        self.hierarchy_order()?;
        if self.entities.iter().any(|entity| {
            !entity.local_transform.translation.is_finite()
                || !entity.local_transform.rotation.is_finite()
                || !entity.local_transform.scale.is_finite()
        }) {
            return Err(EngineError::Unsupported(
                "scene contains non-finite transform".into(),
            ));
        }
        let mut normalized = self.clone();
        for entity in &mut normalized.entities {
            entity.transform = entity.local_transform;
        }
        let text = ron::ser::to_string_pretty(&normalized, ron::ser::PrettyConfig::default())
            .map_err(|error| EngineError::Serialization(error.to_string()))?;
        fs::write(path, text)?;
        Ok(())
    }

    pub fn load_ron(path: impl AsRef<Path>) -> EngineResult<Self> {
        let text = fs::read_to_string(path)?;
        let mut scene: Self =
            ron::from_str(&text).map_err(|error| EngineError::Serialization(error.to_string()))?;
        if scene.schema_version == LEGACY_SCENE_SCHEMA_VERSION {
            let ids = (0..scene.entities.len())
                .map(|index| EntityId::new(index as u64 + 1))
                .collect::<Vec<_>>();
            let legacy_parents = scene
                .entities
                .iter()
                .map(|entity| entity.parent.map(|parent| parent.0 as usize))
                .collect::<Vec<_>>();
            for (index, entity) in scene.entities.iter_mut().enumerate() {
                entity.id = ids[index];
                entity.local_transform = entity.transform;
                entity.parent = match legacy_parents[index] {
                    Some(parent) => Some(*ids.get(parent).ok_or_else(|| {
                        EngineError::Unsupported(format!(
                            "legacy scene entity {index} has missing parent {parent}"
                        ))
                    })?),
                    None => None,
                };
            }
            scene.schema_version = SCENE_SCHEMA_VERSION;
            tracing::info!(scene = %scene.name, "migrated scene schema 1 to schema 4");
        } else if scene.schema_version == SCHEMA_2 {
            for entity in &mut scene.entities {
                entity.local_transform = entity.transform;
            }
            scene.schema_version = SCENE_SCHEMA_VERSION;
            tracing::info!(scene = %scene.name, "migrated scene schema 2 to schema 4");
        } else if scene.schema_version == SCHEMA_3 {
            tracing::info!(scene = %scene.name, "migrated scene schema 3 to schema 4");
            scene.schema_version = SCENE_SCHEMA_VERSION;
        } else if scene.schema_version != SCENE_SCHEMA_VERSION {
            return Err(EngineError::Unsupported(format!(
                "scene schema {}",
                scene.schema_version
            )));
        }
        scene.validate_ids()?;
        scene.hierarchy_order()?;
        for entity in &mut scene.entities {
            if !entity.local_transform.translation.is_finite()
                || !entity.local_transform.rotation.is_finite()
                || !entity.local_transform.scale.is_finite()
            {
                return Err(EngineError::Unsupported(format!(
                    "entity {} has non-finite transform",
                    entity.id.0
                )));
            }
            if let Some(bounds) = entity.render_bounds
                && (!bounds.center.iter().all(|value| value.is_finite())
                    || !bounds.radius.is_finite()
                    || bounds.radius < 0.0)
            {
                return Err(EngineError::Unsupported(format!(
                    "entity {} has invalid render bounds",
                    entity.id.0
                )));
            }
            entity.transform = entity.local_transform;
        }
        Ok(scene)
    }

    pub fn create_entity(&mut self, name: impl Into<String>) -> EntityId {
        self.push_entity(SceneEntity {
            name: name.into(),
            ..SceneEntity::default()
        })
    }

    pub fn delete_entity_tree(&mut self, id: EntityId) -> EngineResult<()> {
        self.validate_ids()?;
        let mut remove = std::collections::HashSet::new();
        let mut changed = true;
        remove.insert(id);
        while changed {
            changed = false;
            for entity in &self.entities {
                if entity.parent.is_some_and(|parent| remove.contains(&parent))
                    && remove.insert(entity.id)
                {
                    changed = true;
                }
            }
        }
        if !self.entities.iter().any(|entity| entity.id == id) {
            return Err(EngineError::AssetNotFound(format!("entity {}", id.0)));
        }
        self.entities.retain(|entity| !remove.contains(&entity.id));
        Ok(())
    }

    pub fn duplicate_entity_tree(&mut self, id: EntityId) -> EngineResult<EntityId> {
        self.validate_ids()?;
        let order = self.hierarchy_order()?;
        let mut ids = std::collections::HashMap::new();
        let mut copies = Vec::new();
        let mut next_id = self.allocate_entity_id().0;
        for &index in &order {
            let source = &self.entities[index];
            if source.id == id
                || source
                    .parent
                    .is_some_and(|parent| ids.contains_key(&parent))
            {
                let new_id = EntityId::new(next_id);
                next_id = next_id.saturating_add(1).max(1);
                ids.insert(source.id, new_id);
                let mut copy = source.clone();
                copy.id = new_id;
                copy.parent = source.parent.and_then(|parent| ids.get(&parent).copied());
                copies.push(copy);
            }
        }
        let root = ids
            .get(&id)
            .copied()
            .ok_or_else(|| EngineError::AssetNotFound(format!("entity {}", id.0)))?;
        self.entities.extend(copies);
        Ok(root)
    }

    pub fn reparent(&mut self, id: EntityId, parent: Option<EntityId>) -> EngineResult<()> {
        self.validate_ids()?;
        if let Some(parent) = parent {
            if parent == id || self.index_of(parent).is_none() {
                return Err(EngineError::Unsupported("invalid reparent target".into()));
            }
            let mut cursor = Some(parent);
            while let Some(current) = cursor {
                if current == id {
                    return Err(EngineError::Unsupported(
                        "reparent would create cycle".into(),
                    ));
                }
                cursor = self
                    .index_of(current)
                    .and_then(|index| self.entities[index].parent);
            }
        }
        let index = self
            .index_of(id)
            .ok_or_else(|| EngineError::AssetNotFound(format!("entity {}", id.0)))?;
        let order = self.hierarchy_order()?;
        let mut worlds = vec![Mat4::IDENTITY; self.entities.len()];
        for &current in &order {
            let local = transform_matrix(self.entities[current].local_transform);
            worlds[current] = self.entities[current]
                .parent
                .and_then(|ancestor| self.index_of(ancestor))
                .map(|ancestor| worlds[ancestor] * local)
                .unwrap_or(local);
        }
        let old_world = worlds[index];
        self.entities[index].parent = parent;
        let parent_world = parent
            .and_then(|ancestor| self.index_of(ancestor))
            .map(|ancestor| worlds[ancestor])
            .unwrap_or(Mat4::IDENTITY);
        let local = parent_world.inverse() * old_world;
        if !local.is_finite() {
            return Err(EngineError::Unsupported(
                "parent transform is singular".into(),
            ));
        }
        let (scale, rotation, translation) = local.to_scale_rotation_translation();
        let transform = TransformState {
            translation,
            rotation,
            scale,
        };
        if !translation.is_finite() || !rotation.is_finite() || !scale.is_finite() {
            return Err(EngineError::Unsupported(
                "reparent produced a non-finite transform".into(),
            ));
        }
        self.entities[index].local_transform = transform;
        self.entities[index].transform = transform;
        Ok(())
    }
}

fn transform_matrix(state: TransformState) -> Mat4 {
    Mat4::from_scale_rotation_translation(state.scale, state.rotation, state.translation)
}

#[derive(Clone, Copy, Debug)]
pub struct SceneHandle(pub Handle<SceneAsset>);
