# Contributing to OpenSST

Thanks for your interest in contributing. This document covers how to build, test, and submit changes.

## Prerequisites

- **Rust** (stable). The repo uses `rust-toolchain.toml`; run `rustup show` to confirm.
- **Linux:** `libasound2-dev` (e.g. `sudo apt install libasound2-dev` or `sudo dnf install alsa-lib-devel`) for the audio crate dependency.
- **mold** (optional): faster linking. Install via your package manager for the best build experience; CI installs it automatically.

## Building

From the repo root:

```bash
cargo build --release
```

Run the game:

```bash
cargo run --release
```

## Testing

Run the test suite:

```bash
cargo test
```

The `procgen` crate has unit tests (universe, flow field). CI runs `cargo test` on every push/PR.

## Submitting changes

1. **Fork** the repo and create a branch from `main`.
2. **Make your changes.** Keep commits focused; use clear messages.
3. **Run `cargo test` and `cargo build --release`** so CI is likely to pass.
4. **Open a pull request** against `main`. Describe what you changed and why.
5. CI will run build and tests; address any feedback.

## Code style

- Use `cargo fmt` before committing.
- Prefer `?` and `if let` over `unwrap()`/`expect()` where errors can occur.
- New logic in the game crate should follow existing patterns (e.g. phase-based update dispatch, overlay in `render/overlay.rs`).

## Project layout

- `crates/game` – main game loop, FPS systems, state, render orchestration.
- `crates/renderer` – wgpu pipelines, meshes, shaders.
- `crates/procgen` – terrain, planets, flow field, bug meshes.
- `crates/physics` – Rapier3D, ragdoll.
- `docs/` – design notes, art direction, improvement backlog.

See `README.md` for a high-level overview and `docs/IMPROVEMENTS.md` for the improvement backlog.
