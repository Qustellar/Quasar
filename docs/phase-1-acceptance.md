# Phase 1 local acceptance

Date: 2026-09-25. Scope: the Rust 2024 workspace in this repository. This is a local engineering baseline, not a published release or cross-platform certification.

## Accepted behavior

| Area | Evidence |
| --- | --- |
| Fixed simulation | 60 Hz accumulator with a four-step cap; interpolation and parent/child propagation tests pass. |
| Scene and assets | Versioned RON round-trip, glTF triangle import, typed handle invalidation, asynchronous CPU loading, and glTF mesh/material hot reload tests pass. Unsupported primitive modes return an error. |
| Physics and rendering | Rapier static/dynamic boxes, gravity, collision events, camera, ambient plus directional light, depth-tested forward rendering, and a nonblank GPU readback pass locally. |
| Editor | The optional in-process overlay shows a multilevel tree, Inspector, pause/step/save controls, diagnostics, and reload status. A parent rotation was saved to a temporary scene and verified after reopening; the temporary file was removed. |
| External consumption | `acceptance/consumer` has only `qst-engine` as a dependency, loads the playground scene, checks the render snapshot and collisions, and observes the box fall from 3.00 to 0.50. |

## Verification commands

These passed locally on Windows with Rust 1.95 stable after loading the VS2022 x64 developer environment:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo test --workspace --all-features
cargo check --workspace
cargo check --workspace --all-features
cargo doc --workspace --no-deps
cargo test -p qst-render -- --ignored
cargo fmt --manifest-path acceptance/consumer/Cargo.toml --check
cargo clippy --manifest-path acceptance/consumer/Cargo.toml --all-targets -- -D warnings
cargo run --manifest-path acceptance/consumer/Cargo.toml --locked
```

The GPU test is ignored in ordinary CI because a GPU adapter is not guaranteed on hosted runners. It passed on this development machine. The workflow file is present but its GitHub jobs have not run.

## Boundaries

The editor is a scene tool in the runtime window, not an IDE. Audio is an interface placeholder. The renderer uses the CPU draw path and requests no optional wgpu GPU features; batching, indirect draws, GPU culling, full PBR, animation, and broader glTF material support are outside this milestone. Schema 1 stores world transforms; rotated nonuniform scale can introduce shear that its transform representation cannot retain exactly.

The 1080p resident working-set target remains below 400 MB, measured in a declared backend and window configuration. The controlled Vulkan run documented in `docs/perf` was below that target; the DX12 run was above it. This is not a portable memory-budget pass, so backend comparisons remain future optimization work.
