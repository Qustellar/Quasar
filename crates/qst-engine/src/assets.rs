use super::*;

impl EngineApp {
    pub fn poll_asset_reloads(&mut self) {
        self.assets.poll_async();
        if !self.reload_pending {
            let changed = self.assets.poll_changed_paths();
            if !changed.is_empty() {
                if let Some(path) = self.gltf_path.as_ref().filter(|path| {
                    changed.iter().any(|changed| {
                        changed.parent() == path.parent()
                            && matches!(
                                changed
                                    .extension()
                                    .and_then(|ext| ext.to_str())
                                    .map(str::to_ascii_lowercase)
                                    .as_deref(),
                                Some(
                                    "gltf"
                                        | "glb"
                                        | "bin"
                                        | "png"
                                        | "jpg"
                                        | "jpeg"
                                        | "webp"
                                        | "ktx2"
                                )
                            )
                    })
                }) {
                    let path = path.clone();
                    let tx = self.reload_tx.clone();
                    self.reload_pending = true;
                    std::thread::spawn(move || {
                        let result = import_gltf_cached(&path).map(|(imported, _)| imported);
                        let _ = tx.send(ReloadResult::Gltf(path, result));
                    });
                } else if let Some(path) = self
                    .scene_path
                    .as_ref()
                    .filter(|path| changed.iter().any(|changed| changed == *path))
                {
                    let is_own_save =
                        self.saved_scene_contents
                            .as_ref()
                            .is_some_and(|(saved_path, bytes)| {
                                saved_path == path
                                    && std::fs::read(path).is_ok_and(|current| current == *bytes)
                            });
                    if !is_own_save {
                        let path = path.clone();
                        let tx = self.reload_tx.clone();
                        self.reload_pending = true;
                        std::thread::spawn(move || {
                            let result = SceneAsset::load_ron(&path);
                            let _ = tx.send(ReloadResult::Scene(path, result));
                        });
                    }
                }
            }
        }
        while let Ok(result) = self.reload_rx.try_recv() {
            self.reload_pending = false;
            match result {
                ReloadResult::Gltf(path, Ok(imported))
                    if self.gltf_path.as_ref() == Some(&path) =>
                {
                    if self.scene_path.is_some() {
                        if let Err(error) = self.validate_scene_assets(&self.scene, Some(&imported))
                        {
                            self.diagnostics.last_reload = Some(format!("Reload failed: {error}"));
                            continue;
                        }
                        self.clear_gltf_assets();
                        self.register_gltf_assets(imported.meshes, imported.textures);
                    } else {
                        let mut imported = imported;
                        imported.scene.source_gltf = Some(path.clone());
                        if let Err(error) = self.apply_gltf_import(imported) {
                            self.diagnostics.last_reload = Some(format!("Reload failed: {error}"));
                            continue;
                        }
                    }
                    self.gltf_path = Some(path);
                    self.diagnostics.last_reload = Some("glTF reloaded".into());
                }
                ReloadResult::Scene(path, Ok(scene)) if self.scene_path.as_ref() == Some(&path) => {
                    let _ = scene;
                    match self.load_scene(&path) {
                        Ok(_) => self.diagnostics.last_reload = Some("Scene reloaded".into()),
                        Err(error) => {
                            self.diagnostics.last_reload = Some(format!("Reload failed: {error}"))
                        }
                    }
                }
                ReloadResult::Gltf(_, Err(error)) | ReloadResult::Scene(_, Err(error)) => {
                    self.diagnostics.last_reload = Some(format!("Reload failed: {error}"));
                    tracing::warn!(%error, "asset reload failed");
                }
                _ => {}
            }
        }
    }
}
