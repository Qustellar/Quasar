use super::*;

pub struct Renderer {
    pub device: Device,
    pub queue: Queue,
    pub surface: Surface<'static>,
    pub config: SurfaceConfiguration,
    pub features: Vec<Box<dyn RenderFeature>>,
    pub render_graph: RenderGraph,
    compiled_graph: CompiledRenderGraph,
    depth_view: wgpu::TextureView,
    forward: ForwardFeature,
    capabilities: RenderCapabilities,
}

impl Renderer {
    pub async fn new(window: Arc<winit::window::Window>) -> EngineResult<Self> {
        Self::new_with_preference(window, RenderPreference::Auto).await
    }

    pub async fn new_with_preference(
        window: Arc<winit::window::Window>,
        preference: RenderPreference,
    ) -> EngineResult<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| EngineError::Runtime(e.to_string()))?;
        tracing::info!("wgpu surface created");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| EngineError::Runtime(e.to_string()))?;
        tracing::info!(adapter = %adapter.get_info().name, backend = ?adapter.get_info().backend, "wgpu adapter selected");
        let capabilities = RenderCapabilities::from_adapter(&adapter);
        preference.select(capabilities)?;
        if !capabilities.storage_buffers || !capabilities.indirect_draw {
            return Err(EngineError::Unsupported(
                "renderer requires vertex storage buffers and indirect execution".into(),
            ));
        }
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|e| EngineError::Runtime(e.to_string()))?;
        tracing::info!("wgpu device ready");
        let size = window.inner_size();
        let surface_capabilities = surface.get_capabilities(&adapter);
        let format = surface_capabilities
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_capabilities.formats[0]);
        let config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| EngineError::Runtime("surface has no default configuration".into()))?;
        let config = SurfaceConfiguration { format, ..config };
        surface.configure(&device, &config);
        tracing::info!(
            width = config.width,
            height = config.height,
            "wgpu surface configured"
        );
        let depth_view = depth_view(&device, config.width, config.height);
        let forward = ForwardFeature::new(
            &device,
            format,
            [config.width, config.height],
            preference,
            capabilities,
        );
        let render_graph = RenderGraph::for_path(preference.select(capabilities)?);
        let compiled_graph = render_graph.compile()?;
        tracing::info!("forward pipeline ready");
        Ok(Self {
            device,
            queue,
            surface,
            config,
            features: Vec::new(),
            render_graph,
            compiled_graph,
            depth_view,
            forward,
            capabilities,
        })
    }

    pub fn capabilities(&self) -> RenderCapabilities {
        self.capabilities
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.depth_view = depth_view(&self.device, width, height);
        self.forward.set_viewport(width, height);
    }

    pub fn upload_mesh(&mut self, handle: Handle<MeshAsset>, mesh: &MeshAsset) {
        self.forward.upload_mesh(&self.device, handle, mesh);
    }

    pub fn upload_textured_mesh(&mut self, handle: Handle<MeshAsset>, mesh: &TexturedMeshAsset) {
        self.forward
            .upload_textured_mesh(&self.device, handle, mesh);
    }

    pub fn upload_texture(&mut self, handle: Handle<TextureAsset>, texture: &TextureAsset) {
        self.forward
            .upload_texture(&self.device, &self.queue, handle, texture);
    }

    pub fn upload_material(&mut self, handle: Handle<MaterialAsset>, material: MaterialAsset) {
        self.forward.upload_material(handle, material);
        self.forward
            .upload_pbr_material(&self.device, handle, material.into());
    }

    pub fn upload_pbr_material(&mut self, handle: Handle<MaterialAsset>, material: PbrMaterial) {
        self.forward
            .upload_pbr_material(&self.device, handle, material);
    }

    pub fn gpu_resource_count(&self) -> usize {
        self.forward.gpu_resource_count()
    }

    pub fn render_stats(&self) -> RenderStats {
        self.forward.stats()
    }

    pub fn render_path(&self) -> RenderPath {
        self.forward.stats().path
    }

    pub fn compiled_render_graph(&self) -> &CompiledRenderGraph {
        &self.compiled_graph
    }

    pub fn rebuild_render_graph(&mut self) -> EngineResult<()> {
        self.compiled_graph = self.render_graph.compile()?;
        Ok(())
    }

    pub fn remove_mesh(&mut self, handle: Handle<MeshAsset>) {
        self.forward.remove_mesh(handle);
    }

    pub fn remove_material(&mut self, handle: Handle<MaterialAsset>) {
        self.forward.remove_material(handle);
    }

    pub fn remove_texture(&mut self, handle: Handle<TextureAsset>) {
        self.forward.remove_texture(handle);
    }

    pub fn render(&mut self, snapshot: &RenderSnapshot) -> EngineResult<()> {
        self.render_with_overlay(snapshot, |_, _, _, _| Ok(()))
    }

    pub fn render_with_overlay(
        &mut self,
        snapshot: &RenderSnapshot,
        overlay: impl FnOnce(
            &Device,
            &Queue,
            &mut wgpu::CommandEncoder,
            &wgpu::TextureView,
        ) -> EngineResult<()>,
    ) -> EngineResult<()> {
        self.forward.extract(snapshot);
        self.forward.prepare(&self.device, &self.queue)?;
        self.forward.queue(snapshot);
        for feature in &mut self.features {
            feature.extract(snapshot);
            feature.prepare(&self.device, &self.queue)?;
            feature.queue(snapshot);
        }
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                self.surface.get_current_texture().map_err(surface_error)?
            }
            Err(error) => return Err(surface_error(error)),
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("quasar-frame"),
            });
        encode_graph(
            &mut encoder,
            &self.compiled_graph,
            &self.forward,
            &self.features,
            RenderTargets {
                color: &view,
                depth: &self.depth_view,
                clear: wgpu::Color {
                    r: 0.08,
                    g: 0.1,
                    b: 0.12,
                    a: 1.0,
                },
            },
        );
        overlay(&self.device, &self.queue, &mut encoder, &view)?;
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}

pub(crate) struct RenderTargets<'a> {
    pub color: &'a wgpu::TextureView,
    pub depth: &'a wgpu::TextureView,
    pub clear: wgpu::Color,
}

/// Shared by surface rendering and headless GPU validation.
pub(crate) fn encode_graph(
    encoder: &mut wgpu::CommandEncoder,
    graph: &CompiledRenderGraph,
    forward: &ForwardFeature,
    features: &[Box<dyn RenderFeature>],
    targets: RenderTargets<'_>,
) {
    for (node, execution) in &graph.steps {
        let _node_span = tracing::debug_span!("render_graph_pass", pass = %node).entered();
        if *execution == RenderExecution::Prepare {
            encoder.insert_debug_marker(node);
        }
        if *execution == RenderExecution::Compute {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(node),
                timestamp_writes: None,
            });
            forward.compute(node, &mut pass);
            for feature in features {
                feature.compute(node, &mut pass);
            }
        }
        if *execution == RenderExecution::Render {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(node),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: targets.color,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(targets.clear),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            forward.render(&mut pass);
            for feature in features {
                feature.render(&mut pass);
            }
        }
    }
}

fn surface_error(error: wgpu::SurfaceError) -> EngineError {
    match error {
        wgpu::SurfaceError::OutOfMemory => EngineError::OutOfMemory,
        other => EngineError::Runtime(format!("surface acquisition failed: {other}")),
    }
}

pub(crate) fn depth_view(device: &Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}
