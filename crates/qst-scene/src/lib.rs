use qst_core::glam::Mat4;
use qst_core::{EngineError, EngineResult, Handle, TransformState};
use qst_ecs::{Component, Entity, World};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

mod gltf;
mod scene;
pub use gltf::{GltfImport, ImportedMesh, ImportedTexture, import_gltf, import_gltf_cached};
pub use qst_core::EntityId;
pub use scene::{
    AnimationChannel, AnimationClip, AnimationInterpolation, AnimationPlayer, AnimationProperty,
    AnimationSampler, AudioSource, BoxCollider, Camera, DirectionalLight,
    LEGACY_SCENE_SCHEMA_VERSION, LocalTransform, MeshRenderer, Parent, PreviousWorldTransform,
    SCENE_SCHEMA_VERSION, SCHEMA_2, SceneAsset, SceneEntity, SceneHandle, Transform,
    WorldTransform,
};
#[cfg(test)]
mod tests;
