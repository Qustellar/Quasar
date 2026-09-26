# Architecture

## Dependency direction

`qst-core` contains shared time, math, errors, handles, and diagnostics. `qst-ecs` wraps `bevy_ecs`; `qst-input` owns keyboard/mouse state; `qst-asset` handles CPU resources; `qst-scene` handles scene data and glTF import; `qst-physics` adapts Rapier; `qst-render` owns wgpu and GPU data; `qst-audio` owns the backend boundary. `qst-engine` composes them and is the only dependency of `playground`. `qst-editor` is enabled only through the engine's `editor` feature.

Large crates have separate implementation modules. `qst-engine` keeps public composition in `lib.rs`, with scene state, asset reloads, simulation, window lifecycle, and metrics in dedicated files. `qst-render` separates snapshot/feature contracts, the forward pipeline, and surface/device management. `qst-scene` separates RON scene data from glTF import. Asset storage, file watching, editor UI, and the egui/wgpu adapter have their own files. Small facades such as `qst-ecs` and `qst-input` stay compact while preserving replacement boundaries.

External crates are Cargo dependencies recorded in `Cargo.lock`; they are not copied into this repository. `wgpu` is used inside `qst-render` and the optional editor overlay, Rapier inside `qst-physics`, and `bevy_ecs` behind `qst-ecs`. A game using `qst-engine` can work through Quasar's scene, asset, physics, and render snapshot APIs without importing those crates directly.

## Frame flow

The winit event loop accumulates elapsed time. At most four 1/60-second fixed steps run per frame. ECS fixed systems run before Rapier. Rapier copies its positions into scene transforms while preserving the preceding state. `EngineApp::render_snapshot` interpolates previous/current transforms using the remaining accumulator. `ForwardFeature` extracts, prepares, sorts and draws the snapshot. An optional egui pass overlays the frame.

Scene schema 3 stores stable non-zero `EntityId` values and local transforms. `parent` records an `EntityId`, so hierarchy, physics synchronization, and editor selection do not depend on array positions. Schema 1 and 2 files are migrated on load; saves always write schema 3. Runtime stages are fixed as Startup, PreUpdate, VariableUpdate, FixedUpdate, Physics, PostUpdate, RenderExtract, and Render. Local, world, and previous-world components are separate extension points for later parallel extraction and interpolation.

The renderer owns the GPU representation of a mesh. The engine keeps CPU mesh, texture, material, animation, and audio records, so device upload is separate from file parsing. A watched glTF change is parsed on a worker; a successful result replaces CPU and GPU records on the main thread. glTF imports use a transparent `.quasar/cache` sidecar keyed by source hash, dependency hashes, and importer version; a corrupt or stale entry falls back to source parsing and is atomically replaced. PNG/JPEG sources decode to RGBA8 `TextureAsset` values. Scene files use RON with a schema version and reject unknown versions.

## Performance policy

`FrameDiagnostics` reports frame, fixed update, and render durations, loaded asset count, GPU resource count, and resident working set where available. Windows also reports private working set and private committed bytes. These are coarse counters, not a benchmark result. The 1080p playground target remains below 400 MB resident working set; record OS, GPU, backend, window mode, resolution stability, editor mode, scene, build profile, and measurement interval before comparing results.

The forward renderer groups snapshot instances by mesh/material and uploads one reusable instance buffer per contiguous group, producing one indexed instanced draw per batch. `FrameDiagnostics` includes instance, batch, draw-call, upload, skipped-instance, stage timing, cache, and dropped fixed-step counters. Potential later work includes render bundles, meshopt decode, texture compression, GPU culling, and indirect draw. These remain extension points, not 0.0.2 defaults.
