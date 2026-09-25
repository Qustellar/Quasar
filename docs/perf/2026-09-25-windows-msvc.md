# 1080p playground baseline, 2026-09-25

## Configuration

- Windows, Rust 1.95.0 stable, `x86_64-pc-windows-msvc`, release profile.
- AMD Radeon(TM) Graphics, wgpu Vulkan backend.
- `assets/playground.ron` with two glTF cube resources, one directional light, one camera, and Rapier dynamic/static boxes.
- Physical surface: 1920 x 1080. `QST_BENCHMARK=1`, `QST_BENCHMARK_WARMUP=600`, `QST_BENCHMARK_FRAMES=600`.
- One local run per mode. Working set is the Windows process resident working set, sampled once per rendered frame after warmup. Numbers below use decimal MB (1,000,000 bytes).

| Mode | Frame p50 | Frame p95 | Fixed p95 | Render p95 | Working set min | Working set peak |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Runtime | 6.991 ms | 10.190 ms | 0.034 ms | 9.194 ms | 388.6 MB | 444.8 MB |
| Editor | 7.004 ms | 10.065 ms | 0.033 ms | 9.086 ms | 389.3 MB | 445.2 MB |

The `<400 MB` resident working-set target was not met over the measured interval. This is a baseline, not evidence of a leak or a representative game workload. A memory breakdown and repeat runs are needed before choosing an optimization.

Build each mode with `cargo build -p playground --release` or `cargo build -p playground --release --features editor` in the VS2022 x64 developer environment. Run the resulting `target/release/playground.exe` with the environment variables above. Rebuild between modes because both commands produce the same executable path.

## 0.0.1 batch counters

The 0.0.1 benchmark line additionally prints `batches_p50` and `draw_calls_p50`. The renderer's deterministic unit benchmark builds 1,024 instances across four mesh/material keys and verifies four batches; the runtime benchmark should be recorded with the same scene, backend, and window configuration before comparing frame or memory numbers.
