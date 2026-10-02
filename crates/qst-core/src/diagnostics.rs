use super::*;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FrameDiagnostics {
    pub frame_seconds: f32,
    pub fixed_update_seconds: f32,
    pub render_seconds: f32,
    pub resident_working_set_bytes: Option<u64>,
    #[serde(default)]
    pub private_working_set_bytes: Option<u64>,
    #[serde(default)]
    pub private_commit_bytes: Option<u64>,
    pub loaded_asset_count: usize,
    pub gpu_resource_count: usize,
    #[serde(default)]
    pub fixed_steps: u32,
    #[serde(default)]
    pub dropped_fixed_steps: u64,
    #[serde(default)]
    pub render_instances: usize,
    #[serde(default)]
    pub render_batches: usize,
    #[serde(default)]
    pub render_draw_calls: usize,
    #[serde(default)]
    pub instance_upload_bytes: u64,
    #[serde(default)]
    pub asset_cache_hits: u64,
    #[serde(default)]
    pub asset_cache_misses: u64,
    #[serde(default)]
    pub render_fallback: bool,
    pub last_reload: Option<String>,
    #[serde(default)]
    pub input_update_seconds: f32,
    #[serde(default)]
    pub scene_load_seconds: f32,
    #[serde(default)]
    pub asset_import_seconds: f32,
    #[serde(default)]
    pub cache_load_seconds: f32,
    #[serde(default)]
    pub gpu_upload_seconds: f32,
    #[serde(default)]
    pub physics_seconds: f32,
    #[serde(default)]
    pub transform_update_seconds: f32,
    #[serde(default)]
    pub animation_update_seconds: f32,
    #[serde(default)]
    pub render_prepare_seconds: f32,
    #[serde(default)]
    pub render_submit_seconds: f32,
    #[serde(default)]
    pub editor_seconds: f32,
    #[serde(default)]
    pub skipped_instances: usize,
    #[serde(default)]
    pub asset_dependency_count: u64,
    #[serde(default)]
    pub asset_rebuild_count: u64,
    #[serde(default)]
    pub visible_instances: usize,
    #[serde(default)]
    pub culled_instances: usize,
    #[serde(default)]
    pub indirect_draw_calls: usize,
    #[serde(default)]
    pub gpu_cull_seconds: f32,
    #[serde(default)]
    pub skinning_seconds: f32,
    #[serde(default)]
    pub render_path: String,
}
