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
}
