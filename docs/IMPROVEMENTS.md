# OpenSST – Improvement Backlog (Huge List)

A broad list of things that could be improved. Use as a backlog; pick by priority.

---

## 1. Code structure & modularization

- [ ] **Shrink `main.rs`** (~5,750 lines) – Move `GameState` definition and large `impl` blocks into a dedicated module (e.g. `game_state.rs` or `state/game_state.rs`); keep `main.rs` as init + event loop + delegation.
- [ ] **Finish render split** – `render/ship.rs` and `render/planet.rs` are 3-line placeholders; move ship-interior and planet/terrain render logic out of `render/mod.rs` (2,547 lines) into these submodules.
- [ ] **Split `update.rs`** (1,313 lines) – Break into submodules by system: e.g. `update/player.rs`, `update/combat.rs`, `update/stratagems.rs`, `update/environment.rs`, `update/physics_cleanup.rs`.
- [ ] **Split `overlay.rs`** (1,070 lines) – Extract HUD, galaxy map, war table UI, drop pod HUD, kill feed, debug overlay into separate functions or submodules.
- [ ] **Extract init from `GameState::new`** – Universe/chunk/player/spawner init is a huge async block; split into smaller constructors or `init_*` helpers.
- [x] **Dedicate event-handling module** – Move `handle_window_event` and key/mouse handling out of `main.rs` into e.g. `events.rs` or `input_handling.rs`. (Done: `events.rs` with `impl GameState` for window/device events.)
- [ ] **Reduce `renderer.rs` size** (1,461 lines) – Split pipeline creation, sky, celestial, overlay rendering into submodules or helper files.

---

## 2. Audio

- [ ] **Wire up the audio crate** – `crates/audio` (Kira, spatial) is implemented and depended on but never instantiated; add `AudioSystem` to `GameState` and drive it from camera/listener.
- [ ] **Weapon sounds** – Fire, reload, empty click, per-weapon variants.
- [ ] **Footsteps** – Surface-aware (metal, dirt, water) or at least generic steps.
- [ ] **Bug sounds** – Idle, attack, death, spawn; spatial so direction matters.
- [ ] **Ambience** – Wind, distant bugs, ship hum, planet atmosphere.
- [ ] **Music** – Menu, ship, combat, extraction; optional so it can be disabled.
- [ ] **Master / SFX / music volume** – Expose in a future options screen.
- [ ] **Or remove audio dependency** – If you don’t plan to use it soon, drop the dependency and document “no audio yet” to avoid confusion.

---

## 3. Testing

- [x] **Unit tests for procgen** – Terrain height sampling, flow field, biome sampling, star system generation (deterministic with seed). (Added: universe determinism + flow_field tests.)
- [ ] **Unit tests for bug mesh** – `BugMeshGenerator` output (vertex count, bounds) for each bug type.
- [ ] **Physics helpers** – Capsule/collider helpers, ragdoll bone setup (if testable in isolation).
- [ ] **Mission/score logic** – Objective completion, wave progression, extraction conditions.
- [ ] **Save/load round-trip** – Serialize `GalacticWarState` (or full save), deserialize, assert equality.
- [ ] **Integration test** – `cargo test` that builds the game crate and runs a few seconds of main loop (if feasible without a window).
- [x] **CI runs tests** – Already in workflow; add at least one test so the step is meaningful. (Procgen tests added.)

---

## 4. Error handling & robustness

- [x] **Replace `unwrap()` / `expect()`** – Replaced in main.rs (deltas), render (bug_instances), update (water_level). A few may remain elsewhere.
- [x] **Window creation** – `event_loop.create_window(...).unwrap()` in `App::resumed`; handle failure and show a message instead of panicking.
- [ ] **Render errors** – Already logged in `handle_window_event`; consider a user-visible “Render error, check logs” message or fallback.
- [ ] **Save load failure** – If `opensst_save.ron` is corrupt or missing, document behavior and optionally show “No save found, starting fresh.”
- [ ] **Asset loading** – When you add file-based assets, use `Result` and clear error messages instead of panics.

---

## 5. Configuration & data-driven design

- [x] **Config file** – `config.ron` for window size, vsync, fullscreen, sensitivity. Loaded at startup; see `config.rs` and repo `config.ron`.
- [x] **Default window size** – Now in config; defaults 1280×720.
- [ ] **Keybindings** – Input is hardcoded (WASD, R, Q, etc.); add a keymap (struct or file) and look up by action so rebinding is possible later.
- [ ] **Magic numbers** – Replace scattered literals (e.g. spawn radius 15–20, flow field 100×100, chunk counts, render distances) with named constants or config.
- [ ] **Mission parameters** – Wave counts, kill targets, timers (e.g. “25 bugs”, “5:00”) are in code; move to data (RON/JSON) or mission definition structs.
- [ ] **Weapon stats** – Damage, fire rate, magazine size, etc. in one place (e.g. `weapons.rs` data or asset) for tuning without digging through logic.

---

## 6. Art, assets & visuals (from ART_DIRECTION.md)

- [ ] **Authored bug meshes** – Replace procedural `BugMeshGenerator` with one canonical mesh per type (Warrior, Charger, Spitter, Tanker, Hopper) for readable silhouettes.
- [ ] **Chitinous material** – Dark base, specular, rim lighting; weak points slightly different.
- [ ] **Bug hole mesh** – Organic crater instead of sphere; resin-like material.
- [ ] **Hive / egg cluster** – Organic blobs, not uniform spheres.
- [ ] **Rock variants** – 3–5 authored rocks per biome instead of generic primitives.
- [ ] **UCF structures** – Beveled edges, panel lines, military grey + rust accents.
- [ ] **Biome color palettes** – Align with ART_DIRECTION (desert, badlands, hive, volcanic, frozen, base).
- [ ] **Terrain normals/depth** – Stronger normal response so terrain doesn’t look flat.
- [ ] **LOD for bugs** – Simplified meshes or lower instance count at 80m+.
- [ ] **HUD styling** – Tactical green/amber, military monospace; high contrast.
- [ ] **Gore/death** – Chitin shards, acid splatter; more satisfying kill feedback.

---

## 7. Gameplay & content

- [ ] **Multiplayer / co-op** – Design doc defers it; networking, replication, host migration are large efforts.
- [ ] **Building / fortification** – Barricades, turrets, deployables (Bastion barricade exists; expand).
- [ ] **More weapons** – Grenades, melee (shovel is in; add more melee or grenade types).
- [ ] **Vehicle support** – Design doc roadmap item; vehicles need physics, controls, and art.
- [ ] **Procedural mission generation** – Objectives, spawn patterns, and difficulty curves from data or procgen.
- [ ] **More mission types** – Beyond Extermination, Bug Hunt, Hold the Line, Defense, Hive Destruction.
- [ ] **Class abilities** – Jetpack, barricade, ammo station, scan pulse, shield dome; balance and polish.
- [ ] **Stratagem variety** – Orbital strike, supply, reinforce, extraction exist; more call-ins (e.g. turret, smoke, orbital barrage).
- [ ] **Difficulty / accessibility** – Difficulty presets or sliders; FOV slider; optional aim assist; subtitle/indicator options.

---

## 8. UX & polish

- [ ] **Options / settings menu** – Volume, sensitivity, keybinds, graphics (vsync, resolution, fullscreen), FOV. (Config file exists; in-game UI not yet.)
- [x] **Pause menu** – Escape in Playing/InShip opens pause; Resume / Quit to main menu; cursor shown.
- [x] **Quit to main menu** – From pause menu (“Quit to main menu”); resets to main menu without exiting.
- [ ] **Loading indicator** – During initial load or planet switch so the player knows the game is working.
- [ ] **Death screen** – “You died” + stats + respawn/return to ship (if you add respawn) or “Mission failed.”
- [ ] **Tutorial or first-time hints** – Basic controls, stratagems, extraction; can be minimal (tooltips or one-time messages).
- [ ] **Rebindable keys** – Depends on config/keymap work above.
- [ ] **Mouse sensitivity** – Configurable; currently likely a constant in input/camera.
- [ ] **FOV slider** – Common ask for comfort and preference.

---

## 9. Performance & scalability

- [ ] **Profile and document** – Identify bottlenecks (CPU vs GPU, which systems); note in README or docs (e.g. “GPU-bound, instancing”).
- [ ] **Culling** – Frustum and distance culling for bugs, props, particles; verify nothing is drawn off-screen.
- [ ] **LOD** – As above; reduce cost for distant bugs and terrain.
- [ ] **Chunk streaming** – Ensure chunk load/unload doesn’t stall; consider background loading.
- [ ] **Particle limits** – Cap gore, tracers, muzzle flashes so low-end machines don’t die.
- [ ] **Physics step** – Already capped (e.g. 3 steps/frame); tune or expose for low framerate.
- [ ] **Reduce allocations in hot paths** – Reuse Vecs, avoid per-frame allocations in render/update where possible.

---

## 10. Platform & deployment

- [ ] **Windows build** – CI is Linux-only; add Windows job (and optionally macOS) so PRs don’t break other platforms.
- [ ] **macOS** – Wgpu supports Metal; document build steps and any caveats.
- [ ] **Release packaging** – Script or doc for building release binaries and what to ship (e.g. `opensst` + `assets/` + README).
- [ ] **Version number** – Bump and document in Cargo.toml / README / changelog when you tag releases.
- [ ] **Changelog** – CHANGELOG.md or release notes so users see what changed.

---

## 11. Documentation

- [ ] **README roadmap** – Fix “Save/load system” to reflect that save/load is done and documented.
- [ ] **Architecture overview** – High-level doc: crates, main loop, render passes, how state flows (for contributors).
- [ ] **CONTRIBUTING.md** – How to build, test, submit PRs, code style (if you want contributors).
- [ ] **CODE_OF_CONDUCT.md** – Optional but good for open source.
- [ ] **Inline docs** – `//!` and `///` on public types and key functions in `engine_core`, `renderer`, `physics`, `procgen`, and game modules.
- [ ] **Design doc updates** – Keep DESIGN_SC_HD2_ST.md and MODULARIZATION.md in sync with what’s implemented.
- [ ] **Comment “why” not “what”** – Replace obvious comments with rationale where non-obvious (e.g. “cap physics steps to avoid death spiral on lag”).

---

## 12. Cleanup & consistency

- [x] **Remove dead code** – e.g. `#[allow(dead_code)]` in `renderer/src/texture.rs`; either use or remove. (Renamed to `_texture`, removed allow.)
- [ ] **Naming consistency** – e.g. `serde_core` in errors might be a typo for `serde`; align crate and type names.
- [ ] **Unify game message style** – Mix of `info`, `warning`; decide when to use each and use consistently.
- [ ] **Stratagem/key names** – Use constants or enums for “B”, “N”, “R”, “V” so they’re in one place and rebindable later.
- [ ] **Logging** – Use `log` levels (debug/info/warn/error) consistently; avoid println in library code.

---

## 13. Security & save integrity

- [ ] **Validate save data** – Check version and structure before loading; reject or migrate old/corrupt saves.
- [ ] **Save location** – Document and consider platform-appropriate dirs (e.g. `~/.local/share/opensst/` on Linux) instead of CWD.
- [ ] **No sensitive data in saves** – Ensure nothing secret (e.g. tokens) is ever written to disk.

---

## 14. Accessibility & inclusivity

- [ ] **Color-blind options** – If HUD or markers rely heavily on color, add patterns or symbols.
- [ ] **Subtitles / indicators** – For any future voice or important audio cues.
- [ ] **Reduce motion** – Option to tone down screen shake, particles, or motion blur if added.
- [ ] **Font size / scaling** – If HUD text is small, allow scaling for readability.

---

## 15. Optional / nice-to-have

- [ ] **Replay or demo** – Record input and replay for debugging or sharing.
- [ ] **Screenshot / photo mode** – Hide HUD, freeze time, free camera (debug noclip is a start).
- [ ] **Stats screen** – Post-mission or in-menu: kills, accuracy, time played, etc.
- [ ] **Achievements** – Local-only is fine; list of goals and completion state.
- [ ] **Modding** – Long-term: script or data-driven mods (e.g. new weapons, missions) would require a clear data format and maybe a small API.

---

## Summary table (by effort)

| Area              | Examples                                                    |
|-------------------|-------------------------------------------------------------|
| Quick wins        | README roadmap fix, remove dead_code, add 1–2 unit tests   |
| Medium (days)     | Config file, options menu, wire audio, extract 1–2 modules  |
| Large (weeks)     | Shrink main.rs, finish render split, multiplayer, vehicles  |
| Ongoing           | Art pass, more content, performance profiling               |

Use this as a living backlog: add items, check off done, and reprioritize as you go.
