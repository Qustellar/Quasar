# Quasar 0.0.3 Plan

## GPU-Driven Rendering and Content Pipeline

Quasar 0.0.3 upgrades the runtime from a verifiable forward-rendering foundation to a replaceable GPU-driven rendering base. The release keeps the CPU instanced path as a first-class fallback while adding a real render graph, optional GPU frustum culling with indirect draws, GPU skinning with CPU fallback, and schema 4 content metadata.

## Scope

- Add schema 4 while continuing to read schema 1, 2, and 3 scenes.
- Add render bounds, skin and skeleton references, audio listener/source data, prefab references, editor metadata, and dependency records.
- Compile and execute a RenderGraph for depth, culling, skinning, forward, and editor passes.
- Add an optional GPU-driven path for frustum culling and indirect draws.
- Add glTF skins, joints, inverse bind matrices, joint weights, and animation bindings.
- Use GPU skinning when supported and CPU skinning as a correctness fallback.
- Keep the embedded egui editor and expose the new render/content state there.
- Add fixed 1,024 and 10,000 instance benchmarks with visible/culled, batch, draw, upload, and timing metrics.

## Non-goals

0.0.3 does not include Hi-Z occlusion, deferred or ray-traced rendering, morph targets, IK, animation state machines, a full prefab override graph, an independent editor binary, networking, scripting, blueprints, project management, or compressed runtime texture formats.

## Compatibility and Release

GPU-driven and GPU skinning features are optional capabilities. Unsupported devices automatically use CPU instancing and CPU skinning. Windows/MSVC is the release validation environment, while public APIs remain platform-neutral. Schema 1-3 files migrate in memory and saves always emit schema 4. The release gate requires formatting, clippy, workspace tests, all-features tests, checks, documentation, and the independent consumer acceptance run. The release creates a local `v0.0.3` tag; GitHub Release creation is separate.
