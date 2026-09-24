use qst_core::glam::Mat4;
use qst_core::{EngineError, EngineResult, Handle, TransformState};
use qst_ecs::{Component, Entity, World};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

mod gltf;
mod scene;
pub use gltf::{GltfImport, ImportedMesh, import_gltf};
pub use scene::{
    BoxCollider, Camera, DirectionalLight, MeshRenderer, SCENE_SCHEMA_VERSION, SceneAsset,
    SceneEntity, SceneHandle, Transform,
};
#[cfg(test)]
mod tests;
