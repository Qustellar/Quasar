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

#[derive(Clone, Copy, Debug)]
pub struct MaterialAsset {
    pub color: [f32; 4],
}

impl MaterialAsset {
    pub const fn from_color(color: [f32; 4]) -> Self {
        Self { color }
    }
    pub const fn base_color(&self) -> [f32; 4] {
        self.color
    }
    pub const fn metallic(&self) -> f32 {
        0.0
    }
    pub const fn roughness(&self) -> f32 {
        0.5
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
pub use forward::RenderStats;
pub use renderer::Renderer;
#[cfg(test)]
use renderer::depth_view;
#[cfg(test)]
mod tests;
