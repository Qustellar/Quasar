# Architecture

## Dependency direction

`qst-core` contains shared time, math, errors, handles, and diagnostics. `qst-ecs` wraps `bevy_ecs`; `qst-input` owns keyboard/mouse state; `qst-asset` handles CPU resources; `qst-scene` handles scene data and glTF import; `qst-physics` adapts Rapier; `qst-render` owns wgpu and GPU data; `qst-audio` owns the backend boundary. `qst-engine` composes them and is the only dependency of `playground`. `qst-editor` is enabled only through the engine's `editor` feature.

Large crates have separate implementation modules. `qst-engine` keeps public composition in `lib.rs`, with scene state, asset reloads, simulation, window lifecycle, and metrics in dedicated files. `qst-render` separates snapshot/feature contracts, the forward pipeline, and surface/device management. `qst-scene` separates RON scene data from glTF import. Asset storage, file watching, editor UI, and the egui/wgpu adapter have their own files. Small facades such as `qst-ecs` and `qst-input` stay compact while preserving replacement boundaries.

External crates are Cargo dependencies recorded in `Cargo.lock`; they are not copied into this repository. `wgpu` is used inside `qst-render` and the optional editor overlay, Rapier inside `qst-physics`, and `bevy_ecs` behind `qst-ecs`. A game using `qst-engine` can work through Quasar's scene, asset, physics, and render snapshot APIs without importing those crates directly.

## Frame flow

The winit event loop accumulates elapsed time. At most four 1/60-second fixed steps run per frame. ECS fixed systems run before Rapier. Rapier copies its positions into scene transforms while preserving the preceding state. `EngineApp::render_snapshot` interpolates previous/current transforms using the remaining accumulator. `ForwardFeature` extracts, prepares, sorts and draws the snapshot. An optional egui pass overlays the frame.

Scene schema 5 stores skins (joint EntityIds, inverse bind matrices, skeleton roots) and stable non-zero `EntityId` values, local transforms, render bounds, skin bindings, listener metadata, and editor/dependency metadata. `parent` records an `EntityId`, so hierarchy, physics synchronization, and editor selection do not depend on array positions. Schema 1-4 files are migrated on load; saves always write schema 5. Runtime stages are fixed as Startup, PreUpdate, VariableUpdate, FixedUpdate, Physics, PostUpdate, RenderExtract, and Render. Local, world, and previous-world components are separate extension points for later parallel extraction and interpolation.

The renderer owns the GPU representation of a mesh. The engine keeps CPU mesh, texture, material, animation, and audio records, so device upload is separate from file parsing. A watched glTF change is parsed on a worker; a successful result replaces CPU and GPU records on the main thread. glTF imports use a transparent `.quasar/cache` sidecar keyed by source hash, dependency hashes, and importer version; a corrupt or stale entry falls back to source parsing and is atomically replaced. PNG/JPEG sources decode to RGBA8 `TextureAsset` values. Scene files use RON with a schema version and reject unknown versions.

## Performance policy

`FrameDiagnostics` reports frame, fixed update, and render durations, loaded asset count, GPU resource count, and resident working set where available. Windows also reports private working set and private committed bytes. These are coarse counters, not a benchmark result. The 1080p playground target remains below 400 MB resident working set; record OS, GPU, backend, window mode, resolution stability, editor mode, scene, build profile, and measurement interval before comparing results.

The forward renderer groups snapshot instances by mesh/material and uploads reusable instance buffers, producing one indexed indirect draw per batch after conservative frustum culling. `FrameDiagnostics` includes instance, batch, draw-call, upload, skipped-instance, culling, stage timing, cache, and dropped fixed-step counters. GPU and CPU culling share the game-facing snapshot API; the graph executes compute culling before the forward render pass when the GPU path is selected.

## GPU rendering and animation in 0.0.4

Animation advances in fixed steps after previous world transforms are captured. glTF TRS channels target stable joint entities; `AnimationProperty::Joint` remains compatible. Snapshot extraction interpolates instance and joint world transforms, then builds `inverse(instance_world) * joint_world * inverse_bind`. Primitive siblings with the same world transform share one palette; matrices are held in `Arc<[Mat4]>`, so extracting/cloning the snapshot into the renderer does not copy matrix arrays. Imported PBR vertices are retained on the CPU for uploads after window creation and reload.

`RenderPreference::Auto` detects compute, vertex storage and indirect execution. Explicit `GpuDriven` rejects missing capabilities. `CpuIndirect` and automatic compute fallback retain vertex skinning. `RenderPreference::select` is tested without an adapter; rendering itself still requires an adapter with vertex storage and indirect support.

The compiled graph carries `RenderExecution::{Prepare, Compute, Render}`. The default GPU order is `depth -> gpu_cull -> skinning -> forward`; the CPU graph omits `gpu_cull`. Prepare nodes mark frame/resource boundaries; globals and palette uploads finish before command encoding. Compute nodes use compute hooks outside render passes, and render nodes invoke draw hooks. The depth attachment is cleared when forward begins. The skinning node denotes palette readiness, not compute pre-skinning.

Each mesh/material batch has its own culling input, compacted vertex-instance output, atomic count and indexed indirect buffer. One 64-thread compute dispatch tests world spheres against six frustum planes; a second dispatch writes draw arguments from the counter. Counts and index counts are refreshed each frame, including reused/grown/shrunk batches. `first_instance` is always zero; no `INDIRECT_FIRST_INSTANCE` feature is requested. Both cull paths inflate skinned bounds conservatively using the palette and original mesh sphere.

The vertex shader branches on a skin sentinel and only reads joint storage for skinned vertices. Position, normal and tangent use weighted joint matrices. The existing PBR approximation remains; nonuniform scale/shear normal handling is not a full inverse-transpose skin-normal implementation.

Stats report submitted skin instances, compute dispatches and invalid-palette fallbacks. CPU visible/culled counts are exact; GPU visibility is pending on-device and is not synchronously read back each frame. GPU readback tests validate the actual indirect counts. GPU completion benchmark results include driver and queue synchronization overhead.
