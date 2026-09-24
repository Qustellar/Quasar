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
    pub last_reload: Option<String>,
}
