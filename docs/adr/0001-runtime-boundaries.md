# ADR 0001: Runtime boundaries

Status: accepted for Phase 1.

Quasar is a code-first Rust runtime. The simulation world owns gameplay state and Rapier synchronization. A separate `RenderSnapshot` carries only camera, light, mesh/material handles, and interpolated transforms into rendering. Rendering never queries the gameplay ECS directly. `qst-asset` owns CPU asset identity and loading state; `qst-render` owns GPU buffers, pipelines, and per-object bind groups. The editor is an optional `qst-engine/editor` feature and does not enter the default runtime dependency path.

`bevy_ecs` supplies storage and scheduling; Quasar does not implement a new sparse set. `wgpu` remains the GPU abstraction. The initial `ForwardFeature` owns the actual forward pipeline, GPU cache, and draw queue. Additional features can use the same extract/prepare/queue/render lifecycle. The initial `RenderGraph` is only an ordered list with resource read/write declarations; it is not a general scheduling engine.

The CPU draw path is the compatibility baseline. Profiling must justify instancing, indirect draws, GPU culling, or a more complex render graph. Resource handles have an index and generation; replacing an asset invalidates old handles. Hot reload parses glTF on a worker and swaps a complete import only after parsing succeeds.
