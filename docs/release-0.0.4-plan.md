# Quasar 0.0.4: GPU driven rendering and animated skinning

## Scope

Schema 5 persists `SceneSkin` names, stable joint EntityIds, inverse bind matrices and skeleton roots. Schema 1-4 scenes migrate on load with an empty skin list. Validation rejects empty/duplicate skin names, duplicate/missing joints, missing roots, mismatched matrix counts and non-finite matrices. The glTF importer uses source node indices for stable IDs, a separate primitive ID range, inherited skin bindings and versioned cache invalidation. An omitted glTF inverse-bind accessor produces identity matrices.

glTF TRS channels animate joint entities during fixed updates. Render extraction interpolates world transforms and produces `inverse(instance_world) * joint_world * inverse_bind` palettes. Primitive siblings share snapshot palettes, and `Arc<[Mat4]>` avoids matrix copies during snapshot extraction/cloning. Complete imported PBR vertices survive deferred window/device upload. Source-backed scene load restores animation clips; reload validation finishes before replacing valid resources.

`RenderPreference::{Auto, CpuIndirect, GpuDriven}` controls capability selection. Auto chooses GPU culling where compute, vertex storage and indirect execution are available. CpuIndirect uses CPU frustum culling and indirect drawing. Both use vertex-stage skinning. Explicit unsupported GpuDriven requests return an error. Rendering still requires vertex storage and indirect support; adapter-free tests cover selection when no adapter is present.

Each GPU batch owns a culling input, compacted output, atomic counter and indirect buffer. Two compute dispatches perform conservative sphere/frustum culling and counter-to-indirect finalization. First-instance is always zero, avoiding the optional INDIRECT_FIRST_INSTANCE feature. Reused buffers track active counts and refreshed index counts. Skin bounds are conservatively expanded before either culling path. The vertex shader transforms position, normals and tangents, branching around storage reads for unskinned instances.

The graph records Prepare, Compute and Render execution types. GPU order is depth, gpu_cull, skinning, forward; CPU graphs omit gpu_cull. Prepare nodes identify resource/clear boundaries, compute hooks execute outside render passes, and forward performs indirect draws. Palette uploads are ready before command encoding; skinning executes in the vertex stage. Diagnostics expose dispatches, submitted skin instances and skin fallbacks. GPU visibility counters remain on the device during normal frames; readback tests validate their actual values.

## Validation

The gate covers formatting, default/all-features workspace tests, all-target/all-features Clippy, check, documentation, independent consumer acceptance, headless indirect readback, skin deformation pixel readback on both culling paths, and 1,024/10,000 instance preparation/culling benchmarks. All Windows Cargo commands run after VS2022 x64 initialization with MSVC link.exe first on PATH; exact commands are in [COMMANDS.md](COMMANDS.md).

The checked-in animated fixture has two primitives and one animated joint. Tests cover palette sharing/interpolation, cache version invalidation, invalid skin rejection and failed reload preservation. GPU readback tests also shrink/reuse buffers and check separate batch index counts. Local results and benchmark methodology are recorded in [the validation report](perf/2026-10-17-gpu-skinning.md).

## Non-goals and release boundary

No Hi-Z occlusion, deferred/ray tracing, morph targets, IK, animation state machines, compute pre-skinning, or complete CPU dynamic vertex skinning fallback. Normals/tangents use the weighted skin transform; the forward PBR approximation does not promise full inverse-transpose correction for nonuniform scale/shear.

Workspace, playground and consumer versions are 0.0.4. This work produces local source, fixtures, tests and reports only. It does not create a tag, GitHub Release, upload or publication, and does not reconcile the existing three-commit main/origin divergence.
