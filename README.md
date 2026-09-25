# Quasar Engine

Quasar 0.0.1 is Qustellar Game's Rust-first 3D engine developer preview. It is a runtime with a thin embedded scene tool, designed around replaceable systems, explicit data flow, stable scene identities, source-backed import caching, and profiling-driven optimization.

## Phase 1 target

The Phase 1 local baseline is a fixed-step runtime that can load and save a scene, display a glTF scene, run Rapier box physics, interpolate rendering, hot-reload assets, and expose coarse frame-time and working-set diagnostics. Both `playground` and the separate `acceptance/consumer` workspace depend only on `qst-engine`.

## Run

```sh
cargo run -p playground
cargo run -p playground --features editor
cargo run -p playground -- path/to/scene.glb
cargo run -p playground -- path/to/saved-scene.ron
```

The default playground loads `assets/playground.ron`, which references `assets/playground.gltf` and contains a falling box, floor, camera, and directional light. `acceptance/consumer` is a separate Cargo workspace that depends only on `qst-engine` and loads the same scene. Passing a glTF or GLB path imports that scene and watches its directory for changes. The editor feature overlays scene and diagnostics panels in the runtime window. `EngineApp::save_scene` and `EngineApp::load_scene` are the RON APIs. Saved scenes imported from glTF retain a source path and restore their mesh and material resources when loaded by a fresh engine process. Programmatically registered resources still need registration before loading their scene.

Scene entities can form a parent-child hierarchy. Schema 2 stores stable `EntityId` values and world transforms; schema 1 files migrate automatically on load. Moving, rotating, or scaling a parent carries its descendants, including cameras and physics boxes. In the optional editor, select an entity in the scene tree to edit its position, rotation, scale, camera, light, or collider values, then save the scene.

## Status

The Phase 1 local acceptance is recorded in [Phase 1 acceptance](docs/phase-1-acceptance.md). Version 0.0.1 adds schema 1 to schema 2 scene migration with stable `EntityId` values, transparent glTF import caching, instanced forward rendering, and render batch diagnostics. Editor selection, parent transform editing, pause, save, and scene reopening were exercised in a Windows window. Broader glTF variants and file writers remain compatibility work beyond this preview.

## Non-goals

Phase 1 does not include a full PBR renderer, custom RHI, self-written ECS storage, scripting or blueprints, an independent IDE, a project/package manager, console certification, a marketplace, Nanite-style virtual geometry, or first-party game development.

## Memory target

The 1080p playground resident working-set target is below 400 MB (decimal). It is **not yet a portable pass**: an initial decorated-window MSVC release run peaked at 444.8 MB without the editor and 445.2 MB with it. Controlled borderless 1920x1080 Vulkan runs later peaked around 386 MB, while DX12 reached 547 MB on the same machine. A full-scene/empty-scene comparison showed about 1 MB difference in peak working set; the renderer, backend, and host environment dominate this baseline. See the [initial benchmark](docs/perf/2026-09-25-windows-msvc.md) and [memory investigation](docs/perf/2026-09-25-memory-investigation.md). Set `QST_BENCHMARK=1` to request 1920x1080; `QST_BENCHMARK_WARMUP` and `QST_BENCHMARK_FRAMES` select the sample interval. Benchmark output flags any resolution change.

## Workspace

The runtime is split into `qst-core`, `qst-ecs`, `qst-asset`, `qst-scene`, `qst-physics`, `qst-render`, `qst-audio`, `qst-editor`, and the public `qst-engine` facade. `playground` is the minimal outside crate.

Architecture decisions are in [ADR 0001](docs/adr/0001-runtime-boundaries.md) and [Architecture](docs/architecture.md).

The recommended integration surface is `qst-engine::prelude`; the lower-level workspace crates remain replaceable implementation boundaries. The 0.0.1 Windows validation uses the VS2022 x64 environment from `vcvars64.bat` before running Cargo.

## License

Quasar is licensed under the [Apache License 2.0](LICENSE).
