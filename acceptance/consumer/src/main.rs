use std::path::Path;
use std::time::Duration;

use qst_engine::{BoxCollider, EngineApp, Transform};

fn falling_box_height(app: &mut EngineApp) -> f32 {
    let mut query = app.world.query::<(&BoxCollider, &Transform)>();
    query
        .iter(&app.world)
        .find(|(collider, _)| collider.dynamic)
        .expect("saved scene has a dynamic box")
        .1
        .current
        .translation
        .y
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scene_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixture.ron");
    let mut app = EngineApp::new();
    app.load_scene(scene_path)?;
    assert_eq!(app.scene.schema_version, 5);

    let initial = app.render_snapshot();
    assert_eq!(initial.instances.len(), 2);
    assert!(initial.camera.is_some());
    assert_eq!(initial.lights.len(), 1);
    let initial_height = falling_box_height(&mut app);

    for _ in 0..120 {
        app.update_fixed(Duration::from_secs_f64(1.0 / 60.0));
    }

    let final_height = falling_box_height(&mut app);
    assert!(final_height < initial_height);
    assert!(final_height < 0.6);
    assert!(app.drain_collisions().any(|collision| collision.started));
    assert_eq!(app.render_snapshot().instances.len(), 2);
    let skin_fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/tests/animated-skin.gltf");
    app.import_gltf(skin_fixture)?;
    app.fixed_time.step_seconds = 0.1;
    app.update_fixed(Duration::from_millis(150));
    let animated = app.render_snapshot();
    assert_eq!(animated.skins.len(), 1);
    assert_eq!(animated.instances.len(), 2);
    assert!((animated.skins[0].matrices[0].w_axis.x - 0.1).abs() < 1e-5);
    println!(
        "consumer accepted animated skin and scene; box y: {initial_height:.2} -> {final_height:.2}"
    );
    Ok(())
}
