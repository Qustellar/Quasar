use super::*;
use qst_core::EntityId;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportedMesh {
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub color: [f32; 4],
    #[serde(default)]
    pub uvs: Vec<[f32; 2]>,
    #[serde(default)]
    pub tangents: Vec<[f32; 4]>,
    #[serde(default)]
    pub base_color_texture: Option<String>,
    #[serde(default)]
    pub metallic_roughness_texture: Option<String>,
    #[serde(default)]
    pub normal_texture: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GltfImport {
    pub scene: SceneAsset,
    pub meshes: Vec<ImportedMesh>,
    #[serde(default)]
    pub animations: Vec<AnimationClip>,
}

pub fn import_gltf(path: impl AsRef<Path>) -> EngineResult<GltfImport> {
    let (document, buffers, _) =
        ::gltf::import(path.as_ref()).map_err(|error| EngineError::Runtime(error.to_string()))?;
    let mut result = GltfImport {
        scene: SceneAsset::new(
            path.as_ref()
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("gltf"),
        ),
        meshes: Vec::new(),
        animations: Vec::new(),
    };
    if let Some(source_scene) = document
        .default_scene()
        .or_else(|| document.scenes().next())
    {
        for node in source_scene.nodes() {
            append_node(&mut result, &buffers, &node, Mat4::IDENTITY, None)?;
        }
    }
    for animation in document.animations() {
        let mut samplers = Vec::new();
        let mut channels = Vec::new();
        let mut duration = 0.0_f32;
        for channel in animation.channels() {
            let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
            let Some(input) = reader.read_inputs() else {
                continue;
            };
            let input: Vec<f32> = input.collect();
            let Some(output) = reader.read_outputs() else {
                continue;
            };
            let (property, output): (AnimationProperty, Vec<[f32; 4]>) = match output {
                ::gltf::animation::util::ReadOutputs::Translations(values) => (
                    AnimationProperty::Translation,
                    values
                        .map(|value| [value[0], value[1], value[2], 0.0])
                        .collect(),
                ),
                ::gltf::animation::util::ReadOutputs::Rotations(values) => (
                    AnimationProperty::Rotation,
                    values
                        .into_f32()
                        .map(|value| [value[0], value[1], value[2], value[3]])
                        .collect(),
                ),
                ::gltf::animation::util::ReadOutputs::Scales(values) => (
                    AnimationProperty::Scale,
                    values
                        .map(|value| [value[0], value[1], value[2], 0.0])
                        .collect(),
                ),
                ::gltf::animation::util::ReadOutputs::MorphTargetWeights(_) => continue,
            };
            duration = duration.max(input.last().copied().unwrap_or(0.0));
            let sampler = samplers.len();
            samplers.push(AnimationSampler {
                input,
                output,
                interpolation: match channel.sampler().interpolation() {
                    ::gltf::animation::Interpolation::Step => AnimationInterpolation::Step,
                    _ => AnimationInterpolation::Linear,
                },
            });
            channels.push(AnimationChannel { property, sampler });
        }
        result.animations.push(AnimationClip {
            name: animation.name().unwrap_or("Animation").into(),
            duration,
            samplers,
            channels,
        });
    }
    Ok(result)
}

const IMPORTER_VERSION: u32 = 1;
const CACHE_MAGIC: &[u8; 8] = b"QSTGLTF1";

#[derive(Serialize, Deserialize)]
struct CacheEnvelope {
    magic: [u8; 8],
    importer_version: u32,
    source_path: String,
    source_hash: [u8; 32],
    dependency_hashes: Vec<[u8; 32]>,
    import: GltfImport,
}

pub fn import_gltf_cached(path: impl AsRef<Path>) -> EngineResult<(GltfImport, bool)> {
    let path = path.as_ref();
    let source = std::fs::read(path)?;
    let source_hash = *blake3::hash(&source).as_bytes();
    let dependency_hashes = gltf_dependency_hashes(path);
    let cache_dir = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".quasar")
        .join("cache");
    let key = blake3::hash(path.to_string_lossy().as_bytes())
        .to_hex()
        .to_string();
    let cache_path = cache_dir.join(format!("{key}.gltf.bin"));
    if let Ok(bytes) = std::fs::read(&cache_path)
        && let Ok((envelope, _)) = bincode::serde::decode_from_slice::<CacheEnvelope, _>(
            &bytes,
            bincode::config::standard(),
        )
        && envelope.magic == *CACHE_MAGIC
        && envelope.importer_version == IMPORTER_VERSION
        && envelope.source_hash == source_hash
        && envelope.dependency_hashes == dependency_hashes
        && envelope.source_path == path.to_string_lossy()
    {
        return Ok((envelope.import, true));
    }

    let imported = import_gltf(path)?;
    let envelope = CacheEnvelope {
        magic: *CACHE_MAGIC,
        importer_version: IMPORTER_VERSION,
        source_path: path.to_string_lossy().into_owned(),
        source_hash,
        dependency_hashes,
        import: imported.clone(),
    };
    if std::fs::create_dir_all(&cache_dir).is_ok()
        && let Ok(encoded) = bincode::serde::encode_to_vec(&envelope, bincode::config::standard())
    {
        let temporary = cache_path.with_extension("tmp");
        if std::fs::write(&temporary, encoded).is_ok() {
            let _ = std::fs::rename(&temporary, &cache_path);
        }
    }
    Ok((imported, false))
}

fn gltf_dependency_hashes(path: &Path) -> Vec<[u8; 32]> {
    let Ok(document) = ::gltf::Gltf::open(path) else {
        return Vec::new();
    };
    document
        .document
        .buffers()
        .filter_map(|buffer| match buffer.source() {
            ::gltf::buffer::Source::Uri(uri) if !uri.starts_with("data:") => {
                let dependency = path.parent().unwrap_or_else(|| Path::new(".")).join(uri);
                std::fs::read(dependency)
                    .ok()
                    .map(|bytes| *blake3::hash(&bytes).as_bytes())
            }
            _ => None,
        })
        .collect()
}

fn append_node(
    result: &mut GltfImport,
    buffers: &[::gltf::buffer::Data],
    node: &::gltf::Node<'_>,
    parent_transform: Mat4,
    parent_index: Option<usize>,
) -> EngineResult<()> {
    let local = Mat4::from_cols_array_2d(&node.transform().matrix());
    let world = parent_transform * local;
    let (scale, rotation, translation) = world.to_scale_rotation_translation();
    let mut entity = SceneEntity {
        id: EntityId::new(result.scene.entities.len() as u64 + 1),
        name: node.name().unwrap_or("Node").into(),
        parent: parent_index.map(|index| EntityId::new(index as u64 + 1)),
        transform: TransformState {
            translation,
            rotation,
            scale,
        },
        ..SceneEntity::default()
    };
    if let Some(camera) = node.camera()
        && let ::gltf::camera::Projection::Perspective(p) = camera.projection()
    {
        entity.camera = Some(Camera {
            fov_y_radians: p.yfov(),
            near: p.znear(),
            far: p.zfar().unwrap_or(1000.0),
        });
    }
    let node_index = result.scene.entities.len();
    result.scene.entities.push(entity);
    if let Some(mesh) = node.mesh() {
        for (primitive_index, primitive) in mesh.primitives().enumerate() {
            if primitive.mode() != ::gltf::mesh::Mode::Triangles {
                return Err(EngineError::Unsupported(format!(
                    "glTF node {} primitive {primitive_index} uses {:?}; only triangles are supported",
                    node.index(),
                    primitive.mode()
                )));
            }
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            let Some(positions) = reader.read_positions() else {
                return Err(EngineError::Unsupported(format!(
                    "glTF node {} primitive {primitive_index} has no positions",
                    node.index()
                )));
            };
            let positions: Vec<[f32; 3]> = positions.collect();
            let normals: Vec<[f32; 3]> = reader
                .read_normals()
                .map(|normals| normals.collect())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
            let uvs = reader
                .read_tex_coords(0)
                .map(|values| {
                    values
                        .into_f32()
                        .map(|value| [value[0], value[1]])
                        .collect()
                })
                .unwrap_or_default();
            let tangents = reader
                .read_tangents()
                .map(|values| values.collect())
                .unwrap_or_default();
            let indices: Vec<u32> = reader
                .read_indices()
                .map(|indices| indices.into_u32().collect())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());
            let name = format!("gltf_mesh_{}_{}", node.index(), primitive_index);
            let color = primitive
                .material()
                .pbr_metallic_roughness()
                .base_color_factor();
            let material = primitive.material();
            let pbr = material.pbr_metallic_roughness();
            let base_color_texture = pbr.base_color_texture().map(|texture| {
                texture
                    .texture()
                    .source()
                    .name()
                    .unwrap_or("texture")
                    .to_owned()
            });
            let metallic_roughness_texture = pbr.metallic_roughness_texture().map(|texture| {
                texture
                    .texture()
                    .source()
                    .name()
                    .unwrap_or("texture")
                    .to_owned()
            });
            let normal_texture = material.normal_texture().map(|texture| {
                texture
                    .texture()
                    .source()
                    .name()
                    .unwrap_or("texture")
                    .to_owned()
            });
            result.meshes.push(ImportedMesh {
                name: name.clone(),
                positions,
                normals,
                indices,
                color,
                uvs,
                tangents,
                base_color_texture,
                metallic_roughness_texture,
                normal_texture,
            });
            result.scene.entities.push(SceneEntity {
                id: EntityId::new(result.scene.entities.len() as u64 + 1),
                name: format!(
                    "{} primitive {}",
                    mesh.name().unwrap_or("Mesh"),
                    primitive_index
                ),
                parent: Some(EntityId::new(node_index as u64 + 1)),
                transform: TransformState {
                    translation,
                    rotation,
                    scale,
                },
                mesh: Some(MeshRenderer {
                    mesh: name.clone(),
                    material: name,
                }),
                ..SceneEntity::default()
            });
        }
    }
    for child in node.children() {
        append_node(result, buffers, &child, world, Some(node_index))?;
    }
    Ok(())
}
