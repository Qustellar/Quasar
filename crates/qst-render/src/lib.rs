use std::collections::HashMap;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use qst_core::{
    AssetId, EngineError, EngineResult, Handle, TransformState,
    glam::{Mat4, Vec3},
};
use wgpu::util::DeviceExt;
use wgpu::{Device, Queue, Surface, SurfaceConfiguration};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct MeshVertexPbr {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tangent: [f32; 4],
    pub uv: [f32; 2],
    pub joints: [u16; 4],
    pub weights: [f32; 4],
}

#[derive(Clone, Debug)]
pub struct SkinnedVertex {
    pub position: [f32; 3],
    pub joints: [u16; 4],
    pub weights: [f32; 4],
}

/// CPU numerical reference; rendering uses vertex-stage skinning on both culling paths.
pub fn skin_vertices(vertices: &[SkinnedVertex], joint_matrices: &[Mat4]) -> Vec<[f32; 3]> {
    vertices
        .iter()
        .map(|vertex| {
            let position = qst_core::glam::Vec4::from_array([
                vertex.position[0],
                vertex.position[1],
                vertex.position[2],
                1.0,
            ]);
            let mut result = qst_core::glam::Vec4::ZERO;
            let mut total = 0.0;
            for index in 0..4 {
                if joint_matrices.is_empty() {
                    break;
                }
                let matrix =
                    joint_matrices[(vertex.joints[index] as usize).min(joint_matrices.len() - 1)];
                let weight = vertex.weights[index].max(0.0);
                result += (matrix * position) * weight;
                total += weight;
            }
            if total > 0.000001 {
                (result / total).truncate().to_array()
            } else {
                vertex.position
            }
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct TexturedMeshAsset {
    pub vertices: Vec<MeshVertexPbr>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct MeshAsset {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureAsset {
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
}

impl TextureAsset {
    pub fn from_bytes(bytes: &[u8]) -> EngineResult<Self> {
        let image = image::load_from_memory(bytes)
            .map_err(|error| EngineError::Runtime(error.to_string()))?
            .to_rgba8();
        Ok(Self {
            width: image.width(),
            height: image.height(),
            rgba8: image.into_raw(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplerAsset {
    pub mag_filter: u32,
    pub min_filter: u32,
    pub repeat_u: bool,
    pub repeat_v: bool,
}

impl Default for SamplerAsset {
    fn default() -> Self {
        Self {
            mag_filter: 9729,
            min_filter: 9987,
            repeat_u: true,
            repeat_v: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialAsset {
    pub color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub base_color_texture: Option<Handle<TextureAsset>>,
    pub metallic_roughness_texture: Option<Handle<TextureAsset>>,
    pub normal_texture: Option<Handle<TextureAsset>>,
}

impl MaterialAsset {
    pub const fn from_color(color: [f32; 4]) -> Self {
        Self {
            color,
            metallic: 0.0,
            roughness: 0.5,
            base_color_texture: None,
            metallic_roughness_texture: None,
            normal_texture: None,
        }
    }
    pub const fn base_color(&self) -> [f32; 4] {
        self.color
    }
    pub const fn metallic(&self) -> f32 {
        self.metallic
    }
    pub const fn roughness(&self) -> f32 {
        self.roughness
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PbrMaterial {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub base_color_texture: Option<Handle<TextureAsset>>,
    pub metallic_roughness_texture: Option<Handle<TextureAsset>>,
    pub normal_texture: Option<Handle<TextureAsset>>,
}

impl PbrMaterial {
    pub const fn from_color(color: [f32; 4]) -> Self {
        Self {
            base_color: color,
            metallic: 0.0,
            roughness: 0.5,
            base_color_texture: None,
            metallic_roughness_texture: None,
            normal_texture: None,
        }
    }
}

impl From<MaterialAsset> for PbrMaterial {
    fn from(material: MaterialAsset) -> Self {
        Self {
            base_color: material.color,
            metallic: material.metallic,
            roughness: material.roughness,
            base_color_texture: material.base_color_texture,
            metallic_roughness_texture: material.metallic_roughness_texture,
            normal_texture: material.normal_texture,
        }
    }
}

impl Default for PbrMaterial {
    fn default() -> Self {
        Self {
            base_color: [1.0; 4],
            metallic: 0.0,
            roughness: 0.5,
            base_color_texture: None,
            metallic_roughness_texture: None,
            normal_texture: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RenderInstance {
    pub entity: u64,
    pub transform: TransformState,
    pub bounds: [f32; 4],
    pub mesh: Handle<MeshAsset>,
    pub material: Handle<MaterialAsset>,
    /// Index into `RenderSnapshot::skins` for this instance.
    pub skin: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct RenderSkin {
    pub matrices: Arc<[Mat4]>,
}

#[derive(Clone, Copy, Debug)]
pub struct RenderCamera {
    pub transform: TransformState,
    pub fov_y_radians: f32,
    pub near: f32,
    pub far: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct RenderLight {
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
}

#[derive(Clone, Debug, Default)]
pub struct RenderSnapshot {
    pub instances: Vec<RenderInstance>,
    pub skins: Vec<RenderSkin>,
    /// Instances whose requested scene skin could not produce a finite palette.
    pub skin_fallbacks: usize,
    pub camera: Option<RenderCamera>,
    pub lights: Vec<RenderLight>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderPreference {
    #[default]
    Auto,
    CpuIndirect,
    GpuDriven,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderCapabilities {
    pub compute: bool,
    pub storage_buffers: bool,
    pub indirect_draw: bool,
}

impl RenderCapabilities {
    pub fn from_adapter(adapter: &wgpu::Adapter) -> Self {
        let flags = adapter.get_downlevel_capabilities().flags;
        let limits = adapter.limits();
        Self {
            compute: flags.contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
                && limits.max_compute_workgroup_size_x >= 64
                && limits.max_compute_invocations_per_workgroup >= 64
                && limits.max_storage_buffers_per_shader_stage >= 3,
            storage_buffers: flags.contains(wgpu::DownlevelFlags::VERTEX_STORAGE)
                && limits.max_storage_buffers_per_shader_stage >= 1,
            indirect_draw: flags.contains(wgpu::DownlevelFlags::INDIRECT_EXECUTION),
        }
    }
    pub const fn supports_gpu_driven(self) -> bool {
        self.compute && self.storage_buffers && self.indirect_draw
    }
}

impl RenderPreference {
    /// Resolve the preference without creating a device, including adapter-free diagnostics.
    pub fn select(self, capabilities: RenderCapabilities) -> EngineResult<RenderPath> {
        match self {
            Self::Auto if capabilities.supports_gpu_driven() => Ok(RenderPath::GpuDriven),
            Self::Auto | Self::CpuIndirect => Ok(RenderPath::Indirect),
            Self::GpuDriven if capabilities.supports_gpu_driven() => Ok(RenderPath::GpuDriven),
            Self::GpuDriven => Err(EngineError::Unsupported(
                "GpuDriven requires compute, vertex storage buffers and indirect execution".into(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderPath {
    #[default]
    CpuInstanced,
    Indirect,
    GpuDriven,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct IndirectDrawIndexedArgs {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub base_vertex: i32,
    pub first_instance: u32,
}

/// Conservative clip-space sphere test used by both the CPU fallback and the
/// optional GPU-driven path. Bounds are expressed in local mesh space.
pub fn sphere_visible(view_projection: Mat4, transform: TransformState, bounds: [f32; 4]) -> bool {
    let center = transform_matrix(transform)
        * qst_core::glam::Vec4::new(bounds[0], bounds[1], bounds[2], 1.0);
    let radius = bounds[3].abs() * transform.scale.abs().max_element();
    let rows = view_projection.transpose();
    [
        rows.w_axis + rows.x_axis,
        rows.w_axis - rows.x_axis,
        rows.w_axis + rows.y_axis,
        rows.w_axis - rows.y_axis,
        rows.z_axis,
        rows.w_axis - rows.z_axis,
    ]
    .iter()
    .all(|plane| plane.dot(center) >= -radius * plane.truncate().length())
}

fn transform_matrix(transform: TransformState) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        transform.scale,
        transform.rotation,
        transform.translation,
    )
}

impl RenderSnapshot {
    pub fn batch_count(&self) -> usize {
        self.instances
            .iter()
            .map(|instance| {
                (
                    instance.mesh.id.index,
                    instance.mesh.id.generation,
                    instance.material.id.index,
                    instance.material.id.generation,
                )
            })
            .collect::<std::collections::HashSet<_>>()
            .len()
    }
}

pub trait RenderFeature: Send {
    fn extract(&mut self, _snapshot: &RenderSnapshot) {}
    fn prepare(&mut self, _device: &Device, _queue: &Queue) -> EngineResult<()> {
        Ok(())
    }
    fn queue(&mut self, _snapshot: &RenderSnapshot) {}
    fn compute<'a>(&'a self, _node: &str, _pass: &mut wgpu::ComputePass<'a>) {}
    fn render<'a>(&'a self, _pass: &mut wgpu::RenderPass<'a>) {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderExecution {
    Prepare,
    Compute,
    Render,
}

#[derive(Clone, Debug)]
pub struct RenderNode {
    pub name: String,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
    pub execution: RenderExecution,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledRenderGraph {
    pub order: Vec<String>,
    pub resources: Vec<String>,
    pub steps: Vec<(String, RenderExecution)>,
}

#[derive(Default)]
pub struct RenderGraph {
    pub nodes: Vec<RenderNode>,
}

impl RenderGraph {
    pub fn add(&mut self, node: RenderNode) {
        self.nodes.push(node);
    }

    pub fn add_pass(
        &mut self,
        name: impl Into<String>,
        reads: impl IntoIterator<Item = impl Into<String>>,
        writes: impl IntoIterator<Item = impl Into<String>>,
    ) {
        self.add_typed_pass(name, RenderExecution::Render, reads, writes);
    }

    pub fn add_typed_pass(
        &mut self,
        name: impl Into<String>,
        execution: RenderExecution,
        reads: impl IntoIterator<Item = impl Into<String>>,
        writes: impl IntoIterator<Item = impl Into<String>>,
    ) {
        self.add(RenderNode {
            name: name.into(),
            execution,
            reads: reads.into_iter().map(Into::into).collect(),
            writes: writes.into_iter().map(Into::into).collect(),
        });
    }

    /// Compiles resource dependencies into a deterministic execution order.
    /// A resource writer must precede every reader, while independent passes
    /// retain insertion order. Duplicate pass names and dependency cycles are
    /// rejected before the renderer touches GPU state.
    pub fn compile(&self) -> EngineResult<CompiledRenderGraph> {
        let mut names = std::collections::HashSet::new();
        for node in &self.nodes {
            if !names.insert(node.name.clone()) {
                return Err(EngineError::Runtime(format!(
                    "duplicate render graph node {}",
                    node.name
                )));
            }
        }
        let mut edges = vec![Vec::new(); self.nodes.len()];
        let mut indegree = vec![0usize; self.nodes.len()];
        let mut writers = HashMap::<String, Vec<usize>>::new();
        let mut resources = std::collections::BTreeSet::new();
        for node in &self.nodes {
            resources.extend(node.reads.iter().cloned());
            resources.extend(node.writes.iter().cloned());
        }
        for (index, node) in self.nodes.iter().enumerate() {
            for resource in &node.writes {
                writers.entry(resource.clone()).or_default().push(index);
            }
        }
        for (index, node) in self.nodes.iter().enumerate() {
            for resource in &node.reads {
                if let Some(producers) = writers.get(resource) {
                    for &producer in producers {
                        if producer != index && !edges[producer].contains(&index) {
                            edges[producer].push(index);
                            indegree[index] += 1;
                        }
                    }
                }
            }
            for resource in &node.writes {
                if let Some(producers) = writers.get(resource)
                    && let Some(&previous) = producers
                        .iter()
                        .take_while(|&&writer| writer < index)
                        .last()
                    && !edges[previous].contains(&index)
                {
                    edges[previous].push(index);
                    indegree[index] += 1;
                }
            }
        }
        let mut ready = (0..self.nodes.len())
            .filter(|&index| indegree[index] == 0)
            .collect::<Vec<_>>();
        let mut order = Vec::with_capacity(self.nodes.len());
        while let Some(index) = ready.first().copied() {
            ready.remove(0);
            order.push(self.nodes[index].name.clone());
            for &next in &edges[index] {
                indegree[next] -= 1;
                if indegree[next] == 0 {
                    ready.push(next);
                }
            }
        }
        if order.len() != self.nodes.len() {
            return Err(EngineError::Runtime(
                "render graph contains a dependency cycle".into(),
            ));
        }
        Ok(CompiledRenderGraph {
            steps: order
                .iter()
                .map(|name| {
                    (
                        name.clone(),
                        self.nodes
                            .iter()
                            .find(|node| &node.name == name)
                            .expect("compiled node")
                            .execution,
                    )
                })
                .collect(),
            order,
            resources: resources.into_iter().collect(),
        })
    }

    pub fn default_forward() -> Self {
        Self::for_path(RenderPath::GpuDriven)
    }

    pub fn for_path(path: RenderPath) -> Self {
        let mut graph = Self::default();
        graph.add_typed_pass(
            "depth",
            RenderExecution::Prepare,
            std::iter::empty::<&str>(),
            ["depth"],
        );
        if path == RenderPath::GpuDriven {
            graph.add_typed_pass(
                "gpu_cull",
                RenderExecution::Compute,
                ["depth"],
                ["visible_instances", "indirect_args"],
            );
        }
        graph.add_typed_pass(
            "skinning",
            RenderExecution::Prepare,
            ["depth", "visible_instances"],
            ["joint_matrices"],
        );
        graph.add_typed_pass(
            "forward",
            RenderExecution::Render,
            ["depth", "indirect_args", "joint_matrices"],
            ["color"],
        );
        graph
    }
}

#[cfg(test)]
mod graph_tests {
    use super::*;

    #[test]
    fn graph_compiles_dependencies_and_resources() {
        let graph = RenderGraph::default_forward();
        let compiled = graph.compile().unwrap();
        assert_eq!(compiled.order, ["depth", "gpu_cull", "skinning", "forward"]);
        assert!(compiled.resources.contains(&"indirect_args".to_owned()));
    }

    #[test]
    fn graph_rejects_cycles() {
        let mut graph = RenderGraph::default();
        graph.add_pass("a", ["b"], ["a"]);
        graph.add_pass("b", ["a"], ["b"]);
        assert!(graph.compile().is_err());
    }
}

mod forward;
mod renderer;

use forward::ForwardFeature;
pub use forward::GpuCullingConfig;
pub use forward::RenderStats;
pub use renderer::Renderer;
#[cfg(test)]
mod tests;
