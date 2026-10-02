# Quasar Engine

Quasar 0.0.3 is Qustellar Game's Rust-first 3D engine developer preview. It is a runtime with a thin embedded scene tool, designed around replaceable systems, explicit data flow, stable scene identities, source-backed import caching, and profiling-driven optimization.

## 0.0.3 target

The 0.0.3 upgrade adds schema 4 content metadata, an executable RenderGraph, conservative frustum culling with indexed indirect draws, skin-ready vertex data with a CPU reference path, and fixed 1,024/10,000 instance grouping benchmarks. A future compute path can replace the culling implementation without changing the snapshot contract.

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

Scene entities can form a parent-child hierarchy. Schema 4 stores stable `EntityId` values, local transforms, render bounds, skin bindings, listener metadata, prefab/editor metadata, and dependency records; schema 1, schema 2, and schema 3 files migrate automatically on load. Runtime `LocalTransform`, `WorldTransform`, and `PreviousWorldTransform` components keep authored data separate from derived render and physics state. In the optional editor, select an entity in the scene tree to edit its transform, camera, light, or collider values, then save the scene.

## Status

The 0.0.1 acceptance remains the compatibility baseline. Version 0.0.2 extends it with schema 3 migration, fixed stage registration, `qst-input`, texture import, animation sampling, audio state management, scene CRUD methods, and richer diagnostics. The forward renderer remains a verifiable baseline rather than a claim of complete AAA PBR.

## 0.0.3 boundaries

0.0.3 does not include skeleton skinning, compressed texture formats, deferred or ray-traced rendering, compute GPU culling, complete transparency sorting, spatial audio, scripting or blueprints, an independent IDE, a project/package manager, console certification, a marketplace, or Nanite-style virtual geometry.

## Memory target

The 1080p playground resident working-set target is below 400 MB (decimal). It is **not yet a portable pass**: an initial decorated-window MSVC release run peaked at 444.8 MB without the editor and 445.2 MB with it. Controlled borderless 1920x1080 Vulkan runs later peaked around 386 MB, while DX12 reached 547 MB on the same machine. A full-scene/empty-scene comparison showed about 1 MB difference in peak working set; the renderer, backend, and host environment dominate this baseline. See the [initial benchmark](docs/perf/2026-09-25-windows-msvc.md) and [memory investigation](docs/perf/2026-09-25-memory-investigation.md). Set `QST_BENCHMARK=1` to request 1920x1080; `QST_BENCHMARK_WARMUP` and `QST_BENCHMARK_FRAMES` select the sample interval. Benchmark output flags any resolution change.

## Workspace

The runtime is split into `qst-core`, `qst-ecs`, `qst-input`, `qst-asset`, `qst-scene`, `qst-physics`, `qst-render`, `qst-audio`, `qst-editor`, and the public `qst-engine` facade. `playground` is the minimal outside crate.

Architecture decisions are in [ADR 0001](docs/adr/0001-runtime-boundaries.md) and [Architecture](docs/architecture.md).

The recommended integration surface is `qst-engine::prelude`; the lower-level workspace crates remain replaceable implementation boundaries. The Windows validation uses the VS2022 x64 environment from `vcvars64.bat` before running Cargo.

## License

Quasar is licensed under the [Apache License 2.0](LICENSE).
