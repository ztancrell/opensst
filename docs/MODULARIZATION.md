# Modularization & Rebrand Plan

## Goals

1. **Modularize** the game crate so it’s easier to work on and ready to open-source.
2. **Rebrand** the project (e.g. to **OpenSST**) and prepare for GitHub release.

---

## Phase 1: Modularize the game crate

The workspace is already split into crates: `engine_core`, `renderer`, `physics`, `audio`, `input`, `procgen`, `game`. The main issue is the **game** crate: **`main.rs` is ~8,800 lines** and holds almost all app logic.

### 1.1 Extract rendering (`render` module) — DONE

- **Added** `crates/game/src/render.rs`.
- **Moved** the entire `GameState::render()` body into `render::run(state: &mut GameState) -> Result<()>`.
- **Kept** `fn render(&mut self) -> Result<()> { render::run(self) }` in `main.rs` so the rest of the codebase is unchanged.
- **Result:** ~2,850 lines in render.rs; all render logic lives in one module.

### 1.2 Extract gameplay update (`update` module) — DONE

- **Added** `crates/game/src/update.rs`.
- **Moved** the body of `GameState::update_gameplay()` into `update::gameplay(state: &mut GameState, dt: f32)`.
- **Result:** ~900 lines in update.rs; main.rs shrunk by ~900 lines.

### 1.3 Extract state types — DONE

- **Add** `crates/game/src/state.rs` (or `state/mod.rs`).
- **Move** into it:
  - `DebugSettings`, `GamePhase`, `ScreenShake`, `KillStreakTracker`
  - `Weather`, `WeatherState`, `WarpSequence`, `DropPhase`, `DropPodSequence`
  - `SquadPod`, `SquadDropSequence`, `ShipState`, `ClothFlag`
  - `InteriorNPC*`, `GalacticWarState`, `GameMessages`, etc.
- **Move** the corresponding `impl` blocks with them.
- **Leave** `GameState` and its largest `impl` blocks in `main.rs` until later, or move them into `state` once the above types are out.
- **Result:** Clearer separation of “game state types” from “application loop and high-level flow.”

### 1.4 Further split of `render` — IN PROGRESS

- Turned `render.rs` into `render/mod.rs` and added submodule placeholders:
  - `render/ship.rs` — ship interior, NPCs, war table (placeholder).
  - `render/planet.rs` — terrain, bugs, squad, extraction, sky, fleet (placeholder).
  - `render/overlay.rs` — **DONE.** HUD, debug info, game messages, galaxy map, war table UI, drop pod HUD, warp overlay, FPS HUD, kill feed, etc. (~1,150 lines extracted).
- **Result:** Overlay logic in `overlay::build(state, sw, sh)`; `render/mod.rs` reduced by ~1,150 lines.

### Summary (Phase 1)

| Step   | Action                    | Effect                          |
|--------|---------------------------|---------------------------------|
| 1.1    | Extract `render::run`     | ~2,850 lines out of `main.rs`   |
| 1.2    | Extract `update::gameplay`| ~2,800 lines out of `main.rs`  |
| 1.3    | Extract `state` types     | Cleaner types, smaller `main`  |
| 1.4    | Split `render` into submodules | Easier navigation          |

After 1.1 and 1.2, `main.rs` is on the order of ~3,000 lines (structs, init, event loop, and delegation).

---

## Phase 2: Rebrand (e.g. OpenSST)

Once the codebase is modularized:

1. **Rename workspace / binary**
   - In root `Cargo.toml`: optional workspace name or description.
   - In `crates/game/Cargo.toml`: binary name `opensst`, and package `name`/`description`.

2. **Window title and strings** — DONE
   - OpenSST branding in place: window title, `main()` banner, README, and in-game text.

3. **README and metadata**
   - Update README with project name (e.g. OpenSST), short description, “inspired by Starship Troopers,” build/run instructions, and license (e.g. MIT).

4. **License**
   - Add `LICENSE` (e.g. MIT) at repo root.

5. **Crate names (optional)**
   - Keep internal crate names (`game`, `procgen`, etc.) or rename for consistency (e.g. `opensst_game`). Low priority; can stay as-is for clarity.

---

## Order of work

1. **Now:** Phase 1.1 — extract render into `render.rs`.
2. **Next:** Phase 1.2 — extract update into `update.rs` (optional).
3. **Then:** Phase 1.3 — extract state types (optional).
4. **When ready:** Phase 2 — rebrand to OpenSST and prep for GitHub.
