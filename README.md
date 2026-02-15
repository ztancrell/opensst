# OpenSST

Open Starship Troopers — a fast, realistic Starship Troopers-inspired first-person shooter with **Euphoria-style physics-driven death animations**, built in Rust using wgpu for high-performance GPU rendering.

**Development:** This project was developed with assistance from [Cursor](https://cursor.com) AI (code generation, refactoring, and tooling).

## Features

### Core Engine
- **Rust-based** for maximum performance and memory safety
- **wgpu rendering** (Vulkan/DX12/Metal) with instanced rendering for massive bug hordes
- **Rapier3D physics** for realistic collision detection and ragdoll simulation
- **Entity-Component-System** architecture using hecs for efficient game logic

### Euphoria-Style Active Ragdoll Physics
- **Muscle-based ragdoll system** - Bugs don't just go limp, they fight to stay alive
- **Balance controller** - Bugs try to recover from hits before falling
- **Procedural death animations** - Every death is unique:
  - Launch phase with impact forces
  - Tumbling through the air
  - Twitching and spasming on the ground
  - Legs curling up in death throes
  - Final settling into death pose
- **Damage response** - Impacts affect muscle strength and control
- **Physics-driven corpses** that persist and interact with the world

### Procedural Bug Generation
- **5 Bug Types**: Warrior, Charger, Spitter, Tanker, Hopper
- **Procedural mesh generation** with bone hierarchies
- **Animated skeletons** with legs, mandibles, tails
- **Type-specific features**: wings for Hoppers, acid sacs for Spitters, heavy armor for Tankers
- **Collision capsules** auto-generated for physics

### Procedural Assets
- **Bug carapace shader** with iridescence and damage effects
- **Terrain shader** with triplanar mapping
- **Gore shader** for ichor splatters and drips
- **Particle shader** for explosions, debris, muzzle flash
- **Procedural sky** with dual suns and atmospheric scattering

#### Combat Features
- **Hit markers** with headshot and kill indicators
- **Floating damage numbers** for visual feedback
- **Kill feed** showing weapon and headshot kills
- **Gore system** with green ichor splatter
- **Muzzle flash** and bullet impact effects

#### Mission System
- **Wave-based survival** with increasing difficulty
- **10 waves** per mission
- **Bug variety increases** with wave progression
- **Score tracking** for kills and damage dealt

### Procedural World
- **Planet generation** with unique biomes
- **Terrain generation** using layered noise
- **Flow-field pathfinding** for horde AI
- **Dynamic spawning** around the player

## Controls

| Key | Action |
|-----|--------|
| **WASD** | Move |
| **Mouse** | Look around |
| **Left Click** | Fire weapon |
| **Right Click** | Aim down sights |
| **Shift** | Sprint |
| **Ctrl** | Crouch |
| **Space** | Jump |
| **R** | Reload |
| **1/2/Scroll** | Switch weapons |
| **Q** | Use ability |
| **Tab** | Toggle HUD |
| **Escape** | Release cursor |

### Debug Controls
| Key | Action |
|-----|--------|
| **F1** | Spawn 10 bugs |
| **F2** | Heal player |
| **F3** | Refill ammo |
| **F4** | Kill all bugs (test ragdolls) |

## Building

```bash
# Ensure Rust is installed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Linux dependencies (for audio)
# Fedora: sudo dnf install alsa-lib-devel
# Ubuntu: sudo apt install libasound2-dev

# From the repo root (after cloning), build and run
cargo run --release
```

## Saves

Progress is stored in **`opensst_save.ron`** in the current working directory (where you run the binary). It holds universe seed, current star system, and galactic war state (liberation, kills, extractions, major orders). The game loads it on startup and saves on successful extraction.

## Project Structure

```
opensst/
├── crates/
│   ├── engine_core/     # Core types, transforms, components
│   ├── renderer/        # wgpu rendering, cameras, meshes
│   │   └── shaders/     # WGSL shaders (PBR, bug, terrain, gore, sky)
│   ├── physics/         # Rapier3D integration, ragdoll system
│   ├── audio/           # Spatial audio (kira)
│   ├── input/           # Input handling
│   ├── procgen/         # Procedural generation (terrain, bugs, textures)
│   └── game/            # Main game loop, FPS systems
├── assets/              # Game assets
└── Cargo.toml           # Workspace configuration
```

## Architecture Highlights

### Physics-Driven Death System
```rust
// When a bug dies, it transitions through death phases:
pub enum DeathPhase {
    Alive,
    Launched,      // Impact force applied
    Falling,       // Ragdoll tumbling
    Twitching,     // Spasms on ground
    CurlingUp,     // Legs curl inward
    Dead,          // Settled, can be cleaned up
}

// Each phase has procedural animation overlays
fn get_death_animation(&self) -> (Vec3, Quat, f32) {
    // Returns position offset, rotation, and scale
    // for procedural death pose
}
```

### Active Ragdoll System
```rust
// Muscles apply forces between body parts
pub struct Muscle {
    pub body_a: usize,
    pub body_b: usize,
    pub max_force: f32,
    pub activation: f32,
    pub muscle_type: MuscleType,
}

// Balance controller fights to keep bugs upright
pub struct BalanceController {
    pub com_position: Vec3,
    pub recovery_urgency: f32,
    pub recovery_direction: Vec3,
}
```

### Instanced Rendering
All bugs are rendered with GPU instancing for maximum performance:
- Collect transforms and colors per bug type
- Upload to instance buffer
- Single draw call per bug mesh type

## Performance

- **10,000+ instance capacity** for massive hordes
- **60 FPS** on mid-range hardware
- **Efficient ECS queries** for AI and physics
- **Batched rendering** minimizes draw calls

## Roadmap

- [ ] Multiplayer networking
- [ ] Building/fortification system
- [ ] Additional weapons (grenades, melee)
- [ ] Vehicle support
- [ ] Procedural mission generation
- [x] Save/load system (see Saves section)

## Credits

Inspired by:
- **Starship Troopers** (1997 film)
- **Starship Troopers: Extermination** (game)
- **Helldivers 2** (visual style)
- **Euphoria/NaturalMotion** (active ragdoll physics)

Built with:
- [wgpu](https://wgpu.rs/) - GPU abstraction
- [Rapier3D](https://rapier.rs/) - Physics engine
- [hecs](https://github.com/Ralith/hecs) - ECS
- [glam](https://github.com/bitshifter/glam-rs) - Math

## License

MIT License
