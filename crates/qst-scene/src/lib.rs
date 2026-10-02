use qst_core::glam::Mat4;
use qst_core::{EngineError, EngineResult, Handle, TransformState};
use qst_ecs::{Component, Entity, World};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

mod gltf;
mod scene;
pub use gltf::{
    GltfImport, ImportedMesh, ImportedSkin, ImportedTexture, import_gltf, import_gltf_cached,
};
pub use qst_core::EntityId;
pub use scene::{
    AnimationChannel, AnimationClip, AnimationInterpolation, AnimationPlayer, AnimationProperty,
    AnimationSampler, AssetDependency, AudioListener, AudioSource, BoxCollider, Camera,
    DirectionalLight, EditorMetadata, LEGACY_SCENE_SCHEMA_VERSION, LocalTransform, MeshRenderer,
    Parent, PrefabReference, PreviousWorldTransform, RenderBounds, SCENE_SCHEMA_VERSION, SCHEMA_2,
    SCHEMA_3, SceneAsset, SceneEntity, SceneHandle, SkinBinding, Transform, WorldTransform,
};
#[cfg(test)]
mod tests;
