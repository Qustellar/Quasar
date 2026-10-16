# Quasar 0.0.4 local validation — 2026-10-17

## Environment and method

Windows/MSVC, Rust 1.95, wgpu 27. Commands initialize VS2022 x64 using `vcvars64.bat`; `where link.exe` confirms `MSVC\14.44.35207\bin\Hostx64\x64\link.exe` is first. The GPU is AMD Radeon(TM) Graphics (integrated, device 5688), Vulkan backend, driver 21.30.40. Results describe this checkout and machine, not portable performance guarantees.

## Validation results

| Check | Result |
| --- | --- |
| Workspace and consumer formatting | Pass |
| Workspace Clippy, all targets / all features, warnings denied | Pass |
| Default workspace tests | Pass; three GPU/benchmark tests intentionally ignored in this gate |
| All-features workspace tests | Pass; same three opt-in tests |
| All-features workspace check and no-deps documentation | Pass |
| Independent consumer check / Clippy / locked execution | Pass, version 0.0.4 |
| GPU indirect readback | Pass on local Vulkan adapter |
| Vertex-stage skinning render/readback | Pass on GPU culling, forced CPU culling, and simulated no-compute fallback |
| Shared RenderGraph execution / feature hook order | Pass; headless pixels use the same encoder function as surface rendering |
| 1,024 / 10,000 CPU/GPU preparation and culling benchmark | Completed, debug profile |
| Animated fixture in a real surface/window | Pass on Vulkan; three warmup + eight measured frames, normal exit |

The consumer verifies the source-backed scene and falling-box collision (y 3.00 → 0.50), then imports the animated skin fixture and verifies the interpolated palette. Scene/runtime tests cover schema 4 migration, schema 5 round-trip, invalid and duplicate skin data, stable joint IDs, inherited primitive bindings, inverse binds, stale/corrupt import caches, source-backed animation restore, shared/interpolated palettes, inverse instance transforms, singular-palette fallback and preserving prior resources after failed imports/reloads.

GPU readback tests submit one visible and one clipped instance, then grow and shrink/reuse batches. Readbacks verify indirect instance and index counts, first-instance zero and compacted skin metadata. Pixel tests first show an unskinned triangle missing the center, then move it to the center using a joint palette; disabling skinning removes it again. Feature-hook traces verify compute before render and no compute hook in fallback graphs. The no-compute path is capability injection on the same Vulkan device, not a claim of testing separate legacy hardware. Adapter-free selection tests cover absent capabilities; rendering itself requires an adapter.

An unrestricted-backend retry encountered a stack overflow during initialization in the thread named `wgpu-hal WGL Instance Thread` (exit 0xc00000fd), before GPU assertions. The headless harness now respects `WGPU_BACKEND`; explicit `WGPU_BACKEND=vulkan` passed all nine render tests, including all three opt-in tests. OpenGL is not validated by this run. The failed retry is retained in the local logs, rather than being counted as a successful run.

The real window smoke used `cargo run -p playground -- assets/tests/animated-skin.gltf` with Vulkan, `QST_BENCHMARK_WARMUP=3` and `QST_BENCHMARK_FRAMES=8`. It completed at 1280×720 with stable resolution, two batches and two draw calls. This short debug smoke is not a performance or memory acceptance run. The Vulkan loader reported missing optional validation-layer and stale Epic overlay manifest files; the surface/render run still completed normally.

## Debug preparation/culling benchmark

Command: `cargo test -p qst-render --lib -- --include-ignored --nocapture --test-threads=1`, with `WGPU_BACKEND=vulkan` after VS2022 initialization.

Three warmup iterations, twenty measured iterations, arithmetic means. One mesh/material batch, a two-matrix palette shared by all instances, 50% outside the frustum. CPU mode uploads visible instances; GPU mode uploads all candidates and executes the two culling/finalization dispatches. `prepare_ms` covers extraction, grouping, conservative skin-bound preparation and uploads. `complete_ms` adds command submission and blocking GPU completion. It excludes rasterization and presentation; these are not GPU timestamp-query measurements or full-frame FPS results. Functional pixel tests cover rasterization separately.

| Instances | Path | Prepare ms | Completion ms | Instance upload bytes |
| ---: | --- | ---: | ---: | ---: |
| 1,024 | CpuIndirect | 1.9668 | 2.4097 | 49,152 |
| 1,024 | GpuDriven | 1.4242 | 1.9204 | 114,688 |
| 10,000 | CpuIndirect | 19.8990 | 20.6096 | 480,000 |
| 10,000 | GpuDriven | 14.6247 | 15.8650 | 1,120,000 |

Upload bytes exclude globals, palettes, counters and indirect parameters. Timings are debug-build local evidence; they do not establish release-build or large-scene speedups. No release-profile performance run or renewed 1080p memory acceptance is claimed.

## Local evidence and publication boundary

Raw command output remains under the ignored `.quasar/validation/` directory: `workspace.txt`, `final-workspace.txt` (final full regression), `consumer-gpu.txt`, `graph-gpu.txt` (failed WGL retry), `graph-gpu-vulkan.txt`, and `playground-vulkan.txt`. Reproduction commands are in [COMMANDS.md](../COMMANDS.md). The checked-in test source and fixture are the durable validation artifacts; local logs are machine-specific.

No tag, GitHub Release, package upload or publication was performed. The existing main/origin history divergence was not reconciled.
