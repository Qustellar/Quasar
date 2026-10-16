use super::*;

#[test]
fn animated_gltf_survives_save_load_and_produces_interpolated_palettes() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tests/animated-skin.gltf");
    let mut app = EngineApp::new();
    app.import_gltf(&fixture).unwrap();
    assert_eq!(app.textured_meshes.len(), 2);
    assert_eq!(app.scene.skins[0].joints, [EntityId::new(3)]);
    app.fixed_time.step_seconds = 0.1;
    app.update_fixed(std::time::Duration::from_millis(150));
    let snapshot = app.render_snapshot();
    assert_eq!(snapshot.instances.len(), 2);
    assert_eq!(
        snapshot.skins.len(),
        1,
        "primitive siblings share a palette"
    );
    let skin = &snapshot.skins[snapshot.instances[0].skin.unwrap() as usize];
    // At t=0.1 the joint has moved 0.2; interpolation alpha=0.5 gives 0.1.
    assert!((skin.matrices[0].w_axis.x - 0.1).abs() < 1e-5);
    let reference = qst_render::skin_vertices(
        &[qst_render::SkinnedVertex {
            position: [-2.0, 0.0, 0.0],
            joints: [0; 4],
            weights: [1.0, 0.0, 0.0, 0.0],
        }],
        &skin.matrices,
    );
    assert!((reference[0][0] + 1.9).abs() < 1e-5);
    let cloned = snapshot.clone();
    assert!(Arc::ptr_eq(
        &cloned.skins[0].matrices,
        &snapshot.skins[0].matrices
    ));

    let path =
        std::env::temp_dir().join(format!("quasar-animated-save-{}.ron", std::process::id()));
    app.save_scene(&path).unwrap();
    let mut restored = EngineApp::new();
    restored.load_scene(&path).unwrap();
    assert!(restored.animation_clips.contains_key("MoveJoint"));
    restored.fixed_time.step_seconds = 0.1;
    restored.update_fixed(std::time::Duration::from_millis(150));
    assert!((restored.render_snapshot().skins[0].matrices[0].w_axis.x - 0.1).abs() < 1e-5);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn failed_skin_reload_preserves_previous_scene_and_resources() {
    let mut app = EngineApp::new();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tests/animated-skin.gltf");
    app.import_gltf(&fixture).unwrap();
    let mut invalid = qst_scene::import_gltf(&fixture).unwrap();
    invalid.scene.skins[0].joints[0] = EntityId::new(999);
    let previous = app.mesh_names.clone();
    assert!(app.apply_gltf_import(invalid).is_err());
    assert_eq!(app.mesh_names, previous);
    assert_eq!(app.render_snapshot().instances.len(), 2);
    assert_eq!(app.render_snapshot().skins.len(), 1);
}

#[test]
fn palette_cancels_instance_world_and_reports_singular_fallback() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tests/animated-skin.gltf");
    let mut app = EngineApp::new();
    app.import_gltf(&fixture).unwrap();
    let root = app.scene_entities[app.scene.index_of(EntityId::new(1)).unwrap()];
    let mesh = app.scene_entities[app.scene.index_of(EntityId::new(2)).unwrap()];
    app.world
        .get_mut::<LocalTransform>(root)
        .unwrap()
        .0
        .translation
        .x = 5.0;
    app.world
        .get_mut::<LocalTransform>(mesh)
        .unwrap()
        .0
        .translation
        .x = 2.0;
    app.propagate_scene_hierarchy(true);
    let snapshot = app.render_snapshot();
    assert!((snapshot.skins[0].matrices[0].w_axis.x + 2.0).abs() < 1e-5);
    assert_eq!(snapshot.instances[0].transform.translation.x, 7.0);
    for (index, source) in app
        .scene
        .entities
        .iter()
        .enumerate()
        .filter(|(_, source)| source.mesh.is_some())
    {
        let entity = app.scene_entities[index];
        let state = TransformState {
            scale: glam::Vec3::ZERO,
            ..source.local_transform
        };
        app.world.get_mut::<WorldTransform>(entity).unwrap().0 = state;
        app.world
            .get_mut::<PreviousWorldTransform>(entity)
            .unwrap()
            .0 = state;
    }
    let snapshot = app.render_snapshot();
    assert_eq!(snapshot.skin_fallbacks, 2);
    assert!(
        snapshot
            .instances
            .iter()
            .all(|instance| instance.skin.is_none())
    );
}

fn hierarchy_scene() -> SceneAsset {
    let mut scene = SceneAsset::new("nested boxes");
    scene.entities = vec![
        SceneEntity {
            id: EntityId::new(1),
            name: "Parent".into(),
            transform: TransformState {
                translation: glam::Vec3::new(1.0, 0.0, 0.0),
                ..TransformState::identity()
            },
            ..SceneEntity::default()
        },
        SceneEntity {
            id: EntityId::new(2),
            name: "Body".into(),
            parent: Some(EntityId::new(1)),
            transform: TransformState {
                translation: glam::Vec3::new(3.0, 3.0, 0.0),
                ..TransformState::identity()
            },
            collider: Some(BoxCollider {
                half_extents: [0.5; 3],
                dynamic: true,
            }),
            ..SceneEntity::default()
        },
        SceneEntity {
            id: EntityId::new(3),
            name: "Child".into(),
            parent: Some(EntityId::new(2)),
            transform: TransformState {
                translation: glam::Vec3::new(4.0, 3.0, 0.0),
                ..TransformState::identity()
            },
            ..SceneEntity::default()
        },
    ];
    scene
}

#[test]
fn fixed_parent_motion_reaches_physics_body_and_grandchild() {
    let mut app = EngineApp::new();
    app.set_scene(hierarchy_scene()).unwrap();
    let root = app.scene_entities[0];
    app.add_fixed_systems(move |mut transforms: qst_ecs::Query<&mut Transform>| {
        transforms.get_mut(root).unwrap().current.translation.x += 1.0;
    });
    app.update_fixed(std::time::Duration::from_millis(17));
    let body = app.world.get::<Transform>(app.scene_entities[1]).unwrap();
    let child = app.world.get::<Transform>(app.scene_entities[2]).unwrap();
    assert!((body.current.translation.x - 5.0).abs() < 1e-4);
    assert!((child.current.translation.x - 9.0).abs() < 1e-4);
    assert!(child.current.translation.y > 5.0);
    assert_eq!(body.previous.translation.x, 4.0);
    assert_eq!(child.previous.translation.x, 8.0);
}

#[test]
fn fixed_parent_rotation_moves_descendants_and_rotates_physics_body() {
    let mut app = EngineApp::new();
    app.set_scene(hierarchy_scene()).unwrap();
    let root = app.scene_entities[0];
    app.add_fixed_systems(move |mut transforms: qst_ecs::Query<&mut Transform>| {
        transforms.get_mut(root).unwrap().current.rotation =
            glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    });
    app.update_fixed(std::time::Duration::from_millis(17));
    let body = app.world.get::<Transform>(app.scene_entities[1]).unwrap();
    let child = app.world.get::<Transform>(app.scene_entities[2]).unwrap();
    assert!((body.current.translation.x - 1.0).abs() < 1e-4);
    assert!((body.current.translation.z + 3.0).abs() < 1e-4);
    assert!((child.current.translation.z + 7.0).abs() < 1e-4);
    assert!(
        (body
            .current
            .rotation
            .dot(glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)))
        .abs()
            > 0.999
    );
}

#[cfg(feature = "editor")]
#[test]
fn editor_parent_rotation_updates_camera_and_light_snapshot() {
    let mut scene = SceneAsset::new("camera rig");
    scene.entities = vec![
        SceneEntity {
            id: EntityId::new(1),
            ..SceneEntity::default()
        },
        SceneEntity {
            id: EntityId::new(2),
            name: "Camera".into(),
            parent: Some(EntityId::new(1)),
            transform: TransformState {
                translation: glam::Vec3::new(0.0, 0.0, 3.0),
                ..TransformState::identity()
            },
            camera: Some(Camera {
                fov_y_radians: 1.0,
                near: 0.1,
                far: 100.0,
            }),
            ..SceneEntity::default()
        },
        SceneEntity {
            id: EntityId::new(3),
            name: "Light".into(),
            parent: Some(EntityId::new(1)),
            light: Some(DirectionalLight {
                direction: [0.0, 0.0, -1.0],
                ..DirectionalLight::default()
            }),
            ..SceneEntity::default()
        },
    ];
    let mut app = EngineApp::new();
    app.set_scene(scene).unwrap();
    app.scene.entities[0].transform.rotation =
        glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    app.apply_editor_change(EntityId::new(1));
    let snapshot = app.render_snapshot();
    let camera = snapshot.camera.unwrap();
    assert!((camera.transform.translation.x - 3.0).abs() < 1e-4);
    assert!(camera.transform.translation.z.abs() < 1e-4);
    assert!((snapshot.lights[0].direction[0] + 1.0).abs() < 1e-4);
    assert!((app.scene.entities[1].transform.translation.z - 3.0).abs() < 1e-4);
}

#[cfg(feature = "editor")]
#[test]
fn editor_parent_move_updates_saved_children_without_resetting_other_edits() {
    let mut app = EngineApp::new();
    app.set_scene(hierarchy_scene()).unwrap();
    app.scene.entities[0].transform.translation.x += 2.0;
    app.apply_editor_change(EntityId::new(1));
    assert_eq!(app.scene.entities[1].transform.translation.x, 3.0);
    assert_eq!(app.scene.entities[2].transform.translation.x, 4.0);
    assert_eq!(
        app.world
            .get::<Transform>(app.scene_entities[1])
            .unwrap()
            .current
            .translation
            .x,
        6.0
    );
    app.scene.entities[0].transform.scale = glam::Vec3::splat(2.0);
    app.apply_editor_change(EntityId::new(1));
    assert_eq!(app.scene.entities[1].transform.scale, glam::Vec3::ONE);
    assert_eq!(
        app.scene.entities[1].collider.unwrap().half_extents,
        [0.5; 3]
    );
    assert_eq!(
        app.world
            .get::<BoxCollider>(app.scene_entities[1])
            .unwrap()
            .half_extents,
        [1.0; 3]
    );
    let scaled_start_y = app
        .world
        .get::<Transform>(app.scene_entities[1])
        .unwrap()
        .current
        .translation
        .y;
    for _ in 0..30 {
        app.update_fixed(std::time::Duration::from_millis(17));
    }
    let fallen_y = app
        .world
        .get::<Transform>(app.scene_entities[1])
        .unwrap()
        .current
        .translation
        .y;
    app.scene.entities[1]
        .collider
        .as_mut()
        .unwrap()
        .half_extents[0] = 0.75;
    app.apply_editor_change(EntityId::new(2));
    let after_edit_y = app
        .world
        .get::<Transform>(app.scene_entities[1])
        .unwrap()
        .current
        .translation
        .y;
    assert!(fallen_y < scaled_start_y);
    assert!((after_edit_y - fallen_y).abs() < 1e-4);
}

#[test]
fn benchmark_skips_warmup_and_uses_nearest_rank_percentiles() {
    let mut benchmark = BenchmarkCapture::new(1, 2);
    let mut diagnostics = FrameDiagnostics {
        frame_seconds: 0.1,
        ..FrameDiagnostics::default()
    };
    assert!(!benchmark.record(&diagnostics, (1920, 1080), (1920, 1080)));
    diagnostics.frame_seconds = 0.01;
    assert!(!benchmark.record(&diagnostics, (1920, 1080), (1920, 1080)));
    diagnostics.frame_seconds = 0.02;
    assert!(benchmark.record(&diagnostics, (1920, 1080), (1920, 1080)));
    assert_eq!(benchmark.frame_ms, [10.0, 20.0]);
    assert_eq!(percentile(&benchmark.frame_ms, 0.95), 20.0);
    assert!(benchmark.resolution_stable);
    let _ = benchmark.record(&diagnostics, (1920, 991), (1920, 1080));
    assert!(!benchmark.resolution_stable);
}

#[cfg(windows)]
#[test]
fn windows_memory_counters_report_working_set() {
    let memory = current_process_memory().expect("Windows process counters are available");
    assert!(memory.working_set_bytes > 0);
    if let Some(private) = memory.private_working_set_bytes {
        assert!(private > 0);
        assert!(memory.private_commit_bytes.unwrap() > 0);
    }
}

#[test]
fn fresh_engine_loads_saved_gltf_scene_without_manual_asset_registration() {
    let dir = std::env::temp_dir().join(format!("quasar-engine-scene-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let gltf_path = dir.join("triangle.gltf");
    let scene_path = dir.join("triangle.ron");
    let buffer_path = dir.join("triangle.bin");
    let mut vertices = Vec::new();
    for value in [0.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
        vertices.extend_from_slice(&value.to_le_bytes());
    }
    std::fs::write(&buffer_path, vertices).unwrap();
    let gltf = r#"{"asset":{"version":"2.0"},"buffers":[{"uri":"triangle.bin","byteLength":36}],"bufferViews":[{"buffer":0,"byteLength":36}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}],"materials":[{"pbrMetallicRoughness":{"baseColorFactor":[1.0,0.0,0.0,1.0]}}],"meshes":[{"primitives":[{"attributes":{"POSITION":0},"material":0}]}],"nodes":[{"mesh":0}],"scenes":[{"nodes":[0]}],"scene":0}"#;
    std::fs::write(&gltf_path, gltf).unwrap();

    EngineApp::new()
        .import_gltf(&gltf_path)
        .unwrap()
        .save_scene(&scene_path)
        .unwrap();
    assert_eq!(
        SceneAsset::load_ron(&scene_path).unwrap().source_gltf,
        Some(PathBuf::from("triangle.gltf"))
    );
    let mut consumer = EngineApp::new();
    consumer.load_scene(&scene_path).unwrap();
    assert_eq!(consumer.render_snapshot().instances.len(), 1);
    assert_eq!(consumer.mesh_assets.len(), 1);
    assert_eq!(consumer.material_assets.len(), 1);
    assert_eq!(
        consumer.scene.source_gltf,
        Some(std::fs::canonicalize(&gltf_path).unwrap())
    );
    std::fs::write(
        &gltf_path,
        gltf.replace("[1.0,0.0,0.0,1.0]", "[0.0,1.0,0.0,1.0]"),
    )
    .unwrap();
    for _ in 0..100 {
        consumer.poll_asset_reloads();
        if consumer.diagnostics.last_reload.as_deref() == Some("glTF reloaded") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        consumer.diagnostics.last_reload.as_deref(),
        Some("glTF reloaded")
    );
    let material = consumer.material_names["gltf_mesh_0_0"];
    assert_eq!(
        consumer.material_assets.get(material).unwrap().color,
        [0.0, 1.0, 0.0, 1.0]
    );
    assert_eq!(consumer.mesh_assets.len(), 1);
    assert_eq!(consumer.material_assets.len(), 1);
    consumer.diagnostics.last_reload = None;
    let mut moved_vertices = Vec::new();
    for value in [0.25_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
        moved_vertices.extend_from_slice(&value.to_le_bytes());
    }
    std::fs::write(&buffer_path, moved_vertices).unwrap();
    for _ in 0..100 {
        consumer.poll_asset_reloads();
        if consumer.diagnostics.last_reload.as_deref() == Some("glTF reloaded") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        consumer.diagnostics.last_reload.as_deref(),
        Some("glTF reloaded")
    );
    let mesh = consumer.mesh_names["gltf_mesh_0_0"];
    assert_eq!(
        consumer.mesh_assets.get(mesh).unwrap().vertices[0].position[0],
        0.25
    );
    assert!(consumer.material_assets.get(material).is_none());
    let material = consumer.material_names["gltf_mesh_0_0"];
    consumer.diagnostics.last_reload = None;
    std::fs::write(
        &gltf_path,
        gltf.replace(
            "\"attributes\":{\"POSITION\":0},\"material\":0",
            "\"attributes\":{\"POSITION\":0},\"material\":0,\"mode\":1",
        ),
    )
    .unwrap();
    for _ in 0..100 {
        consumer.poll_asset_reloads();
        if consumer
            .diagnostics
            .last_reload
            .as_deref()
            .is_some_and(|status| status.starts_with("Reload failed:"))
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        consumer
            .diagnostics
            .last_reload
            .as_deref()
            .is_some_and(|status| status.starts_with("Reload failed:"))
    );
    assert_eq!(consumer.render_snapshot().instances.len(), 1);
    assert_eq!(
        consumer.material_assets.get(material).unwrap().color,
        [0.0, 1.0, 0.0, 1.0]
    );
    assert_eq!(consumer.mesh_assets.len(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}
