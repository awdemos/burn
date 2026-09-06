# Burn

## OVERVIEW

Rust deep-learning framework and tensor library. Burns unifies training and inference in one codebase, with multi-platform backends via CubeCL (CUDA, ROCm, Metal, Vulkan, WebGPU, CPU) and simpler backends (LibTorch, pure-Rust CPU, no_std).

## STRUCTURE

```
crates/               Workspace crates (burn-core, burn-autodiff, burn-cubecl, burn-dataset, burn-nn, burn-optim, ...)
examples/             Ready-to-run examples (mnist, custom-wgpu, etc.)
burn-book/            mdBook user guide
contributor-book/     mdBook contributor guide
xtask/                Maintenance tasks (semver checks, typos, etc.)
```

## COMMANDS

```bash
cargo build                                    # Build the workspace
cargo test                                     # Run tests (long; use --package to scope)
cargo test -p burn-core                        # Run tests for one crate
cargo run --example mnist --release            # Run the MNIST example
cargo xtask typos                              # Check for typos
cargo xtask semver-checks                    # Check semantic versioning
```

## SETUP

- Install Rust 1.78+ (check `Cargo.toml` / `rust-toolchain.toml` if present).
- For GPU backends install the matching vendor toolchain (CUDA / ROCm / Metal SDK).
- Run `cargo build` to fetch and compile dependencies.

## CODE STYLE

- Rust edition 2021.
- Run `cargo fmt` and `cargo clippy --all-targets` before committing.
- Follow `CONTRIBUTING.md` for commit-message and PR conventions.

## DEPLOYMENT

No Dagger module or recognized deployment configuration was found. The project publishes crates to crates.io via GitHub Actions.
