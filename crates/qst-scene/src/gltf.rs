use super::*;

#[derive(Clone, Debug)]
pub struct ImportedMesh {
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub color: [f32; 4],
}

#[derive(Clone, Debug)]
pub struct GltfImport {
    pub scene: SceneAsset,
    pub meshes: Vec<ImportedMesh>,
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
    };
    if let Some(source_scene) = document
        .default_scene()
        .or_else(|| document.scenes().next())
    {
        for node in source_scene.nodes() {
            append_node(&mut result, &buffers, &node, Mat4::IDENTITY, None)?;
        }
    }
    Ok(result)
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
        name: node.name().unwrap_or("Node").into(),
        parent: parent_index,
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
            let indices: Vec<u32> = reader
                .read_indices()
                .map(|indices| indices.into_u32().collect())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());
            let name = format!("gltf_mesh_{}_{}", node.index(), primitive_index);
            let color = primitive
                .material()
                .pbr_metallic_roughness()
                .base_color_factor();
            result.meshes.push(ImportedMesh {
                name: name.clone(),
                positions,
                normals,
                indices,
                color,
            });
            result.scene.entities.push(SceneEntity {
                name: format!(
                    "{} primitive {}",
                    mesh.name().unwrap_or("Mesh"),
                    primitive_index
                ),
                parent: Some(node_index),
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
