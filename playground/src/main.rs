use std::path::{Path, PathBuf};

use qst_engine::EngineApp;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/playground.ron"));
    let mut app = EngineApp::new();
    if std::env::var_os("QST_BENCHMARK").is_some() {
        app.set_window_size(1920, 1080);
    }
    if let Ok(frames) = std::env::var("QST_BENCHMARK_FRAMES") {
        let measured_frames = frames.parse::<u32>()?;
        let warmup_frames = std::env::var("QST_BENCHMARK_WARMUP")
            .ok()
            .map(|value| value.parse::<u32>())
            .transpose()?
            .unwrap_or(600);
        app.set_benchmark_frames(warmup_frames, measured_frames);
    }
    if std::env::var_os("QST_BENCHMARK_EMPTY").is_none() {
        if source
            .extension()
            .is_some_and(|extension| extension == "ron")
        {
            app.load_scene(source)?;
        } else {
            app.import_gltf(source)?;
        }
    }
    app.run()?;
    Ok(())
}
