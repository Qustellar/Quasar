# Quasar 0.0.3 Plan

## GPU-Driven Rendering and Content Pipeline

Quasar 0.0.3 upgrades the runtime from a verifiable forward-rendering foundation to a replaceable rendering base. The release keeps CPU culling and instancing as a first-class correctness path, adds indexed indirect draws, render graph compilation/execution boundaries, skin-ready glTF data with a CPU reference, and schema 4 content metadata. Compute culling and GPU skinning remain explicit replacement points for the next rendering increment.

## Scope

- Add schema 4 while continuing to read schema 1, 2, and 3 scenes.
- Add render bounds, skin and skeleton references, audio listener/source data, prefab references, editor metadata, and dependency records.
- Compile and execute a RenderGraph for depth, culling, skinning, forward, and editor pass boundaries.
- Add conservative CPU frustum culling and indexed indirect draws behind the GPU-driven replacement boundary.
- Add glTF skins, joints, inverse bind matrices, joint weights, and animation bindings.
- Provide skin-ready vertex data and a tested CPU skinning correctness reference.
- Keep the embedded egui editor and expose the new render/content state there.
- Add fixed 1,024 and 10,000 instance grouping benchmarks with visible/culled, batch, draw, upload, and timing metrics.

## Non-goals

0.0.3 does not include Hi-Z occlusion, deferred or ray-traced rendering, morph targets, IK, animation state machines, a full prefab override graph, an independent editor binary, networking, scripting, blueprints, project management, or compressed runtime texture formats.

## Compatibility and Release

The future compute culling and GPU skinning features are optional capabilities with stable replacement boundaries. The shipped renderer uses CPU visibility evaluation plus indexed indirect draws; the CPU skinning reference remains available for correctness tests. Windows/MSVC is the release validation environment, while public APIs remain platform-neutral. Schema 1-3 files migrate in memory and saves always emit schema 4. The release gate requires formatting, clippy, workspace tests, all-features tests, checks, documentation, and the independent consumer acceptance run. The release creates a local `v0.0.3` tag; GitHub Release creation is separate.
