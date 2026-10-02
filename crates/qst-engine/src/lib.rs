use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Instant;

pub use qst_asset::{AssetServer, AssetStorage, LoadState};
#[cfg(feature = "audio")]
pub use qst_audio::RodioAudioBackend;
pub use qst_audio::{
    AudioBackend, AudioClip, AudioFormat, AudioPlaybackState, AudioPlayer,
    AudioSource as AudioRuntimeSource, NullAudioBackend,
};
pub use qst_core::{
    EngineError, EngineResult, EntityId, FixedTime, FrameDiagnostics, Handle, TransformState, glam,
};
pub use qst_ecs::{
    Component, Entity, FixedUpdate, IntoScheduleConfigs, Physics, PostUpdate, PreUpdate, Render,
    RenderExtract, Resource, Schedule, SimulationStep, Startup, VariableUpdate, World,
};
pub use qst_input::{ButtonState, GamepadInput, InputState, KeyboardState, MouseState};
pub use qst_render::{
    IndirectDrawIndexedArgs, MaterialAsset, MeshAsset, MeshVertex, MeshVertexPbr, PbrMaterial,
    RenderFeature, RenderGraph, RenderNode, RenderPath, RenderSnapshot, RenderStats, SamplerAsset,
    TextureAsset, TexturedMeshAsset,
};
pub use qst_scene::{
    AnimationChannel, AnimationClip, AnimationInterpolation, AnimationPlayer, AnimationProperty,
    AnimationSampler, AssetDependency, AudioListener, AudioSource, BoxCollider, Camera,
    DirectionalLight, EditorMetadata, ImportedSkin, LocalTransform, MeshRenderer, Parent,
    PrefabReference, PreviousWorldTransform, RenderBounds, SceneAsset, SceneEntity, SkinBinding,
    Transform, WorldTransform,
};

pub mod prelude {
    pub use crate::{
        AnimationChannel, AnimationClip, AnimationInterpolation, AnimationPlayer,
        AnimationProperty, AnimationSampler, AssetDependency, AudioBackend, AudioClip, AudioFormat,
        AudioListener, AudioPlaybackState, AudioPlayer, BoxCollider, ButtonState, Camera,
        DirectionalLight, EditorMetadata, EngineApp, EngineError, EngineResult, EntityId,
        FixedTime, FrameDiagnostics, GamepadInput, Handle, ImportedSkin, InputState, KeyboardState,
        LocalTransform, MaterialAsset, MeshAsset, MeshRenderer, MeshVertexPbr, MouseState,
        NullAudioBackend, Parent, PbrMaterial, Plugin, PrefabReference, PreviousWorldTransform,
        RenderBounds, RenderFeature, RenderGraph, RenderNode, RenderPath, RenderSnapshot,
        RenderStats, SamplerAsset, SceneAsset, SceneEntity, SkinBinding, TextureAsset,
        TexturedMeshAsset, Transform, WorldTransform,
    };
    #[cfg(feature = "editor")]
    pub use crate::{EditorPlugin, GizmoMode};
}

use qst_ecs::{
    make_fixed_schedule, make_physics_schedule, make_post_update_schedule,
    make_pre_update_schedule, make_render_extract_schedule, make_render_schedule,
    make_startup_schedule, make_variable_schedule,
};
#[cfg(feature = "editor")]
pub use qst_editor::{EditorPlugin, GizmoMode};
#[cfg(feature = "editor")]
use qst_editor::{EditorRuntime, PlayState};
pub use qst_physics::PhysicsCollision;
use qst_physics::{PhysicsConfig, PhysicsWorld};
use qst_render::{RenderCamera, RenderInstance, RenderLight, Renderer};
use qst_scene::import_gltf_cached;
use qst_scene::{GltfImport, ImportedMesh, ImportedTexture};

#[derive(Default)]
struct EngineAudioBackend {
    #[cfg(feature = "audio")]
    backend: qst_audio::RodioAudioBackend,
    #[cfg(not(feature = "audio"))]
    backend: qst_audio::NullAudioBackend,
}

impl qst_audio::AudioBackend for EngineAudioBackend {
    fn play(
        &mut self,
        clip: Handle<AudioClip>,
        data: &AudioClip,
        volume: f32,
        looping: bool,
    ) -> EngineResult<()> {
        self.backend.play(clip, data, volume, looping)
    }
    fn pause(&mut self, clip: Handle<AudioClip>) -> EngineResult<()> {
        self.backend.pause(clip)
    }
    fn stop(&mut self, clip: Handle<AudioClip>) -> EngineResult<()> {
        self.backend.stop(clip)
    }
}
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

fn transform_matrix(state: TransformState) -> glam::Mat4 {
    glam::Mat4::from_scale_rotation_translation(state.scale, state.rotation, state.translation)
}

fn transformed_state(delta: glam::Mat4, state: TransformState) -> Option<TransformState> {
    let matrix = delta * transform_matrix(state);
    if !matrix.is_finite() {
        return None;
    }
    let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
    (scale.is_finite() && rotation.is_finite() && translation.is_finite()).then_some(
        TransformState {
            translation,
            rotation,
            scale,
        },
    )
}

fn scaled_half_extents(
    half_extents: [f32; 3],
    previous: glam::Vec3,
    current: glam::Vec3,
) -> Option<[f32; 3]> {
    if previous.abs().min_element() < 1e-6 {
        return None;
    }
    let ratio = (current / previous).abs();
    ratio.is_finite().then_some([
        half_extents[0] * ratio.x,
        half_extents[1] * ratio.y,
        half_extents[2] * ratio.z,
    ])
}

pub trait Plugin: Send {
    fn build(&self, app: &mut EngineApp);
}

enum ReloadResult {
    Gltf(PathBuf, EngineResult<GltfImport>),
    Scene(PathBuf, EngineResult<SceneAsset>),
}

#[cfg(test)]
use metrics::percentile;
use metrics::{BenchmarkCapture, current_process_memory};

pub struct EngineApp {
    pub world: World,
    pub fixed_schedule: Schedule,
    pub startup_schedule: Schedule,
    pub pre_update_schedule: Schedule,
    pub variable_schedule: Schedule,
    pub physics_schedule: Schedule,
    pub post_update_schedule: Schedule,
    pub render_extract_schedule: Schedule,
    pub render_schedule: Schedule,
    pub fixed_time: FixedTime,
    pub assets: AssetServer,
    pub scene: SceneAsset,
    pub diagnostics: FrameDiagnostics,
    mesh_assets: AssetStorage<MeshAsset>,
    material_assets: AssetStorage<MaterialAsset>,
    texture_assets: AssetStorage<TextureAsset>,
    audio_assets: AssetStorage<AudioClip>,
    mesh_names: HashMap<String, Handle<MeshAsset>>,
    material_names: HashMap<String, Handle<MaterialAsset>>,
    texture_names: HashMap<String, Handle<TextureAsset>>,
    animation_clips: HashMap<String, AnimationClip>,
    audio_names: HashMap<String, Handle<AudioClip>>,
    audio_player: AudioPlayer<EngineAudioBackend>,
    scene_entities: Vec<Entity>,
    hierarchy_order: Vec<usize>,
    scene_index_by_id: HashMap<EntityId, usize>,
    last_scene_transforms: Vec<TransformState>,
    authored_transforms: Vec<TransformState>,
    gltf_mesh_names: Vec<String>,
    gltf_texture_names: Vec<String>,
    gltf_path: Option<PathBuf>,
    scene_path: Option<PathBuf>,
    saved_scene_contents: Option<(PathBuf, Vec<u8>)>,
    reload_tx: Sender<ReloadResult>,
    reload_rx: Receiver<ReloadResult>,
    reload_pending: bool,
    window_size: (u32, u32),
    frame_index: u64,
    benchmark: Option<BenchmarkCapture>,
    physics: PhysicsWorld,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    fatal_error: Option<EngineError>,
    pending_render_features: Vec<Box<dyn RenderFeature>>,
    #[cfg(feature = "editor")]
    editor: Option<EditorRuntime>,
    last_frame: Instant,
}

impl Default for EngineApp {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineApp {
    pub fn new() -> Self {
        let fixed_time = FixedTime::default();
        let (reload_tx, reload_rx) = mpsc::channel();
        let mut world = World::new();
        world.insert_resource(SimulationStep {
            index: 0,
            delta_seconds: fixed_time.step_seconds,
        });
        world.insert_resource(InputState::default());
        Self {
            world,
            fixed_schedule: make_fixed_schedule(),
            startup_schedule: make_startup_schedule(),
            pre_update_schedule: make_pre_update_schedule(),
            variable_schedule: make_variable_schedule(),
            physics_schedule: make_physics_schedule(),
            post_update_schedule: make_post_update_schedule(),
            render_extract_schedule: make_render_extract_schedule(),
            render_schedule: make_render_schedule(),
            fixed_time,
            assets: AssetServer::new(),
            scene: SceneAsset::default(),
            diagnostics: FrameDiagnostics::default(),
            mesh_assets: AssetStorage::default(),
            material_assets: AssetStorage::default(),
            texture_assets: AssetStorage::default(),
            audio_assets: AssetStorage::default(),
            mesh_names: HashMap::new(),
            material_names: HashMap::new(),
            texture_names: HashMap::new(),
            animation_clips: HashMap::new(),
            audio_names: HashMap::new(),
            audio_player: AudioPlayer::default(),
            scene_entities: Vec::new(),
            hierarchy_order: Vec::new(),
            scene_index_by_id: HashMap::new(),
            last_scene_transforms: Vec::new(),
            authored_transforms: Vec::new(),
            gltf_mesh_names: Vec::new(),
            gltf_texture_names: Vec::new(),
            gltf_path: None,
            scene_path: None,
            saved_scene_contents: None,
            reload_tx,
            reload_rx,
            reload_pending: false,
            window_size: (1280, 720),
            frame_index: 0,
            benchmark: None,
            physics: PhysicsWorld::new(PhysicsConfig::default()),
            window: None,
            renderer: None,
            fatal_error: None,
            pending_render_features: Vec::new(),
            last_frame: Instant::now(),
            #[cfg(feature = "editor")]
            editor: None,
        }
    }

    pub fn add_plugin(&mut self, plugin: impl Plugin) -> &mut Self {
        plugin.build(self);
        self
    }

    pub fn add_render_feature(&mut self, feature: impl RenderFeature + 'static) -> &mut Self {
        if let Some(renderer) = &mut self.renderer {
            renderer.features.push(Box::new(feature));
        } else {
            self.pending_render_features.push(Box::new(feature));
        }
        self
    }

    pub fn set_window_size(&mut self, width: u32, height: u32) -> &mut Self {
        self.window_size = (width.max(1), height.max(1));
        self
    }

    pub fn set_benchmark_frames(&mut self, warmup_frames: u32, measured_frames: u32) -> &mut Self {
        self.benchmark = Some(BenchmarkCapture::new(warmup_frames, measured_frames));
        self
    }

    pub fn add_fixed_systems<M>(
        &mut self,
        systems: impl IntoScheduleConfigs<qst_ecs::ScheduleSystem, M>,
    ) -> &mut Self {
        self.fixed_schedule.add_systems(systems);
        self
    }

    pub fn add_startup_systems<M>(
        &mut self,
        systems: impl IntoScheduleConfigs<qst_ecs::ScheduleSystem, M>,
    ) -> &mut Self {
        self.startup_schedule.add_systems(systems);
        self
    }
    pub fn add_pre_update_systems<M>(
        &mut self,
        systems: impl IntoScheduleConfigs<qst_ecs::ScheduleSystem, M>,
    ) -> &mut Self {
        self.pre_update_schedule.add_systems(systems);
        self
    }
    pub fn add_update_systems<M>(
        &mut self,
        systems: impl IntoScheduleConfigs<qst_ecs::ScheduleSystem, M>,
    ) -> &mut Self {
        self.variable_schedule.add_systems(systems);
        self
    }
    pub fn add_physics_systems<M>(
        &mut self,
        systems: impl IntoScheduleConfigs<qst_ecs::ScheduleSystem, M>,
    ) -> &mut Self {
        self.physics_schedule.add_systems(systems);
        self
    }
    pub fn add_post_update_systems<M>(
        &mut self,
        systems: impl IntoScheduleConfigs<qst_ecs::ScheduleSystem, M>,
    ) -> &mut Self {
        self.post_update_schedule.add_systems(systems);
        self
    }
    pub fn add_render_extract_systems<M>(
        &mut self,
        systems: impl IntoScheduleConfigs<qst_ecs::ScheduleSystem, M>,
    ) -> &mut Self {
        self.render_extract_schedule.add_systems(systems);
        self
    }
    pub fn add_render_systems<M>(
        &mut self,
        systems: impl IntoScheduleConfigs<qst_ecs::ScheduleSystem, M>,
    ) -> &mut Self {
        self.render_schedule.add_systems(systems);
        self
    }

    pub fn register_animation_clip(&mut self, clip: AnimationClip) -> &mut Self {
        self.animation_clips.insert(clip.name.clone(), clip);
        self
    }

    pub fn register_audio(
        &mut self,
        name: impl Into<String>,
        clip: AudioClip,
    ) -> Handle<AudioClip> {
        let name = name.into();
        if let Some(old) = self.audio_names.remove(&name) {
            self.audio_assets.remove(old);
        }
        let handle = self.assets.load::<AudioClip>(&name);
        self.audio_assets.insert(handle, clip.clone());
        self.audio_player.register_clip(handle, clip);
        let _ = self.assets.mark_loaded(handle);
        self.audio_names.insert(name, handle);
        handle
    }

    pub fn play_audio(&mut self, source: qst_audio::AudioSource) -> EngineResult<()> {
        self.audio_player.play(source)
    }
    pub fn pause_audio(&mut self, clip: Handle<AudioClip>) -> EngineResult<()> {
        self.audio_player.pause(clip)
    }
    pub fn stop_audio(&mut self, clip: Handle<AudioClip>) -> EngineResult<()> {
        self.audio_player.stop(clip)
    }
}

mod assets;
mod metrics;
mod runtime;
mod scene_runtime;
mod simulation;
#[cfg(test)]
mod tests;
