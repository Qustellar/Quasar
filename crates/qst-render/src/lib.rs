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

#[derive(Clone, Debug)]
pub struct MeshAsset {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Copy, Debug)]
pub struct MaterialAsset {
    pub color: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
pub struct RenderInstance {
    pub entity: u64,
    pub transform: TransformState,
    pub mesh: Handle<MeshAsset>,
    pub material: Handle<MaterialAsset>,
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
    pub camera: Option<RenderCamera>,
    pub lights: Vec<RenderLight>,
}

pub trait RenderFeature: Send {
    fn extract(&mut self, _snapshot: &RenderSnapshot) {}
    fn prepare(&mut self, _device: &Device, _queue: &Queue) -> EngineResult<()> {
        Ok(())
    }
    fn queue(&mut self, _snapshot: &RenderSnapshot) {}
    fn render<'a>(&'a self, _pass: &mut wgpu::RenderPass<'a>) {}
}

#[derive(Clone, Debug)]
pub struct RenderNode {
    pub name: String,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
}

#[derive(Default)]
pub struct RenderGraph {
    pub nodes: Vec<RenderNode>,
}

impl RenderGraph {
    pub fn add(&mut self, node: RenderNode) {
        self.nodes.push(node);
    }
}

mod forward;
mod renderer;

use forward::ForwardFeature;
pub use renderer::Renderer;
#[cfg(test)]
use renderer::depth_view;
#[cfg(test)]
mod tests;
