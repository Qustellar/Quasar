# 1080p memory investigation, 2026-09-25

The [initial release baseline](2026-09-25-windows-msvc.md) exceeded 400 MB. A later 5,000-frame run changed from 1920x1080 to 1920x991, so its frame and memory values cannot serve as a 1080p comparison. The benchmark now uses an undecorated, non-resizable window and reports `resolution_stable` across warmup and measurement.

## Controlled runs

Windows, Rust 1.95 MSVC release, AMD Radeon(TM) Graphics, physical 1920x1080, 600 warmup frames. Values below are decimal MB (1,000,000 bytes). All runs reported `resolution_stable=true`. `QST_BENCHMARK_EMPTY=1` skips scene loading but uses the same wgpu surface and forward renderer.

| Backend / scene | Measured frames | Frame p50 / p95 | Working set min / peak | Private working set peak | Private commit peak |
| --- | ---: | ---: | ---: | ---: | ---: |
| Vulkan / full, run 1 | 600 | 9.058 / 14.562 ms | 347.6 / 386.1 MB | 361.3 MB | 367.2 MB |
| Vulkan / empty | 600 | 5.074 / 8.371 ms | 346.1 / 384.7 MB | 361.0 MB | 367.0 MB |
| Vulkan / full, run 2 | 600 | 5.733 / 9.270 ms | 347.5 / 385.9 MB | 361.1 MB | 367.1 MB |
| Vulkan / full, long run | 5,000 | 6.242 / 12.172 ms | 347.4 / 385.8 MB | 361.1 MB | 367.1 MB |
| DX12 / full | 600 | 0.817 / 3.441 ms | 546.9 / 547.0 MB | 510.3 MB | 514.7 MB |

The full scene added roughly 1.3 MB to peak working set over the empty Vulkan run. Its two small glTF mesh resources are therefore not the cause of the hundreds of MB baseline. DX12 used about 161 MB more peak working set than Vulkan on this adapter. The original decorated-window peaks around 445 MB did not reproduce in later short process probes; window mode and run-to-run conditions must stay in the record. These results do not establish a cross-backend `<400 MB` pass or identify a leak.

To reproduce, build `playground` in release mode and set `QST_BENCHMARK=1`, `QST_BENCHMARK_WARMUP=600`, and `QST_BENCHMARK_FRAMES=600` (or `5000`). Set `WGPU_BACKEND=vulkan` or `dx12`; add `QST_BENCHMARK_EMPTY=1` for the empty control. Compare only results with `resolution_stable=true` and the same scene, backend, and window mode. `benchmark_memory` reports resident working set, private resident working set, and private committed bytes; the latter is not resident memory.
