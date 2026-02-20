# OpenSST: Porting Guide to Godot 3D

This document outlines the **Starship Troopers–inspired FPS** (OpenSST) codebase so you can port it to **Godot 3D**. It covers vision, architecture, systems, data, and a concrete checklist for Godot.

---

## 1. Vision & Design (from DESIGN_SC_HD2_ST.md)

- **Concept:** Full universe sim (Star Citizen–style) + bug-killing FPS (Starship Troopers × Helldivers 2).
- **Loop:** Main menu → Ship (Roger Young) → War table (pick mission/planet) → Approach (cockpit) → EVA to drop pod → Drop to planet → FPS combat, stratagems, extraction → Back to ship; galactic war persists.
- **Principles:** First-person only, one life per drop (extract or die), fleet supports trooper (CAS, extraction, supply), cinematic military tone (’97 movie / Heinlein).

---

## 2. Current Tech Stack (Reference)

| Layer        | Technology |
|-------------|------------|
| Language    | Rust |
| Window/Input| winit |
| Graphics    | wgpu (WebGPU-style; Vulkan/DX12/Metal) |
| Rendering   | Custom pipelines: terrain (heightfield + water), shadows, sky, viewmodel, overlay, bloom |
| Physics     | Rapier3D (rigid bodies, heightfield terrain, raycasts, debris) |
| Audio       | kira |
| ECS         | hecs (entity-component system) |
| Procgen     | Custom (noise, seeds); deterministic terrain/biomes/universe |
| Save        | RON (`opensst_save.ron`), config in `config.ron` |

For Godot: replace wgpu with Godot’s rendering, Rapier with Godot Physics (or keep Rapier via GDExtension if you want), winit with Godot’s main loop and input.

---

## 3. Project Structure

```
bug_horde_engine/
├── crates/
│   ├── engine_core/   # Shared: Transform, Velocity, Health, Time, components
│   ├── renderer/      # wgpu instance, pipelines, camera, meshes, textures, overlay
│   ├── physics/       # Rapier3D wrapper: terrain heightfield, rigid bodies, raycast, debris
│   ├── audio/         # kira: sounds, music
│   ├── input/         # InputState (keyboard, mouse, actions)
│   ├── procgen/       # Universe, StarSystem, Planet, Biome, Terrain, FlowField, textures
│   └── game/          # Main game: state, update, render, all gameplay modules
├── docs/
│   ├── DESIGN_SC_HD2_ST.md   # Vision and roadmap
│   └── PORT_TO_GODOT.md     # This file
```

### Game crate modules (`crates/game/src/`)

| Module | Purpose |
|--------|--------|
| `main.rs` | App entry, event loop, `GameState` struct, chunk manager, ship interior, save/load |
| `state.rs` | `GamePhase`, `DropPhase`, weather, warp, prompts, supply crates, messages |
| `update.rs` | Single `gameplay(state, dt)` that advances all systems per frame |
| `config.rs` | `GameConfig`: window size, vsync, sensitivity; load/save `config.ron` |
| `render/mod.rs` | Frame render orchestration; calls overlay, etc. |
| `render/overlay.rs` | HUD text, crosshair, prompts, kill feed, damage numbers |
| `render/planet.rs` | Planet-from-space rendering |
| `render/ship.rs` | Ship interior / Roger Young rendering |
| `fps.rs` | FPS player, classes, combat (hitscan), mission state, mission types |
| `player.rs` | PlayerController (movement, jump, crouch, prone) |
| `weapons.rs` | WeaponType, Weapon, WeaponSystem, ammo, fire rate, projectiles (tracers visual only) |
| `viewmodel.rs` | First-person arms, shell casings (flying + grounded rigid bodies) |
| `bug.rs` | BugType, Bug, BugBundle, variants, death effects |
| `bug_entity.rs` | PhysicsBug, spawn, death phase, gore, tracks, EffectsManager |
| `authored_bug_meshes.rs` | Procedural mesh data: warrior, charger, spitter, tanker, hopper |
| `horde_ai.rs` | HordeAI: target player/squad, movement, separation |
| `spawner.rs` | BugSpawner: threat level, spawn around player |
| `destruction.rs` | Destructibles (rocks, hive, eggs, hazards, corpses, etc.), chain reactions |
| `authored_env_meshes.rs` | Environment mesh builders (rocks, hive mound, egg cluster, etc.) |
| `biome_features.rs` | Per-biome feature table (destructibles, hazards) |
| `biome_atmosphere.rs` | Per-biome volumetric particles (dust, spores) |
| `effects.rs` | Rain, snow, tracers, ambient dust |
| `smoke.rs` | Smoke grenades and smoke clouds |
| `artillery.rs` | Orbital barrage: shells, muzzle flashes, trails, grounded shells |
| `tac_fighter.rs` | Tac fighter CAS: phases, attack patterns, bombs |
| `extraction.rs` | Extraction dropship, phases, Roger Young parts |
| `fleet.rs` | Corvette/destroyer positions from orbit |
| `squad.rs` | Squad mates: spawn, movement, combat, drop sequence |
| `citizen.rs` | Earth citizens: schedule, waypoints (Earth only) |
| `dialogue.rs` | Dialogue trees, state (Earth NPCs) |
| `earth_territory.rs` | Earth-only: places, roads, buildings, waypoints, territory bounds |
| `hud.rs` | HUD config, crosshair, hit markers, damage numbers, kill feed |
| `skinny.rs` | Skinnies (optional humanoid faction on some planets) |

---

## 4. Game Loop & Phases

### Game phases (`GamePhase`)

- **MainMenu** — Continue / Universe Map / Quit; galaxy map can be open (select system, Enter = travel & board).
- **InShip** — Aboard Roger Young: bulletin, war table (contracts 1–5), walk to drop bay, interact prompts.
- **ApproachPlanet** — Cockpit view; after timer, EVA (zero-G) to drop pod; Enter or 6s to enter pod.
- **DropSequence** — Pod descent (Detach → SpaceFall → AtmosphericEntry → RetroBoost → Impact → Emerge).
- **Playing** — On planet: FPS combat, stratagems, extraction, mission objectives.
- **Victory / Defeat** — Mission end state.
- **Paused** — Pause menu (Resume / Quit to main menu).

### Drop pod phases (`DropPhase`)

Detach → SpaceFall → AtmosphericEntry → RetroBoost → Impact → Emerge. Each phase has altitude, velocity, timers, camera shake, and visual params (planet radius, atmosphere glow). Landing position is chosen and terrain is streamed before Impact.

### Per-frame update order (in `update::gameplay`)

1. Warp sequence (if active) — then return.
2. Environmental hazards (damage/slow).
3. Player input (movement, look, jump, crouch, prone).
4. Orbital time advance; universe position (on planet vs space).
5. Leave planet check (altitude); or in space: planet approach.
6. Terrain chunk streaming + pending rebuilds (artillery/deformation).
7. Earth: citizen AI, dialogue.
8. Squad: movement, combat.
9. Shell casings: sync from physics, settle to grounded, cull.
10. Tac fighters, artillery barrages, extraction dropship.
11. Bug spawner, horde AI, bug physics (ragdoll, death).
12. Combat system (player damage to bugs), mission state (kills, time, objective).
13. Stratagems (B/N/R/V), supply crates, smoke.
14. Viewmodel, screen shake, kill streaks.
15. Time of day, weather, rain/snow.
16. Destruction system (chain reactions, etc.).
17. Debug (noclip, god mode, kill all, teleport, time scale).

Rendering is separate: shadow pass, main scene (terrain, water, entities, sky, viewmodel), overlay (HUD, text), optional bloom/cinematic.

---

## 5. Core Systems (Porting to Godot)

### 5.1 Universe & galaxy

- **Universe:** 100 star systems, spiral disc, seed-based. `Universe::generate(seed, count)`; each system has `StarSystemEntry` (seed, name, position, star_type, visited).
- **Star system:** Generated on visit; contains `Star` + `bodies: Vec<Planet>`. System 0 = Sol; first planet = Earth (special-case `Planet::earth()`).
- **Galaxy map:** Main menu or in-ship (M). Select system; Enter = travel (warp) and board ship. Warp sequence: timer, then `arrive_at_system`, optionally return to ship interior.

**Godot:** Use a singleton or autoload for Universe; scenes or resources for system/planet entries; a dedicated GalaxyMap UI and a warp animation/cutscene.

### 5.2 Planets & terrain

- **Planet:** seed, name, classification (HiveWorld, Colony, Outpost, …), primary/secondary biome, danger_level, infestation, size (Small/Medium/Large/Massive), atmosphere, gravity_mult, day_length_mult, has_skinnies, etc. Earth is a fixed seed and special rules (no bugs, all biomes, territory, citizens).
- **Terrain:** Chunked heightfield. `ChunkManager` streams chunks around camera; each chunk has `TerrainData` (vertices, heightmap, water mesh), mesh, water_mesh, physics heightfield collider. Terrain uses fractal + ridged noise; height range crosses “sea level” so water fills valleys (Minecraft-style). Water level = 0.35 × height_scale.
- **Biomes:** `PlanetBiomes` samples at (x,z) for color and height_scale. Many biome types (Desert, Badlands, HiveWorld, Volcanic, Frozen, Mountain, Swamp, etc.); each has BiomeConfig (base_color, height_scale, roughness, prop_density, bug_density).

**Godot:** Use `HeightMapShape3D` or custom mesh from heightmap; chunk loading around player; shader or mesh for water plane. Biome can be a 2D/3D noise or texture lookup for color and scale. Planet metadata as Resource.

### 5.3 Missions & war table

- **Mission types:** Extermination (survive & extract), Bug Hunt (kill N), Hold the Line (survive T sec), Defense, Hive Destruction, Earth Visit.
- **War table:** Shows CONTRACT: [type] — [planet]. Keys 1–5 select planet; then deploy (approach → drop).
- **Mission state:** mission_type, bugs_killed, bugs_remaining, time_elapsed, peak_bugs_alive, is_failed, kill_target/time_target_secs, objective_complete.
- **Galactic war:** Liberation per planet, kills, extractions, major orders. Stored in save.

**Godot:** MissionType as enum or const; WarTable UI with list of contracts and key shortcuts; mission state as a node or autoload; persistence in save file.

### 5.4 FPS player & combat

- **Player:** FPS controller (WASD, mouse, jump, crouch, prone), health, class (e.g. Trooper, Medic), loadout. Collision vs terrain (sample_height, walkable_height), water (buoyancy, wading), corpse piles, Earth roads/buildings.
- **Combat:** Hitscan (raycast from camera); damage numbers, hit markers, kill feed. No physical projectiles for bullets (tracers are visual only).
- **Weapons:** Rifle, Shotgun, Sniper, Rocket, Flamethrower, MachineGun. Each has damage, fire_rate, reload_time, magazine_size, reserve, range, spread, projectile_count (shotgun). Cooldown and reload state.

**Godot:** CharacterBody3D or custom FPS controller; raycast for hitscan; weapon stats in resources; UI for ammo, crosshair, hit markers, kill feed.

### 5.5 Bugs

- **Bug types:** Warrior (melee), Charger (fast), Spitter (ranged), Tanker (heavy), Hopper (flying/jumping). Each has health, scale, color.
- **AI:** HordeAI: move toward player/squad, separation between bugs. Spawner: spawn around player by threat level.
- **Physics:** Rigid body + collider; on death, ragdoll/effects; corpse piles can be climbed. Shell casings are persistent rigid bodies (fly then settle).

**Godot:** One scene per bug type (or LOD); NavigationAgent or simple steering toward player; spawner node or manager; RigidBody3D or CharacterBody3D; death animation or ragdoll.

### 5.6 Stratagems & fleet

- **Stratagems:** Orbital Strike [B], Supply Drop [N], Reinforce [R], Extraction [V]. Cooldowns, smoke markers; Tac fighter CAS (bombs), artillery barrage (shells from orbit), extraction dropship (land, board, leave).
- **Fleet:** Corvettes/destroyers positions in orbit (for skybox or visible ships).

**Godot:** Input actions for B/N/R/V; timers for cooldowns; scenes for tac fighter, artillery impact, dropship; optional fleet nodes in sky.

### 5.7 Ship interior & EVA

- **Ship:** Roger Young interior (corridors, war table, drop bay). Walk to drop bay; interact to start approach.
- **Approach:** Cockpit view; after timer, SPACE = EVA. Zero-G: WASD + SPACE/Ctrl for up/down; E or 6s to enter pod.
- **Drop:** Pod sequence (see Drop phases above); terrain streamed; landing position resolved; then Emerge into Playing.

**Godot:** Ship as static or baked scene; trigger areas for war table, drop bay; Approach as a subscene or camera script; EVA with different movement script; drop as cutscene or controlled camera + physics.

### 5.8 Earth-specific

- **Territory:** Bounds, waypoints (cities, towns, farms), roads (mesh + colliders), building footprints (push player out).
- **Citizens:** NPCs with schedule, waypoints, time-of-day and weather; dialogue system.
- **Dialogue:** DialogueNode list per dialogue_id; choices 1–4, close with Esc.

**Godot:** Territory as data (e.g. JSON/Resource); NavigationRegion for roads; StaticBody or areas for buildings; NPC scenes with BehaviorTree or state machine; DialogueManager or custom UI.

### 5.9 Destructibles & environment

- **Types:** Rock, BugHole, HiveStructure, EggCluster, HazardPool, BonePile, SporeTower, CrashedShip, etc. Some have chain reactions (e.g. explode when destroyed).
- **Physics:** Some have rigid bodies; debris and gore chunks. Terrain deformation (crater, mound) triggers chunk mesh+collider rebuild.

**Godot:** Scenes per destructible type; health and signals for destruction; optional particle/effect on break; terrain heightmap updates if you support deformation.

### 5.10 Save & config

- **Save (`opensst_save.ron`):** `SaveData { universe_seed, current_system_idx, war_state }`. `war_state` includes planet liberation, kills, extractions, major orders. Load on startup; save on extraction (and optionally on quit).
- **Config (`config.ron`):** window_width, window_height, vsync, fullscreen, sensitivity.

**Godot:** Save as JSON or custom format; store in user:// or project path. Config in ProjectSettings or custom config file.

---

## 6. Data Reference

### Bug types (health, scale, role)

| Type    | Health | Scale (approx) | Role     |
|---------|--------|----------------|----------|
| Warrior | 50     | 1.0            | Melee    |
| Charger | 30    | 0.8×0.7×1.2    | Fast     |
| Spitter | 40     | 0.9            | Ranged   |
| Tanker  | 200    | 2.0            | Heavy    |
| Hopper  | 25     | 0.7×0.6×0.7    | Flying   |

### Weapon stats (damage, fire rate, mag, etc.)

| Weapon      | Damage | Fire rate | Mag | Reserve | Reload | Range | Spread | Projectiles |
|-------------|--------|-----------|-----|---------|--------|-------|--------|-------------|
| Rifle      | 25     | 10/s      | 30  | 180     | 2s     | 100   | 2°    | 1           |
| Shotgun    | 15     | 1.5/s     | 8   | 48      | 2.5s   | 30    | 8°    | 8           |
| Sniper     | 150    | 0.8/s     | 5   | 30      | 3s     | 500   | 0.5°  | 1           |
| Rocket     | 200    | 0.5/s     | 1   | 12      | 3.5s   | 200   | 0°    | 1           |
| Flamethrower| 5     | 30/s      | 100 | 300     | —      | 15    | 10°   | 1           |
| MachineGun | 18     | 18/s      | 200 | 600     | 4s     | 120   | 3°    | 1           |

### Mission types

- Extermination, Bug Hunt, Hold the Line, Defense, Hive Destruction, Earth Visit.

### Planet classifications

HiveWorld, Colony, Outpost, Frontier, Industrial, Research, WarZone, Abandoned.

### Biomes (examples)

Desert, Badlands, HiveWorld, Volcanic, Frozen, Toxic, Mountain, Swamp, Crystalline, Ashlands, Jungle, Wasteland, Tundra, SaltFlat, Storm, Fungal, Scorched, Ruins.

---

## 7. Rendering (High Level)

- **Shadow pass:** Directional light, shadow map (e.g. 2048²), terrain + main entities.
- **Main pass:** Terrain (heightfield mesh + water), destructibles, bugs, squad, citizens, sky, viewmodel (arms + weapon), particles (rain, snow, dust, tracers, smoke), artillery/tac/dropship.
- **Overlay:** HUD (crosshair, ammo, health, mission), prompts ([E] action), kill feed, damage numbers, messages, debug text.
- **Post:** Optional bloom, cinematic (e.g. letterbox).

Terrain and water are chunked; each chunk = one mesh + optional water mesh + one heightfield collider. Deformation (crater/mound) updates heightmap and triggers mesh+collider rebuild.

**Godot:** Use DirectionalLight3D with shadow; WorldEnvironment for sky and fog; mesh instances for terrain chunks; GPUParticles or CPUParticles for effects; SubViewport or CanvasLayer for HUD; optional post-processing for bloom.

---

## 8. Godot 3D Porting Checklist

- [ ] **Project setup:** Godot 3.x or 4.x; folder structure for scenes, scripts, resources.
- [ ] **Universe/galaxy:** Data or scenes for 100 systems; galaxy map UI; warp = load new system + ship scene.
- [ ] **Ship:** Roger Young interior scene; war table UI (contracts 1–5); drop bay trigger.
- [ ] **Approach & EVA:** Cockpit camera; EVA movement (zero-G); drop pod entry trigger.
- [ ] **Drop sequence:** State machine or timeline for Detach→Emerge; camera shake; terrain load at landing.
- [ ] **Planet/terrain:** Chunked heightmap (or GridMap); biome sampling; water at sea level; streaming around player.
- [ ] **Player:** FPS controller; health; class/loadout; collision with terrain, water, buildings.
- [ ] **Weapons:** Weapon resources (stats); hitscan raycast; tracers (visual); ammo UI.
- [ ] **Bugs:** Scenes per type; AI (move to player, separation); spawner; health, death, ragdoll/gore.
- [ ] **Stratagems:** B/N/R/V input; cooldowns; tac fighter, artillery, supply crate, extraction dropship scenes/scripts.
- [ ] **Squad:** Spawn after drop; follow/combat AI; simple meshes or placeholders.
- [ ] **Mission:** MissionType; objectives (kills, time); victory/defeat and extraction.
- [ ] **Galactic war:** Liberation, major orders; save/load (e.g. JSON) with universe_seed, current_system_idx, war_state.
- [ ] **Earth:** Territory data; roads; buildings; citizens + dialogue (optional for first port).
- [ ] **Destructibles:** Optional; at least rocks and simple breakables.
- [ ] **HUD:** Crosshair, ammo, health, mission text, prompts, kill feed, damage numbers.
- [ ] **Audio:** Music and SFX for combat, ship, drop, extraction (kira → AudioStreamPlayer).
- [ ] **Config:** Window, vsync, sensitivity; save to user config.

You can start with a single planet, one bug type, one weapon, and a minimal ship + drop + extraction loop, then add universe, war table, more weapons/bugs, and Earth/dialogue.

---

## 9. File Locations (Quick Reference)

| What | Where |
|------|--------|
| Vision & roadmap | `docs/DESIGN_SC_HD2_ST.md` |
| Game phases, drop, weather, prompts | `crates/game/src/state.rs` |
| Main game state, ship, save, chunk manager | `crates/game/src/main.rs` |
| Per-frame gameplay update | `crates/game/src/update.rs` |
| Universe, star system | `crates/procgen/src/universe.rs`, `star_system.rs` |
| Planet, biomes | `crates/procgen/src/planet.rs`, `biome.rs` |
| Terrain generation | `crates/procgen/src/terrain.rs` |
| Missions, FPS player, combat | `crates/game/src/fps.rs` |
| Weapons | `crates/game/src/weapons.rs` |
| Bugs, AI, spawner | `crates/game/src/bug.rs`, `horde_ai.rs`, `spawner.rs` |
| Destruction | `crates/game/src/destruction.rs` |
| Extraction, fleet | `crates/game/src/extraction.rs`, `fleet.rs` |
| Save format | `SaveData` in `main.rs`; path `opensst_save.ron` |

Good luck with the Godot 3D port.
