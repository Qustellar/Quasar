# Architecture

## Dependency direction

`qst-core` contains shared time, math, errors, handles, and diagnostics. `qst-ecs` wraps `bevy_ecs`; `qst-asset` handles CPU resources; `qst-scene` handles scene data and glTF import; `qst-physics` adapts Rapier; `qst-render` owns wgpu and GPU data. `qst-engine` composes them and is the only dependency of `playground`. `qst-editor` is enabled only through the engine's `editor` feature.

Large crates have separate implementation modules. `qst-engine` keeps public composition in `lib.rs`, with scene state, asset reloads, simulation, window lifecycle, and metrics in dedicated files. `qst-render` separates snapshot/feature contracts, the forward pipeline, and surface/device management. `qst-scene` separates RON scene data from glTF import. Asset storage, file watching, editor UI, and the egui/wgpu adapter have their own files. Small facades such as `qst-ecs` and the Phase 1 audio placeholder stay compact.

External crates are Cargo dependencies recorded in `Cargo.lock`; they are not copied into this repository. `wgpu` is used inside `qst-render` and the optional editor overlay, Rapier inside `qst-physics`, and `bevy_ecs` behind `qst-ecs`. A game using `qst-engine` can work through Quasar's scene, asset, physics, and render snapshot APIs without importing those crates directly.

## Frame flow

The winit event loop accumulates elapsed time. At most four 1/60-second fixed steps run per frame. ECS fixed systems run before Rapier. Rapier copies its positions into scene transforms while preserving the preceding state. `EngineApp::render_snapshot` interpolates previous/current transforms using the remaining accumulator. `ForwardFeature` extracts, prepares, sorts and draws the snapshot. An optional egui pass overlays the frame.

Scene schema 1 stores entity transforms in world coordinates. `parent` records hierarchy without changing existing RON coordinates. During fixed updates, a parent's translation, rotation, or scale change is applied to descendants in hierarchy order; Rapier bodies receive the new pose and box dimensions follow scale changes. The editor shows this hierarchy and lets users edit position, rotation, and scale. Editing a parent also updates descendant values in the saved scene. Nonuniform scale combined with rotation can introduce shear, which the current transform representation cannot preserve exactly.

The renderer owns the GPU representation of a mesh. The engine keeps the CPU mesh and material records, so device upload is separate from file parsing. A watched glTF change is parsed on a worker; a successful result replaces CPU and GPU records on the main thread. Scene files use RON with a schema version and reject unknown versions. A scene imported from glTF stores its source path; loading the RON restores referenced meshes and materials. When the source is under the save directory, the path is stored relative to the RON file.

## Performance policy

`FrameDiagnostics` reports frame, fixed update, and render durations, loaded asset count, GPU resource count, and resident working set where available. Windows also reports private working set and private committed bytes. These are coarse counters, not a benchmark result. The 1080p playground target remains below 400 MB resident working set; record OS, GPU, backend, window mode, resolution stability, editor mode, scene, build profile, and measurement interval before comparing results.

Potential later work includes shared object uniform buffers, instancing, render bundles, meshopt decode, texture compression, and indirect draw. These are reserved extension points, not Phase 1 implementations.
