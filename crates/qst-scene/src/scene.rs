use super::*;

pub const SCENE_SCHEMA_VERSION: u32 = 1;

#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Transform {
    pub previous: TransformState,
    pub current: TransformState,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneEntity {
    pub name: String,
    #[serde(default)]
    pub parent: Option<usize>,
    pub transform: TransformState,
    pub mesh: Option<MeshRenderer>,
    pub camera: Option<Camera>,
    pub light: Option<DirectionalLight>,
    pub collider: Option<BoxCollider>,
}

impl Default for SceneEntity {
    fn default() -> Self {
        Self {
            name: "Entity".into(),
            parent: None,
            transform: TransformState::identity(),
            mesh: None,
            camera: None,
            light: None,
            collider: None,
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

    pub fn instantiate(&self, world: &mut World) -> Vec<Entity> {
        self.entities
            .iter()
            .map(|entity| {
                let mut commands = world.spawn((Transform::from_state(entity.transform),));
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
                commands.id()
            })
            .collect()
    }

    pub fn hierarchy_order(&self) -> EngineResult<Vec<usize>> {
        let mut children = vec![Vec::new(); self.entities.len()];
        let mut roots = Vec::new();
        for (index, entity) in self.entities.iter().enumerate() {
            if let Some(parent) = entity.parent {
                let Some(siblings) = children.get_mut(parent) else {
                    return Err(EngineError::Unsupported(format!(
                        "scene entity {index} has missing parent {parent}"
                    )));
                };
                siblings.push(index);
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

    pub fn save_ron(&self, path: impl AsRef<Path>) -> EngineResult<()> {
        if self.schema_version != SCENE_SCHEMA_VERSION {
            return Err(EngineError::Unsupported(format!(
                "scene schema {}",
                self.schema_version
            )));
        }
        self.hierarchy_order()?;
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|error| EngineError::Serialization(error.to_string()))?;
        fs::write(path, text)?;
        Ok(())
    }

    pub fn load_ron(path: impl AsRef<Path>) -> EngineResult<Self> {
        let text = fs::read_to_string(path)?;
        let scene: Self =
            ron::from_str(&text).map_err(|error| EngineError::Serialization(error.to_string()))?;
        if scene.schema_version != SCENE_SCHEMA_VERSION {
            return Err(EngineError::Unsupported(format!(
                "scene schema {}",
                scene.schema_version
            )));
        }
        scene.hierarchy_order()?;
        Ok(scene)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SceneHandle(pub Handle<SceneAsset>);
