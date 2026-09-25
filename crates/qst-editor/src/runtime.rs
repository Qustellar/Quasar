use super::*;

pub struct EditorRuntime {
    pub plugin: EditorPlugin,
    context: egui::Context,
    input: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    paint_jobs: Vec<egui::ClippedPrimitive>,
    textures_delta: egui::TexturesDelta,
    screen: egui_wgpu::ScreenDescriptor,
}

impl EditorRuntime {
    pub fn new(window: &Window, device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let context = egui::Context::default();
        let input = egui_winit::State::new(
            context.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        Self {
            plugin: EditorPlugin::default(),
            context,
            input,
            renderer: egui_wgpu::Renderer::new(
                device,
                format,
                egui_wgpu::RendererOptions::default(),
            ),
            paint_jobs: Vec::new(),
            textures_delta: Default::default(),
            screen: egui_wgpu::ScreenDescriptor {
                size_in_pixels: [1, 1],
                pixels_per_point: 1.0,
            },
        }
    }

    pub fn on_window_event(&mut self, window: &Window, event: &WindowEvent) {
        let _ = self.input.on_window_event(window, event);
    }

    pub fn prepare(
        &mut self,
        window: &Window,
        scene: &mut SceneAsset,
        runtime_positions: &[Option<[f32; 3]>],
        diagnostics: &FrameDiagnostics,
    ) -> Option<EntityId> {
        let raw_input = self.input.take_egui_input(window);
        let mut changed = None;
        let output = self.context.run(raw_input, |context| {
            changed = self
                .plugin
                .draw(context, scene, runtime_positions, diagnostics);
        });
        self.input
            .handle_platform_output(window, output.platform_output);
        self.paint_jobs = self
            .context
            .tessellate(output.shapes, output.pixels_per_point);
        self.textures_delta.append(output.textures_delta);
        let size = window.inner_size();
        self.screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [size.width, size.height],
            pixels_per_point: output.pixels_per_point,
        };
        changed
    }

    pub fn paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    ) {
        for (id, delta) in &self.textures_delta.set {
            self.renderer.update_texture(device, queue, *id, delta);
        }
        let commands =
            self.renderer
                .update_buffers(device, queue, encoder, &self.paint_jobs, &self.screen);
        if !commands.is_empty() {
            queue.submit(commands);
        }
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("quasar-editor"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        self.renderer
            .render(&mut pass.forget_lifetime(), &self.paint_jobs, &self.screen);
        for id in &self.textures_delta.free {
            self.renderer.free_texture(id);
        }
        self.textures_delta = Default::default();
    }
}
