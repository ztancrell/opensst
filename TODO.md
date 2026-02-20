# OpenSST – TODO List

Generated from a project review. Check off as you go.

---

## Repo & release

- [x] **Add LICENSE file** – README says MIT but there is no `LICENSE` file in the repo. Add `LICENSE` (MIT text) at repo root.
- [x] **Clarify repo name in README** – Build instructions say `cd bug_horde_engine`; project is OpenSST/opensst. Either document the actual repo name or use “clone directory” / “project directory” so it works for any clone name.

---

## Code structure & tech debt

- [ ] **Shrink `main.rs`** – Event handling moved to `events.rs` (~250 lines). Still ~7,000 lines; move `GameState` + big `impl` blocks into a dedicated module so `main.rs` is mostly wiring.
- [ ] **Finish render split** – MODULARIZATION Phase 1.4: `render/ship.rs` and `render/planet.rs` were placeholders. Move ship-interior and planet/terrain render logic out of `render/mod.rs` into those submodules where it makes sense.
- [x] **Audio: use it or drop it** – Game crate no longer depends on `audio`; dependency commented out with note in Cargo.toml and README. Wire up when ready (see docs/IMPROVEMENTS.md).

---

## Testing & CI

- [x] **Add tests** – Unit tests added: procgen (terrain, flow field, universe), and game crate `state::tests` for `DebugSettings` (menu_item_count, toggle_selected). Add more for mission/score or save round-trip as needed.
- [x] **Add CI** – No `.github/workflows`. Add a workflow that runs `cargo build` and `cargo test` (when tests exist) on push/PR (e.g. `stable` Linux and maybe Windows/macOS) so PRs stay buildable and testable.

---

## Features (from README roadmap)

- [ ] Multiplayer networking
- [ ] Building / fortification system
- [ ] Additional weapons (grenades, melee)
- [ ] Vehicle support
- [ ] Procedural mission generation
- [x] Save/load – Design doc says done (`opensst_save.ron`); documented in README (Saves section).

---

## Polish & docs

- [x] **Document save file** – In README or a short “Saves” section: path (`opensst_save.ron`), what’s stored (seed, system, war state), and that it’s loaded on startup and saved on extraction.
- [x] **Debug-only messages** – Several `game_messages.info("[DEBUG] ...")` and similar strings. Consider gating behind `DebugSettings` or `#[cfg(debug_assertions)]` so release builds don’t show debug spam, or move to a dedicated debug overlay.
- [ ] **Art direction follow-up** – ART_DIRECTION.md recommends authored bug meshes and clearer silhouettes; treat as a backlog item when you want to push visual quality.

---

## Optional / later

- [x] **Optional default toolchain** – Add a `rust-toolchain.toml` (e.g. `channel = "stable"`) so contributors and CI use a consistent Rust version.
- [ ] **Contributing / code of conduct** – If you want outside contributors, add CONTRIBUTING.md and optionally a CODE_OF_CONDUCT.md and reference them from the README.
