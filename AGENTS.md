# Burn — Agent Guide

## Overview

Awdeemos fork of [tracel-ai/burn](https://github.com/tracel-ai/burn), a Rust deep
learning framework and tensor library. This is a large Cargo workspace with many
crates, examples, and an `xtask` command runner.

## Project Layout

| Path | Purpose |
|------|---------|
| `crates/` | Core workspace crates: `burn-core`, `burn-ndarray`, `burn-cuda`, `burn-train`, `burn-wgpu`, etc. |
| `examples/` | End-to-end examples (image classification, text generation, etc.) |
| `burn-book/` / `contributor-book/` | Documentation |
| `xtask/` | Task runner for validation, tests, builds, publishing |

## Build Commands

```bash
# Check a CPU-only backend (no GPU deps required)
cargo check -p burn-ndarray

# Build a specific example
cargo build -p mnist
```

## Test Commands

```bash
# Unit tests for the CPU backend
cargo test -p burn-ndarray

# Run integration checks via xtask (requires compatible backends / may need libtorch)
cargo xtask test all --ci dev --backend ndarray
```

## Lint / Validation

```bash
# Fast PR checks (may require GPU backends to fully pass)
cargo xtask validate --backend ndarray

# Format check
cargo fmt -- --check

# Clippy
cargo clippy --workspace -- -D warnings
```

## Key Conventions

- **Workspace crate**: nearly all code lives under `crates/`.
- **Backend trait system**: new hardware support is added as a `Backend` impl.
- **xtask aliases**: `.cargo/config.toml` aliases `cargo xtask` and `cargo run-checks`.
- **Tests use Gherkin-ish Ginkgo via `rstest`/custom harnesses** in many crates.

## Common Gotchas

- Full `cargo xtask validate` may fail locally if `libtorch`/CUDA/ROCm deps are missing.
- The `tch` backend needs a working PyTorch/LibTorch install.
- `cargo check -p burn-ndarray` is a good smoke test that does not require GPU libs.
- Some examples are excluded from the workspace (`examples/notebook`, `examples/raspberry-pi-pico`, `examples/dqn-agent`).

## Deployment

No Dagger/Jenkins workflow present. Use `cargo xtask build` for release artifacts;
crates are published via `cargo xtask publish`.
