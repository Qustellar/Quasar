# Quasar Engine

Quasar 0.0.4 is Qustellar Game's Rust-first 3D engine developer preview. It is a runtime with a thin embedded scene tool, designed around replaceable systems, explicit data flow, stable scene identities, source-backed import caching, and profiling-driven optimization.

## 0.0.4 target

Version 0.0.4 adds schema 5 skins, animated glTF joint transforms, snapshot skin palettes, compute frustum culling and indexed indirect generation, and vertex-stage skinning. `RenderPreference::Auto` selects `GpuDriven` when the adapter supports compute, vertex storage and indirect execution. `CpuIndirect` forces CPU culling while retaining GPU skinning; explicit `GpuDriven` returns an error if unsupported. Call `EngineApp::set_render_preference` before `run`; `Renderer::capabilities()` exposes the detected support. Both paths consume `RenderSnapshot`.

## 0.0.2 foundation

The 0.0.2 foundation adds fixed runtime stages, input state, schema 3 local/world transforms, PNG/JPEG texture decoding, PBR material data, glTF animation clips, an audio backend boundary, editor scene CRUD primitives, and the existing Rapier, hot-reload, and instanced forward-rendering paths. Both `playground` and the separate `acceptance/consumer` workspace depend only on `qst-engine`.

## Run

```sh
cargo run -p playground
cargo run -p playground --features editor
cargo run -p playground -- path/to/scene.glb
cargo run -p playground -- path/to/saved-scene.ron
```

The default playground loads `assets/playground.ron`, which references `assets/playground.gltf` and contains a falling box, floor, camera, and directional light. `acceptance/consumer` is a separate Cargo workspace that depends only on `qst-engine` and loads the same scene. Passing a glTF or GLB path imports that scene and watches its directory for changes. The editor feature overlays scene and diagnostics panels in the runtime window. `EngineApp::save_scene` and `EngineApp::load_scene` are the RON APIs. Saved scenes imported from glTF retain a source path and restore their mesh and material resources when loaded by a fresh engine process. Programmatically registered resources still need registration before loading their scene.

Scene entities can form a parent-child hierarchy. Schema 5 stores global `SceneSkin` records (stable names, joint EntityIds, inverse bind matrices and skeleton roots), alongside stable `EntityId` values, local transforms, render bounds, skin bindings, listener metadata, prefab/editor metadata, and dependency records; schema 1-4 files migrate automatically on load. Runtime `LocalTransform`, `WorldTransform`, and `PreviousWorldTransform` components keep authored data separate from derived render and physics state. In the optional editor, select an entity in the scene tree to edit its transform, camera, light, or collider values, then save the scene.

## Status

The 0.0.1 acceptance remains the compatibility baseline. Version 0.0.2 extends it with schema 3 migration, fixed stage registration, `qst-input`, texture import, animation sampling, audio state management, scene CRUD methods, and richer diagnostics. The forward renderer remains a verifiable baseline rather than a claim of complete AAA PBR.

## 0.0.4 boundaries

0.0.4 does not include Hi-Z occlusion, deferred/ray tracing, morph targets, IK, animation state machines, compute pre-skinning, or full dynamic CPU vertex skinning. CPU fallback covers culling; vertex-stage GPU skinning remains active. Adapters still need vertex storage and indirect execution to render. GPU tests explicitly report a skipped adapter when none is available; adapter-free selection tests always run.

## Memory target

The 1080p playground resident working-set target is below 400 MB (decimal). It is **not yet a portable pass**: an initial decorated-window MSVC release run peaked at 444.8 MB without the editor and 445.2 MB with it. Controlled borderless 1920x1080 Vulkan runs later peaked around 386 MB, while DX12 reached 547 MB on the same machine. A full-scene/empty-scene comparison showed about 1 MB difference in peak working set; the renderer, backend, and host environment dominate this baseline. See the [initial benchmark](docs/perf/2026-09-25-windows-msvc.md) and [memory investigation](docs/perf/2026-09-25-memory-investigation.md). Set `QST_BENCHMARK=1` to request 1920x1080; `QST_BENCHMARK_WARMUP` and `QST_BENCHMARK_FRAMES` select the sample interval. Benchmark output flags any resolution change.

## Workspace

The runtime is split into `qst-core`, `qst-ecs`, `qst-input`, `qst-asset`, `qst-scene`, `qst-physics`, `qst-render`, `qst-audio`, `qst-editor`, and the public `qst-engine` facade. `playground` is the minimal outside crate.

Architecture decisions are in [ADR 0001](docs/adr/0001-runtime-boundaries.md) and [Architecture](docs/architecture.md).

The recommended integration surface is `qst-engine::prelude`; the lower-level workspace crates remain replaceable implementation boundaries. The Windows validation uses the VS2022 x64 environment from `vcvars64.bat` before running Cargo, with MSVC `link.exe` first on PATH. See [commands](docs/COMMANDS.md), the [0.0.4 plan](docs/release-0.0.4-plan.md), and [local validation](docs/perf/2026-10-17-gpu-skinning.md).

Run the animated fixture with `cargo run -p playground -- assets/tests/animated-skin.gltf`. It animates a joint shared by two primitives. GPU tests and the 1,024/10,000 instance preparation/culling benchmark are opt-in: `cargo test -p qst-render --lib -- --ignored --nocapture --test-threads=1`. Benchmark completion time includes submission and GPU waiting, and is not a timestamp-query GPU duration or a whole-frame rendering benchmark.

## License

Quasar is licensed under the [Apache License 2.0](LICENSE).
