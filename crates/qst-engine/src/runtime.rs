use super::*;

impl EngineApp {
    pub fn run(mut self) -> EngineResult<()> {
        #[cfg(feature = "tracy")]
        {
            use tracing_subscriber::prelude::*;
            let _ = tracing_subscriber::registry()
                .with(tracing_subscriber::fmt::layer())
                .with(tracing_tracy::TracyLayer::default())
                .try_init();
        }
        #[cfg(not(feature = "tracy"))]
        let _ = tracing_subscriber::fmt::try_init();
        let event_loop = EventLoop::new().map_err(|e| EngineError::Runtime(e.to_string()))?;
        event_loop
            .run_app(&mut self)
            .map_err(|e| EngineError::Runtime(e.to_string()))?;
        self.fatal_error.take().map_or(Ok(()), Err)
    }
}

impl ApplicationHandler for EngineApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let mut attributes = WindowAttributes::default()
            .with_title("Quasar Engine")
            .with_inner_size(PhysicalSize::new(self.window_size.0, self.window_size.1));
        if self.benchmark.is_some() {
            attributes = attributes.with_decorations(false).with_resizable(false);
        }
        match event_loop.create_window(attributes) {
            Ok(window) => {
                let window = Arc::new(window);
                if let Some(mut input) = self.world.get_resource_mut::<InputState>() {
                    input.focused = true;
                }
                self.startup_schedule.run(&mut self.world);
                match pollster::block_on(Renderer::new(window.clone())) {
                    Ok(mut renderer) => {
                        renderer.features.append(&mut self.pending_render_features);
                        for (&handle, mesh) in self.mesh_names.values().filter_map(|handle| {
                            self.mesh_assets.get(*handle).map(|mesh| (handle, mesh))
                        }) {
                            renderer.upload_mesh(handle, &mesh);
                        }
                        for &handle in self.material_names.values() {
                            if let Some(material) = self.material_assets.get(handle) {
                                renderer.upload_material(handle, *material);
                            }
                        }
                        #[cfg(feature = "editor")]
                        {
                            self.editor = Some(EditorRuntime::new(
                                &window,
                                &renderer.device,
                                renderer.config.format,
                            ));
                        }
                        tracing::info!(
                            width = renderer.config.width,
                            height = renderer.config.height,
                            "renderer ready"
                        );
                        self.renderer = Some(renderer);
                        window.request_redraw();
                        self.window = Some(window);
                    }
                    Err(error) => {
                        tracing::error!(%error, "renderer initialization failed");
                        self.fatal_error = Some(error);
                        event_loop.exit();
                    }
                }
            }
            Err(error) => {
                tracing::error!(%error, "window creation failed");
                self.fatal_error = Some(EngineError::Runtime(error.to_string()));
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        #[cfg(feature = "editor")]
        if let (Some(editor), Some(window)) = (&mut self.editor, &self.window) {
            editor.on_window_event(window, &event);
        }
        if let Some(mut input) = self.world.get_resource_mut::<InputState>() {
            input.handle_window_event(&event);
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                let _span = tracing::info_span!("frame").entered();
                self.poll_asset_reloads();
                let input_start = Instant::now();
                if let Some(mut input) = self.world.get_resource_mut::<InputState>() {
                    input.clear_transient();
                }
                self.pre_update_schedule.run(&mut self.world);
                self.diagnostics.input_update_seconds = input_start.elapsed().as_secs_f32();
                let now = Instant::now();
                let delta = now.duration_since(self.last_frame);
                self.last_frame = now;
                #[cfg(feature = "editor")]
                let paused = self
                    .editor
                    .as_ref()
                    .is_some_and(|editor| editor.plugin.play_state == PlayState::Paused);
                #[cfg(not(feature = "editor"))]
                let paused = false;
                #[cfg(feature = "editor")]
                let step_once = self
                    .editor
                    .as_mut()
                    .is_some_and(|editor| std::mem::take(&mut editor.plugin.step_requested));
                #[cfg(not(feature = "editor"))]
                let step_once = false;
                if !paused {
                    self.update_animations(delta.as_secs_f32());
                    self.variable_schedule.run(&mut self.world);
                    self.update_fixed(delta);
                } else if step_once {
                    self.update_fixed(std::time::Duration::from_secs_f32(
                        self.fixed_time.step_seconds,
                    ));
                }
                #[cfg(feature = "editor")]
                let runtime_positions: Vec<_> = self
                    .scene_entities
                    .iter()
                    .map(|&entity| {
                        self.world
                            .get::<Transform>(entity)
                            .map(|transform| transform.current.translation.to_array())
                    })
                    .collect();
                #[cfg(feature = "editor")]
                if let (Some(editor), Some(window)) = (&mut self.editor, &self.window)
                    && let Some(entity_id) = editor.prepare(
                        window,
                        &mut self.scene,
                        &runtime_positions,
                        &self.diagnostics,
                    )
                {
                    self.apply_editor_change(entity_id);
                }
                #[cfg(feature = "editor")]
                if self
                    .editor
                    .as_mut()
                    .is_some_and(|editor| std::mem::take(&mut editor.plugin.save_requested))
                {
                    let path = self
                        .scene_path
                        .clone()
                        .unwrap_or_else(|| PathBuf::from("quasar-scene.ron"));
                    if let Err(error) = self.save_scene(path) {
                        tracing::warn!(%error, "scene save failed");
                        self.diagnostics.last_reload = Some(format!("Save failed: {error}"));
                    } else {
                        self.diagnostics.last_reload = Some("Scene saved".into());
                    }
                }
                let snapshot = self.render_snapshot();
                self.render_extract_schedule.run(&mut self.world);
                self.render_schedule.run(&mut self.world);
                let start = Instant::now();
                let _render_span = tracing::info_span!("render").entered();
                if let Some(renderer) = &mut self.renderer {
                    #[cfg(feature = "editor")]
                    let result = if let Some(editor) = &mut self.editor {
                        renderer.render_with_overlay(&snapshot, |device, queue, encoder, view| {
                            editor.paint(device, queue, encoder, view);
                            Ok(())
                        })
                    } else {
                        renderer.render(&snapshot)
                    };
                    #[cfg(not(feature = "editor"))]
                    let result = renderer.render(&snapshot);
                    if let Err(error) = result {
                        tracing::warn!(%error, "render failed");
                        if matches!(error, EngineError::OutOfMemory) {
                            self.fatal_error = Some(error);
                            event_loop.exit();
                            return;
                        }
                    }
                    self.diagnostics.gpu_resource_count = renderer.gpu_resource_count();
                    let stats = renderer.render_stats();
                    self.diagnostics.render_instances = stats.instances;
                    self.diagnostics.render_batches = stats.batches;
                    self.diagnostics.render_draw_calls = stats.draw_calls;
                    self.diagnostics.instance_upload_bytes = stats.instance_upload_bytes;
                    self.diagnostics.render_fallback = stats.fallback;
                    self.diagnostics.skipped_instances = stats.skipped_instances;
                }
                self.diagnostics.loaded_asset_count = self.assets.records().count();
                self.diagnostics.render_seconds = start.elapsed().as_secs_f32();
                self.diagnostics.frame_seconds = delta.as_secs_f32();
                let memory = current_process_memory();
                self.diagnostics.resident_working_set_bytes =
                    memory.as_ref().map(|value| value.working_set_bytes);
                self.diagnostics.private_working_set_bytes = memory
                    .as_ref()
                    .and_then(|value| value.private_working_set_bytes);
                self.diagnostics.private_commit_bytes =
                    memory.as_ref().and_then(|value| value.private_commit_bytes);
                self.frame_index += 1;
                if self.frame_index.is_multiple_of(120) {
                    tracing::info!(
                        frame_ms = self.diagnostics.frame_seconds * 1000.0,
                        fixed_ms = self.diagnostics.fixed_update_seconds * 1000.0,
                        render_ms = self.diagnostics.render_seconds * 1000.0,
                        working_set_bytes = ?self.diagnostics.resident_working_set_bytes,
                        assets = self.diagnostics.loaded_asset_count,
                        gpu_resources = self.diagnostics.gpu_resource_count,
                        "frame diagnostics"
                    );
                }
                if let Some(benchmark) = self.benchmark.as_mut()
                    && benchmark.record(
                        &self.diagnostics,
                        self.renderer
                            .as_ref()
                            .map(|renderer| (renderer.config.width, renderer.config.height))
                            .unwrap_or(self.window_size),
                        self.window_size,
                    )
                {
                    let size = self
                        .renderer
                        .as_ref()
                        .map(|renderer| (renderer.config.width, renderer.config.height))
                        .unwrap_or(self.window_size);
                    benchmark.print_summary(size.0, size.1);
                    event_loop.exit();
                    return;
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}
