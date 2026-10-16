# Development and validation commands

On Windows, open an x64 Developer Command Prompt, or initialize VS2022 Build Tools in the same `cmd.exe` session before Cargo:

```bat
call "D:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
where link.exe
cargo fmt --all --check
cargo fmt --manifest-path acceptance/consumer/Cargo.toml --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy --manifest-path acceptance/consumer/Cargo.toml --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
cargo test --workspace --all-features -- --test-threads=1
cargo check --workspace --all-features
cargo doc --workspace --no-deps
cargo run --manifest-path acceptance/consumer/Cargo.toml --locked
```

The first `where link.exe` result must be the MSVC `Hostx64\x64\link.exe`. Keep the MSVC bin directory **first** on PATH. Git/MSYS can provide a GNU `link.exe` that shadows MSVC and produces `link: missing operand` for Rust binaries. Loading `vcvars64.bat` prepends the correct tools; do not append Git/MSYS ahead of them afterward. This repository has no `cargo xtask doctor` command.

PowerShell can run a complete command in the initialized child shell:

```powershell
cmd.exe /d /s /c 'call "D:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" && where link.exe && cargo test --workspace -- --test-threads=1'
```

Run headless GPU validation and the local preparation/culling benchmark explicitly:

```bat
call "D:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
set WGPU_BACKEND=vulkan
cargo test -p qst-render --lib -- --ignored --nocapture --test-threads=1
cargo test -p qst-render --release --lib cpu_gpu_instance_benchmark -- --ignored --nocapture --test-threads=1
cargo run -p playground -- assets/tests/animated-skin.gltf
```

The GPU tests print the selected adapter and report SKIP when none exists. Pure preference/graph fallback tests run in ordinary workspace tests. A machine without any adapter cannot render; CPU culling fallback still uses the GPU for skinning and indirect rendering. Use `WGPU_BACKEND=dx12` or `WGPU_BACKEND=vulkan` in the same shell to select a backend for comparisons. The local AMD validation explicitly selected Vulkan after an unrestricted run hit a WGL initialization stack overflow; see the [report](perf/2026-10-17-gpu-skinning.md). Linux uses the same Cargo commands without `vcvars64.bat`; all-features audio builds require ALSA development headers.

Validation and local benchmark output do not publish anything. Creating a tag, GitHub Release, package upload or other release publication requires a separate explicit request.
