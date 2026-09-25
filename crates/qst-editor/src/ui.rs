use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayState {
    Playing,
    Paused,
}

pub struct EditorPlugin {
    pub play_state: PlayState,
    pub selected_entity: Option<EntityId>,
    pub step_requested: bool,
    pub save_requested: bool,
}

impl Default for EditorPlugin {
    fn default() -> Self {
        Self {
            play_state: PlayState::Playing,
            selected_entity: None,
            step_requested: false,
            save_requested: false,
        }
    }
}

impl EditorPlugin {
    pub fn draw(
        &mut self,
        context: &egui::Context,
        scene: &mut SceneAsset,
        runtime_positions: &[Option<[f32; 3]>],
        diagnostics: &FrameDiagnostics,
    ) -> Option<EntityId> {
        let mut changed = None;
        let order = scene
            .hierarchy_order()
            .unwrap_or_else(|_| (0..scene.entities.len()).collect());
        let mut depths = vec![0; scene.entities.len()];
        egui::SidePanel::left("quasar-scene-tree")
            .resizable(true)
            .show(context, |ui| {
                ui.heading("Scene");
                for &index in &order {
                    let entity = &scene.entities[index];
                    depths[index] = entity
                        .parent
                        .and_then(|parent| scene.index_of(parent))
                        .and_then(|parent| depths.get(parent).copied())
                        .map_or(0, |depth| depth + 1);
                    ui.horizontal(|ui| {
                        ui.add_space(depths[index] as f32 * 14.0);
                        if ui
                            .selectable_label(self.selected_entity == Some(entity.id), &entity.name)
                            .clicked()
                        {
                            self.selected_entity = Some(entity.id);
                        }
                    });
                }
            });
        egui::SidePanel::right("quasar-inspector")
            .resizable(true)
            .show(context, |ui| {
                ui.heading("Inspector");
                if let Some((index, entity)) = self
                    .selected_entity
                    .and_then(|id| scene.index_of(id))
                    .and_then(|index| scene.entities.get_mut(index).map(|entity| (index, entity)))
                {
                    let mut edited = ui.text_edit_singleline(&mut entity.name).changed();
                    let mut p = entity.transform.translation.to_array();
                    ui.label("Start position");
                    ui.horizontal(|ui| {
                        for value in &mut p {
                            edited |= ui
                                .add_sized([56.0, 20.0], egui::DragValue::new(value).speed(0.1))
                                .changed();
                        }
                    });
                    entity.transform.translation = p.into();
                    let (x, y, z) = entity.transform.rotation.to_euler(glam::EulerRot::XYZ);
                    let mut degrees = [x.to_degrees(), y.to_degrees(), z.to_degrees()];
                    let mut rotated = false;
                    ui.label("Rotation (deg)");
                    ui.horizontal(|ui| {
                        for value in &mut degrees {
                            rotated |= ui
                                .add_sized([56.0, 20.0], egui::DragValue::new(value).speed(0.5))
                                .changed();
                        }
                    });
                    if rotated {
                        edited = true;
                        entity.transform.rotation = glam::Quat::from_euler(
                            glam::EulerRot::XYZ,
                            degrees[0].to_radians(),
                            degrees[1].to_radians(),
                            degrees[2].to_radians(),
                        );
                    }
                    let mut scale = entity.transform.scale.to_array();
                    ui.label("Scale");
                    ui.horizontal(|ui| {
                        for value in &mut scale {
                            edited |= ui
                                .add_sized(
                                    [56.0, 20.0],
                                    egui::DragValue::new(value).range(0.01..=1000.0).speed(0.05),
                                )
                                .changed();
                        }
                    });
                    entity.transform.scale = scale.into();
                    if let Some(Some(position)) = runtime_positions.get(index) {
                        ui.label(format!(
                            "Live position {:.2}, {:.2}, {:.2}",
                            position[0], position[1], position[2]
                        ));
                    }
                    if let Some(camera) = &mut entity.camera {
                        edited |= ui
                            .add(
                                egui::DragValue::new(&mut camera.fov_y_radians)
                                    .range(0.1..=3.0)
                                    .speed(0.01)
                                    .prefix("FOV "),
                            )
                            .changed();
                    }
                    if let Some(light) = &mut entity.light {
                        edited |= ui
                            .add(
                                egui::DragValue::new(&mut light.intensity)
                                    .range(0.0..=100.0)
                                    .speed(0.05)
                                    .prefix("Light "),
                            )
                            .changed();
                    }
                    if let Some(collider) = &mut entity.collider {
                        for (axis, value) in collider.half_extents.iter_mut().enumerate() {
                            edited |= ui
                                .add(
                                    egui::DragValue::new(value)
                                        .range(0.01..=1000.0)
                                        .speed(0.05)
                                        .prefix(format!("Collider {} ", axis)),
                                )
                                .changed();
                        }
                    }
                    if edited {
                        changed = Some(entity.id);
                    }
                }
                ui.separator();
                ui.label(format!(
                    "Frame {:.2} ms",
                    diagnostics.frame_seconds * 1000.0
                ));
                ui.label(format!(
                    "Fixed {:.2} ms",
                    diagnostics.fixed_update_seconds * 1000.0
                ));
                ui.label(format!(
                    "Render {:.2} ms",
                    diagnostics.render_seconds * 1000.0
                ));
                if let Some(bytes) = diagnostics.resident_working_set_bytes {
                    ui.label(format!("Working set {:.1} MiB", bytes as f64 / 1_048_576.0));
                }
                if let Some(bytes) = diagnostics.private_working_set_bytes {
                    ui.label(format!("Private {:.1} MiB", bytes as f64 / 1_048_576.0));
                }
                ui.label(format!(
                    "Assets {}  GPU {}",
                    diagnostics.loaded_asset_count, diagnostics.gpu_resource_count
                ));
                if let Some(status) = &diagnostics.last_reload {
                    ui.label(status);
                }
            });
        egui::TopBottomPanel::top("quasar-controls").show(context, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .button(if self.play_state == PlayState::Playing {
                        "Pause"
                    } else {
                        "Play"
                    })
                    .clicked()
                {
                    self.play_state = if self.play_state == PlayState::Playing {
                        PlayState::Paused
                    } else {
                        PlayState::Playing
                    };
                }
                if ui
                    .add_enabled(
                        self.play_state == PlayState::Paused,
                        egui::Button::new("Step"),
                    )
                    .clicked()
                {
                    self.step_requested = true;
                }
                if ui.button("Save").clicked() {
                    self.save_requested = true;
                }
            });
        });
        changed
    }
}
