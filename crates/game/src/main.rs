//! OpenSST - Open Starship Troopers: Extermination-inspired FPS with Euphoria-style physics

mod biome_atmosphere;
mod biome_features;
mod bug;
mod config;
mod render;
mod state;
mod update;

pub use state::{DropPhase, GameMessage, GameMessages, GamePhase, SupplyCrate};
use state::{
    ApproachFlightState, DebugSettings, DropPodSequence, InteractPrompt, KillStreakTracker,
    ScreenShake, SquadDropSequence, WarpSequence, Weather,
    DEPLOY_KEY, INTERACT_KEY,
};
mod authored_bug_meshes;
mod authored_env_meshes;
mod skinny;
mod bug_entity;
mod destruction;
mod effects;
mod fleet;
mod extraction;
mod fps;
mod horde_ai;
mod hud;
mod player;
mod smoke;
mod spawner;
mod squad;
mod artillery;
mod citizen;
mod dialogue;
mod earth_territory;
mod events;
mod tac_fighter;
mod viewmodel;
mod weapons;

use anyhow::Result;
use engine_core::{Health, Lifetime, Time, Transform, Velocity};
use rand::{Rng, SeedableRng};
use glam::{DVec3, Quat, Vec3};
use hecs::{Entity, World};
use input::InputState;
use physics::PhysicsWorld;
use procgen::{BiomeType, FlowField, Planet, PlanetBiomes, PlanetClassification, StarSystem, Universe, TerrainConfig, VoxelChunk};
use rapier3d::prelude::ColliderHandle;
use renderer::{Camera, CelestialBodyInstance, InstanceData, Mesh, OverlayTextBuilder, Renderer, DEFORM_HALF_SIZE, DEFORM_TEXTURE_SIZE};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::KeyCode,
    window::{CursorGrabMode, Window, WindowId},
};

use biome_atmosphere::{AtmoParticleKind, BiomeAtmosphere};
use bug::{Bug, BugBundle, BugType, VariantDeathEffect};
use skinny::{Skinny, SkinnyType};
use bug_entity::{DeathPhase, EffectsManager, GoreType, PhysicsBug, TrackKind, update_bug_physics};
use destruction::{
    AbandonedOutpost, BiomeDestructible, BiomeLandmark, BonePile, BugCorpse, BugHole, BurnCrater,
    CachedRenderData, ChainEffect, ChainReaction, CrashedShip, Debris, Destructible, DestructiblePhysics,
    DestructionSystem, EggCluster, EnvironmentProp, EnvironmentalHazard, HazardPool, HazardType,
    HiveStructure, HiveNest, HiveTunnelEntrance, LandmarkType, Rock, SporeTower,
    ENV_MESH_GROUP_COUNT, MESH_GROUP_ROCK, MESH_GROUP_BUG_HOLE, MESH_GROUP_HIVE_MOUND,
    MESH_GROUP_EGG_CLUSTER, MESH_GROUP_PROP_SPHERE, MESH_GROUP_CUBE,
    MESH_GROUP_LANDMARK, MESH_GROUP_HAZARD, MESH_GROUP_HIVE_CAVE_ENTRANCE,
};
use biome_features::get_biome_feature_table;
use effects::{AmbientDust, DustShape, RainDrop, SnowParticle, TracerProjectile};
use extraction::{ExtractionDropship, ExtractionMessage, ExtractionPhase, roger_young_parts};
use horde_ai::apply_separation;
use fps::{BugCombatSystem, CombatSystem, FPSPlayer, MissionState, PlayerClass};
use horde_ai::HordeAI;
use hud::HUDSystem;
use smoke::{SmokeCloud, SmokeGrenade, SmokeParticle};
use spawner::BugSpawner;
use citizen::{despawn_citizens, spawn_earth_citizens, update_citizens, Citizen};
use squad::{despawn_squad, spawn_squad, update_squad_combat, update_squad_movement, SquadMate, SquadMateKind};
use dialogue::DialogueState;
use artillery::{ArtilleryBarrage, ArtilleryMuzzleFlash, ArtilleryShell, ArtilleryTrailParticle, GroundedArtilleryShell};
use tac_fighter::{TacBomb, TacFighter, TacFighterPhase};
use viewmodel::{GroundedShellCasing, ShellCasing, ShellCasingType, ViewmodelAnimState};
use weapons::{WeaponSystem, WeaponType};

/// Main game state with full Euphoria-style physics integration
pub struct GameState {
    // Core systems
    world: World,
    time: Time,
    input: InputState,
    physics: PhysicsWorld,

    // Renderer
    renderer: Renderer,
    camera: Camera,

    // Authored STE-style bug meshes
    bug_meshes: AuthoredBugMeshes,
    environment_meshes: EnvironmentMeshes,
    gore_mesh: Mesh,
    particle_mesh: Mesh,
    tracer_mesh: Mesh,      // Proper bullet-shaped diamond mesh
    flash_mesh: Mesh,       // Multi-pointed star for muzzle flashes
    billboard_mesh: Mesh,   // Camera-facing quad for billboard particles

    /// Heightfield for terrain deformation (footprints in snow/sand). 256x256 f32s, world follows player.
    deformation_buffer: Vec<f32>,
    /// Snow accumulation (weather-driven). 256x256 f32s, same layout as deformation; center of 128m tile.
    snow_accumulation_buffer: Vec<f32>,
    snow_accumulation_origin: (f32, f32),

    // Effects
    effects: EffectsManager,

    // FPS Systems
    player: FPSPlayer,
    combat: CombatSystem,
    bug_combat: BugCombatSystem,
    hud: HUDSystem,
    mission: MissionState,

    // Game systems
    horde_ai: HordeAI,
    spawner: BugSpawner,
    weapon_system: WeaponSystem,

    // Terrain (infinite chunked)
    chunk_manager: ChunkManager,
    planet: Planet,

    // Universe navigation
    universe_seed: u64,
    universe: Universe,
    current_system: StarSystem,
    current_system_idx: usize,
    current_planet_idx: Option<usize>,   // None = in open space
    universe_position: DVec3,            // true position in solar system coords
    orbital_time: f64,                   // drives planet orbits
    /// Real-time seconds since game start (or scaled). Drives planet rotation for day/night.
    universe_time_sec: f64,

    // Galaxy map
    galaxy_map_open: bool,
    galaxy_map_selected: usize,
    warp_sequence: Option<WarpSequence>,
    /// Galaxy position when warp started (for FTL interpolation so Roger Young "moves" to target system).
    warp_start_galaxy_position: Option<DVec3>,
    /// When true, warp completion returns to ship interior (FTL from war table) instead of space.
    warp_return_to_ship: bool,

    // Drop pod deployment
    drop_pod: Option<DropPodSequence>,
    /// Squad drop pods coming from orbit after the player lands (player can look up and see them).
    squad_drop_pods: Option<SquadDropSequence>,
    ship_state: Option<ShipState>,
    /// Planet we're deploying to (set when starting approach, used when starting drop).
    deploy_planet_idx: Option<usize>,
    /// Approach phase: cockpit view timer (sec); when flyable, this is None and approach_flight_state is Some.
    approach_timer: f32,
    /// Approach flight: flyable craft position/velocity (Star Citizen piloting).
    approach_flight_state: Option<ApproachFlightState>,
    // Galactic War Table
    war_state: GalacticWarState,

    // Earth settlement (citizens + dialogue) — only when planet.name == "Earth"
    settlement_center: Option<Vec3>,
    /// Global waypoints for territory (cities, towns, farms). Set when landing on Earth.
    earth_waypoints: Option<Vec<(f32, f32)>>,
    /// Roads and walking paths mesh (drawn when on Earth). Built at landing.
    earth_roads_mesh: Option<Mesh>,
    /// Road segment colliders (removed when leaving Earth).
    earth_road_colliders: Vec<ColliderHandle>,
    /// Building cuboid colliders on Earth (removed when leaving Earth).
    earth_building_colliders: Vec<ColliderHandle>,
    dialogue_state: DialogueState,
    /// Current interact prompt (war table, drop bay, talk to NPC). Set each frame; overlay draws same style for all.
    pub(crate) interaction_prompt: Option<InteractPrompt>,

    // Sky and weather (dynamic)
    time_of_day: f32,       // 0 = dawn, 0.25 = noon, 0.5 = dusk, 0.75 = night
    weather: Weather,
    rain_drops: Vec<RainDrop>,
    snow_particles: Vec<SnowParticle>,

    // Destructible environment
    destruction: DestructionSystem,

    // On-screen messages (replaces console logging)
    game_messages: GameMessages,

    // Game state
    phase: GamePhase,
    /// Main menu selection: 0 = Continue/Play, 1 = Universe Map, 2 = Quit.
    main_menu_selected: usize,
    /// When true, main menu is showing the galaxy map; Enter = travel to selected system and board ship.
    main_menu_galaxy_open: bool,
    /// True if a saved campaign was loaded (show "Continue" instead of "Play").
    has_save: bool,
    /// When Paused: 0 = Resume, 1 = Quit to main menu.
    pause_menu_selected: usize,
    /// Phase to restore when resuming from Paused.
    previous_phase: Option<GamePhase>,
    running: bool,
    /// Smoothed delta time for consistent motion (avoids laggy feel from frame spikes).
    smoothed_dt: f32,

    // Stats
    total_gore_spawned: u32,
    physics_bodies_active: u32,

    // Visible tracer projectiles (visual only; damage is hitscan)
    tracer_projectiles: Vec<TracerProjectile>,

    // Developer debug settings
    debug: DebugSettings,

    // FPS player controller state
    player_velocity: Vec3,
    player_grounded: bool,
    /// Movement speed multiplier from environmental hazards (quicksand, blizzard, etc.). 1.0 = normal.
    hazard_slow_multiplier: f32,

    // Ground tracks (footprints in snow/sand — Dune / Helldivers 2 style)
    last_player_track_pos: Option<Vec3>,
    ground_track_bug_timer: f32,
    squad_track_last: HashMap<Entity, Vec3>,
    /// Seconds until next shovel dig allowed (hold LMB to dig repeatedly).
    shovel_dig_cooldown: f32,

    // Cinematic effects
    screen_shake: ScreenShake,
    camera_recoil: f32,               // Current recoil pitch offset (decays back to 0)
    crouch_hold_timer: f32,           // Hold Ctrl to go prone (Helldivers 2 style)
    kill_streaks: KillStreakTracker,
    ambient_dust: AmbientDust,
    biome_atmosphere: BiomeAtmosphere, // Per-biome volumetric particles

    // Viewmodel animation
    viewmodel_anim: ViewmodelAnimState,
    shell_casings: Vec<ShellCasing>,
    grounded_shell_casings: Vec<GroundedShellCasing>,

    // Smoke grenades
    smoke_grenades: Vec<SmokeGrenade>,   // In-flight grenades
    smoke_clouds: Vec<SmokeCloud>,       // Active smoke clouds
    smoke_grenade_cooldown: f32,         // Cooldown timer

    // Tac Fighter fleet — multiple fighters can be on station (Starship Troopers style)
    tac_fighters: Vec<TacFighter>,
    tac_bombs: Vec<TacBomb>,
    tac_fighter_cooldown: f32,           // Time until next fighter can be requested
    tac_fighter_available: bool,         // Whether CAS is available

    // Orbital artillery — red smoke designates; 6 shells fired one after another; rearm like tac fighters
    artillery_shells: Vec<ArtilleryShell>,
    artillery_muzzle_flashes: Vec<ArtilleryMuzzleFlash>,
    artillery_trail_particles: Vec<ArtilleryTrailParticle>,
    grounded_artillery_shells: Vec<GroundedArtilleryShell>,
    artillery_barrage: Option<ArtilleryBarrage>,
    artillery_cooldown: f32,

    // Stratagems (Helldivers 2 style): B = Orbital Strike, N = Supply Drop, R = Reinforce
    supply_crates: Vec<SupplyCrate>,
    supply_drop_cooldown: f32,
    supply_drop_smoke: Vec<SmokeCloud>,   // Smoke at each supply drop LZ (same style as LZ green)
    reinforce_cooldown: f32,
    reinforce_smoke: Option<SmokeCloud>,   // Smoke when Reinforce is called
    orbital_strike_smoke: Option<SmokeCloud>, // Red smoke when B is pressed (like tac designator)

    // Extraction dropship
    extraction: Option<ExtractionDropship>,
    extraction_cooldown: f32,            // Cooldown between extraction calls
    extraction_squadmates_aboard: Vec<Entity>, // NO TROOPER LEFT BEHIND — squadmates picked up with player
    extraction_collider: Option<ColliderHandle>, // Hull collider for player/bug collision
    lz_smoke: Option<SmokeCloud>,        // Green smoke marker at LZ

    /// Mission type for next drop (set at war table; used when drop launches).
    next_mission_type: fps::MissionType,

    /// Base defense mode (UCF planet + Hold the Line / Defense): center and inner radius.
    /// Bugs spawn outside this perimeter; player and squad spawn on walls.
    defense_base: Option<(Vec3, f32)>,

}

/// State for the ship interior phase before deploying.
struct ShipState {
    /// Timer for ambient animations.
    timer: f32,
    /// Whether the player has pressed deploy.
    deploy_requested: bool,
    /// Index of the planet to drop to (selected on war table).
    target_planet_idx: usize,
    /// Mission type for this drop (cycle at war table with 1/2/3).
    selected_mission_type: fps::MissionType,
    /// Is the player currently interacting with the war table?
    war_table_active: bool,
    /// Position of the holographic war table in ship-local space.
    war_table_pos: Vec3,
    /// Position of the drop pod bay trigger.
    drop_bay_pos: Vec3,
    /// UCF flag (port wall).
    ucf_flag: ClothFlag,
    /// Mobile Infantry flag (starboard wall).
    mi_flag: ClothFlag,
}

/// Cloth-simulated flag using Verlet integration with distance constraints.
/// The flag is a grid of particles; the top row is pinned to a pole.
struct ClothFlag {
    /// Particle positions (row-major: cols across, rows down).
    positions: Vec<Vec3>,
    /// Previous positions (for Verlet integration).
    prev_positions: Vec<Vec3>,
    /// Which particles are pinned (top row attached to pole).
    pinned: Vec<bool>,
    /// Grid dimensions.
    cols: usize,
    rows: usize,
    /// Rest distance between adjacent particles.
    rest_dist: f32,
    /// World-space origin of top-left pin.
    origin: Vec3,
    /// Direction the flag hangs away from the wall (unit vector).
    hang_dir: Vec3,
    /// Direction along the pole (unit vector, "right" for the flag).
    pole_dir: Vec3,
    /// Base color grid: each cell [row][col] has an RGBA color.
    colors: Vec<[f32; 4]>,
    /// Accumulated time for wind variation.
    wind_time: f32,
}

impl ClothFlag {
    /// Create a new flag.
    /// `origin`: world position of top-left attachment point.
    /// `pole_dir`: unit vector along the pole (direction of increasing column).
    /// `hang_dir`: unit vector the flag hangs toward (away from wall, perpendicular to pole).
    /// `width`, `height`: physical dimensions in meters.
    /// `cols`, `rows`: grid resolution.
    /// `colors`: flat vec of [r,g,b,a] per cell (cols * rows).
    fn new(
        origin: Vec3,
        pole_dir: Vec3,
        hang_dir: Vec3,
        width: f32,
        height: f32,
        cols: usize,
        rows: usize,
        colors: Vec<[f32; 4]>,
    ) -> Self {
        let rest_dist_x = width / (cols - 1).max(1) as f32;
        let rest_dist_y = height / (rows - 1).max(1) as f32;
        let rest_dist = rest_dist_x.min(rest_dist_y);
        let down = Vec3::new(0.0, -1.0, 0.0);

        let mut positions = Vec::with_capacity(cols * rows);
        let mut pinned = Vec::with_capacity(cols * rows);

        for r in 0..rows {
            for c in 0..cols {
                let x_off = c as f32 * rest_dist_x;
                let y_off = r as f32 * rest_dist_y;
                let pos = origin + pole_dir * x_off + down * y_off;
                positions.push(pos);
                // Pin the top row
                pinned.push(r == 0);
            }
        }

        let prev_positions = positions.clone();

        Self {
            positions,
            prev_positions,
            pinned,
            cols,
            rows,
            rest_dist: rest_dist_x, // horizontal rest distance
            origin,
            hang_dir,
            pole_dir,
            colors,
            wind_time: 0.0,
        }
    }

    /// Step the cloth simulation.
    fn update(&mut self, dt: f32) {
        let dt = dt.min(0.033); // cap to prevent explosion
        self.wind_time += dt;

        let gravity = Vec3::new(0.0, -4.0, 0.0);
        let damping = 0.98;

        // Wind: oscillating gusts in the hang direction with turbulence
        let wind_base = 3.0 + (self.wind_time * 1.2).sin() * 2.0;
        let wind_gust = (self.wind_time * 3.7).sin() * (self.wind_time * 0.8).cos() * 1.5;
        let wind_force = self.hang_dir * (wind_base + wind_gust);
        // Cross-wind turbulence
        let cross = self.pole_dir * (self.wind_time * 2.3).sin() * 0.8;

        // Verlet integration
        for i in 0..self.positions.len() {
            if self.pinned[i] { continue; }

            let pos = self.positions[i];
            let prev = self.prev_positions[i];
            let vel = (pos - prev) * damping;

            // Per-particle wind variation based on grid position
            let r = i / self.cols;
            let c = i % self.cols;
            let wave = ((c as f32 * 0.5 + self.wind_time * 4.0).sin() * 0.3
                + (r as f32 * 0.7 + self.wind_time * 2.5).cos() * 0.2)
                * self.hang_dir;

            let accel = gravity + wind_force + cross + wave;
            let new_pos = pos + vel + accel * dt * dt;

            self.prev_positions[i] = pos;
            self.positions[i] = new_pos;
        }

        // Distance constraints (iterate multiple times for stiffness)
        let rest_x = self.rest_dist;
        let rest_y = (self.positions.len() > self.cols)
            .then(|| {
                let origin_y_span = self.positions[0].y - self.positions[self.cols].y;
                origin_y_span.abs().max(rest_x)
            })
            .unwrap_or(rest_x);

        for _iter in 0..5 {
            // Horizontal constraints
            for r in 0..self.rows {
                for c in 0..(self.cols - 1) {
                    let i = r * self.cols + c;
                    let j = i + 1;
                    self.apply_constraint(i, j, rest_x);
                }
            }
            // Vertical constraints
            for r in 0..(self.rows - 1) {
                for c in 0..self.cols {
                    let i = r * self.cols + c;
                    let j = i + self.cols;
                    self.apply_constraint(i, j, rest_y);
                }
            }
        }
    }

    fn apply_constraint(&mut self, i: usize, j: usize, rest: f32) {
        let delta = self.positions[j] - self.positions[i];
        let dist = delta.length();
        if dist < 0.0001 { return; }
        let correction = delta * (1.0 - rest / dist) * 0.5;

        if !self.pinned[i] { self.positions[i] += correction; }
        if !self.pinned[j] { self.positions[j] -= correction; }
    }

    /// Generate renderable quad instances for this flag.
    /// Returns a list of (matrix, color) for each cloth cell.
    fn render_instances(&self) -> Vec<(glam::Mat4, [f32; 4])> {
        let mut instances = Vec::new();

        for r in 0..(self.rows - 1) {
            for c in 0..(self.cols - 1) {
                let tl = self.positions[r * self.cols + c];
                let tr = self.positions[r * self.cols + c + 1];
                let bl = self.positions[(r + 1) * self.cols + c];
                let br = self.positions[(r + 1) * self.cols + c + 1];

                // Quad center
                let center = (tl + tr + bl + br) * 0.25;
                // Approximate quad normal
                let edge_h = (tr - tl + br - bl) * 0.5;
                let edge_v = (bl - tl + br - tr) * 0.5;
                let normal = edge_h.cross(edge_v);
                let normal_len = normal.length();
                if normal_len < 0.0001 { continue; }

                // Scale from edge lengths
                let sx = ((tr - tl).length() + (br - bl).length()) * 0.5;
                let sy = ((bl - tl).length() + (br - tr).length()) * 0.5;

                // Build rotation from edges
                let right = edge_h.normalize_or_zero();
                let up_approx = edge_v.normalize_or_zero();
                let fwd = right.cross(up_approx).normalize_or_zero();
                let corrected_up = fwd.cross(right).normalize_or_zero();

                let rot_mat = glam::Mat3::from_cols(right, corrected_up, fwd);
                let rotation = Quat::from_mat3(&rot_mat);

                let matrix = glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(sx, 0.01, sy), // flat quad: X-width, Z-height, Y-thin
                    rotation,
                    center,
                );

                // Cell color from the pattern
                let color_idx = r * (self.cols - 1) + c;
                let color = if color_idx < self.colors.len() {
                    self.colors[color_idx]
                } else {
                    [0.5, 0.5, 0.5, 1.0]
                };

                instances.push((matrix, color));
            }
        }

        instances
    }
}

/// Generate the color pattern for the United Citizen Federation flag (franchise).
/// Green field with gold/white eagle emblem — the UCF green from Starship Troopers.
fn ucf_flag_colors(cols: usize, rows: usize) -> Vec<[f32; 4]> {
    let cell_cols = cols - 1;
    let cell_rows = rows - 1;
    let mut colors = Vec::with_capacity(cell_cols * cell_rows);

    // UCF green (franchise): deep green field
    let green = [0.05, 0.35, 0.12, 1.0];
    let green_light = [0.08, 0.42, 0.18, 1.0];
    let gold = [0.82, 0.68, 0.12, 1.0];
    let white = [0.95, 0.95, 0.92, 1.0];

    let cx = cell_cols as f32 / 2.0;
    let cy = cell_rows as f32 / 2.0;

    for r in 0..cell_rows {
        for c in 0..cell_cols {
            let rf = r as f32;
            let cf = c as f32;
            let dx = (cf - cx).abs() / cx;
            let dy = (rf - cy).abs() / cy;

            // Central emblem: eagle (simplified — wings and body)
            let diamond = dx + dy;
            let is_sword = dx < 0.07 && dy < 0.65;           // vertical center (body)
            let is_crossguard = dy < 0.12 && dx < 0.32 && rf > cy * 0.45 && rf < cy * 0.85;
            let is_wing = (dy - dx * 0.55).abs() < 0.11 && dy < 0.5 && dx < 0.55;
            let is_border = r == 0 || r == cell_rows - 1 || c == 0 || c == cell_cols - 1;
            let is_border2 = r == 1 || r == cell_rows - 2 || c == 1 || c == cell_cols - 2;

            let color = if is_border {
                gold
            } else if is_border2 {
                [0.45, 0.55, 0.20, 1.0] // darker green inner border
            } else if is_sword || is_crossguard {
                white
            } else if is_wing {
                gold
            } else if diamond < 0.48 {
                green_light
            } else {
                green
            };

            colors.push(color);
        }
    }
    colors
}

/// Generate the color pattern for the Mobile Infantry flag (Starship Troopers film).
/// Yellow and red with eagle emblem — the MI flag from the movie.
fn mi_flag_colors(cols: usize, rows: usize) -> Vec<[f32; 4]> {
    let cell_cols = cols - 1;
    let cell_rows = rows - 1;
    let mut colors = Vec::with_capacity(cell_cols * cell_rows);

    let red = [0.72, 0.08, 0.05, 1.0];
    let red_dark = [0.50, 0.05, 0.03, 1.0];
    let yellow = [0.98, 0.85, 0.12, 1.0];
    let yellow_dark = [0.88, 0.72, 0.08, 1.0];

    let cx = cell_cols as f32 / 2.0;
    let cy = cell_rows as f32 / 2.0;

    for r in 0..cell_rows {
        for c in 0..cell_cols {
            let rf = r as f32;
            let cf = c as f32;
            let dx = (cf - cx).abs() / cx;
            let dy = (rf - cy).abs() / cy;

            // Eagle emblem (film MI flag): simplified wings and body in yellow on red
            let is_sword = dx < 0.06 && dy < 0.6;             // vertical center (body)
            let is_crossguard = dy < 0.10 && dx < 0.30 && rf > cy * 0.4 && rf < cy * 0.82;
            let is_wing = (dy - dx * 0.5).abs() < 0.10 && dy < 0.48 && dx < 0.52;
            let diamond = dx + dy;
            let is_center = diamond < 0.45;

            // Yellow border / stripe
            let is_border = r == 0 || r == cell_rows - 1 || c == 0 || c == cell_cols - 1;
            let is_border2 = r == 1 || r == cell_rows - 2 || c == 1 || c == cell_cols - 2;

            let color = if is_border {
                yellow
            } else if is_border2 {
                yellow_dark
            } else if is_sword || is_crossguard {
                yellow
            } else if is_wing {
                yellow
            } else if is_center {
                red_dark
            } else {
                red
            };

            colors.push(color);
        }
    }
    colors
}

/// A piece of the Roger Young interior geometry.
struct ShipInteriorPart {
    pos: Vec3,
    scale: Vec3,
    color: [f32; 4],
    /// 0 = rock (angular), 1 = sphere, 2 = flash (glow/emissive)
    mesh_type: u8,
}

/// NPCs in the Roger Young interior: Fleet crew, Mobile Infantry, Marauder suits, and Johnny Rico.
#[derive(Clone, Copy)]
pub(crate) enum InteriorNPCKind {
    /// Fleet personnel — ship crew, naval gray uniforms (1997 ST / Federation aesthetic).
    Fleet,
    /// Fleet officer / pilot — same as Fleet but with cap (lived-in variety).
    FleetOfficer,
    /// Mobile Infantry — troopers in tan/sand armor, black visor (film-accurate).
    MobileInfantry,
    /// Marauder — powered suit units, industrial Federation gray, amber HUD.
    Marauder,
    /// Johnny Rico — squad leader; tan armor, red rank accent.
    JohnnyRico,
}

/// One NPC placed in the ship interior. Position in CIC/local space; facing = Y-axis yaw (radians).
/// color_tint multiplies RGB for lived-in variation (e.g. [0.95, 0.97, 1.0] = slight blue, [0.9, 0.88, 0.92] = worn).
pub(crate) struct InteriorNPC {
    pub(crate) position: Vec3,
    pub(crate) facing_yaw_rad: f32,
    pub(crate) kind: InteriorNPCKind,
    pub(crate) color_tint: [f32; 3],
    /// Display name for nametag when player looks at this NPC
    pub(crate) name: &'static str,
    /// Index into dialogue_content (dialogue.rs): 5=Fleet, 6=FleetOfficer, 7=MobileInfantry, 8=Marauder, 9=Johnny Rico.
    pub(crate) dialogue_id: usize,
}

/// One primitive of an NPC (local offset from NPC base, scale, color, mesh_type).
pub(crate) struct InteriorNPCPart {
    local_offset: Vec3,
    scale: Vec3,
    color: [f32; 4],
    mesh_type: u8,
}

pub(crate) fn interior_npc_parts(kind: InteriorNPCKind) -> Vec<InteriorNPCPart> {
    use InteriorNPCKind::*;
    // Fleet: naval gray/charcoal — 1997 ST ship crew, utilitarian
    let fleet = vec![
        InteriorNPCPart { local_offset: Vec3::new(0.0, 1.55, 0.0), scale: Vec3::new(0.18, 0.18, 0.18), color: [0.38, 0.40, 0.44, 1.0], mesh_type: 1 }, // head
        InteriorNPCPart { local_offset: Vec3::new(0.0, 1.05, 0.0), scale: Vec3::new(0.22, 0.35, 0.12), color: [0.16, 0.18, 0.22, 1.0], mesh_type: 0 }, // torso
        InteriorNPCPart { local_offset: Vec3::new(0.0, 0.4, 0.0), scale: Vec3::new(0.12, 0.4, 0.1), color: [0.12, 0.14, 0.18, 1.0], mesh_type: 0 },
    ];
    // Fleet officer: same + cap (Federation officer/pilot look)
    let fleet_officer = vec![
        InteriorNPCPart { local_offset: Vec3::new(0.0, 1.55, 0.0), scale: Vec3::new(0.18, 0.18, 0.18), color: [0.36, 0.38, 0.42, 1.0], mesh_type: 1 },
        InteriorNPCPart { local_offset: Vec3::new(0.0, 1.72, 0.0), scale: Vec3::new(0.28, 0.06, 0.22), color: [0.08, 0.09, 0.11, 1.0], mesh_type: 0 }, // cap
        InteriorNPCPart { local_offset: Vec3::new(0.0, 1.05, 0.0), scale: Vec3::new(0.22, 0.35, 0.12), color: [0.18, 0.20, 0.24, 1.0], mesh_type: 0 },
        InteriorNPCPart { local_offset: Vec3::new(0.0, 0.4, 0.0), scale: Vec3::new(0.12, 0.4, 0.1), color: [0.14, 0.16, 0.20, 1.0], mesh_type: 0 },
    ];
    // Mobile Infantry: tan/sand armor, black visor — film-accurate lived-in trooper
    let mi = vec![
        InteriorNPCPart { local_offset: Vec3::new(0.0, 1.6, 0.0), scale: Vec3::new(0.22, 0.22, 0.22), color: [0.42, 0.36, 0.28, 1.0], mesh_type: 1 }, // helmet (tan)
        InteriorNPCPart { local_offset: Vec3::new(0.0, 1.62, 0.08), scale: Vec3::new(0.14, 0.06, 0.04), color: [0.06, 0.06, 0.08, 0.95], mesh_type: 0 }, // visor (black)
        InteriorNPCPart { local_offset: Vec3::new(0.0, 1.1, 0.0), scale: Vec3::new(0.35, 0.4, 0.2), color: [0.38, 0.32, 0.26, 1.0], mesh_type: 0 },
        InteriorNPCPart { local_offset: Vec3::new(-0.22, 1.25, 0.0), scale: Vec3::new(0.15, 0.12, 0.18), color: [0.40, 0.34, 0.28, 1.0], mesh_type: 0 },
        InteriorNPCPart { local_offset: Vec3::new(0.22, 1.25, 0.0), scale: Vec3::new(0.15, 0.12, 0.18), color: [0.40, 0.34, 0.28, 1.0], mesh_type: 0 },
        InteriorNPCPart { local_offset: Vec3::new(0.0, 0.45, 0.0), scale: Vec3::new(0.2, 0.45, 0.15), color: [0.32, 0.28, 0.22, 1.0], mesh_type: 0 },
    ];
    // Marauder: industrial Federation gray, worn metal, amber HUD (ship systems style)
    let marauder = vec![
        InteriorNPCPart { local_offset: Vec3::new(0.0, 1.85, 0.0), scale: Vec3::new(0.35, 0.28, 0.35), color: [0.18, 0.17, 0.16, 1.0], mesh_type: 0 },
        InteriorNPCPart { local_offset: Vec3::new(0.0, 1.88, 0.2), scale: Vec3::new(0.2, 0.08, 0.06), color: [0.85, 0.45, 0.12, 0.9], mesh_type: 2 }, // amber visor
        InteriorNPCPart { local_offset: Vec3::new(0.0, 1.35, 0.0), scale: Vec3::new(0.55, 0.5, 0.35), color: [0.15, 0.14, 0.13, 1.0], mesh_type: 0 },
        InteriorNPCPart { local_offset: Vec3::new(-0.38, 1.55, 0.0), scale: Vec3::new(0.28, 0.22, 0.32), color: [0.17, 0.16, 0.15, 1.0], mesh_type: 0 },
        InteriorNPCPart { local_offset: Vec3::new(0.38, 1.55, 0.0), scale: Vec3::new(0.28, 0.22, 0.32), color: [0.17, 0.16, 0.15, 1.0], mesh_type: 0 },
        InteriorNPCPart { local_offset: Vec3::new(0.0, 0.55, 0.0), scale: Vec3::new(0.32, 0.55, 0.25), color: [0.13, 0.12, 0.11, 1.0], mesh_type: 0 },
        InteriorNPCPart { local_offset: Vec3::new(0.0, 1.3, 0.2), scale: Vec3::new(0.2, 0.15, 0.08), color: [0.75, 0.40, 0.10, 0.8], mesh_type: 2 }, // chest HUD amber
    ];
    // Johnny Rico — squad leader; tan armor, red rank accent (lived-in hero)
    let rico = vec![
        InteriorNPCPart { local_offset: Vec3::new(0.0, 1.62, 0.0), scale: Vec3::new(0.24, 0.24, 0.24), color: [0.44, 0.36, 0.28, 1.0], mesh_type: 1 },
        InteriorNPCPart { local_offset: Vec3::new(0.0, 1.64, 0.09), scale: Vec3::new(0.15, 0.065, 0.045), color: [0.48, 0.12, 0.10, 0.95], mesh_type: 2 },
        InteriorNPCPart { local_offset: Vec3::new(0.0, 1.12, 0.0), scale: Vec3::new(0.38, 0.42, 0.22), color: [0.40, 0.32, 0.26, 1.0], mesh_type: 0 },
        InteriorNPCPart { local_offset: Vec3::new(0.0, 1.18, 0.24), scale: Vec3::new(0.12, 0.08, 0.06), color: [0.52, 0.14, 0.12, 0.9], mesh_type: 2 },
        InteriorNPCPart { local_offset: Vec3::new(-0.24, 1.28, 0.0), scale: Vec3::new(0.16, 0.13, 0.19), color: [0.42, 0.34, 0.28, 1.0], mesh_type: 0 },
        InteriorNPCPart { local_offset: Vec3::new(0.24, 1.28, 0.0), scale: Vec3::new(0.16, 0.13, 0.19), color: [0.42, 0.34, 0.28, 1.0], mesh_type: 0 },
        InteriorNPCPart { local_offset: Vec3::new(0.0, 0.46, 0.0), scale: Vec3::new(0.22, 0.46, 0.16), color: [0.35, 0.28, 0.22, 1.0], mesh_type: 0 },
    ];
    match kind {
        Fleet => fleet,
        FleetOfficer => fleet_officer,
        MobileInfantry => mi,
        Marauder => marauder,
        JohnnyRico => rico,
    }
}

/// Dialogue IDs for ship crew (must match dialogue_content in dialogue.rs): 5=Fleet, 6=FleetOfficer, 7=MI, 8=Marauder, 9=Rico.
const DIALOGUE_FLEET: usize = 5;
const DIALOGUE_FLEET_OFFICER: usize = 6;
const DIALOGUE_MI: usize = 7;
const DIALOGUE_MARAUDER: usize = 8;
const DIALOGUE_RICO: usize = 9;

/// NPCs placed around the Roger Young CIC, corridor, and drop bay. Tints give lived-in variation.
pub(crate) fn roger_young_interior_npcs() -> Vec<InteriorNPC> {
    use InteriorNPCKind::*;
    vec![
        // ── CIC: Fleet at helm ──
        InteriorNPC { position: Vec3::new(-2.0, 0.0, 12.2), facing_yaw_rad: 0.0, kind: FleetOfficer, color_tint: [0.97, 0.98, 1.02], name: "Lt. Parks", dialogue_id: DIALOGUE_FLEET_OFFICER },
        InteriorNPC { position: Vec3::new(2.0, 0.0, 12.2), facing_yaw_rad: 0.0, kind: FleetOfficer, color_tint: [0.98, 0.97, 1.01], name: "Ensign Levy", dialogue_id: DIALOGUE_FLEET_OFFICER },
        // Fleet at consoles
        InteriorNPC { position: Vec3::new(-8.5, 0.0, -5.0), facing_yaw_rad: std::f32::consts::FRAC_PI_2, kind: Fleet, color_tint: [1.02, 1.0, 0.98], name: "Ops Specialist Chen", dialogue_id: DIALOGUE_FLEET },
        InteriorNPC { position: Vec3::new(8.5, 0.0, -5.0), facing_yaw_rad: -std::f32::consts::FRAC_PI_2, kind: Fleet, color_tint: [0.96, 0.98, 1.0], name: "Comms Officer Brice", dialogue_id: DIALOGUE_FLEET },
        // ── War table ──
        InteriorNPC { position: Vec3::new(0.0, 0.0, 4.3), facing_yaw_rad: std::f32::consts::PI, kind: JohnnyRico, color_tint: [1.0, 0.98, 0.96], name: "Johnny Rico", dialogue_id: DIALOGUE_RICO },
        InteriorNPC { position: Vec3::new(-2.7, 0.0, 2.0), facing_yaw_rad: -std::f32::consts::FRAC_PI_2, kind: MobileInfantry, color_tint: [0.92, 0.95, 0.9], name: "Sgt. Zim", dialogue_id: DIALOGUE_MI },
        InteriorNPC { position: Vec3::new(2.7, 0.0, 2.0), facing_yaw_rad: std::f32::consts::FRAC_PI_2, kind: MobileInfantry, color_tint: [1.05, 1.02, 0.98], name: "Cpl. Higgins", dialogue_id: DIALOGUE_MI },
        // ── Aft corridor ──
        InteriorNPC { position: Vec3::new(-1.5, 0.0, -18.0), facing_yaw_rad: 0.0, kind: MobileInfantry, color_tint: [0.88, 0.9, 0.86], name: "Trooper Flores", dialogue_id: DIALOGUE_MI },
        InteriorNPC { position: Vec3::new(1.8, 0.0, -19.0), facing_yaw_rad: 0.1, kind: MobileInfantry, color_tint: [1.02, 0.99, 0.97], name: "Trooper Kowalski", dialogue_id: DIALOGUE_MI },
        // ── Drop bay ──
        InteriorNPC { position: Vec3::new(-2.5, 0.0, -28.5), facing_yaw_rad: 0.0, kind: Marauder, color_tint: [1.0, 0.97, 0.94], name: "Marauder Pilot Acevedo", dialogue_id: DIALOGUE_MARAUDER },
        InteriorNPC { position: Vec3::new(2.5, 0.0, -28.5), facing_yaw_rad: 0.0, kind: Marauder, color_tint: [0.96, 0.98, 1.0], name: "Marauder Pilot Dienes", dialogue_id: DIALOGUE_MARAUDER },
        InteriorNPC { position: Vec3::new(0.0, 0.0, -26.0), facing_yaw_rad: -std::f32::consts::FRAC_PI_2, kind: Fleet, color_tint: [0.9, 0.88, 0.86], name: "Tech Martinez", dialogue_id: DIALOGUE_FLEET },
        InteriorNPC { position: Vec3::new(0.0, 0.0, -24.0), facing_yaw_rad: std::f32::consts::PI, kind: MobileInfantry, color_tint: [0.94, 0.92, 0.9], name: "Cpl. Rasczak", dialogue_id: DIALOGUE_MI },
    ]
}

/// Generate the walkable interior of the Roger Young CIC/bridge area.
/// Origin is (0, 0, 0) = center of the CIC floor.
pub(crate) fn roger_young_interior_parts() -> Vec<ShipInteriorPart> {
    let steel = [0.18, 0.19, 0.22, 1.0];
    let dark_steel = [0.12, 0.13, 0.16, 1.0];
    let floor_color = [0.10, 0.11, 0.14, 1.0];
    let ceiling = [0.14, 0.15, 0.18, 1.0];
    let accent_blue = [0.15, 0.25, 0.45, 1.0];
    let light_warm = [0.92, 0.94, 0.98, 0.9];  // Cool white fluorescent (no orange piss filter)
    let light_strip = [0.4, 0.6, 1.0, 0.7];
    let red_alert = [0.8, 0.1, 0.05, 0.6];
    let console_glow = [0.2, 0.5, 0.8, 0.8];
    let fed_gold = [0.7, 0.6, 0.2, 0.9];
    let grate = [0.08, 0.08, 0.10, 1.0];
    let hangar_steel = [0.15, 0.16, 0.19, 1.0];

    vec![
        // ══════ FLOOR ══════
        // Main CIC deck — wide rectangular room
        ShipInteriorPart { pos: Vec3::new(0.0, -0.1, 0.0), scale: Vec3::new(20.0, 0.2, 30.0), color: floor_color, mesh_type: 0 },
        // Floor grating (center walkway)
        ShipInteriorPart { pos: Vec3::new(0.0, -0.05, 0.0), scale: Vec3::new(3.0, 0.05, 26.0), color: grate, mesh_type: 0 },
        // Floor grating cross strips
        ShipInteriorPart { pos: Vec3::new(0.0, -0.05, -6.0), scale: Vec3::new(18.0, 0.05, 0.3), color: grate, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(0.0, -0.05, 6.0), scale: Vec3::new(18.0, 0.05, 0.3), color: grate, mesh_type: 0 },

        // ══════ WALLS ══════
        // Port wall (left, -X) — two window cutouts: main (z -8..8) and aft (z -12..-9)
        ShipInteriorPart { pos: Vec3::new(-10.0, 2.0, -13.5), scale: Vec3::new(0.4, 4.5, 3.0), color: steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(-10.0, 2.0, -8.5), scale: Vec3::new(0.4, 4.5, 1.0), color: steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(-10.0, 2.0, 11.5), scale: Vec3::new(0.4, 4.5, 7.0), color: steel, mesh_type: 0 },
        // Starboard wall (right, +X) — same two window cutouts
        ShipInteriorPart { pos: Vec3::new(10.0, 2.0, -13.5), scale: Vec3::new(0.4, 4.5, 3.0), color: steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(10.0, 2.0, -8.5), scale: Vec3::new(0.4, 4.5, 1.0), color: steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(10.0, 2.0, 11.5), scale: Vec3::new(0.4, 4.5, 7.0), color: steel, mesh_type: 0 },
        // Forward wall (+Z) — larger viewscreen opening (real-time space/planets)
        ShipInteriorPart { pos: Vec3::new(-6.5, 2.0, 15.0), scale: Vec3::new(3.5, 4.5, 0.4), color: steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(6.5, 2.0, 15.0), scale: Vec3::new(3.5, 4.5, 0.4), color: steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(0.0, 4.25, 15.0), scale: Vec3::new(14.0, 0.5, 0.4), color: steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(0.0, 0.6, 15.0), scale: Vec3::new(14.0, 0.5, 0.4), color: steel, mesh_type: 0 },
        // Aft wall with door opening (-Z) — split into two halves
        ShipInteriorPart { pos: Vec3::new(-6.5, 2.0, -15.0), scale: Vec3::new(7.0, 4.5, 0.4), color: steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(6.5, 2.0, -15.0), scale: Vec3::new(7.0, 4.5, 0.4), color: steel, mesh_type: 0 },
        // Aft door header
        ShipInteriorPart { pos: Vec3::new(0.0, 3.8, -15.0), scale: Vec3::new(6.0, 0.7, 0.4), color: dark_steel, mesh_type: 0 },

        // ══════ CEILING ══════
        ShipInteriorPart { pos: Vec3::new(0.0, 4.5, 0.0), scale: Vec3::new(20.0, 0.3, 30.0), color: ceiling, mesh_type: 0 },
        // Ceiling support beams (crosswise)
        ShipInteriorPart { pos: Vec3::new(0.0, 4.2, -10.0), scale: Vec3::new(20.0, 0.3, 0.5), color: dark_steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(0.0, 4.2, 0.0), scale: Vec3::new(20.0, 0.3, 0.5), color: dark_steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(0.0, 4.2, 10.0), scale: Vec3::new(20.0, 0.3, 0.5), color: dark_steel, mesh_type: 0 },

        // ══════ CEILING LIGHTS ══════
        // Harsh fluorescent strips (Federation utilitarian)
        ShipInteriorPart { pos: Vec3::new(-4.0, 4.1, 0.0), scale: Vec3::new(0.3, 0.15, 24.0), color: light_warm, mesh_type: 2 },
        ShipInteriorPart { pos: Vec3::new(4.0, 4.1, 0.0), scale: Vec3::new(0.3, 0.15, 24.0), color: light_warm, mesh_type: 2 },
        // Center light
        ShipInteriorPart { pos: Vec3::new(0.0, 4.1, 0.0), scale: Vec3::new(0.2, 0.1, 8.0), color: light_strip, mesh_type: 2 },

        // ══════ WAR TABLE (center of room) ══════
        // Table base pedestal
        ShipInteriorPart { pos: Vec3::new(0.0, 0.0, 2.0), scale: Vec3::new(1.5, 0.5, 1.5), color: dark_steel, mesh_type: 0 },
        // Table surface (holographic projector)
        ShipInteriorPart { pos: Vec3::new(0.0, 0.9, 2.0), scale: Vec3::new(4.0, 0.15, 3.0), color: [0.08, 0.12, 0.2, 1.0], mesh_type: 0 },
        // Table rim
        ShipInteriorPart { pos: Vec3::new(0.0, 1.0, 2.0), scale: Vec3::new(4.2, 0.06, 3.2), color: accent_blue, mesh_type: 0 },
        // Holographic projector glow (pulsing, rendered as emissive)
        ShipInteriorPart { pos: Vec3::new(0.0, 1.1, 2.0), scale: Vec3::new(3.5, 0.05, 2.5), color: console_glow, mesh_type: 2 },

        // ══════ BRIDGE CONSOLES (forward wall) ══════
        // Main viewscreen frame — larger opening (x ±5.5, y 1.1..3.75) for real-time space/planets
        ShipInteriorPart { pos: Vec3::new(-5.8, 2.4, 14.6), scale: Vec3::new(0.4, 3.0, 0.2), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(5.8, 2.4, 14.6), scale: Vec3::new(0.4, 3.0, 0.2), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(0.0, 3.95, 14.6), scale: Vec3::new(12.0, 0.2, 0.2), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(0.0, 0.85, 14.6), scale: Vec3::new(12.0, 0.2, 0.2), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        // Viewscreen bezel glow
        ShipInteriorPart { pos: Vec3::new(-5.6, 2.4, 14.52), scale: Vec3::new(0.06, 2.8, 0.06), color: [0.08, 0.15, 0.28, 0.5], mesh_type: 2 },
        ShipInteriorPart { pos: Vec3::new(5.6, 2.4, 14.52), scale: Vec3::new(0.06, 2.8, 0.06), color: [0.08, 0.15, 0.28, 0.5], mesh_type: 2 },
        ShipInteriorPart { pos: Vec3::new(0.0, 3.8, 14.52), scale: Vec3::new(11.0, 0.06, 0.06), color: [0.08, 0.15, 0.28, 0.5], mesh_type: 2 },
        ShipInteriorPart { pos: Vec3::new(0.0, 0.95, 14.52), scale: Vec3::new(11.0, 0.06, 0.06), color: [0.08, 0.15, 0.28, 0.5], mesh_type: 2 },
        // Helm console (forward, below viewscreen)
        ShipInteriorPart { pos: Vec3::new(0.0, 0.7, 13.0), scale: Vec3::new(6.0, 0.8, 2.0), color: dark_steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(0.0, 1.15, 13.5), scale: Vec3::new(5.5, 0.1, 1.2), color: console_glow, mesh_type: 2 },
        // Helm chairs
        ShipInteriorPart { pos: Vec3::new(-2.0, 0.5, 12.0), scale: Vec3::new(0.6, 1.0, 0.6), color: dark_steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(2.0, 0.5, 12.0), scale: Vec3::new(0.6, 1.0, 0.6), color: dark_steel, mesh_type: 0 },

        // ══════ SIDE WINDOWS (port/starboard — large + aft window, real-time space) ══════
        // Port: main window frame (cutout z -8..8, y 0.2..3.8) — thin bezel
        ShipInteriorPart { pos: Vec3::new(-9.8, 2.0, -8.0), scale: Vec3::new(0.06, 3.6, 0.12), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(-9.8, 2.0, 8.0), scale: Vec3::new(0.06, 3.6, 0.12), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(-9.8, 3.8, 0.0), scale: Vec3::new(0.06, 0.12, 16.2), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(-9.8, 0.2, 0.0), scale: Vec3::new(0.06, 0.12, 16.2), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(-9.78, 3.65, 0.0), scale: Vec3::new(0.02, 0.1, 16.0), color: [0.08, 0.15, 0.28, 0.5], mesh_type: 2 },
        ShipInteriorPart { pos: Vec3::new(-9.78, 0.35, 0.0), scale: Vec3::new(0.02, 0.1, 16.0), color: [0.08, 0.15, 0.28, 0.5], mesh_type: 2 },
        // Port: aft window frame (cutout z -12..-9)
        ShipInteriorPart { pos: Vec3::new(-9.8, 2.0, -12.0), scale: Vec3::new(0.06, 3.2, 0.12), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(-9.8, 2.0, -9.0), scale: Vec3::new(0.06, 3.2, 0.12), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(-9.8, 3.6, -10.5), scale: Vec3::new(0.06, 0.12, 3.2), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(-9.8, 0.4, -10.5), scale: Vec3::new(0.06, 0.12, 3.2), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(-9.78, 3.45, -10.5), scale: Vec3::new(0.02, 0.08, 2.8), color: [0.08, 0.15, 0.28, 0.5], mesh_type: 2 },
        ShipInteriorPart { pos: Vec3::new(-9.78, 0.55, -10.5), scale: Vec3::new(0.02, 0.08, 2.8), color: [0.08, 0.15, 0.28, 0.5], mesh_type: 2 },
        // Starboard: main window frame
        ShipInteriorPart { pos: Vec3::new(9.8, 2.0, -8.0), scale: Vec3::new(0.06, 3.6, 0.12), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(9.8, 2.0, 8.0), scale: Vec3::new(0.06, 3.6, 0.12), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(9.8, 3.8, 0.0), scale: Vec3::new(0.06, 0.12, 16.2), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(9.8, 0.2, 0.0), scale: Vec3::new(0.06, 0.12, 16.2), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(9.78, 3.65, 0.0), scale: Vec3::new(0.02, 0.1, 16.0), color: [0.08, 0.15, 0.28, 0.5], mesh_type: 2 },
        ShipInteriorPart { pos: Vec3::new(9.78, 0.35, 0.0), scale: Vec3::new(0.02, 0.1, 16.0), color: [0.08, 0.15, 0.28, 0.5], mesh_type: 2 },
        // Starboard: aft window frame
        ShipInteriorPart { pos: Vec3::new(9.8, 2.0, -12.0), scale: Vec3::new(0.06, 3.2, 0.12), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(9.8, 2.0, -9.0), scale: Vec3::new(0.06, 3.2, 0.12), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(9.8, 3.6, -10.5), scale: Vec3::new(0.06, 0.12, 3.2), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(9.8, 0.4, -10.5), scale: Vec3::new(0.06, 0.12, 3.2), color: [0.06, 0.08, 0.12, 1.0], mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(9.78, 3.45, -10.5), scale: Vec3::new(0.02, 0.08, 2.8), color: [0.08, 0.15, 0.28, 0.5], mesh_type: 2 },
        ShipInteriorPart { pos: Vec3::new(9.78, 0.55, -10.5), scale: Vec3::new(0.02, 0.08, 2.8), color: [0.08, 0.15, 0.28, 0.5], mesh_type: 2 },
        // ══════ SIDE CONSOLES ══════
        // Port side crew stations
        ShipInteriorPart { pos: Vec3::new(-9.0, 0.7, -5.0), scale: Vec3::new(1.5, 0.8, 8.0), color: dark_steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(-9.0, 1.15, -5.0), scale: Vec3::new(1.0, 0.1, 7.5), color: console_glow, mesh_type: 2 },
        // Port screens (on wall)
        ShipInteriorPart { pos: Vec3::new(-9.6, 2.5, -5.0), scale: Vec3::new(0.1, 1.5, 6.0), color: [0.1, 0.2, 0.35, 0.8], mesh_type: 2 },
        // Starboard crew stations
        ShipInteriorPart { pos: Vec3::new(9.0, 0.7, -5.0), scale: Vec3::new(1.5, 0.8, 8.0), color: dark_steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(9.0, 1.15, -5.0), scale: Vec3::new(1.0, 0.1, 7.5), color: console_glow, mesh_type: 2 },
        ShipInteriorPart { pos: Vec3::new(9.6, 2.5, -5.0), scale: Vec3::new(0.1, 1.5, 6.0), color: [0.1, 0.2, 0.35, 0.8], mesh_type: 2 },

        // ══════ FEDERATION INSIGNIA (forward wall, gold) ══════
        ShipInteriorPart { pos: Vec3::new(0.0, 3.8, 14.3), scale: Vec3::new(1.5, 0.8, 0.1), color: fed_gold, mesh_type: 2 },

        // ══════ SUPPORT COLUMNS ══════
        ShipInteriorPart { pos: Vec3::new(-5.0, 2.0, 8.0), scale: Vec3::new(0.5, 4.0, 0.5), color: steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(5.0, 2.0, 8.0), scale: Vec3::new(0.5, 4.0, 0.5), color: steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(-5.0, 2.0, -8.0), scale: Vec3::new(0.5, 4.0, 0.5), color: steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(5.0, 2.0, -8.0), scale: Vec3::new(0.5, 4.0, 0.5), color: steel, mesh_type: 0 },

        // ══════ RED ALERT LIGHTS (along ceiling edges) ══════
        ShipInteriorPart { pos: Vec3::new(-9.5, 3.8, 7.0), scale: Vec3::new(0.3, 0.3, 0.3), color: red_alert, mesh_type: 2 },
        ShipInteriorPart { pos: Vec3::new(9.5, 3.8, 7.0), scale: Vec3::new(0.3, 0.3, 0.3), color: red_alert, mesh_type: 2 },
        ShipInteriorPart { pos: Vec3::new(-9.5, 3.8, -7.0), scale: Vec3::new(0.3, 0.3, 0.3), color: red_alert, mesh_type: 2 },
        ShipInteriorPart { pos: Vec3::new(9.5, 3.8, -7.0), scale: Vec3::new(0.3, 0.3, 0.3), color: red_alert, mesh_type: 2 },

        // ══════ CORRIDOR (aft, leading to drop bay) ══════
        // Corridor floor
        ShipInteriorPart { pos: Vec3::new(0.0, -0.1, -20.0), scale: Vec3::new(6.0, 0.2, 10.0), color: floor_color, mesh_type: 0 },
        // Corridor walls
        ShipInteriorPart { pos: Vec3::new(-3.0, 2.0, -20.0), scale: Vec3::new(0.3, 4.5, 10.0), color: hangar_steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(3.0, 2.0, -20.0), scale: Vec3::new(0.3, 4.5, 10.0), color: hangar_steel, mesh_type: 0 },
        // Corridor ceiling
        ShipInteriorPart { pos: Vec3::new(0.0, 4.5, -20.0), scale: Vec3::new(6.0, 0.3, 10.0), color: ceiling, mesh_type: 0 },
        // Corridor lights
        ShipInteriorPart { pos: Vec3::new(0.0, 4.1, -18.0), scale: Vec3::new(0.2, 0.1, 1.0), color: light_warm, mesh_type: 2 },
        ShipInteriorPart { pos: Vec3::new(0.0, 4.1, -22.0), scale: Vec3::new(0.2, 0.1, 1.0), color: light_warm, mesh_type: 2 },
        // ══════ DROP BAY (aft end of corridor) ══════
        // Drop bay floor
        ShipInteriorPart { pos: Vec3::new(0.0, -0.1, -28.0), scale: Vec3::new(8.0, 0.2, 6.0), color: grate, mesh_type: 0 },
        // Drop bay walls
        ShipInteriorPart { pos: Vec3::new(-4.0, 2.0, -28.0), scale: Vec3::new(0.3, 4.5, 6.0), color: hangar_steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(4.0, 2.0, -28.0), scale: Vec3::new(0.3, 4.5, 6.0), color: hangar_steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(0.0, 2.0, -31.0), scale: Vec3::new(8.0, 4.5, 0.3), color: hangar_steel, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(0.0, 4.5, -28.0), scale: Vec3::new(8.0, 0.3, 6.0), color: ceiling, mesh_type: 0 },
        // Drop pod cradle
        ShipInteriorPart { pos: Vec3::new(0.0, 0.3, -28.0), scale: Vec3::new(2.0, 0.6, 2.0), color: [0.2, 0.2, 0.25, 1.0], mesh_type: 0 },
        // Drop bay warning stripes (amber lights)
        ShipInteriorPart { pos: Vec3::new(-3.5, 1.5, -28.0), scale: Vec3::new(0.2, 0.2, 5.0), color: [1.0, 0.6, 0.1, 0.6], mesh_type: 2 },
        ShipInteriorPart { pos: Vec3::new(3.5, 1.5, -28.0), scale: Vec3::new(0.2, 0.2, 5.0), color: [1.0, 0.6, 0.1, 0.6], mesh_type: 2 },
        // "DROP BAY" sign glow
        ShipInteriorPart { pos: Vec3::new(0.0, 3.5, -15.3), scale: Vec3::new(3.0, 0.4, 0.1), color: [1.0, 0.4, 0.1, 0.7], mesh_type: 2 },

        // ══════ WALL DETAILS (pipes, panels, vents) ══════
        // Pipe runs along port wall
        ShipInteriorPart { pos: Vec3::new(-9.6, 3.5, 0.0), scale: Vec3::new(0.15, 0.15, 28.0), color: [0.25, 0.26, 0.28, 1.0], mesh_type: 1 },
        ShipInteriorPart { pos: Vec3::new(-9.6, 0.5, 0.0), scale: Vec3::new(0.15, 0.15, 28.0), color: [0.25, 0.26, 0.28, 1.0], mesh_type: 1 },
        // Pipe runs along starboard wall
        ShipInteriorPart { pos: Vec3::new(9.6, 3.5, 0.0), scale: Vec3::new(0.15, 0.15, 28.0), color: [0.25, 0.26, 0.28, 1.0], mesh_type: 1 },
        ShipInteriorPart { pos: Vec3::new(9.6, 0.5, 0.0), scale: Vec3::new(0.15, 0.15, 28.0), color: [0.25, 0.26, 0.28, 1.0], mesh_type: 1 },
        // Wall panel accents
        ShipInteriorPart { pos: Vec3::new(-9.7, 2.0, 3.0), scale: Vec3::new(0.05, 2.0, 4.0), color: accent_blue, mesh_type: 0 },
        ShipInteriorPart { pos: Vec3::new(9.7, 2.0, 3.0), scale: Vec3::new(0.05, 2.0, 4.0), color: accent_blue, mesh_type: 0 },

        // ══════ FLAG MOUNTING BRACKETS ══════
        // UCF flag bracket (port wall) — brass mounting plate
        ShipInteriorPart { pos: Vec3::new(-9.65, 3.85, 8.0), scale: Vec3::new(0.1, 0.15, 0.4), color: [0.35, 0.32, 0.18, 1.0], mesh_type: 0 },
        // "UCF" label plaque below flag
        ShipInteriorPart { pos: Vec3::new(-9.65, 1.6, 6.5), scale: Vec3::new(0.06, 0.25, 1.2), color: [0.08, 0.12, 0.25, 1.0], mesh_type: 0 },
        // Plaque text glow
        ShipInteriorPart { pos: Vec3::new(-9.6, 1.6, 6.5), scale: Vec3::new(0.04, 0.15, 0.9), color: [0.5, 0.45, 0.15, 0.7], mesh_type: 2 },

        // MI flag bracket (starboard wall) — brass mounting plate
        ShipInteriorPart { pos: Vec3::new(9.65, 3.85, 8.0), scale: Vec3::new(0.1, 0.15, 0.4), color: [0.35, 0.32, 0.18, 1.0], mesh_type: 0 },
        // "MI" label plaque below flag
        ShipInteriorPart { pos: Vec3::new(9.65, 1.6, 6.5), scale: Vec3::new(0.06, 0.25, 1.2), color: [0.08, 0.12, 0.25, 1.0], mesh_type: 0 },
        // Plaque text glow
        ShipInteriorPart { pos: Vec3::new(9.6, 1.6, 6.5), scale: Vec3::new(0.04, 0.15, 0.9), color: [0.7, 0.1, 0.05, 0.7], mesh_type: 2 },

        // ══════ SMALL SPOTLIGHTS on flags ══════
        ShipInteriorPart { pos: Vec3::new(-8.5, 4.0, 8.0), scale: Vec3::new(0.3, 0.2, 0.3), color: [0.9, 0.85, 0.7, 0.5], mesh_type: 2 },
        ShipInteriorPart { pos: Vec3::new(8.5, 4.0, 8.0), scale: Vec3::new(0.3, 0.2, 0.3), color: [0.9, 0.85, 0.7, 0.5], mesh_type: 2 },
    ]
}

// ── Galactic War Table (Helldivers 2 style) ─────────────────────────────

/// Liberation status for a single planet in the current system.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct PlanetWarStatus {
    /// Liberation progress (0.0 = fully bug-controlled, 1.0 = fully liberated)
    liberation: f32,
    /// Whether there's an active player operation on this planet.
    active_operation: bool,
    /// Accumulated kills on this planet (contributes to liberation).
    total_kills: u32,
    /// Number of successful extractions from this planet.
    successful_extractions: u32,
    /// Whether the planet has been fully liberated.
    liberated: bool,
    /// Defense urgency: if > 0 the bugs are counter-attacking (liberation decays).
    defense_urgency: f32,
    /// Per-planet time of day (0 = dawn, 0.25 = noon, 0.5 = dusk, 0.75 = night).
    #[serde(default = "default_time_of_day")]
    time_of_day: f32,
    /// Per-planet weather (each planet has its own conditions for variety).
    #[serde(default)]
    weather: Weather,
}

fn default_time_of_day() -> f32 {
    0.25 // Noon as sensible default for loaded saves
}

impl PlanetWarStatus {
    fn new() -> Self {
        let mut rng = rand::thread_rng();
        Self {
            liberation: 0.0,
            active_operation: false,
            total_kills: 0,
            successful_extractions: 0,
            liberated: false,
            defense_urgency: 0.0,
            time_of_day: rng.gen::<f32>(),
            weather: Weather::random(),
        }
    }
}

/// A Major Order — global objective issued by Fleet Command.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct MajorOrder {
    /// Order title.
    title: String,
    /// Descriptive text.
    description: String,
    /// Target planet indices involved (if any).
    target_planets: Vec<usize>,
    /// Current progress (0.0..1.0).
    progress: f32,
    /// Whether completed.
    completed: bool,
    /// Reward text.
    reward: String,
}

/// Supply line connecting two planets.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct SupplyLine {
    from: usize,
    to: usize,
    /// Whether this line is contested (bugs threatening supply route).
    contested: bool,
}

/// Full galactic war state for the current star system.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct GalacticWarState {
    /// Per-planet war status (indexed same as `current_system.bodies`).
    planets: Vec<PlanetWarStatus>,
    /// Active major orders.
    major_orders: Vec<MajorOrder>,
    /// Supply lines between planets.
    supply_lines: Vec<SupplyLine>,
    /// Currently selected planet on the war table.
    selected_planet: usize,
    /// Total bugs killed across all planets in this system.
    system_kills: u32,
    /// War table hologram rotation angle.
    holo_rotation: f32,
    /// Scrolling news ticker text.
    ticker_offset: f32,
}

impl GalacticWarState {
    fn new(num_planets: usize) -> Self {
        let mut planets = Vec::with_capacity(num_planets);
        let mut rng = rand::thread_rng();
        for _ in 0..num_planets {
            let mut status = PlanetWarStatus::new();
            // Some planets start partially liberated
            status.liberation = rng.gen::<f32>() * 0.3;
            // Random defense urgency
            status.defense_urgency = if rng.gen::<f32>() > 0.7 { rng.gen::<f32>() * 0.5 } else { 0.0 };
            planets.push(status);
        }

        // Generate supply lines (connect sequential planets + some cross-links)
        let mut supply_lines = Vec::new();
        for i in 0..num_planets.saturating_sub(1) {
            supply_lines.push(SupplyLine { from: i, to: i + 1, contested: rng.gen::<f32>() > 0.6 });
        }
        // Cross-links for non-trivial topology
        if num_planets > 3 {
            supply_lines.push(SupplyLine { from: 0, to: num_planets - 1, contested: rng.gen::<f32>() > 0.5 });
        }
        if num_planets > 5 {
            supply_lines.push(SupplyLine { from: 1, to: num_planets - 2, contested: true });
        }

        // Generate initial major order
        let mut major_orders = Vec::new();
        if num_planets > 0 {
            let target = rng.gen_range(0..num_planets);
            major_orders.push(MajorOrder {
                title: "OPERATION: IRON RAIN".to_string(),
                description: "Fleet Command orders the liberation of a key strategic world.".to_string(),
                target_planets: vec![target],
                progress: 0.0,
                completed: false,
                reward: "Medal of Valor + Weapon Requisition".to_string(),
            });
        }
        if num_planets > 2 {
            major_orders.push(MajorOrder {
                title: "DEFEND SUPPLY LINES".to_string(),
                description: "Bug counter-offensive threatening critical supply routes. Hold the line!".to_string(),
                target_planets: vec![],
                progress: 0.3,
                completed: false,
                reward: "Orbital Strike Clearance".to_string(),
            });
        }

        Self {
            planets,
            major_orders,
            supply_lines,
            selected_planet: 0,
            system_kills: 0,
            holo_rotation: 0.0,
            ticker_offset: 0.0,
        }
    }

    /// Update war state each frame (called from ship phase).
    fn update(&mut self, dt: f32) {
        self.holo_rotation += dt * 0.2;
        self.ticker_offset += dt * 40.0; // scrolling ticker speed

        // Bug counter-attacks slowly erode liberation on contested planets
        for status in &mut self.planets {
            if status.defense_urgency > 0.0 && !status.liberated {
                status.liberation = (status.liberation - status.defense_urgency * 0.001 * dt).max(0.0);
            }
        }

        // Update major order progress from planet liberations
        for order in &mut self.major_orders {
            if order.completed { continue; }
            if !order.target_planets.is_empty() {
                let total: f32 = order.target_planets.iter()
                    .filter_map(|&i| self.planets.get(i))
                    .map(|p| p.liberation)
                    .sum();
                order.progress = total / order.target_planets.len().max(1) as f32;
                if order.progress >= 1.0 {
                    order.completed = true;
                }
            }
        }
    }

    /// Record kills from a mission (call after extraction or gameplay).
    fn record_kills(&mut self, planet_idx: usize, kills: u32) {
        self.system_kills += kills;
        if let Some(status) = self.planets.get_mut(planet_idx) {
            status.total_kills += kills;
            // Each kill contributes to liberation progress
            let liberation_per_kill = 0.0005; // 2000 kills to fully liberate
            status.liberation = (status.liberation + kills as f32 * liberation_per_kill).min(1.0);
            if status.liberation >= 1.0 {
                status.liberated = true;
            }
        }
    }

    /// Record a successful extraction.
    fn record_extraction(&mut self, planet_idx: usize) {
        if let Some(status) = self.planets.get_mut(planet_idx) {
            status.successful_extractions += 1;
            // Extractions boost liberation significantly
            status.liberation = (status.liberation + 0.05).min(1.0);
            if status.liberation >= 1.0 {
                status.liberated = true;
            }
        }
    }
}

/// Persisted galactic war + universe (save file).
#[derive(serde::Serialize, serde::Deserialize)]
struct SaveData {
    universe_seed: u64,
    current_system_idx: usize,
    war_state: GalacticWarState,
}

fn galactic_war_save_path() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join("opensst_save.ron")
}

fn save_galactic_war(universe_seed: u64, current_system_idx: usize, war_state: &GalacticWarState) {
    let data = SaveData {
        universe_seed,
        current_system_idx,
        war_state: war_state.clone(),
    };
    let path = galactic_war_save_path();
    if let Ok(s) = ron::ser::to_string_pretty(&data, ron::ser::PrettyConfig::default()) {
        if let Err(e) = std::fs::write(&path, s) {
            log::warn!("Failed to save galactic war: {}", e);
        }
    }
}

fn load_galactic_war() -> Option<(u64, usize, GalacticWarState)> {
    let path = galactic_war_save_path();
    let s = std::fs::read_to_string(&path).ok()?;
    let data: SaveData = ron::from_str(&s).ok()?;
    Some((data.universe_seed, data.current_system_idx, data.war_state))
}

/// Authored STE-style bug meshes (replaces procedural BugMeshGenerator).
struct AuthoredBugMeshes {
    warrior: Mesh,
    charger: Mesh,
    spitter: Mesh,
    tanker: Mesh,
    hopper: Mesh,
}

impl AuthoredBugMeshes {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            warrior: Self::upload(device, authored_bug_meshes::build_warrior()),
            charger: Self::upload(device, authored_bug_meshes::build_charger()),
            spitter: Self::upload(device, authored_bug_meshes::build_spitter()),
            tanker: Self::upload(device, authored_bug_meshes::build_tanker()),
            hopper: Self::upload(device, authored_bug_meshes::build_hopper()),
        }
    }

    fn upload(device: &wgpu::Device, (vertices, indices): (Vec<renderer::Vertex>, Vec<u32>)) -> Mesh {
        Mesh::from_data(device, &vertices, &indices)
    }

    fn get(&self, bug_type: BugType) -> &Mesh {
        match bug_type {
            BugType::Warrior => &self.warrior,
            BugType::Charger => &self.charger,
            BugType::Spitter => &self.spitter,
            BugType::Tanker => &self.tanker,
            BugType::Hopper => &self.hopper,
        }
    }
}

/// Environment meshes
struct EnvironmentMeshes {
    ground: Mesh,
    cube: Mesh,
    rock: Mesh,
    rock_chunk: Mesh,
    rock_boulder: Mesh,
    /// First-person viewmodel (rifle)
    gun: Mesh,
    /// Bug hole rim (flattened sphere crater)
    bug_hole: Mesh,
    /// Hive mound (stretched sphere)
    hive_mound: Mesh,
    /// Egg cluster (small sphere)
    egg_cluster: Mesh,
    /// Crystal spike
    crystal: Mesh,
    /// Generic prop sphere (for varied decorations)
    prop_sphere: Mesh,
    /// Unit quad in XZ (normal Y) for Minecraft-style billboard particles (rain, snow, dust)
    billboard_quad: Mesh,
    /// Beveled unit cube (UCF buildings — chamfered edges)
    beveled_cube: Mesh,
    /// Heinlein Skinnies (tall, gaunt humanoid mesh)
    skinny_mesh: Mesh,
    /// Hive cave / tunnel entrance (arched surface hole, Minecraft-style)
    hive_cave_entrance: Mesh,
}

impl EnvironmentMeshes {
    fn new(device: &wgpu::Device) -> Self {
        let (v, idx) = authored_env_meshes::build_bug_hole();
        let bug_hole = Mesh::from_data(device, &v, &idx);
        let (v, idx) = authored_env_meshes::build_hive_cave_entrance();
        let hive_cave_entrance = Mesh::from_data(device, &v, &idx);
        let (v, idx) = authored_env_meshes::build_hive_mound();
        let hive_mound = Mesh::from_data(device, &v, &idx);
        let (v, idx) = authored_env_meshes::build_egg_cluster();
        let egg_cluster = Mesh::from_data(device, &v, &idx);
        let (v, idx) = authored_env_meshes::build_rock();
        let rock = Mesh::from_data(device, &v, &idx);
        let (v, idx) = authored_env_meshes::build_rock_chunk();
        let rock_chunk = Mesh::from_data(device, &v, &idx);
        let (v, idx) = authored_env_meshes::build_rock_boulder();
        let rock_boulder = Mesh::from_data(device, &v, &idx);
        // Solid unit cube: corners connect (used for Earth buildings, UCF landmarks, etc.)
        let cube = Mesh::cube(device);
        let (v, idx) = authored_env_meshes::build_beveled_cube();
        let beveled_cube = Mesh::from_data(device, &v, &idx);
        let (v, idx) = authored_env_meshes::build_skinny();
        let skinny_mesh = Mesh::from_data(device, &v, &idx);

        Self {
            ground: Mesh::plane(device, 200.0),
            cube,
            rock,
            rock_chunk,
            rock_boulder,
            gun: Mesh::rifle_viewmodel(device),
            bug_hole,
            hive_mound,
            egg_cluster,
            crystal: Mesh::sphere(device, 1.0, 6, 4),       // Crystal spike (stretched via transform)
            prop_sphere: Mesh::sphere(device, 1.0, 8, 6),   // Generic decoration
            billboard_quad: Mesh::plane(device, 1.0),      // Camera-facing particle quads (Minecraft style)
            beveled_cube,
            skinny_mesh,
            hive_cave_entrance,
        }
    }
}

// ================= Chunked Infinite Terrain =================

/// Per-chunk data: terrain heightmap, GPU mesh, water mesh, and physics collider.
struct TerrainChunkData {
    voxel: VoxelChunk,
    mesh: Mesh,
    water_mesh: Option<Mesh>,
    collider_handle: ColliderHandle,
}

/// Manages an infinite grid of terrain chunks around the player.
struct ChunkManager {
    chunks: HashMap<(i32, i32), TerrainChunkData>,
    chunk_size: f32,
    chunk_resolution: u32,
    view_distance: i32,
    planet_seed: u64,
    height_scale: f32,
    frequency: f64,
    /// Multi-biome sampler for per-vertex biome colors and height variation.
    planet_biomes: PlanetBiomes,
    /// If true, terrain is smooth (no voxel quantization) and gentler — e.g. terraformed Earth.
    use_smooth_terrain: bool,
    /// Chunks that need mesh+collider rebuild; drained each frame (throttled) to avoid artillery lag.
    pending_chunk_rebuilds: Vec<(i32, i32)>,
}

impl ChunkManager {
    fn new(
        planet_seed: u64,
        height_scale: f32,
        frequency: f64,
        planet_biomes: PlanetBiomes,
        use_smooth_terrain: bool,
    ) -> Self {
        Self {
            chunks: HashMap::new(),
            chunk_size: 96.0,   // larger chunks = more terrain per chunk, more destruction area
            chunk_resolution: 128, // finer heightmap for more deformation detail
            view_distance: 5,  // big infinite voxel world: load more chunks (Minecraft-style draw)
            planet_seed,
            height_scale,
            frequency,
            planet_biomes,
            use_smooth_terrain,
            pending_chunk_rebuilds: Vec::new(),
        }
    }

    /// Remove all chunks and their physics colliders.
    fn clear_all(&mut self, physics: &mut PhysicsWorld) {
        self.pending_chunk_rebuilds.clear();
        for (_, chunk) in self.chunks.drain() {
            physics.remove_collider(chunk.collider_handle);
        }
    }

    /// Reinitialize for a new planet (clears chunks, updates params).
    /// When use_smooth_terrain is true (e.g. terraformed Earth), terrain is smooth and no voxel quantization.
    fn reset_for_planet(
        &mut self,
        planet_seed: u64,
        height_scale: f32,
        frequency: f64,
        planet_biomes: PlanetBiomes,
        use_smooth_terrain: bool,
        physics: &mut PhysicsWorld,
    ) {
        self.clear_all(physics);
        self.planet_seed = planet_seed;
        self.height_scale = height_scale;
        self.frequency = frequency;
        self.planet_biomes = planet_biomes;
        self.use_smooth_terrain = use_smooth_terrain;
        self.chunk_resolution = if use_smooth_terrain { 160 } else { 128 };
    }

    /// Map a world-space X or Z coordinate to the chunk index that contains it.
    /// Chunks are CENTERED at (cx * chunk_size), spanning [cx*cs - cs/2, cx*cs + cs/2].
    /// So we offset by half a chunk before flooring.
    fn world_to_chunk(coord: f32, chunk_size: f32) -> i32 {
        ((coord + chunk_size * 0.5) / chunk_size).floor() as i32
    }

    /// Player's current chunk coordinate.
    fn player_chunk(pos: Vec3, chunk_size: f32) -> (i32, i32) {
        (
            Self::world_to_chunk(pos.x, chunk_size),
            Self::world_to_chunk(pos.z, chunk_size),
        )
    }

    /// Force-load all chunks covering a horizontal range around the origin.
    /// Used before spawn_biome_content so sample_height returns valid terrain (not 0 for missing chunks).
    fn ensure_chunks_loaded_for_spawn(
        &mut self,
        scatter_range: f32,
        device: &wgpu::Device,
        physics: &mut PhysicsWorld,
    ) {
        let half = scatter_range * 0.5;
        let min_cx = Self::world_to_chunk(-half, self.chunk_size);
        let max_cx = Self::world_to_chunk(half, self.chunk_size);
        let min_cz = Self::world_to_chunk(-half, self.chunk_size);
        let max_cz = Self::world_to_chunk(half, self.chunk_size);
        for cz in min_cz..=max_cz {
            for cx in min_cx..=max_cx {
                if !self.chunks.contains_key(&(cx, cz)) {
                    let chunk = self.generate_chunk(cx, cz, device, physics);
                    self.chunks.insert((cx, cz), chunk);
                }
            }
        }
    }

    /// Load/unload chunks around player. Dynamically adjusts view distance by altitude.
    /// Batches chunk loading to max 2 per frame to avoid hitches.
    fn update(&mut self, player_pos: Vec3, device: &wgpu::Device, physics: &mut PhysicsWorld) {
        // Dynamic view distance: increase at higher altitudes for better orbital view
        let altitude = player_pos.y.max(0.0);
        self.view_distance = if altitude > 600.0 { 5 }
            else if altitude > 300.0 { 4 }
            else if altitude > 100.0 { 3 }
            else { 3 };

        let (pcx, pcz) = Self::player_chunk(player_pos, self.chunk_size);
        let vd = self.view_distance;

        // Quick check: count how many chunks *should* exist vs *do* exist in range
        // If all loaded already, skip the expensive sort + alloc
        let mut any_missing = false;
        'outer: for dz in -vd..=vd {
            for dx in -vd..=vd {
                if !self.chunks.contains_key(&(pcx + dx, pcz + dz)) {
                    any_missing = true;
                    break 'outer;
                }
            }
        }

        if any_missing {
            // Collect coords that should be loaded, sorted by distance to player chunk
            let total = ((2 * vd + 1) * (2 * vd + 1)) as usize;
            let mut desired: Vec<(i32, i32)> = Vec::with_capacity(total);
            for dz in -vd..=vd {
                for dx in -vd..=vd {
                    desired.push((pcx + dx, pcz + dz));
                }
            }
            // Sort closest first so we prioritize nearby chunks
            desired.sort_unstable_by_key(|&(cx, cz)| (cx - pcx).abs() + (cz - pcz).abs());

            // Load missing chunks (batched: max 2 per frame to spread GPU/CPU load)
            let mut loaded = 0;
            for &(cx, cz) in &desired {
                if loaded >= 2 { break; }
                if !self.chunks.contains_key(&(cx, cz)) {
                    let chunk = self.generate_chunk(cx, cz, device, physics);
                    self.chunks.insert((cx, cz), chunk);
                    loaded += 1;
                }
            }
        }

        // Unload distant chunks (beyond view_distance + 2)
        let unload_dist = vd + 2;
        let to_remove: Vec<(i32, i32)> = self
            .chunks
            .keys()
            .filter(|&&(cx, cz)| {
                (cx - pcx).abs() > unload_dist || (cz - pcz).abs() > unload_dist
            })
            .cloned()
            .collect();
        for key in to_remove {
            if let Some(chunk) = self.chunks.remove(&key) {
                physics.remove_collider(chunk.collider_handle);
            }
        }
    }

    fn generate_chunk(
        &self,
        cx: i32,
        cz: i32,
        device: &wgpu::Device,
        physics: &mut PhysicsWorld,
    ) -> TerrainChunkData {
        let config = TerrainConfig {
            size: self.chunk_size,
            resolution: self.chunk_resolution,
            height_scale: self.height_scale,
            frequency: self.frequency,
            offset_x: cx as f32 * self.chunk_size,
            offset_z: cz as f32 * self.chunk_size,
            seed: self.planet_seed,
            ..Default::default()
        };
        let voxel = VoxelChunk::generate(&config, Some(&self.planet_biomes));

        // Build GPU mesh from voxel (culled cube faces; water excluded for transparent pass)
        let (terrain_vertices, terrain_indices) = voxel.to_mesh();
        let vertices: Vec<renderer::Vertex> = terrain_vertices
            .iter()
            .map(|v| renderer::Vertex {
                position: v.position,
                normal: v.normal,
                tex_coords: v.uv,
                color: v.color,
            })
            .collect();
        let mesh = Mesh::from_data(device, &vertices, &terrain_indices);

        // Transparent water mesh (Minecraft-style)
        let (water_vertices, water_indices) = voxel.to_water_mesh();
        let water_mesh = if water_vertices.is_empty() {
            None
        } else {
            let wv: Vec<renderer::Vertex> = water_vertices
                .iter()
                .map(|v| renderer::Vertex {
                    position: v.position,
                    normal: v.normal,
                    tex_coords: v.uv,
                    color: v.color,
                })
                .collect();
            Some(Mesh::from_data(device, &wv, &water_indices))
        };

        // Add physics heightfield from voxel top surface (translation = chunk min corner, not center)
        let heightmap = voxel.to_heightmap();
        let nrows = voxel.nz + 1;
        let ncols = voxel.nx + 1;
        let offset_min_x = voxel.offset_x - self.chunk_size * 0.5;
        let offset_min_z = voxel.offset_z - self.chunk_size * 0.5;
        let collider_handle = physics.add_terrain_heightfield_at(
            &heightmap,
            nrows,
            ncols,
            self.chunk_size,
            self.chunk_size,
            offset_min_x,
            offset_min_z,
        );

        TerrainChunkData {
            voxel,
            mesh,
            water_mesh,
            collider_handle,
        }
    }

    /// Sample raw heightmap height at a world position (no curvature applied).
    fn sample_height(&self, x: f32, z: f32) -> f32 {
        let cx = Self::world_to_chunk(x, self.chunk_size);
        let cz = Self::world_to_chunk(z, self.chunk_size);
        if let Some(chunk) = self.chunks.get(&(cx, cz)) {
            chunk.voxel.sample_height(x, z)
        } else {
            0.0 // Chunk not loaded, fallback
        }
    }

    /// Planet water level (world Y) if this planet has water. None for desert/volcanic etc.
    pub fn water_level(&self) -> Option<f32> {
        self.planet_biomes
            .biomes
            .iter()
            .any(|b| procgen::BiomeConfig::from_type(*b).has_water())
            .then(|| 0.35 * self.height_scale)
    }

    /// True when the surface at (x,z) is water (not just "below water level").
    /// Crater floors and dry terrain below sea level are not treated as water.
    pub fn is_in_water(&self, x: f32, z: f32) -> bool {
        let cx = Self::world_to_chunk(x, self.chunk_size);
        let cz = Self::world_to_chunk(z, self.chunk_size);
        if let Some(chunk) = self.chunks.get(&(cx, cz)) {
            chunk.voxel.surface_block_at(x, z) == Some(procgen::BlockId::Water)
        } else {
            false
        }
    }

    /// Effective walkable height (terrain or water surface). Use for spawn and object collision.
    pub fn walkable_height(&self, x: f32, z: f32) -> f32 {
        let terrain_y = self.sample_height(x, z);
        let water_level = self.water_level().unwrap_or(f32::NEG_INFINITY);
        terrain_y.max(water_level)
    }

    /// Sample terrain height, using fallback when chunk isn't loaded (avoids spawning in floor).
    fn sample_height_or(&self, x: f32, z: f32, fallback: f32) -> f32 {
        let cx = Self::world_to_chunk(x, self.chunk_size);
        let cz = Self::world_to_chunk(z, self.chunk_size);
        if let Some(chunk) = self.chunks.get(&(cx, cz)) {
            chunk.voxel.sample_height(x, z)
        } else {
            fallback
        }
    }

    /// Simulate terrain collapse (sand/gravel physics). No-op for voxel terrain; returns keys to rebuild.
    fn simulate_terrain_collapse(
        &mut self,
        chunk_keys: &[(i32, i32)],
        _device: &wgpu::Device,
        _physics: &mut PhysicsWorld,
    ) -> Vec<(i32, i32)> {
        // Voxel terrain: no heightfield collapse; just return deformed chunks for rebuild
        chunk_keys.to_vec()
    }

    /// Sync height at shared edges between modified chunks. No-op for voxel; returns keys to rebuild.
    fn sync_chunk_edge_heights(&mut self, modified_keys: &[(i32, i32)]) -> Vec<(i32, i32)> {
        modified_keys.to_vec()
    }

    /// Flatten terrain inside a circle to a single height (e.g. city core). Returns chunk keys modified.
    fn flatten_circle(
        &mut self,
        center_x: f32,
        center_z: f32,
        radius: f32,
        flat_height: f32,
    ) -> Vec<(i32, i32)> {
        let r2 = radius * radius;
        let min_cx = Self::world_to_chunk(center_x - radius, self.chunk_size);
        let max_cx = Self::world_to_chunk(center_x + radius, self.chunk_size);
        let min_cz = Self::world_to_chunk(center_z - radius, self.chunk_size);
        let max_cz = Self::world_to_chunk(center_z + radius, self.chunk_size);
        let mut modified = Vec::new();
        for cz in min_cz..=max_cz {
            for cx in min_cx..=max_cx {
                let key = (cx, cz);
                let Some(chunk) = self.chunks.get_mut(&key) else { continue };
                let mut any = false;
                for iz in 0..chunk.voxel.nz {
                    for ix in 0..chunk.voxel.nx {
                        let wx = chunk.voxel.world_x(ix);
                        let wz = chunk.voxel.world_z(iz);
                        let dx = wx - center_x;
                        let dz = wz - center_z;
                        if dx * dx + dz * dz <= r2 {
                            if chunk.voxel.set_column_height(ix, iz, flat_height) {
                                any = true;
                            }
                        }
                    }
                }
                if any {
                    modified.push(key);
                }
            }
        }
        modified
    }

    /// Flatten terrain in an axis-aligned rectangle to a single height (e.g. building plot).
    /// Returns chunk keys that were modified (caller should sync edges and rebuild).
    fn flatten_rect(
        &mut self,
        min_x: f32,
        max_x: f32,
        min_z: f32,
        max_z: f32,
        flat_height: f32,
    ) -> Vec<(i32, i32)> {
        let min_cx = Self::world_to_chunk(min_x, self.chunk_size);
        let max_cx = Self::world_to_chunk(max_x, self.chunk_size);
        let min_cz = Self::world_to_chunk(min_z, self.chunk_size);
        let max_cz = Self::world_to_chunk(max_z, self.chunk_size);
        let mut modified = Vec::new();
        for cz in min_cz..=max_cz {
            for cx in min_cx..=max_cx {
                let key = (cx, cz);
                let Some(chunk) = self.chunks.get_mut(&key) else { continue };
                let mut any = false;
                for iz in 0..chunk.voxel.nz {
                    for ix in 0..chunk.voxel.nx {
                        let wx = chunk.voxel.world_x(ix);
                        let wz = chunk.voxel.world_z(iz);
                        if wx >= min_x && wx <= max_x && wz >= min_z && wz <= max_z {
                            if chunk.voxel.set_column_height(ix, iz, flat_height) {
                                any = true;
                            }
                        }
                    }
                }
                if any {
                    modified.push(key);
                }
            }
        }
        modified
    }

    /// Flatten terrain in a rotated road segment (center, half extents, rotation) to a single height.
    fn flatten_road_segment(
        &mut self,
        cx: f32,
        cz: f32,
        half_len: f32,
        half_w: f32,
        rotation_y_rad: f32,
        flat_height: f32,
    ) -> Vec<(i32, i32)> {
        let c = rotation_y_rad.cos();
        let s = rotation_y_rad.sin();
        let extent = half_len + half_w;
        let min_x = cx - extent;
        let max_x = cx + extent;
        let min_z = cz - extent;
        let max_z = cz + extent;
        let min_cx = Self::world_to_chunk(min_x, self.chunk_size);
        let max_cx = Self::world_to_chunk(max_x, self.chunk_size);
        let min_cz = Self::world_to_chunk(min_z, self.chunk_size);
        let max_cz = Self::world_to_chunk(max_z, self.chunk_size);
        let mut modified = Vec::new();
        for cz_key in min_cz..=max_cz {
            for cx_key in min_cx..=max_cx {
                let key = (cx_key, cz_key);
                let Some(chunk) = self.chunks.get_mut(&key) else { continue };
                let mut any = false;
                for iz in 0..chunk.voxel.nz {
                    for ix in 0..chunk.voxel.nx {
                        let world_x = chunk.voxel.world_x(ix);
                        let world_z = chunk.voxel.world_z(iz);
                        let rel_x = world_x - cx;
                        let rel_z = world_z - cz;
                        let local_along = rel_x * s + rel_z * c;
                        let local_across = -rel_x * c + rel_z * s;
                        if local_along.abs() <= half_len && local_across.abs() <= half_w {
                            if chunk.voxel.set_column_height(ix, iz, flat_height) {
                                any = true;
                            }
                        }
                    }
                }
                if any {
                    modified.push(key);
                }
            }
        }
        modified
    }

    /// Rebuild mesh and collider for a chunk after terrain modification.
    fn rebuild_chunk_mesh_and_collider(
        &mut self,
        key: (i32, i32),
        device: &wgpu::Device,
        physics: &mut PhysicsWorld,
    ) {
        if let Some(chunk) = self.chunks.get_mut(&key) {
            let (terrain_vertices, terrain_indices) = chunk.voxel.to_mesh();
            let vertices: Vec<renderer::Vertex> = terrain_vertices
                .iter()
                .map(|v| renderer::Vertex {
                    position: v.position,
                    normal: v.normal,
                    tex_coords: v.uv,
                    color: v.color,
                })
                .collect();
            chunk.mesh = Mesh::from_data(device, &vertices, &terrain_indices);
            let (water_vertices, water_indices) = chunk.voxel.to_water_mesh();
            chunk.water_mesh = if water_vertices.is_empty() {
                None
            } else {
                let wv: Vec<renderer::Vertex> = water_vertices
                    .iter()
                    .map(|v| renderer::Vertex {
                        position: v.position,
                        normal: v.normal,
                        tex_coords: v.uv,
                        color: v.color,
                    })
                    .collect();
                Some(Mesh::from_data(device, &wv, &water_indices))
            };
            physics.remove_collider(chunk.collider_handle);
            let heightmap = chunk.voxel.to_heightmap();
            let nrows = chunk.voxel.nz + 1;
            let ncols = chunk.voxel.nx + 1;
            let offset_min_x = chunk.voxel.offset_x - self.chunk_size * 0.5;
            let offset_min_z = chunk.voxel.offset_z - self.chunk_size * 0.5;
            chunk.collider_handle = physics.add_terrain_heightfield_at(
                &heightmap,
                nrows,
                ncols,
                self.chunk_size,
                self.chunk_size,
                offset_min_x,
                offset_min_z,
            );
        }
    }

    /// Apply crater deformation at a world position. Rebuilds mesh + collider for affected chunks.
    fn deform_at(
        &mut self,
        world_pos: Vec3,
        radius: f32,
        _depth: f32,
        _device: &wgpu::Device,
        _physics: &mut PhysicsWorld,
    ) {
        let min_cx = Self::world_to_chunk(world_pos.x - radius, self.chunk_size);
        let max_cx = Self::world_to_chunk(world_pos.x + radius, self.chunk_size);
        let min_cz = Self::world_to_chunk(world_pos.z - radius, self.chunk_size);
        let max_cz = Self::world_to_chunk(world_pos.z + radius, self.chunk_size);

        let mut affected_keys = Vec::new();
        for cz in min_cz..=max_cz {
            for cx in min_cx..=max_cx {
                if let Some(chunk) = self.chunks.get_mut(&(cx, cz)) {
                    if chunk.voxel.deform_sphere(
                        world_pos.x,
                        world_pos.y,
                        world_pos.z,
                        radius,
                    ) {
                        affected_keys.push((cx, cz));
                    }
                }
            }
        }
        if !affected_keys.is_empty() {
            let to_rebuild = self.sync_chunk_edge_heights(&affected_keys);
            self.pending_chunk_rebuilds.extend(to_rebuild);
        }
    }

    /// Ace of Spades–style blocky dig: one block removed at the cell containing world_pos.
    /// If water_level is Some, water fills the crater below that world Y (flowing physics).
    /// Rebuilds mesh + collider for affected chunks.
    fn deform_at_blocky(
        &mut self,
        world_pos: Vec3,
        block_size: f32,
        device: &wgpu::Device,
        physics: &mut PhysicsWorld,
        water_level: Option<f32>,
    ) {
        let radius = block_size;
        let min_cx = Self::world_to_chunk(world_pos.x - radius, self.chunk_size);
        let max_cx = Self::world_to_chunk(world_pos.x + radius, self.chunk_size);
        let min_cz = Self::world_to_chunk(world_pos.z - radius, self.chunk_size);
        let max_cz = Self::world_to_chunk(world_pos.z + radius, self.chunk_size);

        let mut affected_keys = Vec::new();
        for cz in min_cz..=max_cz {
            for cx in min_cx..=max_cx {
                if let Some(chunk) = self.chunks.get_mut(&(cx, cz)) {
                    if chunk.voxel.deform_sphere(
                        world_pos.x,
                        world_pos.y,
                        world_pos.z,
                        radius,
                    ) {
                        if let Some(wl) = water_level {
                            chunk.voxel.fill_water_in_sphere_below(
                                world_pos.x,
                                world_pos.y,
                                world_pos.z,
                                radius,
                                wl,
                            );
                        }
                        affected_keys.push((cx, cz));
                    }
                }
            }
        }
        if !affected_keys.is_empty() {
            let to_rebuild = self.sync_chunk_edge_heights(&affected_keys);
            self.pending_chunk_rebuilds.extend(to_rebuild);
        }
    }

    /// Blocky mound: raise one block at the cell containing world_pos (excavated dirt pile).
    fn deform_mound_at_blocky(
        &mut self,
        world_pos: Vec3,
        block_size: f32,
        _device: &wgpu::Device,
        _physics: &mut PhysicsWorld,
    ) {
        let radius = block_size;
        let min_cx = Self::world_to_chunk(world_pos.x - radius, self.chunk_size);
        let max_cx = Self::world_to_chunk(world_pos.x + radius, self.chunk_size);
        let min_cz = Self::world_to_chunk(world_pos.z - radius, self.chunk_size);
        let max_cz = Self::world_to_chunk(world_pos.z + radius, self.chunk_size);

        let mut affected_keys = Vec::new();
        for cz in min_cz..=max_cz {
            for cx in min_cx..=max_cx {
                if let Some(chunk) = self.chunks.get_mut(&(cx, cz)) {
                    if chunk.voxel.fill_sphere(
                        world_pos.x,
                        world_pos.y,
                        world_pos.z,
                        radius,
                        procgen::BlockId::Dirt,
                    ) {
                        affected_keys.push((cx, cz));
                    }
                }
            }
        }
        if !affected_keys.is_empty() {
            let to_rebuild = self.sync_chunk_edge_heights(&affected_keys);
            self.pending_chunk_rebuilds.extend(to_rebuild);
        }
    }

    /// Process up to `max_per_frame` pending chunk mesh+collider rebuilds (reduces artillery lag).
    fn process_pending_rebuilds(
        &mut self,
        device: &wgpu::Device,
        physics: &mut PhysicsWorld,
        max_per_frame: usize,
    ) {
        let n = self.pending_chunk_rebuilds.len().min(max_per_frame);
        for _ in 0..n {
            if let Some(key) = self.pending_chunk_rebuilds.pop() {
                self.rebuild_chunk_mesh_and_collider(key, device, physics);
            }
        }
    }

    /// Apply mound deformation (raise terrain) at a world position. Rebuilds mesh + collider.
    fn deform_mound_at(
        &mut self,
        world_pos: Vec3,
        radius: f32,
        _height: f32,
        _device: &wgpu::Device,
        _physics: &mut PhysicsWorld,
    ) {
        let min_cx = Self::world_to_chunk(world_pos.x - radius, self.chunk_size);
        let max_cx = Self::world_to_chunk(world_pos.x + radius, self.chunk_size);
        let min_cz = Self::world_to_chunk(world_pos.z - radius, self.chunk_size);
        let max_cz = Self::world_to_chunk(world_pos.z + radius, self.chunk_size);

        let mut affected_keys = Vec::new();
        for cz in min_cz..=max_cz {
            for cx in min_cx..=max_cx {
                if let Some(chunk) = self.chunks.get_mut(&(cx, cz)) {
                    if chunk.voxel.fill_sphere(
                        world_pos.x,
                        world_pos.y,
                        world_pos.z,
                        radius,
                        procgen::BlockId::Dirt,
                    ) {
                        affected_keys.push((cx, cz));
                    }
                }
            }
        }
        if !affected_keys.is_empty() {
            let to_rebuild = self.sync_chunk_edge_heights(&affected_keys);
            self.pending_chunk_rebuilds.extend(to_rebuild);
        }
    }

    /// Check if a collider handle belongs to a terrain chunk.
    fn is_terrain_collider(&self, handle: ColliderHandle) -> bool {
        self.chunks.values().any(|chunk| chunk.collider_handle == handle)
    }

    /// Render visible chunks with frustum culling. Call after update_terrain uniform.
    fn render_visible(
        &self,
        renderer: &Renderer,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        camera: &Camera,
    ) {
        let cam_pos = camera.position();
        let cam_fwd = camera.forward();
        // Horizontal-only forward for ground-level culling
        let cam_fwd_h = Vec3::new(cam_fwd.x, 0.0, cam_fwd.z).normalize_or_zero();
        let near_dist_sq = (self.chunk_size * 1.5) * (self.chunk_size * 1.5);

        for (&(cx, cz), chunk) in &self.chunks {
            let chunk_center = Vec3::new(
                cx as f32 * self.chunk_size,
                0.0,
                cz as f32 * self.chunk_size,
            );
            let to_chunk = Vec3::new(
                chunk_center.x - cam_pos.x,
                0.0,
                chunk_center.z - cam_pos.z,
            );
            let dist_sq = to_chunk.length_squared();

            // Always render the nearest chunks (player is standing on them)
            if dist_sq > near_dist_sq {
                // Frustum cull: skip chunks that are behind the camera
                let dot = to_chunk.normalize_or_zero().dot(cam_fwd_h);
                if dot < -0.3 {
                    continue;
                }
            }

            renderer.render_terrain(encoder, view, &chunk.mesh);
            if let Some(ref water_mesh) = chunk.water_mesh {
                renderer.render_water(encoder, view, water_mesh);
            }
        }
    }

    /// Render visible terrain chunks into the shadow map. Same culling as render_visible.
    fn render_visible_shadow(
        &self,
        renderer: &Renderer,
        pass: &mut wgpu::RenderPass,
        camera: &Camera,
    ) {
        let cam_pos = camera.position();
        let cam_fwd = camera.forward();
        let cam_fwd_h = Vec3::new(cam_fwd.x, 0.0, cam_fwd.z).normalize_or_zero();
        let near_dist_sq = (self.chunk_size * 1.5) * (self.chunk_size * 1.5);

        for (&(cx, cz), chunk) in &self.chunks {
            let chunk_center = Vec3::new(
                cx as f32 * self.chunk_size,
                0.0,
                cz as f32 * self.chunk_size,
            );
            let to_chunk = Vec3::new(
                chunk_center.x - cam_pos.x,
                0.0,
                chunk_center.z - cam_pos.z,
            );
            let dist_sq = to_chunk.length_squared();

            if dist_sq > near_dist_sq {
                let dot = to_chunk.normalize_or_zero().dot(cam_fwd_h);
                if dot < -0.3 {
                    continue;
                }
            }

            renderer.render_terrain_shadow(pass, &chunk.mesh);
        }
    }
}

impl GameState {
    async fn new(window: Arc<Window>) -> Result<Self> {
        // Initialize renderer
        let renderer = Renderer::new(window.clone()).await?;

        // Create camera
        let mut camera = Camera::new(Vec3::new(0.0, 2.0, 10.0));
        let (width, height) = renderer.dimensions();
        camera.set_aspect(width, height);

        // Create procedural meshes
        let bug_meshes = AuthoredBugMeshes::new(renderer.device());
        let environment_meshes = EnvironmentMeshes::new(renderer.device());
        
        // Gore splatter mesh (flat quad)
        let gore_mesh = Mesh::plane(renderer.device(), 1.0);
        
        // Particle mesh (small quad for billboards)
        let particle_mesh = Mesh::plane(renderer.device(), 0.1);

        // Proper bullet tracer mesh (elongated diamond shape, not a flat quad!)
        let tracer_mesh = Mesh::bullet_tracer(renderer.device());

        // Muzzle flash mesh (multi-pointed star visible from any angle)
        let flash_mesh = Mesh::muzzle_flash(renderer.device());

        // Billboard quad for camera-facing particles (dust, sparks, etc.)
        let billboard_mesh = Mesh::billboard_quad(renderer.device(), 1.0);

        // Initialize ECS world
        let mut world = World::new();

        // Initialize physics
        let mut physics = PhysicsWorld::new();

        // Generate universe and initial star system (or load persisted galactic war)
        let universe_seed: u64 = 42;
        let mut universe = Universe::generate(universe_seed, 100);
        let mut current_system_idx = 0;
        let mut current_system = universe.generate_system(current_system_idx);
        let num_system_planets = current_system.bodies.len();
        let mut war_state_initial = GalacticWarState::new(num_system_planets);

        let mut effective_seed = universe_seed;
        let mut has_save = false;
        if let Some((saved_seed, saved_sys_idx, saved_war)) = load_galactic_war() {
            universe = Universe::generate(saved_seed, 100);
            current_system = universe.generate_system(saved_sys_idx);
            current_system_idx = saved_sys_idx;
            effective_seed = saved_seed;
            has_save = true;
            if saved_war.planets.len() == current_system.bodies.len() {
                war_state_initial = saved_war;
            }
        }

        // Land on the first planet in the system
        let first_planet_idx = 0;
        let planet = current_system.bodies[first_planet_idx].planet.clone();
        let initial_biome = planet.primary_biome;

        let biome_config = planet.get_biome_config();
        let planet_biomes = planet.biome_sampler();

        // Create ChunkManager for infinite terrain. Earth: terraformed (gentler, smooth).
        let (init_height, init_freq, init_smooth) = if planet.name == "Earth" {
            (10.0, 0.012, true)
        } else {
            (
                15.0 * biome_config.height_scale,
                0.02 * biome_config.frequency_scale as f64,
                false,
            )
        };
        let mut chunk_manager = ChunkManager::new(
            planet.seed,
            init_height,
            init_freq,
            planet_biomes,
            init_smooth,
        );
        // Pre-load chunks around the origin so the player has terrain at spawn
        chunk_manager.update(Vec3::ZERO, renderer.device(), &mut physics);

        // Initialize FPS player on terrain (Hunter class by default); use walkable height to avoid spawning underwater
        let spawn_y = chunk_manager.walkable_height(0.0, 0.0) + 1.8;
        let spawn_pos = Vec3::new(0.0, spawn_y, 0.0);
        let player = FPSPlayer::new(
            PlayerClass::Hunter,
            "Trooper".to_string(),
            spawn_pos,
        );

        // Start camera at player position (on terrain)
        camera.transform.position = spawn_pos;

        // Create flow field for AI
        let flow_field = FlowField::new(100, 100, 2.0, glam::Vec2::new(-100.0, -100.0));
        let horde_ai = HordeAI::new(flow_field);

        // Bug spawner (planet danger sets bug count and mix; spawn rate from planet.bug_spawn_rate())
        let mut spawner = BugSpawner::new(planet.bug_spawn_rate(), planet.danger_level);
        let biome_table = get_biome_feature_table(planet.primary_biome);
        spawner.set_biome_variant(biome_table.bug_variant, biome_table.variant_chance);

        // Mission state (infinite horde)
        let mission = MissionState::new_horde();

        // Biome content (rocks, bug holes, etc.) will be spawned after GameState is constructed

        let mut game: Result<Self> = Ok(Self {
            world,
            time: Time::new(),
            input: InputState::new(),
            physics,
            renderer,
            camera,
            bug_meshes,
            environment_meshes,
            gore_mesh,
            particle_mesh,
            tracer_mesh,
            flash_mesh,
            billboard_mesh,
            deformation_buffer: vec![0.0; (DEFORM_TEXTURE_SIZE * DEFORM_TEXTURE_SIZE) as usize],
            snow_accumulation_buffer: vec![0.0; (DEFORM_TEXTURE_SIZE * DEFORM_TEXTURE_SIZE) as usize],
            snow_accumulation_origin: (0.0, 0.0),
            effects: EffectsManager::new(),
            player,
            combat: CombatSystem::new(),
            bug_combat: BugCombatSystem::new(),
            hud: HUDSystem::new(),
            mission,
            horde_ai,
            spawner,
            weapon_system: WeaponSystem::new(),
            chunk_manager,
            planet,
            universe_seed: effective_seed,
            universe,
            current_system,
            current_system_idx,
            current_planet_idx: Some(first_planet_idx),
            universe_position: DVec3::ZERO,
            orbital_time: 0.0,
            universe_time_sec: 0.0,
            galaxy_map_open: false,
            galaxy_map_selected: 0,
            warp_sequence: None,
            warp_start_galaxy_position: None,
            warp_return_to_ship: false,
            drop_pod: None,
            squad_drop_pods: None,
            ship_state: None,
            deploy_planet_idx: None,
            approach_timer: 0.0,
            approach_flight_state: None,
            war_state: war_state_initial,
            settlement_center: None,
            earth_waypoints: None,
            earth_roads_mesh: None,
            earth_road_colliders: Vec::new(),
            earth_building_colliders: Vec::new(),
            dialogue_state: DialogueState::default(),
            interaction_prompt: None,
            time_of_day: 0.25,  // start at noon
            weather: Weather::new(),
            rain_drops: Vec::new(),
            snow_particles: Vec::new(),
            destruction: DestructionSystem::new(),
            game_messages: GameMessages::new(),
            phase: GamePhase::MainMenu,
            main_menu_selected: 0,
            main_menu_galaxy_open: false,
            has_save,
            pause_menu_selected: 0,
            previous_phase: None,
            running: true,
            smoothed_dt: 1.0 / 60.0,
            total_gore_spawned: 0,
            physics_bodies_active: 0,
            tracer_projectiles: Vec::new(),
            debug: DebugSettings::new(),
            player_velocity: Vec3::ZERO,
            player_grounded: false,
            hazard_slow_multiplier: 1.0,
            last_player_track_pos: None,
            ground_track_bug_timer: 0.0,
            squad_track_last: HashMap::new(),
            shovel_dig_cooldown: 0.0,
            screen_shake: ScreenShake::new(),
            camera_recoil: 0.0,
            crouch_hold_timer: 0.0,
            kill_streaks: KillStreakTracker::new(),
            ambient_dust: AmbientDust::new(),
            biome_atmosphere: BiomeAtmosphere::new(initial_biome),

            viewmodel_anim: ViewmodelAnimState::new(),
            shell_casings: Vec::new(),
            grounded_shell_casings: Vec::new(),

            smoke_grenades: Vec::new(),
            smoke_clouds: Vec::new(),
            smoke_grenade_cooldown: 0.0,

            tac_fighters: Vec::new(),
            tac_bombs: Vec::new(),
            tac_fighter_cooldown: 45.0, // First tac fighter after 45 seconds
            tac_fighter_available: true,

            artillery_shells: Vec::new(),
            artillery_muzzle_flashes: Vec::new(),
            artillery_trail_particles: Vec::new(),
            grounded_artillery_shells: Vec::new(),
            artillery_barrage: None,
            artillery_cooldown: 0.0,

            supply_crates: Vec::new(),
            supply_drop_cooldown: 0.0,
            supply_drop_smoke: Vec::new(),
            reinforce_cooldown: 0.0,
            reinforce_smoke: None,
            orbital_strike_smoke: None,

    extraction: None,
    extraction_cooldown: 0.0,
    extraction_squadmates_aboard: Vec::new(),
    extraction_collider: None,
    lz_smoke: None,
    next_mission_type: fps::MissionType::Extermination,
    defense_base: None,
});

        if let Ok(ref mut state) = game {
            // Reduce physics tick rate to 30Hz (bugs are kinematic; ragdolls are fine at 30)
            state.time.set_fixed_rate(30.0);

            // Main menu: camera in space looking at planet orbit (Starship Troopers 2005 style)
            state.current_planet_idx = None; // See all celestial bodies from orbit
            state.camera.transform.position = Vec3::new(0.0, 0.0, 1200.0);
            state.camera.set_yaw_pitch(0.0, -0.15); // Look slightly down toward planet

            // Cursor visible for menu selection
            state.renderer.window.set_cursor_visible(true);
            let _ = state.renderer.window.set_cursor_grab(CursorGrabMode::None);
        }

        game
    }

    fn update(&mut self) {
        self.time.update();
        let raw_dt = self.time.delta_seconds();
        // Cap delta to avoid huge steps from hitches (keeps motion consistent).
        let capped = (raw_dt * self.debug.time_scale).min(0.05);
        // Smooth delta so brief frame spikes don't cause one jerky frame. Use 0.4 (was 0.2) so
        // the game responds faster to frame time changes — overly aggressive smoothing can make
        // the game feel laggy even at high FPS.
        const SMOOTH: f32 = 0.4;
        self.smoothed_dt = self.smoothed_dt * (1.0 - SMOOTH) + capped * SMOOTH;
        let dt = self.smoothed_dt;

        // Process debug actions (execute one-shot requests)
        self.process_debug_actions();

        // Time of day: real-time dynamic cycle from star position and planet rotation (per-system, per-planet)
        if !self.debug.freeze_time_of_day {
            let (_, tod) = self.compute_sun_direction_and_time_of_day(self.current_planet_idx);
            self.time_of_day = tod;
        }
        self.weather.update(dt);

        // Persist time/weather to current planet (each planet maintains its own conditions)
        if let Some(planet_idx) = self.current_planet_idx {
            if let Some(status) = self.war_state.planets.get_mut(planet_idx) {
                status.time_of_day = self.time_of_day;
                status.weather = self.weather.clone();
            }
        }

        self.interaction_prompt = None;

        match self.phase {
            GamePhase::MainMenu => self.update_main_menu(dt),
            GamePhase::InShip => self.update_ship(dt),
            GamePhase::ApproachPlanet => self.update_approach(dt),
            GamePhase::DropSequence => self.update_drop_sequence(dt),
            GamePhase::Playing => self.update_gameplay(dt),
            GamePhase::Paused => self.update_paused(dt),
            GamePhase::Victory | GamePhase::Defeat => {
                self.update_camera_only(dt);
            }
            _ => {}
        }

        // Sync camera to renderer for phases that update it in their update (DropSequence does its own).
        if self.phase == GamePhase::DropSequence {
            self.renderer.update_camera(&self.camera, self.planet_radius_for_curvature());
        }

        // Dialogue input: run every frame when dialogue is open (ship or Earth) so Escape and 1–4 work in both.
        if self.dialogue_state.is_open() {
            if self.input.is_key_pressed(KeyCode::Escape) {
                self.dialogue_state = DialogueState::Closed;
            } else {
                let idx = if self.input.is_key_pressed(KeyCode::Digit1) { 0 }
                    else if self.input.is_key_pressed(KeyCode::Digit2) { 1 }
                    else if self.input.is_key_pressed(KeyCode::Digit3) { 2 }
                    else if self.input.is_key_pressed(KeyCode::Digit4) { 3 }
                    else { 4 };
                if idx < 4 {
                    if let Some((_, choices)) = self.dialogue_state.current_line_and_choices() {
                        if idx < choices.len() {
                            self.dialogue_state.select_choice(idx);
                        }
                    }
                }
            }
        }

        // Clear input for next frame
        self.input.begin_frame();
    }

    /// Process one-shot debug actions (kill all bugs, teleport, etc.).
    fn process_debug_actions(&mut self) {
        if self.debug.kill_all_bugs_requested {
            self.debug.kill_all_bugs_requested = false;
            let mut killed = 0u32;
            for (_, health) in self.world.query_mut::<&mut Health>() {
                health.take_damage(10000.0);
                killed += 1;
            }
            #[cfg(debug_assertions)]
            self.game_messages.warning(format!("[DEBUG] Killed {} entities", killed));
        }

        if self.debug.teleport_origin_requested {
            self.debug.teleport_origin_requested = false;
            if self.current_planet_idx.is_some() {
                let y = self.chunk_manager.sample_height(0.0, 0.0) + 3.0;
                self.camera.transform.position = Vec3::new(0.0, y, 0.0);
                self.player.position = self.camera.transform.position;
                self.player_velocity = Vec3::ZERO;
                #[cfg(debug_assertions)]
                self.game_messages.info("[DEBUG] Teleported to origin");
            }
        }

        // God mode: heal player every frame
        if self.debug.god_mode && self.player.health < self.player.max_health {
            self.player.health = self.player.max_health;
            self.player.armor = self.player.max_armor;
        }

        // Infinite ammo: refill every frame
        if self.debug.infinite_ammo {
            for weapon in &mut self.player.weapons {
                weapon.current_ammo = weapon.magazine_size;
                weapon.reserve_ammo = weapon.magazine_size * 10;
            }
        }
    }

    /// Update main menu: Continue/Play, Universe Map, Quit. Universe Map opens galaxy; Enter = travel and board.
    fn update_main_menu(&mut self, dt: f32) {
        if self.main_menu_galaxy_open {
            // Galaxy map from main menu: M = close, arrows = select system, Enter = travel to system and board Roger Young
            let num_systems = self.universe.systems.len();
            if self.input.is_key_pressed(KeyCode::KeyM) || self.input.is_key_pressed(KeyCode::Escape) {
                self.main_menu_galaxy_open = false;
                self.galaxy_map_open = false;
            } else if num_systems > 0 {
                if self.input.is_key_pressed(KeyCode::ArrowUp) || self.input.is_key_pressed(KeyCode::KeyW) {
                    self.galaxy_map_selected = if self.galaxy_map_selected == 0 { num_systems - 1 } else { self.galaxy_map_selected - 1 };
                }
                if self.input.is_key_pressed(KeyCode::ArrowDown) || self.input.is_key_pressed(KeyCode::KeyS) {
                    self.galaxy_map_selected = (self.galaxy_map_selected + 1) % num_systems;
                }
                if self.input.is_key_pressed(KeyCode::Enter) || self.input.is_key_pressed(KeyCode::Space) {
                    // Travel to selected system and board ship (Star Citizen style: pick destination then board)
                    self.current_system_idx = self.galaxy_map_selected;
                    self.current_system = self.universe.generate_system(self.galaxy_map_selected);
                    let num_planets = self.current_system.bodies.len();
                    self.war_state = GalacticWarState::new(num_planets);
                    self.current_planet_idx = Some(0);
                    self.planet = self.current_system.bodies[0].planet.clone();
                    self.main_menu_galaxy_open = false;
                    self.galaxy_map_open = false;
                    self.begin_ship_phase(0);
                    let _ = self.renderer.window.set_cursor_grab(CursorGrabMode::Locked)
                        .or_else(|_| self.renderer.window.set_cursor_grab(CursorGrabMode::Confined));
                    self.renderer.window.set_cursor_visible(false);
                    self.input.set_cursor_locked(true);
                self.game_messages.info(format!("FEDERATION DESTROYER \"ROGER YOUNG\" - {} SYSTEM", self.current_system.name));
                self.game_messages.info(format!("Star: {} ({:?}) | {} planets", self.current_system.star.name, self.current_system.star.star_type, num_planets));
                self.game_messages.info("Approach the WAR TABLE [E] — pick planet and mission. Drop bay is aft.");
                self.game_messages.warning("Press [SPACE] to deploy drop pod!");
                }
            }
            self.game_messages.update(dt);
            return;
        }

        // Menu navigation: Up/Down or W/S (3 items: Continue/Play, Universe Map, Quit)
        if self.input.is_key_pressed(KeyCode::ArrowUp) || self.input.is_key_pressed(KeyCode::KeyW) {
            self.main_menu_selected = self.main_menu_selected.saturating_sub(1);
        }
        if self.input.is_key_pressed(KeyCode::ArrowDown) || self.input.is_key_pressed(KeyCode::KeyS) {
            self.main_menu_selected = (self.main_menu_selected + 1).min(2);
        }

        // Select: Enter, Space, or Left Click
        if self.input.is_key_pressed(KeyCode::Enter)
            || self.input.is_key_pressed(KeyCode::Space)
            || self.input.is_mouse_pressed(winit::event::MouseButton::Left)
        {
            if self.main_menu_selected == 0 {
                // Continue / Play — transition to ship interior (lock cursor for FPS)
                self.current_planet_idx = Some(0);
                self.planet = self.current_system.bodies[0].planet.clone();
                let first_planet = 0;
                self.begin_ship_phase(first_planet);
                let _ = self.renderer.window.set_cursor_grab(CursorGrabMode::Locked)
                    .or_else(|_| self.renderer.window.set_cursor_grab(CursorGrabMode::Confined));
                self.renderer.window.set_cursor_visible(false);
                self.input.set_cursor_locked(true);
                let (biome_display, danger_display) = if self.planet.name == "Earth" {
                    (self.chunk_manager.planet_biomes.biomes.iter().map(|b| format!("{:?}", b)).collect::<Vec<_>>().join(", "), "—".to_string())
                } else if self.planet.has_unknown_intel {
                    ("???".to_string(), "???".to_string())
                } else {
                    let biomes: String = self.chunk_manager.planet_biomes.biomes.iter().map(|b| format!("{:?}", b)).collect::<Vec<_>>().join(", ");
                    (biomes, self.planet.danger_level.to_string())
                };
                self.game_messages.info(format!("FEDERATION DESTROYER \"ROGER YOUNG\" - {} SYSTEM", self.current_system.name));
                self.game_messages.info(format!("Star: {} ({:?}) | {} planets", self.current_system.star.name, self.current_system.star.star_type, self.current_system.bodies.len()));
                self.game_messages.info(format!("TARGET: {} | Biomes: {} | Danger: {}", self.planet.name, biome_display, danger_display));
                self.game_messages.warning("Press [SPACE] to deploy drop pod!");
            } else if self.main_menu_selected == 1 {
                // Universe Map — open galaxy (Star Citizen style: choose system then board)
                self.main_menu_galaxy_open = true;
                self.galaxy_map_open = true;
                self.galaxy_map_selected = self.current_system_idx;
            } else {
                // Quit
                self.running = false;
            }
        }

        // Escape = Quit from menu (only when not in galaxy sub-view)
        if self.input.is_key_pressed(KeyCode::Escape) {
            self.running = false;
        }

        self.game_messages.update(dt);
    }

    /// Update when paused: only menu input and message decay.
    fn update_paused(&mut self, dt: f32) {
        self.game_messages.update(dt);
    }

    /// Transition to main menu (from pause or ship). Resets cursor and menu state.
    /// Clears terrain and world so returning to Play doesn't show stale planet content in the ship.
    fn transition_to_main_menu(&mut self) {
        self.phase = GamePhase::MainMenu;
        self.main_menu_selected = 0;
        self.main_menu_galaxy_open = false;
        self.galaxy_map_open = false;
        self.pause_menu_selected = 0;
        self.previous_phase = None;
        self.ship_state = None;
        self.drop_pod = None;
        self.extraction = None;
        self.current_planet_idx = None;
        self.camera.transform.position = Vec3::new(0.0, 0.0, 1200.0);
        self.camera.set_yaw_pitch(0.0, -0.15);
        self.renderer.window.set_cursor_visible(true);
        let _ = self.renderer.window.set_cursor_grab(CursorGrabMode::None);
        self.input.set_cursor_locked(false);

        // Clear terrain and world so "Play" -> ship doesn't show previous mission's terrain/corpses
        self.chunk_manager.clear_all(&mut self.physics);
        let all_entities: Vec<hecs::Entity> = self.world.iter().map(|e| e.entity()).collect();
        for entity in all_entities {
            let _ = self.world.despawn(entity);
        }
        self.effects = EffectsManager::new();
        self.rain_drops.clear();
        self.snow_particles.clear();
        self.artillery_shells.clear();
        self.artillery_muzzle_flashes.clear();
        self.artillery_trail_particles.clear();
        self.grounded_artillery_shells.clear();
        for c in self.shell_casings.drain(..) {
            self.physics.remove_body(c.body_handle);
        }
        for s in self.grounded_shell_casings.drain(..) {
            self.physics.remove_body(s.body_handle);
        }
        self.artillery_barrage = None;
        self.extraction_squadmates_aboard.clear();
        self.last_player_track_pos = None;
        self.ground_track_bug_timer = 0.0;
        self.squad_track_last.clear();
    }

    /// Update while aboard the Federation destroyer.
    fn update_ship(&mut self, dt: f32) {
        // FTL from war table / galaxy map: Roger Young actually warps through galaxy space with visual feedback
        if let Some(ref mut warp) = self.warp_sequence {
            warp.timer += dt;
            // Capture start position once (ship's galaxy position when warp began)
            if self.warp_start_galaxy_position.is_none() {
                self.warp_start_galaxy_position = Some(self.universe.systems[self.current_system_idx].position);
            }
            let start = self.warp_start_galaxy_position.unwrap_or(self.universe.systems[self.current_system_idx].position);
            let target_pos = self.universe.systems[warp.target_system_idx].position;
            // Smooth ease: fast in middle (smooth_step)
            let t = warp.progress();
            let ease = t * t * (3.0 - 2.0 * t);
            let ease_f64 = ease as f64;
            self.universe_position = DVec3::new(
                start.x + (target_pos.x - start.x) * ease_f64,
                start.y + (target_pos.y - start.y) * ease_f64,
                start.z + (target_pos.z - start.z) * ease_f64,
            );
            // Bridge view: stand at front looking out viewscreen
            self.camera.transform.position = Vec3::new(0.0, 1.7, 10.0);
            self.camera.set_yaw_pitch(0.0, 0.0);
            self.renderer.update_camera(&self.camera, 0.0);
            if warp.is_complete() {
                let target_idx = warp.target_system_idx;
                let return_to_ship = self.warp_return_to_ship;
                self.warp_sequence = None;
                self.warp_start_galaxy_position = None;
                self.warp_return_to_ship = false;
                self.arrive_at_system(target_idx);
                if return_to_ship {
                    self.begin_ship_phase(0);
                }
            }
            self.game_messages.update(dt);
            return;
        }

        if let Some(ref mut ship) = self.ship_state {
            ship.timer += dt;
            // Update cloth flag physics
            ship.ucf_flag.update(dt);
            ship.mi_flag.update(dt);
        }

        // Update war table state
        self.war_state.update(dt);

        // Read ship state info before movement
        let war_table_active = self.ship_state.as_ref().map_or(false, |s| s.war_table_active);
        let war_table_pos = self.ship_state.as_ref().map_or(Vec3::ZERO, |s| s.war_table_pos);
        let drop_bay_pos = self.ship_state.as_ref().map_or(Vec3::ZERO, |s| s.drop_bay_pos);

        // ── FPS movement inside the ship: artificial 1G (earth-like gravity) ──
        // Floor clamp and horizontal movement simulate gravity; no zero-G in interior.
        if !war_table_active {
            // Mouse look (uses camera's built-in yaw/pitch system)
            let mouse_delta = self.input.mouse_delta();
            if self.input.is_cursor_locked() {
                self.camera.process_mouse(mouse_delta.x, mouse_delta.y);
            }

            // WASD movement (clamped to ship interior bounds)
            let speed = 5.0;
            let forward = self.camera.transform.forward();
            let right = self.camera.transform.right();
            let move_dir_forward = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
            let move_dir_right = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();

            let mut move_vec = Vec3::ZERO;
            if self.input.is_key_held(KeyCode::KeyW) { move_vec += move_dir_forward; }
            if self.input.is_key_held(KeyCode::KeyS) { move_vec -= move_dir_forward; }
            if self.input.is_key_held(KeyCode::KeyD) { move_vec += move_dir_right; }
            if self.input.is_key_held(KeyCode::KeyA) { move_vec -= move_dir_right; }

            if move_vec.length_squared() > 0.01 {
                move_vec = move_vec.normalize() * speed * dt;
                self.camera.transform.position += move_vec;
            }

            // Clamp to ship interior bounds
            // CIC main room: X[-9,9], Z[-14,14]
            // Corridor: X[-2.5,2.5], Z[-14,-25]
            // Drop bay: X[-3.5,3.5], Z[-25,-30.5]
            let pos = &mut self.camera.transform.position;
            pos.y = 1.7; // eye height
            let z = pos.z;
            if z > -14.0 {
                // Main CIC room
                pos.x = pos.x.clamp(-9.0, 9.0);
                pos.z = pos.z.clamp(-14.0, 14.0);
            } else if z > -25.0 {
                // Corridor
                pos.x = pos.x.clamp(-2.5, 2.5);
            } else {
                // Drop bay
                pos.x = pos.x.clamp(-3.5, 3.5);
                pos.z = pos.z.clamp(-30.5, -25.0);
            }

            self.player.position = self.camera.transform.position;
        }

        // ── War table interaction ──
        let dist_to_table = Vec3::new(
            self.camera.transform.position.x - war_table_pos.x,
            0.0,
            self.camera.transform.position.z - war_table_pos.z,
        ).length();

        if self.input.is_key_pressed(KeyCode::KeyE) {
            if let Some(ref mut ship) = self.ship_state {
                if ship.war_table_active {
                    ship.war_table_active = false;
                } else if dist_to_table < 4.0 {
                    ship.war_table_active = true;
                }
            }
        }

        // War table navigation (only when active)
        let num_systems = self.universe.systems.len();
        let num_planets = self.current_system.bodies.len();

        // Change star system (↑/↓ or W/Q) — FTL jump to new system (Helldivers 2 style), then return to ship
        if war_table_active && num_systems > 0 {
            let next_sys = self.input.is_key_pressed(KeyCode::ArrowUp) || self.input.is_key_pressed(KeyCode::KeyW);
            let prev_sys = self.input.is_key_pressed(KeyCode::ArrowDown) || self.input.is_key_pressed(KeyCode::KeyQ);
            if next_sys {
                let next_idx = (self.current_system_idx + 1) % num_systems;
                let sys_name = self.universe.systems[next_idx].name.clone();
                self.warp_sequence = Some(WarpSequence::new(next_idx));
                self.warp_return_to_ship = true;
                self.game_messages.info(format!("FTL jump to {}...", sys_name));
            }
            if prev_sys {
                let prev_idx = if self.current_system_idx == 0 { num_systems - 1 } else { self.current_system_idx - 1 };
                let sys_name = self.universe.systems[prev_idx].name.clone();
                self.warp_sequence = Some(WarpSequence::new(prev_idx));
                self.warp_return_to_ship = true;
                self.game_messages.info(format!("FTL jump to {}...", sys_name));
            }
        }

        if war_table_active && num_planets > 0 {
            if self.input.is_key_pressed(KeyCode::ArrowLeft) || self.input.is_key_pressed(KeyCode::KeyA) {
                self.war_state.selected_planet = if self.war_state.selected_planet == 0 {
                    num_planets - 1
                } else {
                    self.war_state.selected_planet - 1
                };
                if let Some(ref mut ship) = self.ship_state {
                    ship.target_planet_idx = self.war_state.selected_planet;
                }
                let planet = &self.current_system.bodies[self.war_state.selected_planet].planet;
                self.planet = planet.clone();
            }
            if self.input.is_key_pressed(KeyCode::ArrowRight) || self.input.is_key_pressed(KeyCode::KeyD) {
                self.war_state.selected_planet = (self.war_state.selected_planet + 1) % num_planets;
                if let Some(ref mut ship) = self.ship_state {
                    ship.target_planet_idx = self.war_state.selected_planet;
                }
                let planet = &self.current_system.bodies[self.war_state.selected_planet].planet;
                self.planet = planet.clone();
            }
            // Mission type: 1 = Extermination, 2 = Bug Hunt, 3 = Hold the Line (Helldivers 2 style)
            if self.input.is_key_pressed(KeyCode::Digit1) {
                self.next_mission_type = fps::MissionType::Extermination;
                if let Some(ref mut ship) = self.ship_state {
                    ship.selected_mission_type = fps::MissionType::Extermination;
                }
                self.game_messages.info("Mission: EXTERMINATION — Survive and extract when ready.".to_string());
            }
            if self.input.is_key_pressed(KeyCode::Digit2) {
                self.next_mission_type = fps::MissionType::BugHunt;
                if let Some(ref mut ship) = self.ship_state {
                    ship.selected_mission_type = fps::MissionType::BugHunt;
                }
                self.game_messages.info("Mission: BUG HUNT — Kill 25 bugs, then extract.".to_string());
            }
            if self.input.is_key_pressed(KeyCode::Digit3) {
                self.next_mission_type = fps::MissionType::HoldTheLine;
                if let Some(ref mut ship) = self.ship_state {
                    ship.selected_mission_type = fps::MissionType::HoldTheLine;
                }
                self.game_messages.info("Mission: HOLD THE LINE — Survive 5:00, then extract.".to_string());
            }
            if self.input.is_key_pressed(KeyCode::Digit4) {
                self.next_mission_type = fps::MissionType::Defense;
                if let Some(ref mut ship) = self.ship_state {
                    ship.selected_mission_type = fps::MissionType::Defense;
                }
                self.game_messages.info("Mission: DEFENSE — Hold position 4:00, then extract.".to_string());
            }
            if self.input.is_key_pressed(KeyCode::Digit5) {
                self.next_mission_type = fps::MissionType::HiveDestruction;
                if let Some(ref mut ship) = self.ship_state {
                    ship.selected_mission_type = fps::MissionType::HiveDestruction;
                }
                self.game_messages.info("Mission: HIVE DESTRUCTION — 40 kills, then extract.".to_string());
            }
        }

        // ── Deploy: walk to the drop bay and press Space ──
        let dist_to_bay = Vec3::new(
            self.camera.transform.position.x - drop_bay_pos.x,
            0.0,
            self.camera.transform.position.z - drop_bay_pos.z,
        ).length();

        // Dynamic interaction prompt (same style as dialogue; overlay draws from this)
        if !war_table_active {
            if dist_to_table < 4.0 {
                self.interaction_prompt = Some(InteractPrompt {
                    key: INTERACT_KEY,
                    action: "ACCESS WAR TABLE".to_string(),
                });
            } else if dist_to_bay < 4.0 {
                self.interaction_prompt = Some(InteractPrompt {
                    key: DEPLOY_KEY,
                    action: format!("DEPLOY TO {}", self.planet.name),
                });
            } else {
                const TALK_RANGE_SQ: f32 = 3.0 * 3.0;
                let cam_pos = self.camera.position();
                let mut nearest: Option<(f32, &'static str, usize)> = None;
                for npc in roger_young_interior_npcs() {
                    let dist_sq = npc.position.distance_squared(cam_pos);
                    if dist_sq < TALK_RANGE_SQ {
                        if nearest.as_ref().map(|(d, _, _)| *d > dist_sq).unwrap_or(true) {
                            nearest = Some((dist_sq, npc.name, npc.dialogue_id));
                        }
                    }
                }
                if let Some((_, name, dialogue_id)) = nearest {
                    self.interaction_prompt = Some(InteractPrompt {
                        key: INTERACT_KEY,
                        action: format!("Talk to {}", name),
                    });
                    if self.input.is_key_pressed(KeyCode::KeyE) && !self.dialogue_state.is_open() {
                        self.dialogue_state = DialogueState::Open {
                            speaker_entity: None,
                            speaker_name: name.to_string(),
                            dialogue_id,
                            node_index: 0,
                            showing_choices: true,
                        };
                    }
                }
            }
        }

        if self.input.is_key_pressed(KeyCode::Space) && dist_to_bay < 4.0 {
            if let Some(ship) = self.ship_state.take() {
                let planet_idx = ship.target_planet_idx;
                let planet = &self.current_system.bodies[planet_idx].planet;
                if let Some(status) = self.war_state.planets.get_mut(planet_idx) {
                    status.active_operation = true;
                }
                if planet.name == "Earth" {
                    // Roger Young stays in orbit; dropship takes MI trooper to Earth for resupply & visit
                    self.transition_to_earth_visit(planet_idx);
                } else {
                    self.deploy_planet_idx = Some(planet_idx);
                    self.approach_flight_state = None;
                    self.transition_approach_to_drop();
                }
            }
        }

        // Update renderer camera so the 3D interior renders correctly
        self.renderer.update_camera(&self.camera, self.planet_radius_for_curvature());

        self.game_messages.update(dt);
    }

    /// Update approach phase: flyable craft (Star Citizen–style piloting) or legacy timer.
    fn update_approach(&mut self, dt: f32) {
        if let Some(ref mut flight) = self.approach_flight_state {
            // Flyable approach: mouse = look, W/S = throttle
            let mouse_delta = self.input.mouse_delta();
            self.camera.process_mouse(mouse_delta.x, mouse_delta.y);

            let fwd = self.camera.forward();
            const THRUST: f32 = 35.0;
            if self.input.is_key_held(KeyCode::KeyW) {
                flight.velocity += fwd * THRUST * dt;
            }
            if self.input.is_key_held(KeyCode::KeyS) {
                flight.velocity -= fwd * THRUST * dt;
            }
            flight.velocity *= 0.98f32.powf(dt * 60.0 / 60.0); // gentle drag
            flight.position += flight.velocity * dt;

            self.camera.transform.position = flight.position;
            self.approach_timer += dt;

            const MIN_APPROACH_TIME: f32 = 4.0;
            const MAX_APPROACH_TIME: f32 = 15.0;
            let exit_to_drop = self.input.is_key_pressed(KeyCode::Space)
                || (self.approach_timer >= MIN_APPROACH_TIME && self.input.is_key_pressed(KeyCode::KeyE))
                || self.approach_timer >= MAX_APPROACH_TIME;

            if exit_to_drop {
                self.approach_flight_state = None;
                self.transition_approach_to_drop();
            }
        } else {
            // Legacy timer-only approach (shouldn't run if we always use flight)
            self.approach_timer += dt;
            if self.approach_timer >= 5.0 || self.input.is_key_pressed(KeyCode::Space) {
                self.transition_approach_to_drop();
            }
        }
        self.renderer.update_camera(&self.camera, self.planet_radius_for_curvature());
        self.game_messages.update(dt);
    }

    /// Deploy to Earth via dropship (no drop pod). Roger Young stays in orbit; trooper visits for resupply & R&R.
    fn transition_to_earth_visit(&mut self, planet_idx: usize) {
        self.prepare_planet_for_drop(planet_idx);
        self.mission = fps::MissionState::new_earth_visit();
        self.ambient_dust.particles.clear();
        self.biome_atmosphere.particles.clear();
        self.rain_drops.clear();
        self.snow_particles.clear();
        self.complete_earth_visit();
    }

    /// Land on Earth (dropship pad at city center). No crater, no squad pods — bustling Federation world.
    fn complete_earth_visit(&mut self) {
        // Load all chunks covering the full territory so road/building height samples and colliders are correct.
        let (min_x, max_x, min_z, max_z) = earth_territory::territory_bounds();
        let extent = (max_x - min_x).max(max_z - min_z) + 128.0;
        let scatter_range = extent.max(400.0);
        self.chunk_manager.ensure_chunks_loaded_for_spawn(
            scatter_range,
            self.renderer.device(),
            &mut self.physics,
        );

        // Flatten terrain so the city sits on flat ground: city core first, then roads, then building lots.
        const BUILDING_LOT_MARGIN: f32 = 6.0;  // extra flat ground around each building (meters)
        const ROAD_SHOULDER_MARGIN: f32 = 4.0; // extra flat width each side of roads
        const CITY_CORE_RADIUS: f32 = 62.0;   // Buenos Aires Metro core — one flat plateau
        let mut modified: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();
        // Phase 1: one flat plateau for the capital so the core is never mangled.
        let core_height = self.chunk_manager.sample_height(0.0, 0.0);
        for key in self.chunk_manager.flatten_circle(0.0, 0.0, CITY_CORE_RADIUS, core_height) {
            modified.insert(key);
        }
        // Phase 2: compute flat heights for roads and building lots (no mutable borrow yet).
        let mut building_flats: Vec<(f32, f32, f32, f32, f32)> = Vec::new();
        for &(bx, bz, sx, _sy, sz) in earth_territory::earth_building_boxes() {
            let flat_h = earth_territory::building_footprint_base_y(bx, bz, sx, sz, |x, z| self.chunk_manager.sample_height(x, z));
            let hx = sx * 0.5 + BUILDING_LOT_MARGIN;
            let hz = sz * 0.5 + BUILDING_LOT_MARGIN;
            building_flats.push((bx - hx, bx + hx, bz - hz, bz + hz, flat_h));
        }
        let mut road_flats: Vec<(f32, f32, f32, f32, f32, f32)> = Vec::new();
        for (cx, cz, half_len, half_w, rot) in earth_territory::road_collider_segments() {
            let s = rot.sin();
            let c = rot.cos();
            let corners = [
                (cx + half_len * s - half_w * c, cz + half_len * c + half_w * s),
                (cx + half_len * s + half_w * c, cz + half_len * c - half_w * s),
                (cx - half_len * s + half_w * c, cz - half_len * c - half_w * s),
                (cx - half_len * s - half_w * c, cz - half_len * c + half_w * s),
            ];
            let flat_h = corners
                .iter()
                .map(|&(x, z)| self.chunk_manager.sample_height(x, z))
                .fold(f32::NEG_INFINITY, f32::max);
            road_flats.push((cx, cz, half_len, half_w + ROAD_SHOULDER_MARGIN, rot, flat_h));
        }
        // Phase 3: flatten roads then building lots (lots override so they stay uniform where they cross roads).
        for (cx, cz, half_len, half_w, rot, flat_h) in road_flats {
            for key in self.chunk_manager.flatten_road_segment(cx, cz, half_len, half_w, rot, flat_h) {
                modified.insert(key);
            }
        }
        for (min_x, max_x, min_z, max_z, flat_h) in building_flats {
            for key in self.chunk_manager.flatten_rect(min_x, max_x, min_z, max_z, flat_h) {
                modified.insert(key);
            }
        }
        let modified: Vec<(i32, i32)> = modified.into_iter().collect();
        if !modified.is_empty() {
            let to_rebuild = self.chunk_manager.sync_chunk_edge_heights(&modified);
            for key in to_rebuild {
                self.chunk_manager.rebuild_chunk_mesh_and_collider(
                    key,
                    self.renderer.device(),
                    &mut self.physics,
                );
            }
        }

        let landing = Vec3::ZERO; // Dropship pad / city center
        let spawn_y = self.chunk_manager.walkable_height(landing.x, landing.z) + 1.8;
        let spawn_pos = Vec3::new(landing.x, spawn_y, landing.z);

        self.camera.transform.position = spawn_pos;
        self.camera.transform.rotation = Quat::IDENTITY;
        self.player.position = spawn_pos;
        self.squad_drop_pods = None; // No squad on resupply run

        self.settlement_center = Some(landing);
        self.earth_waypoints = Some(earth_territory::all_waypoints_global());
        earth_territory::spawn_territory_citizens(
            &mut self.world,
            |x, z| self.chunk_manager.sample_height(x, z),
        );
        earth_territory::spawn_earth_buildings(
            &mut self.world,
            |x, z| self.chunk_manager.sample_height(x, z),
        );
        let (road_verts, road_idx) = earth_territory::build_earth_roads_mesh(|x, z| self.chunk_manager.sample_height(x, z));
        self.earth_roads_mesh = Some(Mesh::from_data(
            self.renderer.device(),
            &road_verts,
            &road_idx,
        ));
        for (cx, cz, half_len, half_w, rotation_y_rad) in earth_territory::road_collider_segments() {
            let terrain_y = self.chunk_manager.sample_height(cx, cz);
            let cy = terrain_y - 0.05;
            let center = Vec3::new(cx, cy, cz);
            let half_extents = Vec3::new(half_len, 0.05, half_w);
            let handle = self.physics.add_static_cuboid(center, rotation_y_rad, half_extents);
            self.earth_road_colliders.push(handle);
        }
        for &(bx, bz, sx, sy, sz) in earth_territory::earth_building_boxes() {
            let base_y = earth_territory::building_footprint_base_y(bx, bz, sx, sz, |x, z| self.chunk_manager.sample_height(x, z));
            let cy = base_y + sy * 0.5;
            let center = Vec3::new(bx, cy, bz);
            let half_extents = Vec3::new(sx * 0.5, sy * 0.5, sz * 0.5);
            let handle = self.physics.add_static_cuboid(center, 0.0, half_extents);
            self.earth_building_colliders.push(handle);
        }

        self.game_messages.success("DROPSHIP TOUCHED DOWN. Welcome home, trooper.".to_string());
        self.game_messages.info("Roger Young remains in Earth orbit. UCF safe zone — no bugs on the homeworld.".to_string());
        self.game_messages.info("Resupply and visit. Cities, towns, farms — this is what we're fighting for. [V] when ready to return.".to_string());
        self.game_messages.info("WASD = move | Shift = sprint | E = talk to citizens | M = galaxy map | V = return to ship".to_string());

        self.phase = GamePhase::Playing;
    }

    /// Transition from approach phase directly to drop sequence (EVA removed).
    fn transition_approach_to_drop(&mut self) {
        let planet_idx = self.deploy_planet_idx.unwrap_or(0);
        self.deploy_planet_idx = None;

        let planet = &self.current_system.bodies[planet_idx].planet;
        self.game_messages.warning("DROP POD LAUNCHED! BRACE FOR IMPACT!".to_string());
        if planet.name == "Earth" {
            self.game_messages.info("Entering Earth's atmosphere — homeworld. Smooth transition: space → orbit → atmosphere → surface.".to_string());
        }
        self.game_messages.info("\"Come on you apes, you wanna live forever?!\"".to_string());

        self.prepare_planet_for_drop(planet_idx);
        self.ambient_dust.particles.clear();
        self.biome_atmosphere.particles.clear();
        self.rain_drops.clear();
        self.snow_particles.clear();

        self.drop_pod = Some(DropPodSequence::new(planet_idx));
        self.phase = GamePhase::DropSequence;

        self.camera.transform.position = Vec3::new(0.0, 2500.0, 0.0);
        self.camera.transform.rotation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2 * 0.8);
    }

    /// Update drop pod descent sequence — real-time continuous descent.
    fn update_drop_sequence(&mut self, dt: f32) {
        let sequence_complete = if let Some(ref mut pod) = self.drop_pod {
            let complete = pod.update(dt);

            // Resolve landing position once we've streamed terrain under the pod (so sample_height is valid).
            if !pod.terrain_ready && pod.altitude < 2000.0 {
                let (sample_x, sample_z, landing_y) = if let Some((center, _)) = self.defense_base {
                    (center.x, center.z, center.y)
                } else {
                    let sx = self.camera.transform.position.x;
                    let sz = self.camera.transform.position.z;
                    (sx, sz, self.chunk_manager.sample_height(sx, sz))
                };
                pod.landing_pos = Vec3::new(sample_x, landing_y, sample_z);
                pod.terrain_ready = true;
            }

            // Camera Y = terrain height + pod altitude (real-time position)
            let ground_y = if pod.terrain_ready { pod.landing_pos.y } else { 0.0 };
            let cam_y = match pod.phase {
                DropPhase::Impact => {
                    let p = (pod.phase_timer / 1.5).min(1.0);
                    ground_y + 3.0 + (1.0 - p) * 5.0
                }
                DropPhase::Emerge => {
                    let p = (pod.phase_timer / 2.0).min(1.0);
                    let ease = p * p * (3.0 - 2.0 * p);
                    ground_y + 2.0 + ease * 1.0
                }
                _ => ground_y + pod.altitude,
            };

            // Apply lateral drift from detach
            self.camera.transform.position.x += pod.lateral_vel.x * dt;
            self.camera.transform.position.z += pod.lateral_vel.z * dt;
            pod.lateral_vel *= 0.995;
            self.camera.transform.position.y = cam_y;

            // Camera rotation: yaw + pitch + roll from pod state
            let yaw_quat = Quat::from_rotation_y(pod.camera_yaw);
            let pitch_quat = Quat::from_rotation_x(pod.camera_pitch);
            let roll_quat = Quat::from_rotation_z(pod.camera_roll);
            self.camera.transform.rotation = yaw_quat * pitch_quat * roll_quat;

            // Apply camera shake on top
            self.camera.transform.position += pod.shake_offset;

            // Helldivers 2–style: follow the drop pod and stream terrain under it for the entire descent.
            // Chunk center = pod position on the ground plane so terrain loads along the pod’s path.
            let stream_center = Vec3::new(
                self.camera.transform.position.x,
                ground_y + 10.0,
                self.camera.transform.position.z,
            );
            self.chunk_manager.update(
                stream_center,
                self.renderer.device(),
                &mut self.physics,
            );

            complete
        } else {
            true
        };

        if sequence_complete {
            self.complete_drop();
        }

        // Time of day and weather are advanced in the main update loop (realtime) — visible during drop

        // Update on-screen messages
        self.game_messages.update(dt);
    }

    /// Prepare the planet's terrain and content for a drop (called before drop sequence).
    fn prepare_planet_for_drop(&mut self, planet_idx: usize) {
        let body = &self.current_system.bodies[planet_idx];
        let planet = body.planet.clone();
        let biome_config = planet.get_biome_config();
        let planet_biomes = planet.biome_sampler();

        self.current_planet_idx = Some(planet_idx);

        // Reset terrain for this planet. Earth: terraformed — gentler hills, smooth (no voxel) terrain.
        let (height_scale, frequency, use_smooth_terrain) = if planet.name == "Earth" {
            (10.0, 0.012, true)
        } else {
            (
                15.0 * biome_config.height_scale,
                0.02 * biome_config.frequency_scale as f64,
                false,
            )
        };
        self.chunk_manager.reset_for_planet(
            planet.seed,
            height_scale,
            frequency,
            planet_biomes,
            use_smooth_terrain,
            &mut self.physics,
        );

        // Generate terrain chunks around the landing zone
        self.chunk_manager.update(Vec3::ZERO, self.renderer.device(), &mut self.physics);
        // Force-load all chunks in spawn range so sample_height returns valid terrain (avoids objects spawning at y=0)
        let scatter_range = self.chunk_manager.chunk_size * 3.0;
        self.chunk_manager.ensure_chunks_loaded_for_spawn(
            scatter_range,
            self.renderer.device(),
            &mut self.physics,
        );

        // Base defense: UCF planet + Hold the Line or Defense mission
        let is_ucf = matches!(planet.classification,
            PlanetClassification::Colony | PlanetClassification::Outpost
            | PlanetClassification::Industrial | PlanetClassification::Research,
        );
        let is_defense_mission = matches!(self.next_mission_type,
            fps::MissionType::HoldTheLine | fps::MissionType::Defense,
        );
        let is_base_defense = is_ucf && is_defense_mission;

        self.defense_base = None;

        // Spawn biome content (skip UCF structures and use larger clearance when base defense)
        self.spawn_biome_content(&planet, is_base_defense);

        if is_base_defense {
            self.spawn_defense_base();
        }

        // Reset game systems
        self.spawner = spawner::BugSpawner::new(planet.bug_spawn_rate(), planet.danger_level);
        let biome_table = get_biome_feature_table(planet.primary_biome);
        self.spawner.set_biome_variant(biome_table.bug_variant, biome_table.variant_chance);
        self.mission = match self.next_mission_type {
            fps::MissionType::Extermination => fps::MissionState::new_horde(),
            fps::MissionType::BugHunt => fps::MissionState::new_bug_hunt(25),
            fps::MissionType::HoldTheLine => fps::MissionState::new_hold_the_line(300.0),
            fps::MissionType::Defense => fps::MissionState::new_defense(240.0),
            fps::MissionType::HiveDestruction => fps::MissionState::new_hive_destruction(40),
            _ => fps::MissionState::new_horde(),
        };
        // Time of day from real-time cycle (star + planet rotation); weather from saved conditions
        let (_, tod) = self.compute_sun_direction_and_time_of_day(Some(planet_idx));
        self.time_of_day = tod;
        let planet_status = &self.war_state.planets[planet_idx];
        self.weather = planet_status.weather.clone();

        // Reset biome atmosphere for the new planet's biome
        self.biome_atmosphere.reset(planet.primary_biome);
        self.ambient_dust = AmbientDust::new();

        self.planet = planet;
    }

    /// Complete the drop pod sequence: create massive impact crater and transition to Playing.
    fn complete_drop(&mut self) {
        if let Some(pod) = self.drop_pod.take() {
            let landing = pod.landing_pos;
            let terrain_y = self.chunk_manager.sample_height(landing.x, landing.z);
            let is_base_defense = self.defense_base.is_some();

            if !is_base_defense {
                // ---- MASSIVE IMPACT CRATER ----
                self.chunk_manager.deform_at(
                    landing,
                    10.0, 6.0,
                    self.renderer.device(),
                    &mut self.physics,
                );
                self.chunk_manager.deform_at(
                    landing,
                    16.0, 2.0,
                    self.renderer.device(),
                    &mut self.physics,
                );
                for i in 0..4 {
                    let angle = i as f32 * std::f32::consts::FRAC_PI_2 + 0.3;
                    let offset = Vec3::new(angle.cos() * 12.0, 0.0, angle.sin() * 12.0);
                    self.chunk_manager.deform_at(
                        landing + offset,
                        3.0, 1.5,
                        self.renderer.device(),
                        &mut self.physics,
                    );
                }
            }

            // ---- IMPACT EFFECTS ----
            for i in 0..8 {
                let angle = i as f32 * std::f32::consts::TAU / 8.0;
                let offset = Vec3::new(angle.cos() * 5.0, 2.0 + (i as f32) * 0.5, angle.sin() * 5.0);
                self.effects.spawn_muzzle_flash(landing + offset, Vec3::Y);
            }
            self.screen_shake.add_trauma(1.0);

            // Position player at landing (base center for defense, crater for normal)
            let spawn_y = terrain_y + 3.0;
            self.camera.transform.position = Vec3::new(landing.x, spawn_y, landing.z);
            self.camera.transform.rotation = Quat::IDENTITY;
            self.player.position = self.camera.transform.position;

            self.squad_drop_pods = Some(SquadDropSequence::new(landing, terrain_y));

            if is_base_defense {
                self.game_messages.success("BASE DEFENSE! Hold the walls, trooper!".to_string());
                self.game_messages.info("UCF Firebase — the bug horde is coming. Hold the perimeter!".to_string());
            } else {
                self.game_messages.success("DROP POD DOWN! Move out, trooper!".to_string());
                if self.planet.name == "Earth" {
                    self.game_messages.info("Surface: Earth. Defend the homeworld — Starship Troopers style.".to_string());
                    self.settlement_center = Some(landing);
                    spawn_earth_citizens(
                        &mut self.world,
                        landing,
                        |x, z| self.chunk_manager.sample_height(x, z),
                        14,
                    );
                    self.game_messages.info("Settlement nearby — citizens on schedule. Press [E] near a citizen to talk.".to_string());
                } else {
                    self.settlement_center = None;
                    self.earth_waypoints = None;
                    self.earth_roads_mesh = None;
                    for h in self.earth_road_colliders.drain(..) {
                        self.physics.remove_collider(h);
                    }
                    for h in self.earth_building_colliders.drain(..) {
                        self.physics.remove_collider(h);
                    }
                }
                self.game_messages.info("Look up — squad drop pods inbound from the Roger Young in orbit!".to_string());
                self.game_messages.info(format!("IMPACT SITE: crater radius 16m | {:.0}m deep", 6.0));
            }
            self.game_messages.info("WASD = move | Shift = sprint | Space/Ctrl = up/down | M = galaxy map | R = next planet".to_string());
            self.game_messages.info("G = smoke | T = Tac Fighter | V = extraction | X = entrenchment shovel".to_string());
        }

        self.phase = GamePhase::Playing;
    }

    fn update_gameplay(&mut self, dt: f32) {
        update::gameplay(self, dt);
    }

    fn spawn_physics_bugs(&mut self, dt: f32) {
        // Earth is a UCF safe zone — no bugs on the homeworld.
        if self.planet.name == "Earth" {
            return;
        }
        // Continuous horde spawning — no waves, no pauses, just bugs.
        let spawn_positions: Vec<((BugType, Option<bug::BugVariant>), Vec3)> = {
            let mut positions = Vec::new();

            // Accumulate spawn pressure based on current spawn rate
            self.spawner.spawn_timer += dt * self.spawner.spawn_rate;

            // Each full unit of spawn_timer = one bug to spawn
            let approx_alive = self.mission.bugs_remaining as usize;
            // Fallback terrain when spawn chunk isn't loaded (player is on valid terrain)
            let fallback_terrain = self.chunk_manager.sample_height(
                self.player.position.x,
                self.player.position.z,
            );
            let fallback_y = if fallback_terrain != 0.0 {
                fallback_terrain
            } else {
                self.player.position.y - 1.5 // approx ground when player standing
            };

            while self.spawner.spawn_timer >= 1.0 && approx_alive + positions.len() < self.spawner.max_bugs {
                self.spawner.spawn_timer -= 1.0;

                let angle = rand::random::<f32>() * std::f32::consts::TAU;
                // Base defense: spawn bugs OUTSIDE the perimeter so they come to you
                let min_dist = if let Some((_, radius)) = self.defense_base {
                    (radius + 10.0).max(self.spawner.min_spawn_distance)
                } else {
                    self.spawner.min_spawn_distance
                };
                let dist = min_dist
                    + rand::random::<f32>() * (self.spawner.max_spawn_distance - min_dist);

                let spawn_x = self.player.position.x + angle.cos() * dist;
                let spawn_z = self.player.position.z + angle.sin() * dist;
                let terrain_y = self.chunk_manager.sample_height_or(spawn_x, spawn_z, fallback_y);

                let type_and_variant = self.spawner.random_bug_type();
                let scale = type_and_variant.0.scale();
                // Same formula as terrain snap in update.rs: feet on surface
                let half_height = scale.y * 0.6 + 0.15;
                let pos = Vec3::new(spawn_x, terrain_y + half_height, spawn_z);
                positions.push((type_and_variant, pos));
            }

            // Cap accumulated timer so we don't get a burst after lag spikes
            self.spawner.spawn_timer = self.spawner.spawn_timer.min(3.0);

            positions
        };

        // Spawn bugs with physics bodies
        for ((bug_type, variant), position) in spawn_positions {
            let bug = Bug::new_with_variant(bug_type, variant);
            let scale = bug_type.scale();

            // Create physics body for the bug
            let body_handle = self.physics.add_kinematic_body(position);
            let collider_handle = self.physics.add_capsule_collider(body_handle, scale.y * 0.5, scale.x * 0.5);

            let physics_bug = PhysicsBug {
                body_handle: Some(body_handle),
                collider_handle: Some(collider_handle),
                ..Default::default()
            };

            self.world.spawn((
                Transform {
                    position,
                    rotation: Quat::IDENTITY,
                    scale,
                },
                Velocity::default(),
                Health::new(bug.effective_health()),
                bug,
                physics_bug,
                engine_core::AIComponent::new(85.0, 2.5, 1.0),  // Extermination: large aggro = constant pressure
            ));
        }
    }

    /// Update bug holes: tick spawn timers and spawn bugs near active holes.
    /// Holes speed up with horde difficulty — the longer you survive, the faster they vomit bugs.
    fn update_bug_holes(&mut self, dt: f32) {
        // Only process when on a planet
        if self.current_planet_idx.is_none() {
            return;
        }
        // Earth is a UCF safe zone — no bug holes on the homeworld.
        if self.planet.name == "Earth" {
            return;
        }

        let player_pos = self.player.position;
        let max_spawn_dist = 120.0; // Only holes within this range spawn bugs
        let hole_rate_mult = self.spawner.hole_spawn_rate_multiplier();

        // Collect spawn requests from bug holes
        let mut spawn_requests: Vec<Vec3> = Vec::new();

        for (_entity, (transform, bug_hole, destructible)) in
            self.world.query_mut::<(&Transform, &mut BugHole, &Destructible)>()
        {
            // Skip destroyed holes
            if destructible.health <= 0.0 {
                continue;
            }

            // Only tick holes near the player
            let dist_to_player = transform.position.distance(player_pos);
            if dist_to_player > max_spawn_dist {
                continue;
            }

            bug_hole.spawn_timer += dt;
            let effective_interval = bug_hole.spawn_interval * hole_rate_mult;
            if bug_hole.spawn_timer >= effective_interval && bug_hole.active_bugs < bug_hole.max_active_bugs {
                bug_hole.spawn_timer = 0.0;
                bug_hole.active_bugs += 1;

                // Spawn position: near the hole with some random offset
                let offset_angle = rand::random::<f32>() * std::f32::consts::TAU;
                let offset_dist = 1.0 + rand::random::<f32>() * 3.0;
                let spawn_pos = Vec3::new(
                    transform.position.x + offset_angle.cos() * offset_dist,
                    transform.position.y + 0.5,
                    transform.position.z + offset_angle.sin() * offset_dist,
                );
                spawn_requests.push(spawn_pos);
            }
        }

        // Spawn enemies (bugs or Skinnies on planets that have them)
        let spawn_skinny_chance = if self.planet.has_skinnies { 0.22 } else { 0.0 };
        let fallback_terrain = self.chunk_manager.sample_height(
            self.player.position.x,
            self.player.position.z,
        );
        let fallback_y = if fallback_terrain != 0.0 {
            fallback_terrain
        } else {
            self.player.position.y - 1.5
        };

        for mut spawn_pos in spawn_requests {
            let terrain_y = self.chunk_manager.sample_height_or(
                spawn_pos.x,
                spawn_pos.z,
                fallback_y,
            );

            let spawn_skinny = spawn_skinny_chance > 0.0 && rand::random::<f32>() < spawn_skinny_chance;
            if spawn_skinny {
                let skinny_type = self.random_skinny_type();
                let skinny = Skinny::new(skinny_type);
                let scale = skinny_type.scale();
                let half_height = scale.y * 0.6 + 0.15;
                spawn_pos.y = terrain_y + half_height;
                let body_handle = self.physics.add_kinematic_body(spawn_pos);
                let collider_handle = self.physics.add_capsule_collider(body_handle, scale.y * 0.5, scale.x * 0.5);
                let physics_bug = PhysicsBug {
                    body_handle: Some(body_handle),
                    collider_handle: Some(collider_handle),
                    ..Default::default()
                };
                self.world.spawn((
                    Transform { position: spawn_pos, rotation: Quat::IDENTITY, scale },
                    Velocity::default(),
                    engine_core::Health::new(skinny.effective_health()),
                    skinny,
                    physics_bug,
                    engine_core::AIComponent::new(75.0, 2.5, 1.0),  // Skinnies: aggressive
                ));
            } else {
                let (bug_type, variant) = self.random_bug_type();
                let bug = Bug::new_with_variant(bug_type, variant);
                let scale = bug_type.scale();
                let half_height = scale.y * 0.6 + 0.15;
                spawn_pos.y = terrain_y + half_height;
                let body_handle = self.physics.add_kinematic_body(spawn_pos);
                let collider_handle = self.physics.add_capsule_collider(body_handle, scale.y * 0.5, scale.x * 0.5);
                let physics_bug = PhysicsBug {
                    body_handle: Some(body_handle),
                    collider_handle: Some(collider_handle),
                    ..Default::default()
                };
                self.world.spawn((
                    Transform { position: spawn_pos, rotation: Quat::IDENTITY, scale },
                    Velocity::default(),
                    engine_core::Health::new(bug.effective_health()),
                    bug,
                    physics_bug,
                    engine_core::AIComponent::new(85.0, 2.5, 1.0),  // Extermination: large aggro = constant pressure
                ));
            }
        }
    }

    fn random_skinny_type(&mut self) -> SkinnyType {
        let r = rand::random::<f32>();
        if r < 0.6 { SkinnyType::Grunt }
        else if r < 0.85 { SkinnyType::Sniper }
        else { SkinnyType::Officer }
    }

    /// Update environmental hazards: timed bursts, proximity triggers, and persistent DoT.
    /// Applies player damage/slow and sets hazard_slow_multiplier for movement.
    fn update_environmental_hazards(&mut self, dt: f32) {
        use destruction::HazardType;

        self.hazard_slow_multiplier = 1.0;
        let player_pos = self.player.position;
        let god_mode = self.debug.god_mode;

        for (_, (transform, hazard)) in
            self.world.query_mut::<(&Transform, &mut EnvironmentalHazard)>()
        {
            hazard.timer += dt;
            let pos = transform.position;
            let dist = (player_pos.x - pos.x).powi(2) + (player_pos.z - pos.z).powi(2)
                + (player_pos.y - pos.y).powi(2);
            let r_sq = hazard.radius * hazard.radius;
            let in_radius = dist <= r_sq;

            // Timed hazards: active for a short burst every interval (e.g. geyser eruption)
            const BURST_DURATION: f32 = 2.0;
            if hazard.interval > 0.0 {
                let cycle = hazard.timer % hazard.interval;
                hazard.active = cycle < BURST_DURATION;
            } else {
                // Persistent (LavaFlow, PoisonGas, etc.): always "active" when in radius
                hazard.active = in_radius;
            }

            if !in_radius {
                continue;
            }

            let dir_to_player = (player_pos - pos).normalize_or_zero();

            // Slow-only hazards: reduce movement
            match hazard.hazard_type {
                HazardType::Quicksand | HazardType::Sandstorm | HazardType::Blizzard => {
                    self.hazard_slow_multiplier *= 0.35;
                }
                _ => {}
            }

            // Damage when active (timed burst or persistent)
            let should_damage = hazard.active && hazard.damage > 0.0 && !god_mode;
            if should_damage {
                // DPS: apply damage * dt, clamped so one frame doesn't one-shot
                let dps = hazard.damage;
                let amount = (dps * dt).min(dps * 0.25);
                if amount > 0.0 {
                    self.player.take_damage(amount, Some(dir_to_player));
                }
            }
        }
    }

    /// Delegate to the spawner's difficulty-based type selection and biome variant.
    fn random_bug_type(&mut self) -> (BugType, Option<bug::BugVariant>) {
        self.spawner.random_bug_type()
    }

    fn process_dying_bugs(&mut self) {
        let mut gore_spawns: Vec<(Vec3, Vec3, f32)> = Vec::new();
        let mut gore_debris_spawns: Vec<(Vec3, Vec3, f32, [f32; 4])> = Vec::new();
        let mut death_effects: Vec<(Vec3, VariantDeathEffect)> = Vec::new();

        for (_, (transform, physics_bug, health, bug)) in
            self.world.query_mut::<(&Transform, &mut PhysicsBug, &Health, &Bug)>()
        {
            if health.is_dead() && !physics_bug.gore_spawned {
                physics_bug.gore_spawned = true;
                let pos = transform.position;
                let size = transform.scale.x;
                let gore_dir = if physics_bug.impact_velocity.length() > 0.1 {
                    physics_bug.impact_velocity.normalize()
                } else { Vec3::Y };
                gore_spawns.push((pos, gore_dir, size));
                self.total_gore_spawned += 1;
                let mut bug_color = bug.bug_type.color();
                if let Some(v) = bug.variant {
                    let t = v.color_tint();
                    bug_color[0] *= t[0];
                    bug_color[1] *= t[1];
                    bug_color[2] *= t[2];
                }
                gore_debris_spawns.push((pos, gore_dir, size, bug_color));
                if let Some(v) = bug.variant {
                    let effect = v.death_effect();
                    if effect != VariantDeathEffect::None {
                        death_effects.push((pos, effect));
                    }
                }
            }
        }

        for (_, (transform, physics_bug, health, _skinny)) in
            self.world.query_mut::<(&Transform, &mut PhysicsBug, &Health, &Skinny)>()
        {
            if health.is_dead() && !physics_bug.gore_spawned {
                physics_bug.gore_spawned = true;
                let pos = transform.position;
                let size = transform.scale.x;
                let gore_dir = if physics_bug.impact_velocity.length() > 0.1 {
                    physics_bug.impact_velocity.normalize()
                } else { Vec3::Y };
                gore_spawns.push((pos, gore_dir, size));
                self.total_gore_spawned += 1;
                gore_debris_spawns.push((pos, gore_dir, size, [0.4, 0.15, 0.12, 1.0]));
            }
        }

        for (pos, dir, size) in gore_spawns {
            self.effects.spawn_gore(pos, dir, size);
        }

        for (pos, dir, size, color) in gore_debris_spawns {
            self.destruction.spawn_bug_gore_debris(
                &mut self.world,
                pos,
                dir,
                color,
                size,
                &mut self.physics,
            );
        }

        // Variant death effects
        for (pos, effect) in death_effects {
            match effect {
                VariantDeathEffect::SpawnMiniBugs => {
                    let count = 3 + (rand::random::<u32>() % 3); // 3-5
                    for _ in 0..count {
                        let angle = rand::random::<f32>() * std::f32::consts::TAU;
                        let off = 0.8 + rand::random::<f32>() * 0.8;
                        let spawn_pos = pos + Vec3::new(angle.cos() * off, 0.0, angle.sin() * off);
                        let fallback = self.chunk_manager.sample_height(
                            self.player.position.x,
                            self.player.position.z,
                        );
                        let fallback_y = if fallback != 0.0 { fallback } else { self.player.position.y - 1.5 };
                        let terrain_y = self.chunk_manager.sample_height_or(spawn_pos.x, spawn_pos.z, fallback_y);
                        let bug_type = BugType::Warrior;
                        let bug = Bug::new(bug_type); // no variant for mini bugs
                        let scale = bug_type.scale();
                        let half_height = scale.y * 0.6 + 0.15;
                        let spawn_pos = Vec3::new(spawn_pos.x, terrain_y + half_height, spawn_pos.z);
                        let body_handle = self.physics.add_kinematic_body(spawn_pos);
                        let collider_handle = self.physics.add_capsule_collider(body_handle, scale.y * 0.5, scale.x * 0.5);
                        self.world.spawn((
                            Transform { position: spawn_pos, rotation: Quat::IDENTITY, scale },
                            Velocity::default(),
                            Health::new(bug.effective_health()),
                            bug,
                            PhysicsBug { body_handle: Some(body_handle), collider_handle: Some(collider_handle), ..Default::default() },
                            engine_core::AIComponent::new(85.0, 2.5, 1.0),  // Extermination: large aggro = constant pressure
                        ));
                    }
                }
                VariantDeathEffect::Explosion => {
                    const RADIUS: f32 = 5.0;
                    const DAMAGE: f32 = 35.0;
                    let player_pos = self.player.position;
                    let dist = (player_pos - pos).length();
                    if dist < RADIUS && self.player.is_alive && !self.debug.god_mode {
                        let amount = DAMAGE * (1.0 - dist / RADIUS * 0.5);
                        let dir = (player_pos - pos).normalize_or_zero();
                        self.player.take_damage(amount, Some(dir));
                        self.screen_shake.add_trauma(0.2);
                    }
                    for (_, (t, health)) in self.world.query_mut::<(&Transform, &mut Health)>() {
                        if health.is_dead() { continue; }
                        let d = (t.position - pos).length();
                        if d < RADIUS {
                            let amount = DAMAGE * (1.0 - d / RADIUS * 0.5);
                            health.take_damage(amount);
                        }
                    }
                }
                VariantDeathEffect::FireHazard => {
                    let (radius, damage, _interval) = hazard_params(HazardType::LavaFlow);
                    let hazard = EnvironmentalHazard {
                        hazard_type: HazardType::LavaFlow,
                        radius,
                        damage,
                        timer: 0.0,
                        interval: 0.0,
                        active: true,
                    };
                    let t = Transform {
                        position: pos,
                        rotation: Quat::IDENTITY,
                        scale: Vec3::new(radius * 2.0, 0.1, radius * 2.0),
                    };
                    let color = hazard_visual_color(HazardType::LavaFlow);
                    let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color, mesh_group: MESH_GROUP_HAZARD };
                    self.world.spawn((t, hazard, cached));
                }
                VariantDeathEffect::SlowZone => {
                    let (radius, damage, interval) = hazard_params(HazardType::Quicksand);
                    let hazard = EnvironmentalHazard {
                        hazard_type: HazardType::Quicksand,
                        radius,
                        damage,
                        timer: 0.0,
                        interval,
                        active: true,
                    };
                    let t = Transform {
                        position: pos,
                        rotation: Quat::IDENTITY,
                        scale: Vec3::new(radius * 2.0, 0.1, radius * 2.0),
                    };
                    let color = hazard_visual_color(HazardType::Quicksand);
                    let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color, mesh_group: MESH_GROUP_HAZARD };
                    self.world.spawn((t, hazard, cached));
                }
                VariantDeathEffect::AcidPool => {
                    let (radius, damage, interval) = hazard_params(HazardType::PoisonGas);
                    let hazard = EnvironmentalHazard {
                        hazard_type: HazardType::PoisonGas,
                        radius,
                        damage,
                        timer: 0.0,
                        interval,
                        active: true,
                    };
                    let t = Transform {
                        position: pos,
                        rotation: Quat::IDENTITY,
                        scale: Vec3::new(radius * 2.0, 0.1, radius * 2.0),
                    };
                    let color = hazard_visual_color(HazardType::PoisonGas);
                    let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color, mesh_group: MESH_GROUP_HAZARD };
                    self.world.spawn((t, hazard, cached));
                }
                VariantDeathEffect::None => {}
            }
        }
    }

    fn count_living_bugs(&self) -> usize {
        let mut count = 0;
        for (_, (health, physics_bug)) in self.world.query::<(&Health, &PhysicsBug)>().iter() {
            if !health.is_dead() || physics_bug.death_phase != DeathPhase::Dead {
                count += 1;
            }
        }
        count
    }

    /// Planet radius for shader curvature (d^2/2R). 0 when not on a planet surface.
    fn planet_radius_for_curvature(&self) -> f32 {
        if self.current_planet_idx.is_none() {
            return 0.0;
        }
        match self.planet.size {
            procgen::PlanetSize::Small => 2000.0,
            procgen::PlanetSize::Medium => 3000.0,
            procgen::PlanetSize::Large => 5000.0,
            procgen::PlanetSize::Massive => 8000.0,
        }
    }

    fn update_camera_only(&mut self, _dt: f32) {
        let mouse_delta = self.input.mouse_delta();
        if self.input.is_cursor_locked() {
            self.camera.process_mouse(mouse_delta.x, mouse_delta.y);
            self.player.yaw = self.camera.yaw();
            self.player.pitch = self.camera.pitch();
        }
        self.renderer.update_camera(&self.camera, self.planet_radius_for_curvature());
    }

    /// Player movement controller. Switches between FPS walking and noclip fly based on debug settings.
    fn handle_player_input(&mut self, dt: f32) {
        // If the player is locked inside the extraction boat, skip all input
        let in_boat = self.extraction.as_ref().map_or(false, |e: &ExtractionDropship| e.player_camera_locked());
        if in_boat {
            return;
        }

        // Mouse look (always active when cursor is locked)
        let mouse_delta = self.input.mouse_delta();
        if self.input.is_cursor_locked() {
            self.camera.process_mouse(mouse_delta.x, mouse_delta.y);
            self.player.yaw = self.camera.yaw();
            self.player.pitch = self.camera.pitch();
            self.player.look_direction = self.camera.forward();
        }

        if self.current_planet_idx.is_none() {
            // --- ZERO-G SPACE FLIGHT ---
            // Thrust-based movement: velocity persists, no gravity. Used for approach phase and any time in space.
            self.handle_zero_g_movement(dt);
        } else if self.debug.noclip {
            // --- NOCLIP (debug on planet) ---
            self.handle_noclip_movement(dt);
        } else {
            // --- FPS WALKING MODE ---
            // Ground-based movement with gravity, jumping, and terrain collision
            self.handle_fps_movement(dt);
        }
    }

    /// Zero-G space flight simulation: thrust (WASD + Space/Ctrl) accelerates the player; velocity persists with no gravity.
    fn handle_zero_g_movement(&mut self, dt: f32) {
        const THRUST: f32 = 28.0;
        const MAX_SPEED: f32 = 120.0;

        let movement = self.input.get_movement_input();
        let move_y = if self.input.is_key_held(KeyCode::Space) {
            1.0
        } else if self.input.is_crouching() {
            -1.0
        } else {
            0.0
        };

        let forward = self.camera.transform.forward();
        let right = self.camera.transform.right();
        let up = self.camera.transform.up();

        let mut thrust_dir = forward * movement.y + right * movement.x + up * move_y;
        if thrust_dir.length_squared() > 0.01 {
            thrust_dir = thrust_dir.normalize();
            self.player_velocity += thrust_dir * (THRUST * dt);
        }

        let speed = self.player_velocity.length();
        if speed > MAX_SPEED {
            self.player_velocity = self.player_velocity.normalize() * MAX_SPEED;
        }

        self.camera.transform.position += self.player_velocity * dt;
        self.player.position = self.camera.transform.position;

        self.player_grounded = false;
    }

    /// Noclip free-fly camera movement (debug mode on planet).
    fn handle_noclip_movement(&mut self, dt: f32) {
        let movement = self.input.get_movement_input();
        let move_y = if self.input.is_key_held(KeyCode::Space) {
            1.0
        } else if self.input.is_crouching() {
            -1.0
        } else {
            0.0
        };

        let altitude = self.camera.transform.position.y.max(0.0);
        let base_speed = if self.current_planet_idx.is_none() {
            let nearest_dist = self.current_system.nearest_body(
                self.universe_position, self.orbital_time
            ).map_or(10000.0, |(_, d)| d as f32);
            100.0 + (nearest_dist / 50.0).min(2000.0)
        } else {
            let alt_mult = 1.0 + (altitude / 100.0).powf(0.7);
            25.0 * alt_mult
        };
        let speed = if self.input.is_sprinting() {
            base_speed * 4.0
        } else {
            base_speed
        };

        self.camera.process_fly(movement, move_y, speed, dt);
        self.player.position = self.camera.transform.position;
        self.player_grounded = false;
    }

    /// FPS ground-based movement with gravity, terrain collision, jumping, and head bob.
    fn handle_fps_movement(&mut self, dt: f32) {
        let movement = self.input.get_movement_input();

        // Crouch / prone (Helldivers 2 style): hold Ctrl = crouch; hold 0.5s = prone
        let crouch_input = self.input.is_crouching();
        if crouch_input {
            self.crouch_hold_timer += dt;
            if self.crouch_hold_timer >= 0.5 {
                self.player.is_prone = true;
                self.player.is_crouching = false;
            } else {
                self.player.is_prone = false;
                self.player.is_crouching = true;
            }
        } else {
            self.crouch_hold_timer = 0.0;
            self.player.is_prone = false;
            self.player.is_crouching = false;
        }
        let is_crouching = self.player.is_crouching;
        let is_prone = self.player.is_prone;

        // Movement speed from class loadout
        let move_speed = self.player.move_speed;
        let sprint_mult = self.player.sprint_multiplier;
        let is_sprinting = self.input.is_sprinting() && self.player.stamina > 0.0 && !is_prone;
        let is_ads = self.player.is_aiming;
        let mut base_speed = if is_sprinting {
            move_speed * sprint_mult
        } else if is_prone {
            move_speed * 0.2  // Very slow when prone (belly crawl)
        } else if is_crouching {
            move_speed * 0.5
        } else {
            move_speed
        };
        // ADS slows movement (Helldivers 2 / Starship Troopers Extermination style)
        if is_ads {
            base_speed *= 0.5; // Significant slowdown for precision aiming
        }
        let speed = base_speed * self.hazard_slow_multiplier;

        // Horizontal movement: project camera forward/right onto horizontal plane
        let forward = self.camera.forward();
        let right = self.camera.right();
        let forward_flat = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
        let right_flat = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();

        let mut move_dir = forward_flat * movement.y + right_flat * movement.x;
        if move_dir.length_squared() > 0.0 {
            move_dir = move_dir.normalize();
        }

        // Apply horizontal velocity with acceleration/deceleration
        let target_horizontal = move_dir * speed;
        let accel = if self.player_grounded { 40.0 } else { 10.0 }; // less air control
        let decel = if self.player_grounded { 30.0 } else { 2.0 };

        // Smoothly interpolate horizontal velocity
        let current_h = Vec3::new(self.player_velocity.x, 0.0, self.player_velocity.z);
        let target_h = Vec3::new(target_horizontal.x, 0.0, target_horizontal.z);

        if target_h.length_squared() > 0.01 {
            // Accelerate toward target
            let diff = target_h - current_h;
            let step = diff.normalize_or_zero() * accel * dt;
            if step.length() > diff.length() {
                self.player_velocity.x = target_h.x;
                self.player_velocity.z = target_h.z;
            } else {
                self.player_velocity.x += step.x;
                self.player_velocity.z += step.z;
            }
        } else {
            // Decelerate to stop
            let h_speed = current_h.length();
            if h_speed > 0.1 {
                let friction = decel * dt;
                let new_speed = (h_speed - friction).max(0.0);
                let ratio = new_speed / h_speed;
                self.player_velocity.x *= ratio;
                self.player_velocity.z *= ratio;
            } else {
                self.player_velocity.x = 0.0;
                self.player_velocity.z = 0.0;
            }
        }

        // Jump (cannot jump when prone or crouching — must stand first)
        if self.player_grounded && self.input.is_jump_pressed() && !is_prone && !is_crouching {
            self.player_velocity.y = 9.0; // jump impulse
            self.player_grounded = false;
        }

        // Gravity
        if !self.player_grounded {
            self.player_velocity.y -= 25.0 * dt; // gravity
            // Terminal velocity
            self.player_velocity.y = self.player_velocity.y.max(-50.0);
        }

        // Apply velocity to position
        let mut new_pos = self.camera.transform.position + self.player_velocity * dt;

        // On Earth: push out of building footprints so player cannot walk through UCF buildings.
        // Do not push when the player is on a road — lets them walk on roads without being shoved.
        if self.planet.name == "Earth" {
            let on_road = earth_territory::road_ground_y_at(
                new_pos.x,
                new_pos.z,
                |a, b| self.chunk_manager.sample_height(a, b),
            ).is_some();
            if !on_road {
                let (px, pz) = earth_territory::push_out_of_building_footprints(new_pos.x, new_pos.z);
                new_pos.x = px;
                new_pos.z = pz;
            }
        }

        // Terrain collision: sample ground height at new position
        let terrain_y = self.chunk_manager.sample_height(new_pos.x, new_pos.z);
        let is_in_water = self.chunk_manager.is_in_water(new_pos.x, new_pos.z);
        let water_level = self.chunk_manager.water_level().unwrap_or(f32::NEG_INFINITY);
        let eye_height = if is_prone { 0.4 } else if is_crouching { 1.2 } else { 1.8 };

        // Water physics: buoyancy, gentle wading slowdown (not immersion-breaking)
        if is_in_water {
            let submersion = (water_level - new_pos.y + eye_height).clamp(0.0, eye_height + 0.5);
            let buoyancy = submersion / (eye_height + 0.5); // 0 = at surface, 1 = fully submerged
            let buoyancy_force = buoyancy * 18.0;
            self.player_velocity.y += (buoyancy_force - 25.0) * dt;
            self.player_velocity.y = self.player_velocity.y.clamp(-8.0, 6.0);
            // Gentle water drag (was 4.0 — way too aggressive)
            self.player_velocity.x *= 1.0 - 1.2 * dt;
            self.player_velocity.z *= 1.0 - 1.2 * dt;
            // Wading: ~85% speed (was 0.5 = 50% — felt like running through glue)
            self.player_velocity.x *= 0.92;
            self.player_velocity.z *= 0.92;
        }

        // Corpse pile climbing: check nearby corpses to raise effective ground height
        // This gives the Starship Troopers: Extermination feel of walking over bug piles
        // In water: ground is water surface (we float)
        let mut ground_y = if is_in_water {
            water_level
        } else {
            terrain_y
        };
        // On Earth: roads and paths are solid — use road surface as ground when standing on one
        if !is_in_water && self.planet.name == "Earth" {
            if let Some(road_y) = earth_territory::road_ground_y_at(
                new_pos.x,
                new_pos.z,
                |a, b| self.chunk_manager.sample_height(a, b),
            ) {
                ground_y = ground_y.max(road_y);
            }
        }
        if !is_in_water {
            let player_xz = Vec3::new(new_pos.x, 0.0, new_pos.z);
            for (_, (corpse_transform, _corpse)) in self.world.query::<(&Transform, &BugCorpse)>().iter() {
                let corpse_xz = Vec3::new(corpse_transform.position.x, 0.0, corpse_transform.position.z);
                let dist_sq = (player_xz - corpse_xz).length_squared();
                // Effective collision radius based on bug scale
                let corpse_radius = corpse_transform.scale.x.max(corpse_transform.scale.z) * 0.8;
                if dist_sq < corpse_radius * corpse_radius {
                    // Top of this corpse
                    let corpse_top = corpse_transform.position.y + corpse_transform.scale.y * 0.35;
                    if corpse_top > ground_y {
                        // Smooth blend: closer to center = higher, edges taper off
                        let dist = dist_sq.sqrt();
                        let blend = 1.0 - (dist / corpse_radius).min(1.0);
                        let smooth_blend = blend * blend * (3.0 - 2.0 * blend); // smoothstep
                        let effective_top = terrain_y + (corpse_top - terrain_y) * smooth_blend;
                        if effective_top > ground_y {
                            ground_y = effective_top;
                        }
                    }
                }
            }
        }
        // Knee-deep snow: stand on terrain + accumulated snow (weather-driven)
        if !is_in_water {
            ground_y += self.sample_snow_depth(new_pos.x, new_pos.z);
        }

        let feet_y = new_pos.y - eye_height;

        if feet_y <= ground_y {
            // Below ground/corpse: snap up to surface
            new_pos.y = ground_y + eye_height;
            self.player_velocity.y = 0.0;
            self.player_grounded = true;
        } else if self.player_grounded {
            // Currently grounded: stick to terrain/corpse surface on slopes
            let gap = feet_y - ground_y;
            if gap < 1.5 {
                // Within stick range: smooth step up/down over corpse piles
                new_pos.y = ground_y + eye_height;
                self.player_velocity.y = 0.0;
            } else {
                // Walked off a ledge or cliff: become airborne
                self.player_grounded = false;
            }
        } else {
            // Airborne: check if we've landed on terrain or corpse pile
            if feet_y - ground_y < 0.05 && self.player_velocity.y <= 0.0 {
                new_pos.y = ground_y + eye_height;
                self.player_velocity.y = 0.0;
                self.player_grounded = true;
            }
        }

        // Head bob when walking on ground
        if self.player_grounded {
            let h_speed = Vec3::new(self.player_velocity.x, 0.0, self.player_velocity.z).length();
            if h_speed > 1.0 {
                let bob_freq = if is_sprinting { 12.0 } else { 8.0 };
                let bob_amount = if is_sprinting { 0.06 } else { 0.03 };
                let bob = (self.time.elapsed_seconds() * bob_freq).sin() * bob_amount * (h_speed / speed).min(1.0);
                new_pos.y += bob;
            }
        }

        // Update camera and player position
        self.camera.transform.position = new_pos;
        self.player.position = new_pos;
        self.player.velocity = self.player_velocity;
        self.player.is_grounded = self.player_grounded;
        self.player.is_sprinting = is_sprinting;
        self.player.is_crouching = is_crouching;
        self.player.is_prone = is_prone;

        // Update player stamina (sprinting drains it)
        if is_sprinting && self.player_grounded {
            self.player.stamina -= 20.0 * dt;
            if self.player.stamina <= 0.0 {
                self.player.stamina = 0.0;
            }
        } else {
            self.player.stamina = (self.player.stamina + 15.0 * dt).min(self.player.max_stamina);
        }
    }

    /// Minecraft Steve scale: block size matches voxel (1m).
    const SHOVEL_BLOCK_SIZE: f32 = 1.0;

    /// Entrenchment shovel: Ace of Spades–style dig (LMB = remove one block, no auto pile).
    fn handle_entrenchment_shovel_dig(&mut self) {
        let origin = self.camera.position();
        let direction = self.camera.forward();
        let max_range = 6.0;

        let hits = self.physics.raycast_all(origin, direction, max_range);
        let hit = hits
            .into_iter()
            .find(|h| self.chunk_manager.is_terrain_collider(h.collider) && h.distance <= max_range);

        if let Some(hit) = hit {
            // Snap to block center (same grid as voxel)
            let dig_center = Self::shovel_snap_to_block_center(hit.point);

            const MIN_TERRAIN_WORLD_Y: f32 = 24.0;
            let water_level = self.chunk_manager.water_level().map(|wl| MIN_TERRAIN_WORLD_Y + wl);
            self.chunk_manager.deform_at_blocky(
                dig_center,
                Self::SHOVEL_BLOCK_SIZE,
                self.renderer.device(),
                &mut self.physics,
                water_level,
            );

            self.chunk_manager.process_pending_rebuilds(
                self.renderer.device(),
                &mut self.physics,
                8,
            );

            self.effects.spawn_bullet_impact(hit.point, hit.normal, false);
            for _ in 0..2 {
                let offset = Vec3::new(
                    (rand::random::<f32>() - 0.5) * 0.6,
                    0.0,
                    (rand::random::<f32>() - 0.5) * 0.6,
                );
                self.effects.spawn_bullet_impact(hit.point + offset, hit.normal, false);
            }
            if Self::biome_has_snow_or_sand(self.planet.primary_biome) {
                let dig_y = self.chunk_manager.sample_height(hit.point.x, hit.point.z) + 0.02;
                self.effects.spawn_ground_track(
                    Vec3::new(hit.point.x, dig_y, hit.point.z),
                    direction.z.atan2(direction.x),
                    TrackKind::ShovelDig,
                );
            }

            self.screen_shake.add_trauma(0.035);
            self.game_messages.info("Block dug".to_string());
        } else {
            self.game_messages.info("Aim at terrain to dig (LMB) or place (RMB)".to_string());
        }
    }

    /// Entrenchment shovel: Ace of Spades–style build (RMB = place one block on the face you're looking at).
    fn handle_entrenchment_shovel_place(&mut self) {
        let origin = self.camera.position();
        let direction = self.camera.forward();
        let max_range = 6.0;

        let hits = self.physics.raycast_all(origin, direction, max_range);
        let hit = hits
            .into_iter()
            .find(|h| self.chunk_manager.is_terrain_collider(h.collider) && h.distance <= max_range);

        if let Some(hit) = hit {
            // Place one block in the adjacent voxel (out from the hit face)
            let place_center = hit.point + hit.normal * Self::SHOVEL_BLOCK_SIZE;
            let place_snapped = Self::shovel_snap_to_block_center(place_center);

            self.chunk_manager.deform_mound_at_blocky(
                place_snapped,
                Self::SHOVEL_BLOCK_SIZE,
                self.renderer.device(),
                &mut self.physics,
            );

            self.chunk_manager.process_pending_rebuilds(
                self.renderer.device(),
                &mut self.physics,
                8,
            );

            self.screen_shake.add_trauma(0.02);
            self.game_messages.info("Block placed".to_string());
        } else {
            self.game_messages.info("Aim at terrain to dig (LMB) or place (RMB)".to_string());
        }
    }

    /// Snap world position to voxel block center (2m grid).
    fn shovel_snap_to_block_center(p: Vec3) -> Vec3 {
        let b = Self::SHOVEL_BLOCK_SIZE;
        Vec3::new(
            (p.x / b).floor() * b + b * 0.5,
            (p.y / b).floor() * b + b * 0.5,
            (p.z / b).floor() * b + b * 0.5,
        )
    }

    fn handle_weapon_fire(&mut self) {
        if !self.player.is_alive {
            return;
        }

        // Entrenching shovel (slot 4): Ace of Spades — LMB = dig, RMB = place block
        if self.player.is_shovel_equipped() {
            if self.current_planet_idx.is_some() {
                let dt = self.smoothed_dt;
                self.shovel_dig_cooldown = (self.shovel_dig_cooldown - dt).max(0.0);
                if self.input.is_fire_held() && self.shovel_dig_cooldown <= 0.0 {
                    self.handle_entrenchment_shovel_dig();
                    self.shovel_dig_cooldown = 0.22; // ~4–5 digs per second while holding
                }
                if self.input.is_mouse_pressed(winit::event::MouseButton::Right) {
                    self.handle_entrenchment_shovel_place();
                }
            }
            return;
        }

        if !self.input.is_fire_held() {
            return;
        }

        {
            let weapon = self.player.current_weapon();
            if !weapon.can_fire() {
                if weapon.current_ammo == 0 && weapon.reserve_ammo > 0 && !weapon.is_reloading {
                    self.player.current_weapon_mut().start_reload();
                    self.viewmodel_anim.trigger_switch();
                }
                return;
            }
        }

        let (range, spread, projectile_count, damage) = {
            let weapon = self.player.current_weapon();
            (weapon.range, weapon.spread, weapon.projectile_count, weapon.damage)
        };

        // Bipod: machine gun gets massive stability when prone (Helldivers 2 style)
        let bipod_active = self.player.is_prone
            && self.player.current_weapon().weapon_type == WeaponType::MachineGun;
        let effective_spread = if bipod_active { spread * 0.25 } else { spread };
        let recoil_mult = if bipod_active { 0.35 } else { 1.0 };
        let shake_mult = if bipod_active { 0.4 } else { 1.0 };

        self.player.current_weapon_mut().fire();

        // --- Cinematic: weapon recoil kick ---
        let recoil_amount = (if damage > 40.0 { 0.04 } else if damage > 20.0 { 0.025 } else { 0.015 }) * recoil_mult;
        self.camera_recoil += recoil_amount;

        // --- Cinematic: screen shake from firing ---
        let shake_amount = (if damage > 40.0 { 0.15 } else if damage > 20.0 { 0.08 } else { 0.04 }) * shake_mult;
        self.screen_shake.add_trauma(shake_amount);

        // Spawn muzzle flash
        let muzzle_pos = self.camera.position() + self.camera.forward() * 0.5;
        self.effects.spawn_muzzle_flash(muzzle_pos, self.camera.forward());

        // --- Viewmodel recoil animation ---
        self.viewmodel_anim.fire_kick = 1.0;
        self.viewmodel_anim.fire_flash_timer = 0.0;

        // --- Eject shell casings (weapon-specific: shotgun = 8 hulls, etc.) ---
        {
            let wt = self.player.current_weapon().weapon_type;
            let shell_type = match wt {
                WeaponType::Rifle => ShellCasingType::Rifle,
                WeaponType::Shotgun => ShellCasingType::Shotgun,
                WeaponType::Sniper => ShellCasingType::Sniper,
                WeaponType::MachineGun => ShellCasingType::MachineGun,
                WeaponType::Rocket => ShellCasingType::Rocket,
                WeaponType::Flamethrower => ShellCasingType::Flamethrower,
            };
            let (size, vel_scale, count) = match wt {
                WeaponType::Rifle => (0.015, 1.0, 1),
                WeaponType::Shotgun => (0.022, 1.2, projectile_count as usize),  // 8 hulls per shot
                WeaponType::Sniper => (0.020, 1.3, 1),
                WeaponType::MachineGun => (0.017, 1.1, 1),
                WeaponType::Rocket => (0.018, 0.9, 1),
                WeaponType::Flamethrower => (0.010, 0.7, 1),
            };
            let cam_right = self.camera.forward().cross(Vec3::Y).normalize_or_zero();
            let cam_up = cam_right.cross(self.camera.forward()).normalize_or_zero();
            for _ in 0..count {
                let eject_pos = self.camera.position()
                    + self.camera.forward() * 0.3
                    + cam_right * (0.15 + (rand::random::<f32>() - 0.5) * 0.04)
                    + cam_up * (-0.02 + (rand::random::<f32>() - 0.5) * 0.03);
                let eject_vel = cam_right * (8.0 + rand::random::<f32>() * 4.0) * vel_scale
                    + cam_up * (2.0 + rand::random::<f32>() * 3.0)
                    + self.camera.forward() * (rand::random::<f32>() * 2.0 - 1.0);
                let angular_vel = Vec3::new(
                    (rand::random::<f32>() - 0.5) * 30.0,
                    (rand::random::<f32>() - 0.5) * 30.0,
                    (rand::random::<f32>() - 0.5) * 30.0,
                );
                let rotation = Quat::from_euler(
                    glam::EulerRot::XYZ,
                    rand::random::<f32>() * std::f32::consts::TAU,
                    rand::random::<f32>() * std::f32::consts::TAU,
                    0.0,
                );
                let radius = size * 1.2; // sphere collider for casing
                let (body_handle, collider_handle) = self.physics.add_shell_casing_body(
                    eject_pos,
                    rotation,
                    eject_vel,
                    angular_vel,
                    radius,
                );
                self.shell_casings.push(ShellCasing {
                    position: eject_pos,
                    rotation,
                    body_handle,
                    collider_handle,
                    lifetime: 4.0,
                    size,
                    shell_type,
                });
            }
        }

        let origin = self.camera.position();
        let direction = self.camera.forward();

        let tracer_speed = 180.0;
        let tracer_lifetime = 0.25;

        for _ in 0..projectile_count {
            let spread_rad = effective_spread.to_radians();
            let spread_x = (rand::random::<f32>() - 0.5) * spread_rad * 2.0;
            let spread_y = (rand::random::<f32>() - 0.5) * spread_rad * 2.0;
            let spread_rotation = Quat::from_euler(glam::EulerRot::XYZ, spread_x, spread_y, 0.0);
            let spread_direction = spread_rotation * direction;

            // Spawn visible tracer
            self.tracer_projectiles.push(TracerProjectile {
                position: origin + direction * 0.3,
                velocity: spread_direction.normalize() * tracer_speed,
                lifetime: tracer_lifetime,
            });

            let dir = spread_direction.normalize();
            let physics_hit = self.physics.raycast(origin, dir, range);
            let max_dist = physics_hit.as_ref().map(|h| h.distance).unwrap_or(range);

            // Helldivers 2 / Starship Troopers Extermination: player can destroy corpses by shooting
            if let Some((corpse_entity, hit_point, hit_normal)) =
                self.raycast_corpse(origin, dir, max_dist)
            {
                self.effects.spawn_bullet_impact(hit_point, hit_normal, true);
                self.effects.spawn_gore(hit_point, hit_normal, 0.5);
                self.world.despawn(corpse_entity).ok();
            } else if let Some(hit) = physics_hit {
                self.effects.spawn_bullet_impact(hit.point, hit.normal, false);
                let hit_entity = self.entity_for_collider(hit.collider);
                self.check_bug_hits(origin, dir, hit.point, damage, hit_entity);
                self.check_destructible_hits(hit.point, damage);

                // Terrain destruction: remove voxel blocks where the shot hits (chunks out of terrain)
                if self.chunk_manager.is_terrain_collider(hit.collider) {
                    const VOXEL_BLOCK_SIZE: f32 = 1.0; // match procgen voxel block size (Minecraft Steve)
                    const MIN_TERRAIN_WORLD_Y: f32 = 24.0; // match procgen baseline for water level
                    let radius = if damage > 40.0 { VOXEL_BLOCK_SIZE * 1.5 } else if damage > 20.0 { VOXEL_BLOCK_SIZE } else { VOXEL_BLOCK_SIZE * 0.6 };
                    let water_level = self.chunk_manager.water_level().map(|wl| MIN_TERRAIN_WORLD_Y + wl);
                    self.chunk_manager.deform_at_blocky(
                        hit.point,
                        radius,
                        self.renderer.device(),
                        &mut self.physics,
                        water_level,
                    );
                }
            }
        }
    }

    /// Find the entity that owns the given collider (bug or destructible).
    fn entity_for_collider(&self, collider: ColliderHandle) -> Option<hecs::Entity> {
        for (entity, physics_bug) in self.world.query::<&PhysicsBug>().iter() {
            if physics_bug.collider_handle == Some(collider) {
                return Some(entity);
            }
        }
        for (entity, phys) in self.world.query::<&DestructiblePhysics>().iter() {
            if phys.collider_handle == collider {
                return Some(entity);
            }
        }
        None
    }

    /// Ray-sphere test: find closest BugCorpse hit by ray within max_dist.
    /// Returns (entity, hit_point, hit_normal) for player corpse destruction.
    fn raycast_corpse(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_dist: f32,
    ) -> Option<(hecs::Entity, Vec3, Vec3)> {
        let mut closest: Option<(hecs::Entity, f32, Vec3, Vec3)> = None;
        for (entity, (transform, _)) in self.world.query::<(&Transform, &BugCorpse)>().iter() {
            let center = transform.position;
            let radius = transform.scale.x.max(transform.scale.z) * 0.6;
            let m = origin - center;
            let m_dot_d = m.dot(direction);
            let m_len_sq = m.length_squared();
            let disc = m_dot_d * m_dot_d - m_len_sq + radius * radius;
            if disc < 0.0 {
                continue;
            }
            let sqrt_d = disc.sqrt();
            let t1 = -m_dot_d - sqrt_d;
            let t2 = -m_dot_d + sqrt_d;
            let t = if t1 > 0.0 {
                t1
            } else if t2 > 0.0 {
                t2
            } else {
                continue;
            };
            if t > max_dist {
                continue;
            }
            let hit_point = origin + direction * t;
            let hit_normal = (hit_point - center).normalize_or_zero();
            let is_closer = closest.as_ref().map_or(true, |(_, d, _, _)| t < *d);
            if is_closer {
                closest = Some((entity, t, hit_point, hit_normal));
            }
        }
        closest.map(|(e, _, p, n)| (e, p, n))
    }

    fn check_bug_hits(
        &mut self,
        origin: Vec3,
        direction: Vec3,
        hit_point: Vec3,
        base_damage: f32,
        hit_entity: Option<hecs::Entity>,
    ) {
        // Only damage the entity actually hit by the ray (e.g. bug); if ray hit terrain, hit_entity is None.
        let hit_radius = 0.8;
        let mut candidates: Vec<(hecs::Entity, Vec3, f32)> = Vec::new();
        if let Some(e) = hit_entity {
            if let Ok(mut q) = self.world.query_one::<(&Transform, &Bug)>(e) {
                if let Some((transform, _)) = q.get() {
                    let dist = transform.position.distance(hit_point);
                    if dist < hit_radius + transform.scale.x {
                        candidates.push((e, transform.position, dist));
                    }
                }
            } else if let Ok(mut q) = self.world.query_one::<(&Transform, &Skinny)>(e) {
                if let Some((transform, _)) = q.get() {
                    let dist = transform.position.distance(hit_point);
                    if dist < hit_radius + transform.scale.x {
                        candidates.push((e, transform.position, dist));
                    }
                }
            }
        }

        for (entity, bug_pos, _dist) in candidates {
            let hit_height = hit_point.y - bug_pos.y;
            let is_headshot = hit_height > 0.3;
            let damage = if is_headshot { base_damage * 2.0 } else { base_damage };

            // Store impact direction for ragdoll
            if let Ok(mut physics_bug) = self.world.get::<&mut PhysicsBug>(entity) {
                physics_bug.impact_velocity = direction * damage * 0.5;
            }

            if let Ok(mut health) = self.world.get::<&mut Health>(entity) {
                health.take_damage(damage);
                let was_kill = health.is_dead();

                // Spawn blood splatter on hit
                self.effects.spawn_bullet_impact(hit_point, -direction, true);

                self.combat.hit_markers.push(crate::fps::HitMarker {
                    is_kill: was_kill,
                    is_headshot,
                    lifetime: 0.3,
                });

                self.combat.damage_numbers.push(crate::fps::DamageNumber {
                    position: hit_point + Vec3::Y * 0.5,
                    damage,
                    is_critical: is_headshot,
                    lifetime: 1.0,
                    velocity: Vec3::new(
                        rand::random::<f32>() * 2.0 - 1.0,
                        3.0,
                        rand::random::<f32>() * 2.0 - 1.0,
                    ),
                });

                if was_kill {
                    self.player.kills += 1;
                    self.player.damage_dealt += damage;

                    // Cinematic: kill streak + extra shake on kills
                    self.kill_streaks.register_kill();
                    self.screen_shake.add_trauma(0.12);

                    // Headshot kills get extra screen shake
                    if is_headshot {
                        self.screen_shake.add_trauma(0.15);
                    }

                    let victim_name = if let Ok(bug) = self.world.get::<&Bug>(entity) {
                        format!("{:?}", bug.bug_type)
                    } else if let Ok(skinny) = self.world.get::<&Skinny>(entity) {
                        skinny.skinny_type.display_name().to_string()
                    } else {
                        "Enemy".to_string()
                    };
                    self.combat.kill_feed.push(crate::fps::KillFeedEntry {
                        killer: self.player.callsign.clone(),
                        victim: victim_name,
                        weapon: self.player.current_weapon().weapon_type,
                        was_headshot: is_headshot,
                        lifetime: 5.0,
                    });
                }
            }
        }
    }

    fn check_destructible_hits(&mut self, hit_point: Vec3, damage: f32) {
        let hit_radius = 1.2;

        // Query ALL destructible entities (rocks, bug holes, hive structures, eggs, props)
        let to_damage: Vec<(hecs::Entity, Vec3, u32, f32)> = self
            .world
            .query::<(&Transform, &Destructible)>()
            .iter()
            .filter_map(|(entity, (transform, destructible))| {
                if destructible.health <= 0.0 {
                    return None;
                }
                let dist = transform.position.distance(hit_point);
                let radius = transform.scale.x * 1.5;
                if dist < hit_radius + radius {
                    Some((entity, transform.position, destructible.debris_count, destructible.debris_size))
                } else {
                    None
                }
            })
            .collect();

        let mut to_spawn_debris: Vec<(Vec3, u32, f32)> = Vec::new();
        let mut chain_reactions: Vec<(Vec3, f32, f32)> = Vec::new(); // (center, radius, damage)
        for (entity, pos, debris_count, debris_size) in to_damage {
            let destroyed = self.world.get::<&mut Destructible>(entity).map_or(false, |mut d| d.damage(damage));
            if destroyed {
                to_spawn_debris.push((pos, debris_count, debris_size));
                if let Ok(chain) = self.world.get::<&ChainReaction>(entity) {
                    chain_reactions.push((pos, chain.radius, chain.damage));
                }
            }
        }
        for (pos, debris_count, debris_size) in to_spawn_debris {
            self.destruction.spawn_debris(&mut self.world, pos, debris_count, debris_size, &mut self.physics);
        }
        for (center, radius, chain_damage) in chain_reactions {
            self.apply_chain_reaction(center, radius, chain_damage);
        }

        // Remove all destroyed destructible entities (and their physics bodies)
        let to_remove: Vec<hecs::Entity> = self
            .world
            .query::<&Destructible>()
            .iter()
            .filter(|(_, d)| d.health <= 0.0)
            .map(|(e, _)| e)
            .collect();
        for e in to_remove {
            if let Ok(phys) = self.world.get::<&DestructiblePhysics>(e) {
                self.physics.remove_body(phys.body_handle);
            }
            let _ = self.world.despawn(e);
        }
    }

    /// Apply chain reaction from a destroyed destructible: radius damage to destructibles, bugs, and player.
    fn apply_chain_reaction(&mut self, center: Vec3, radius: f32, damage: f32) {
        self.destruction.apply_explosion(
            &mut self.world,
            &mut self.physics,
            center,
            radius,
            damage,
        );
        let player_pos = self.player.position;
        let dist = (player_pos - center).length();
        if dist < radius && self.player.is_alive && !self.debug.god_mode {
            let falloff = 1.0 - (dist / radius) * 0.5;
            let amount = damage * falloff;
            let dir = (player_pos - center).normalize_or_zero();
            self.player.take_damage(amount, Some(dir));
            self.screen_shake.add_trauma((amount / 50.0).min(0.4));
        }
        for (_, (transform, health)) in self.world.query_mut::<(&Transform, &mut Health)>() {
            if health.is_dead() {
                continue;
            }
            let d = (transform.position - center).length();
            if d < radius {
                let falloff = 1.0 - (d / radius) * 0.5;
                health.take_damage(damage * falloff);
            }
        }
    }

    /// Cycle to the next planet in the current star system (R key).
    fn regenerate_planet(&mut self) {
        let num_planets = self.current_system.bodies.len();
        if num_planets == 0 {
            self.game_messages.warning("No planets in this system!");
            return;
        }

        // Determine the next planet index
        let next_idx = match self.current_planet_idx {
            Some(idx) => (idx + 1) % num_planets,
            None => 0,
        };

        self.game_messages.warning(format!(
            "Travelling to planet {}/{}...",
            next_idx + 1,
            num_planets
        ));

        // Clear old state: remove destructible physics bodies then despawn all entities
        self.chunk_manager.clear_all(&mut self.physics);
        let destructible_bodies: Vec<physics::RigidBodyHandle> = self
            .world
            .query::<&DestructiblePhysics>()
            .iter()
            .map(|(_, p)| p.body_handle)
            .collect();
        for h in destructible_bodies {
            self.physics.remove_body(h);
        }
        let all_entities: Vec<hecs::Entity> = self.world.iter().map(|e| e.entity()).collect();
        for entity in all_entities {
            let _ = self.world.despawn(entity);
        }
        self.effects = EffectsManager::new();
        self.rain_drops.clear();
        self.snow_particles.clear();
        self.tracer_projectiles.clear();
        self.last_player_track_pos = None;
        self.ground_track_bug_timer = 0.0;
        self.squad_track_last.clear();
        self.current_planet_idx = None;

        // Go to ship phase for drop pod deployment
        self.begin_ship_phase(next_idx);
    }

    /// True for biomes where trooper and bug footprints/trails are visible (snow, sand).
    fn biome_has_snow_or_sand(b: BiomeType) -> bool {
        matches!(
            b,
            BiomeType::Desert | BiomeType::Frozen | BiomeType::Wasteland | BiomeType::Badlands
                | BiomeType::Tundra | BiomeType::SaltFlat
        )
    }

    /// Sample snow accumulation at world (x, z). Returns 0 if outside the 128m tile or no snow.
    fn sample_snow_depth(&self, x: f32, z: f32) -> f32 {
        let (ox, oz) = self.snow_accumulation_origin;
        let world_size = 2.0 * DEFORM_HALF_SIZE;
        let texels_per_unit = (DEFORM_TEXTURE_SIZE as f32) / world_size;
        let i_f = (x - ox + DEFORM_HALF_SIZE) * texels_per_unit;
        let j_f = (z - oz + DEFORM_HALF_SIZE) * texels_per_unit;
        if i_f < -0.5 || i_f > DEFORM_TEXTURE_SIZE as f32 - 0.5
            || j_f < -0.5 || j_f > DEFORM_TEXTURE_SIZE as f32 - 0.5
        {
            return 0.0;
        }
        let i0 = (i_f - 0.5).floor() as i32;
        let j0 = (j_f - 0.5).floor() as i32;
        let i1 = (i0 + 1).min(DEFORM_TEXTURE_SIZE as i32 - 1);
        let j1 = (j0 + 1).min(DEFORM_TEXTURE_SIZE as i32 - 1);
        let i0 = i0.max(0);
        let j0 = j0.max(0);
        let fx = (i_f - 0.5 - i0 as f32).clamp(0.0, 1.0);
        let fy = (j_f - 0.5 - j0 as f32).clamp(0.0, 1.0);
        let idx = |i: i32, j: i32| i as usize + j as usize * (DEFORM_TEXTURE_SIZE as usize);
        let s00 = self.snow_accumulation_buffer[idx(i0, j0)];
        let s10 = self.snow_accumulation_buffer[idx(i1, j0)];
        let s01 = self.snow_accumulation_buffer[idx(i0, j1)];
        let s11 = self.snow_accumulation_buffer[idx(i1, j1)];
        s00 * (1.0 - fx) * (1.0 - fy) + s10 * fx * (1.0 - fy) + s01 * (1.0 - fx) * fy + s11 * fx * fy
    }

    /// Emit ground tracks (footprints) for player, squad, and bugs when moving on snow/sand.
    fn emit_ground_tracks(&mut self, dt: f32) {
        // ---- Player ----
        if self.player.is_alive && self.player.is_grounded {
            let vel_xz = Vec3::new(self.player_velocity.x, 0.0, self.player_velocity.z);
            if vel_xz.length_squared() > 0.12 {
                let foot_x = self.player.position.x;
                let foot_z = self.player.position.z;
                let foot_y = self.chunk_manager.sample_height(foot_x, foot_z)
                    + self.sample_snow_depth(foot_x, foot_z)
                    + 0.02;
                let foot_pos = Vec3::new(foot_x, foot_y, foot_z);
                let should_emit = match &self.last_player_track_pos {
                    None => true,
                    Some(last) => foot_pos.distance_squared(*last) > 0.22 * 0.22,
                };
                if should_emit {
                    let yaw = vel_xz.normalize().to_array();
                    let rotation_y = yaw[2].atan2(yaw[0]);
                    self.effects
                        .spawn_ground_track(foot_pos, rotation_y, TrackKind::TrooperFoot);
                    self.last_player_track_pos = Some(foot_pos);
                }
            }
        }

        // ---- Squad mates ----
        self.squad_track_last
            .retain(|e, _| self.world.contains(*e));
        for (entity, (transform, velocity, _)) in self
            .world
            .query::<(&Transform, &Velocity, &SquadMate)>()
            .iter()
        {
            if velocity.linear.length_squared() < 0.08 {
                continue;
            }
            let foot_x = transform.position.x;
            let foot_z = transform.position.z;
            let foot_y = self.chunk_manager.sample_height(foot_x, foot_z)
                + self.sample_snow_depth(foot_x, foot_z)
                + 0.02;
            let foot_pos = Vec3::new(foot_x, foot_y, foot_z);
            let should_emit = self
                .squad_track_last
                .get(&entity)
                .map_or(true, |last| foot_pos.distance_squared(*last) > 0.25 * 0.25);
            if should_emit {
                let vel_xz = Vec3::new(velocity.linear.x, 0.0, velocity.linear.z);
                let yaw = vel_xz.normalize().to_array();
                let rotation_y = yaw[2].atan2(yaw[0]);
                self.effects
                    .spawn_ground_track(foot_pos, rotation_y, TrackKind::TrooperFoot);
                self.squad_track_last.insert(entity, foot_pos);
            }
        }

        // ---- Bugs (throttled: up to 6 per 0.18s) ----
        self.ground_track_bug_timer += dt;
        if self.ground_track_bug_timer >= 0.18 {
            self.ground_track_bug_timer = 0.0;
            let player_pos = self.player.position;
            let mut count = 0u32;
            const MAX_BUG_TRACKS_PER_TICK: u32 = 6;
            const BUG_TRACK_RADIUS_SQ: f32 = 60.0 * 60.0;
            for (_, (transform, velocity, health, physics_bug)) in self.world.query::<(
                &Transform,
                &Velocity,
                &Health,
                &PhysicsBug,
            )>().iter()
            {
                if count >= MAX_BUG_TRACKS_PER_TICK {
                    break;
                }
                if health.is_dead() || physics_bug.is_ragdoll {
                    continue;
                }
                let vel_xz_sq = velocity.linear.x * velocity.linear.x
                    + velocity.linear.z * velocity.linear.z;
                if vel_xz_sq < 0.5 {
                    continue;
                }
                let dx = transform.position.x - player_pos.x;
                let dz = transform.position.z - player_pos.z;
                if dx * dx + dz * dz > BUG_TRACK_RADIUS_SQ {
                    continue;
                }
                let foot_x = transform.position.x;
                let foot_z = transform.position.z;
                let foot_y = self.chunk_manager.sample_height(foot_x, foot_z)
                    + self.sample_snow_depth(foot_x, foot_z)
                    + 0.02;
                let foot_pos = Vec3::new(foot_x, foot_y, foot_z);
                let vel_xz = Vec3::new(velocity.linear.x, 0.0, velocity.linear.z);
                let yaw = vel_xz.normalize().to_array();
                let rotation_y = yaw[2].atan2(yaw[0]);
                self.effects
                    .spawn_ground_track(foot_pos, rotation_y, TrackKind::BugFoot);
                count += 1;
            }
        }
    }

    /// Leave the current planet and enter open space.
    fn leave_planet(&mut self) {
        if let Some(idx) = self.current_planet_idx {
            self.game_messages.info(format!("Leaving {} orbit...", self.planet.name));
            let planet_pos = self.current_system.bodies[idx].orbital_position(self.orbital_time);

            // Convert planet-local position to solar system position
            self.universe_position = planet_pos + DVec3::new(
                self.camera.position().x as f64,
                self.camera.position().y as f64,
                self.camera.position().z as f64,
            );

            self.current_planet_idx = None;
            self.defense_base = None;
            self.settlement_center = None;
            self.earth_waypoints = None;
            self.earth_roads_mesh = None;
            for h in self.earth_road_colliders.drain(..) {
                self.physics.remove_collider(h);
            }
            for h in self.earth_building_colliders.drain(..) {
                self.physics.remove_collider(h);
            }
            self.dialogue_state = DialogueState::Closed;

            // Clear terrain chunks (we're in space now)
            self.chunk_manager.clear_all(&mut self.physics);

            // Despawn ground entities
            let all_entities: Vec<hecs::Entity> = self.world.iter().map(|e| e.entity()).collect();
            for entity in all_entities {
                let _ = self.world.despawn(entity);
            }
            self.effects = EffectsManager::new();
            self.rain_drops.clear();
            self.snow_particles.clear();
            self.artillery_shells.clear();
            self.artillery_muzzle_flashes.clear();
            self.artillery_trail_particles.clear();
            self.grounded_artillery_shells.clear();
            for c in self.shell_casings.drain(..) {
                self.physics.remove_body(c.body_handle);
            }
            for s in self.grounded_shell_casings.drain(..) {
                self.physics.remove_body(s.body_handle);
            }
            self.artillery_barrage = None;
            self.extraction_squadmates_aboard.clear();
            self.last_player_track_pos = None;
            self.ground_track_bug_timer = 0.0;
            self.squad_track_last.clear();

            // Teleport camera to the universe position; zero velocity for space
            self.camera.transform.position = Vec3::new(
                self.universe_position.x as f32,
                self.universe_position.y as f32,
                self.universe_position.z as f32,
            );
            self.player_velocity = Vec3::ZERO;
        }
    }

    /// Check if the player is close enough to a planet to land.
    /// Transitions to the ship interior phase instead of instant landing.
    fn check_planet_approach(&mut self) {
        let player_pos = DVec3::new(
            self.camera.position().x as f64,
            self.camera.position().y as f64,
            self.camera.position().z as f64,
        );

        for (i, body) in self.current_system.bodies.iter().enumerate() {
            let body_pos = body.orbital_position(self.orbital_time);
            let dist = (player_pos - body_pos).length();
            let approach_radius = body.planet.visual_radius() as f64 * 5.0;

            if dist < approach_radius {
                // Transition to ship interior for drop pod deployment
                self.begin_ship_phase(i);
                return;
            }
        }
    }

    /// Federation Bulletin / sector report (Helldivers 2 style) when entering ship.
    fn push_sector_bulletin(&mut self) {
        let num_planets = self.war_state.planets.len();
        let total_lib: f32 = self.war_state.planets.iter().map(|p| p.liberation).sum();
        let avg_lib = if num_planets > 0 { total_lib / num_planets as f32 } else { 0.0 };
        let pct = (avg_lib * 100.0) as u32;
        self.game_messages.info(format!(
            "FEDERATION BULLETIN — {} System: {} planets | Sector liberation: {}%",
            self.current_system.name, num_planets, pct,
        ));
        if let Some(order) = self.war_state.major_orders.iter().find(|o| !o.completed) {
            self.game_messages.info(format!("Major order: {} — {}", order.title, order.description));
        }
    }

    /// Enter the ship interior phase for a given planet (pre-drop staging).
    fn begin_ship_phase(&mut self, planet_idx: usize) {
        // If we were still on a planet (e.g. quit to menu then Play without having cleared), clear now
        if self.current_planet_idx.is_some() {
            self.leave_planet();
        }
        self.push_sector_bulletin();
        self.game_messages.info(format!("ROGER YOUNG — {} System", self.current_system.name));
        let body = &self.current_system.bodies[planet_idx];
        let planet = &body.planet;
        if planet.name == "Earth" {
            self.game_messages.success("Orbiting Earth — homeworld. This is what we're fighting for.".to_string());
        }
        self.game_messages.info(format!("Approach the WAR TABLE [E] — change system with ↑/↓ or W/Q, then pick a planet."));
        self.game_messages.info(format!("At war table: 1=Extermination 2=Bug Hunt 3=Hold the Line 4=Defense 5=Hive Destruction. Drop bay is aft."));

        let war_table_pos = Vec3::new(0.0, 0.0, 2.0);
        let drop_bay_pos = Vec3::new(0.0, 0.0, -28.0);

        // Flag grid resolution
        let flag_cols = 16;
        let flag_rows = 12;
        let flag_w = 3.0;
        let flag_h = 2.0;

        // UCF flag: port wall (-X side), mounted high, hanging toward center (+X)
        let ucf_flag = ClothFlag::new(
            Vec3::new(-9.4, 3.8, 8.0),  // top-left pin (near wall, high up)
            Vec3::new(0.0, 0.0, -1.0),  // pole runs along -Z (flag extends left-to-right on wall)
            Vec3::new(1.0, 0.0, 0.0),   // hangs toward center (+X, away from port wall)
            flag_w, flag_h,
            flag_cols, flag_rows,
            ucf_flag_colors(flag_cols, flag_rows),
        );

        // MI flag: starboard wall (+X side), mounted high, hanging toward center (-X)
        let mi_flag = ClothFlag::new(
            Vec3::new(9.4, 3.8, 8.0),   // top-left pin
            Vec3::new(0.0, 0.0, -1.0),  // pole runs along -Z
            Vec3::new(-1.0, 0.0, 0.0),  // hangs toward center (-X, away from starboard wall)
            flag_w, flag_h,
            flag_cols, flag_rows,
            mi_flag_colors(flag_cols, flag_rows),
        );

        // Set up ship state (preserve next_mission_type so player choice persists)
        self.ship_state = Some(ShipState {
            timer: 0.0,
            deploy_requested: false,
            target_planet_idx: planet_idx,
            selected_mission_type: self.next_mission_type,
            war_table_active: false,
            war_table_pos,
            drop_bay_pos,
            ucf_flag,
            mi_flag,
        });

        // Set the war table to this planet
        self.war_state.selected_planet = planet_idx;

        // Update planet reference for HUD display
        self.planet = planet.clone();

        // Position player inside the CIC (near aft end, facing forward toward the war table)
        self.camera.transform.position = Vec3::new(0.0, 1.7, -5.0);
        self.camera.set_yaw_pitch(0.0, 0.0); // face forward (+Z)
        self.player.position = self.camera.transform.position;
        self.player.is_alive = true;
        self.player_velocity = Vec3::ZERO;

        self.phase = GamePhase::InShip;
    }

    /// Switch the war table to a different star system (stays in ship; new procgen planets/biomes).
    fn switch_war_table_system(&mut self, system_idx: usize) {
        self.current_system_idx = system_idx;
        self.current_system = self.universe.generate_system(system_idx);
        let seed = self.current_system.seed;
        self.orbital_time = ((seed % 100000) as f64 * 0.123).rem_euclid(628.0);
        let num_planets = self.current_system.bodies.len();
        self.war_state = GalacticWarState::new(num_planets);
        self.war_state.selected_planet = 0;
        if let Some(ref mut ship) = self.ship_state {
            ship.target_planet_idx = 0;
        }
        if num_planets > 0 {
            self.planet = self.current_system.bodies[0].planet.clone();
        }
    }

    /// Complete a successful extraction — player boards the retrieval boat and
    /// returns to the Federation Destroyer in orbit.
    fn complete_extraction(&mut self) {
        let planet_idx = self.current_planet_idx.unwrap_or(0);

        // Stats summary
        let kills = self.mission.bugs_killed;
        let time = self.mission.time_survived_str();
        let peak = self.mission.peak_bugs_alive;
        let threat = self.spawner.threat_level.name();

        // Record kills and extraction in the galactic war state
        self.war_state.record_kills(planet_idx, kills);
        self.war_state.record_extraction(planet_idx);
        if let Some(status) = self.war_state.planets.get_mut(planet_idx) {
            status.active_operation = false;
        }
        save_galactic_war(self.universe_seed, self.current_system_idx, &self.war_state);

        if self.planet.name == "Earth" {
            self.game_messages.success("Dropship returning to Roger Young. Good visit, trooper.".to_string());
            self.game_messages.info("Remember what we're fighting for. The Federation thanks you.".to_string());
        } else {
            self.game_messages.success(format!(
                "EXTRACTION COMPLETE | Kills: {} | Survived: {} | Peak bugs: {} | Threat: {}",
                kills, time, peak, threat,
            ));
            self.game_messages.info("\"I'm from Buenos Aires, and I say kill 'em all!\"".to_string());
        }

        // Clean up the planet (despawn entities, clear terrain)
        self.leave_planet();

        // Reset horde systems
        self.spawner = BugSpawner::new(self.planet.bug_spawn_rate(), self.planet.danger_level);
        let biome_table = get_biome_feature_table(self.planet.primary_biome);
        self.spawner.set_biome_variant(biome_table.bug_variant, biome_table.variant_chance);
        self.mission = MissionState::new_horde();
        self.smoke_grenades.clear();
        self.smoke_clouds.clear();
        self.tac_fighters.clear();
        self.tac_bombs.clear();
        self.artillery_shells.clear();
        self.artillery_muzzle_flashes.clear();
        self.artillery_trail_particles.clear();
        self.artillery_barrage = None;
        // Remove extraction hull collider if still active
        if let Some(ref mut dropship) = self.extraction {
            if let Some(body_h) = dropship.hull_body.take() {
                self.physics.remove_body(body_h);
            }
        }
        self.extraction = None;
        self.extraction_cooldown = 0.0;
        self.extraction_collider = None;
        self.lz_smoke = None;
        self.supply_drop_smoke.clear();
        self.reinforce_smoke = None;
        self.orbital_strike_smoke = None;

        // Transition back to ship interior
        self.begin_ship_phase(planet_idx);
    }

    // enter_planet is now handled by prepare_planet_for_drop() and the drop pod sequence.
}

// Helpers for biome feature spawning (landmarks, hazards, destructibles).
// Returns (scale_shape: Vec3, scale_variation, color, mesh_group) so each landmark has a distinct shape (pillars tall, pools flat, etc.).
fn landmark_visuals(
    landmark_type: LandmarkType,
    _primary: BiomeType,
    rock_color: &[f32; 4],
    prop_color: &[f32; 4],
    pool_color: &[f32; 4],
) -> (Vec3, f32, [f32; 4], u8) {
    use glam::Vec3 as V3;
    match landmark_type {
        LandmarkType::RockArch => (V3::new(2.2, 0.9, 1.0), 0.3, *rock_color, MESH_GROUP_ROCK),
        LandmarkType::MesaPillar | LandmarkType::CliffSpire => (V3::new(0.9, 2.2, 0.9), 0.35, *rock_color, MESH_GROUP_ROCK),
        LandmarkType::ObsidianSpire => (V3::new(0.5, 2.5, 0.5), 0.3, *rock_color, MESH_GROUP_ROCK),
        LandmarkType::DeadTree => (V3::new(0.4, 2.0, 0.4), 0.4, *rock_color, MESH_GROUP_ROCK),
        LandmarkType::RustedVehicle => (V3::new(1.8, 0.6, 0.9), 0.25, *rock_color, MESH_GROUP_ROCK),
        LandmarkType::SandDuneRidge | LandmarkType::DriedRavine | LandmarkType::AshDrift => {
            (V3::new(2.0, 0.5, 1.2), 0.4, [rock_color[0] * 0.9, rock_color[1] * 0.9, rock_color[2] * 0.9, 1.0], MESH_GROUP_ROCK)
        }
        LandmarkType::OasisPool | LandmarkType::FrozenLake | LandmarkType::MuddyPool | LandmarkType::PrismaticPool => {
            (V3::new(2.5, 0.25, 2.5), 0.35, *pool_color, MESH_GROUP_ROCK)
        }
        LandmarkType::ResinNode | LandmarkType::MutantGrowth | LandmarkType::EmberMound => {
            (V3::new(0.8, 1.0, 0.8), 0.4, *prop_color, MESH_GROUP_HIVE_MOUND)
        }
        LandmarkType::PulsingEggWall | LandmarkType::OrganicTunnel => {
            (V3::new(1.2, 1.0, 0.4), 0.3, [0.35, 0.25, 0.18, 1.0], MESH_GROUP_HIVE_MOUND)
        }
        LandmarkType::LavaRiver | LandmarkType::Geyser | LandmarkType::AcidGeyser | LandmarkType::GasVent => {
            (V3::new(1.2, 0.6, 1.8), 0.35, *pool_color, MESH_GROUP_ROCK)
        }
        LandmarkType::IcePillar | LandmarkType::CrystalPillar | LandmarkType::MirrorShard => {
            (V3::new(0.45, 2.2, 0.45), 0.35, *prop_color, MESH_GROUP_ROCK)
        }
        LandmarkType::GlacialRidge => (V3::new(1.5, 0.8, 0.6), 0.3, *prop_color, MESH_GROUP_ROCK),
        LandmarkType::BoulderField | LandmarkType::RadiationCrater => (V3::new(1.2, 0.7, 1.2), 0.4, *rock_color, MESH_GROUP_ROCK),
        LandmarkType::WaterfallCliff | LandmarkType::CollapsedRuin | LandmarkType::TwistedRebar => {
            (V3::new(1.0, 1.5, 0.8), 0.35, *rock_color, MESH_GROUP_ROCK)
        }
        LandmarkType::FogBank => (V3::new(3.0, 0.4, 2.5), 0.3, [0.5, 0.5, 0.5, 0.4], MESH_GROUP_ROCK),
        LandmarkType::GiantAlienTree => (V3::new(0.7, 2.8, 0.7), 0.35, *prop_color, MESH_GROUP_ROCK),
        LandmarkType::VineWall => (V3::new(0.3, 1.8, 2.0), 0.4, *prop_color, MESH_GROUP_ROCK),
        LandmarkType::BioluminescentFlower => (V3::new(0.5, 0.8, 0.5), 0.4, [0.4, 0.8, 0.5, 1.0], MESH_GROUP_PROP_SPHERE),
        LandmarkType::CanyonWall => (V3::new(2.0, 1.5, 0.6), 0.3, *rock_color, MESH_GROUP_ROCK),
        // UCF (Federation) structures: blocky military/colony buildings
        LandmarkType::UCFColony => (V3::new(2.5, 1.8, 2.2), 0.25, [0.42, 0.44, 0.48, 1.0], MESH_GROUP_CUBE),
        LandmarkType::UCFBase => (V3::new(1.8, 2.2, 1.8), 0.3, [0.35, 0.38, 0.40, 1.0], MESH_GROUP_CUBE),
        LandmarkType::UCFBaseWall => (V3::new(4.0, 3.5, 2.0), 0.0, [0.32, 0.35, 0.38, 1.0], MESH_GROUP_CUBE),
        // Caves and abandoned UCF structures
        LandmarkType::CaveEntrance => (V3::new(2.5, 1.2, 1.8), 0.35, [0.22, 0.20, 0.18, 1.0], MESH_GROUP_BUG_HOLE),
        LandmarkType::HiveCaveEntrance => (V3::new(3.2, 1.5, 2.5), 0.4, [0.14, 0.10, 0.08, 1.0], MESH_GROUP_HIVE_CAVE_ENTRANCE),
        LandmarkType::AbandonedUCFResearchStation => (V3::new(2.2, 2.0, 2.0), 0.3, [0.38, 0.40, 0.42, 1.0], MESH_GROUP_CUBE),
        LandmarkType::AbandonedUCFBase => (V3::new(2.5, 2.5, 2.5), 0.35, [0.30, 0.32, 0.34, 1.0], MESH_GROUP_CUBE),
    }
}

fn hazard_params(hazard_type: HazardType) -> (f32, f32, f32) {
    match hazard_type {
        HazardType::Sandstorm => (15.0, 0.0, 20.0),
        HazardType::Rockslide => (8.0, 25.0, 12.0),
        HazardType::SporeBurst => (6.0, 8.0, 8.0),
        HazardType::GeyserEruption => (5.0, 30.0, 10.0),
        HazardType::LavaFlow => (4.0, 15.0, 0.0),
        HazardType::Blizzard => (18.0, 0.0, 25.0),
        HazardType::IceCrack => (6.0, 20.0, 15.0),
        HazardType::PoisonGas => (7.0, 5.0, 6.0),
        HazardType::Avalanche => (10.0, 35.0, 14.0),
        HazardType::Quicksand => (5.0, 0.0, 0.0),
        HazardType::Leeches => (4.0, 3.0, 5.0),
        HazardType::CrystalResonance => (8.0, 20.0, 0.0),
        HazardType::EmberStorm => (12.0, 10.0, 18.0),
        HazardType::CarnivorousPlant => (3.0, 20.0, 4.0),
        HazardType::RadiationZone => (6.0, 8.0, 0.0),
    }
}

fn hazard_visual_color(hazard_type: HazardType) -> [f32; 4] {
    match hazard_type {
        HazardType::Sandstorm => [0.9, 0.85, 0.7, 0.3],
        HazardType::Rockslide => [0.4, 0.35, 0.3, 0.5],
        HazardType::SporeBurst => [0.3, 0.5, 0.2, 0.5],
        HazardType::GeyserEruption => [0.7, 0.8, 0.9, 0.6],
        HazardType::LavaFlow => [0.95, 0.4, 0.1, 0.7],
        HazardType::Blizzard => [0.7, 0.8, 0.95, 0.4],
        HazardType::IceCrack => [0.5, 0.7, 0.9, 0.5],
        HazardType::PoisonGas => [0.4, 0.7, 0.2, 0.5],
        HazardType::Avalanche => [0.6, 0.6, 0.65, 0.5],
        HazardType::Quicksand => [0.45, 0.35, 0.25, 0.6],
        HazardType::Leeches => [0.2, 0.25, 0.15, 0.6],
        HazardType::CrystalResonance => [0.6, 0.4, 0.8, 0.4],
        HazardType::EmberStorm => [0.9, 0.4, 0.1, 0.5],
        HazardType::CarnivorousPlant => [0.2, 0.5, 0.15, 0.6],
        HazardType::RadiationZone => [0.2, 0.9, 0.2, 0.5],
    }
}

fn chain_reaction_params(landmark_type: LandmarkType) -> (f32, f32, ChainEffect) {
    match landmark_type {
        LandmarkType::RockArch | LandmarkType::MesaPillar => (4.0, 40.0, ChainEffect::Collapse),
        LandmarkType::ResinNode => (5.0, 30.0, ChainEffect::Explosion),
        LandmarkType::ObsidianSpire => (6.0, 50.0, ChainEffect::Explosion),
        LandmarkType::IcePillar => (5.0, 25.0, ChainEffect::CrystalShatter),
        LandmarkType::GasVent => (4.0, 35.0, ChainEffect::AcidSplash),
        LandmarkType::BoulderField => (5.0, 45.0, ChainEffect::BoulderRoll),
        LandmarkType::DeadTree | LandmarkType::GiantAlienTree => (8.0, 60.0, ChainEffect::Collapse),
        LandmarkType::CrystalPillar => (6.0, 30.0, ChainEffect::CrystalShatter),
        LandmarkType::EmberMound => (5.0, 40.0, ChainEffect::FireSpread),
        LandmarkType::RustedVehicle => (6.0, 55.0, ChainEffect::Explosion),
        LandmarkType::UCFColony | LandmarkType::UCFBase | LandmarkType::UCFBaseWall => (6.0, 45.0, ChainEffect::Explosion),
        LandmarkType::CaveEntrance => (5.0, 35.0, ChainEffect::Collapse),
        LandmarkType::HiveCaveEntrance => (6.5, 55.0, ChainEffect::Collapse),
        LandmarkType::PulsingEggWall => (4.0, 22.0, ChainEffect::AcidSplash),
        LandmarkType::OrganicTunnel => (5.0, 40.0, ChainEffect::Collapse),
        LandmarkType::AbandonedUCFResearchStation | LandmarkType::AbandonedUCFBase => (6.0, 50.0, ChainEffect::Explosion),
        _ => (3.0, 20.0, ChainEffect::Explosion),
    }
}

impl GameState {
    /// Spawn biome-specific content (rocks, bug holes, hive structures, eggs, decorations).
    /// Called when entering a planet to populate it with appropriate environment objects.
    /// When is_base_defense, skips UCF structures (we build our own base) and uses larger clearance.
    fn spawn_biome_content(&mut self, planet: &Planet, is_base_defense: bool) {
        let scatter_range = self.chunk_manager.chunk_size * 3.0;
        let mut rng = rand::rngs::StdRng::seed_from_u64(planet.seed.wrapping_add(777));

        // Player clearance zone: don't spawn props near the expected landing area.
        // Base defense: larger clearance for the base perimeter (~25m radius).
        let clearance_radius = if is_base_defense { 30.0_f32 } else { 12.0_f32 };
        let clearance_sq = clearance_radius * clearance_radius;

        // Determine what biomes are present on this planet
        let biomes = &self.chunk_manager.planet_biomes.biomes;
        let has_hive = biomes.contains(&BiomeType::HiveWorld);
        let primary = planet.primary_biome;

        // Pre-compute biome-dependent colors for cached rendering
        let rock_color: [f32; 4] = match primary {
            BiomeType::Desert | BiomeType::Badlands => [0.55, 0.45, 0.32, 1.0],
            BiomeType::Volcanic | BiomeType::Ashlands | BiomeType::Scorched => [0.25, 0.22, 0.20, 1.0],
            BiomeType::Frozen | BiomeType::Tundra => [0.55, 0.58, 0.62, 1.0],
            BiomeType::Toxic | BiomeType::Swamp => [0.35, 0.38, 0.30, 1.0],
            BiomeType::Crystalline => [0.50, 0.48, 0.55, 1.0],
            BiomeType::Mountain | BiomeType::Ruins => [0.48, 0.46, 0.44, 1.0],
            BiomeType::HiveWorld => [0.35, 0.28, 0.22, 1.0],
            BiomeType::Jungle | BiomeType::Fungal => [0.38, 0.40, 0.30, 1.0],
            BiomeType::Wasteland => [0.40, 0.38, 0.35, 1.0],
            BiomeType::SaltFlat => [0.82, 0.80, 0.78, 1.0],
            BiomeType::Storm => [0.32, 0.34, 0.36, 1.0],
            _ => [0.45, 0.42, 0.40, 1.0],
        };
        let prop_color: [f32; 4] = match primary {
            BiomeType::Crystalline => [0.55, 0.45, 0.70, 1.0],  // Prismatic purple
            BiomeType::Jungle => [0.22, 0.45, 0.14, 1.0],      // Rich jungle green
            BiomeType::Swamp => [0.30, 0.35, 0.22, 1.0],       // Murky bayou
            BiomeType::Frozen | BiomeType::Tundra => [0.60, 0.65, 0.72, 1.0], // Ice blue
            BiomeType::Volcanic | BiomeType::Scorched => [0.30, 0.18, 0.12, 1.0], // Obsidian black
            BiomeType::Ashlands => [0.32, 0.30, 0.28, 1.0],     // Ash gray
            BiomeType::Toxic => [0.35, 0.40, 0.25, 1.0],       // Sickly green
            BiomeType::Desert | BiomeType::SaltFlat => [0.52, 0.45, 0.35, 1.0], // Sandy/salt
            BiomeType::Badlands => [0.48, 0.38, 0.32, 1.0],    // Red rock
            BiomeType::Mountain => [0.42, 0.44, 0.46, 1.0],    // Alpine gray
            BiomeType::Wasteland | BiomeType::Ruins => [0.38, 0.35, 0.30, 1.0], // Rust, decay
            BiomeType::HiveWorld => [0.35, 0.28, 0.22, 1.0],   // Organic brown
            BiomeType::Fungal => [0.38, 0.32, 0.42, 1.0],      // Purple fungal
            BiomeType::Storm => [0.35, 0.36, 0.38, 1.0],      // Storm grey
            _ => [0.48, 0.45, 0.40, 1.0],
        };
        let pool_color: [f32; 4] = match primary {
            BiomeType::Toxic | BiomeType::Swamp | BiomeType::Fungal => [0.2, 0.65, 0.1, 1.0],
            BiomeType::Volcanic | BiomeType::Ashlands | BiomeType::Scorched => [0.85, 0.3, 0.05, 1.0],
            BiomeType::Frozen | BiomeType::Tundra => [0.3, 0.6, 0.85, 1.0],
            BiomeType::Crystalline => [0.6, 0.2, 0.7, 1.0],
            BiomeType::Storm => [0.25, 0.4, 0.5, 1.0],
            _ => [0.3, 0.5, 0.2, 1.0],
        };

        // ---- Bug holes (count varies by biome) — Earth is UCF safe zone, no holes ----
        let is_earth = planet.name == "Earth";
        let bug_hole_count = if is_earth {
            0
        } else if has_hive {
            rng.gen_range(35..60)
        } else {
            match primary {
                BiomeType::Toxic | BiomeType::Swamp | BiomeType::Fungal => rng.gen_range(6..16),
                BiomeType::Jungle => rng.gen_range(5..12),
                BiomeType::Badlands | BiomeType::Ruins => rng.gen_range(4..11),
                BiomeType::Desert | BiomeType::Storm => rng.gen_range(3..9),
                BiomeType::Volcanic | BiomeType::Ashlands | BiomeType::Scorched => rng.gen_range(2..6),
                BiomeType::Frozen | BiomeType::Crystalline | BiomeType::SaltFlat => rng.gen_range(2..6),
                BiomeType::Wasteland => rng.gen_range(2..6),
                BiomeType::Mountain | BiomeType::Tundra => rng.gen_range(1..5),
                _ => rng.gen_range(3..8),
            }
        };

        for _ in 0..bug_hole_count {
            let x = (rng.gen::<f32>() - 0.5) * scatter_range;
            let z = (rng.gen::<f32>() - 0.5) * scatter_range;
            if x * x + z * z < clearance_sq { continue; }
            let y = self.chunk_manager.sample_height(x, z);
            let scale = 1.5 + rng.gen::<f32>() * 1.5;
            // Extermination: holes vomit bugs faster and in larger bursts
            let spawn_interval = if has_hive { 2.0 + rng.gen::<f32>() * 2.5 } else { 4.0 + rng.gen::<f32>() * 4.0 };
            let max_bugs = if has_hive { 12 } else { 6 };
            let t = Transform {
                position: Vec3::new(x, y - scale * 0.2, z),
                rotation: Quat::from_rotation_y(rng.gen::<f32>() * std::f32::consts::TAU),
                scale: Vec3::new(scale, scale * 0.4, scale),
            };
            let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color: [0.18, 0.14, 0.10, 1.0], mesh_group: MESH_GROUP_BUG_HOLE };
            self.world.spawn((t, Destructible::new(200.0 + scale * 50.0, 6, 0.4), BugHole::new(spawn_interval, max_bugs), cached));
        }

        // ---- Hive structures (only on HiveWorlds, never on Earth) ----
        if has_hive && !is_earth {
            // Hive tunnel entrances: Minecraft-style cave mouths; bugs pour out; awesome collapse when destroyed
            let tunnel_count = rng.gen_range(15..32);
            for _ in 0..tunnel_count {
                let x = (rng.gen::<f32>() - 0.5) * scatter_range;
                let z = (rng.gen::<f32>() - 0.5) * scatter_range;
                if x * x + z * z < clearance_sq { continue; }
                let y = self.chunk_manager.sample_height(x, z);
                let scale = 2.5 + rng.gen::<f32>() * 1.8;
                let spawn_interval = 2.5 + rng.gen::<f32>() * 2.5;
                let t = Transform {
                    position: Vec3::new(x, y - scale * 0.25, z),
                    rotation: Quat::from_rotation_y(rng.gen::<f32>() * std::f32::consts::TAU),
                    scale: Vec3::new(scale, scale * 0.5, scale),
                };
                let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color: [0.12, 0.08, 0.06, 1.0], mesh_group: MESH_GROUP_HIVE_CAVE_ENTRANCE };
                self.world.spawn((
                    t,
                    Destructible::new(550.0 + scale * 40.0, 22, 0.52),
                    BugHole::new(spawn_interval, 16),
                    ChainReaction { radius: 6.5, damage: 58.0, effect: ChainEffect::Collapse },
                    HiveTunnelEntrance,
                    cached,
                ));
            }

            // Hive nests: organic mounds full of eggs — explode in goo and chain-react
            let nest_count = rng.gen_range(28..55);
            for _ in 0..nest_count {
                let x = (rng.gen::<f32>() - 0.5) * scatter_range;
                let z = (rng.gen::<f32>() - 0.5) * scatter_range;
                if x * x + z * z < clearance_sq { continue; }
                let y = self.chunk_manager.sample_height(x, z);
                let scale = 1.2 + rng.gen::<f32>() * 1.0;
                let t = Transform {
                    position: Vec3::new(x, y + scale * 0.4, z),
                    rotation: Quat::from_rotation_y(rng.gen::<f32>() * std::f32::consts::TAU),
                    scale: Vec3::new(scale, scale * 1.2, scale),
                };
                let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color: [0.55, 0.48, 0.32, 1.0], mesh_group: MESH_GROUP_HIVE_MOUND };
                self.world.spawn((
                    t,
                    Destructible::new(220.0 + scale * 50.0, 18, 0.35),
                    ChainReaction { radius: 4.5, damage: 30.0, effect: ChainEffect::AcidSplash },
                    HiveNest,
                    cached,
                ));
            }

            let hive_count = rng.gen_range(24..48);
            for _ in 0..hive_count {
                let x = (rng.gen::<f32>() - 0.5) * scatter_range;
                let z = (rng.gen::<f32>() - 0.5) * scatter_range;
                if x * x + z * z < clearance_sq { continue; }
                let y = self.chunk_manager.sample_height(x, z);
                let scale = 1.0 + rng.gen::<f32>() * 2.0;
                let t = Transform {
                    position: Vec3::new(x, y + scale * 0.3, z),
                    rotation: Quat::from_rotation_y(rng.gen::<f32>() * std::f32::consts::TAU),
                    scale: Vec3::new(scale, scale * 1.5, scale),
                };
                let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color: [0.30, 0.20, 0.15, 1.0], mesh_group: MESH_GROUP_HIVE_MOUND };
                self.world.spawn((
                    t,
                    Destructible::new(420.0 + scale * 100.0, 16, 0.4),
                    ChainReaction { radius: 5.5, damage: 38.0, effect: ChainEffect::Explosion },
                    HiveStructure,
                    cached,
                ));
            }

            // Egg clusters: tons of eggs; chain-pop in acid goo when destroyed
            let egg_count = rng.gen_range(95..175);
            for _ in 0..egg_count {
                let x = (rng.gen::<f32>() - 0.5) * scatter_range;
                let z = (rng.gen::<f32>() - 0.5) * scatter_range;
                if x * x + z * z < clearance_sq { continue; }
                let y = self.chunk_manager.sample_height(x, z);
                let scale = 0.22 + rng.gen::<f32>() * 0.45;
                let t = Transform {
                    position: Vec3::new(x, y + scale * 0.5, z),
                    rotation: Quat::from_rotation_y(rng.gen::<f32>() * std::f32::consts::TAU),
                    scale: Vec3::splat(scale),
                };
                let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color: [0.60, 0.55, 0.35, 1.0], mesh_group: MESH_GROUP_EGG_CLUSTER };
                self.world.spawn((
                    t,
                    Destructible::new(28.0, 10, 0.22),
                    ChainReaction { radius: 3.2, damage: 14.0, effect: ChainEffect::AcidSplash },
                    EggCluster,
                    cached,
                ));
            }
        }

        // ---- Generic rocks (always present, count varies by biome) ----
        let rock_count = match primary {
            BiomeType::Mountain | BiomeType::Badlands => rng.gen_range(70..120),
            BiomeType::Desert | BiomeType::Wasteland => rng.gen_range(40..80),
            BiomeType::HiveWorld => rng.gen_range(25..50),
            BiomeType::Frozen => rng.gen_range(35..65),
            BiomeType::Volcanic | BiomeType::Ashlands => rng.gen_range(50..90),
            BiomeType::Crystalline => rng.gen_range(30..55),
            BiomeType::Jungle => rng.gen_range(8..18),  // Dense canopy hides ground; few exposed rocks
            BiomeType::Swamp => rng.gen_range(12..25),
            BiomeType::Toxic => rng.gen_range(20..40),
            _ => rng.gen_range(35..65),
        };
        for _ in 0..rock_count {
            let x = (rng.gen::<f32>() - 0.5) * scatter_range;
            let z = (rng.gen::<f32>() - 0.5) * scatter_range;
            if x * x + z * z < clearance_sq { continue; }
            let y = self.chunk_manager.sample_height(x, z);
            let scale = 0.3 + rng.gen::<f32>() * 0.6;
            let t = Transform {
                position: Vec3::new(x, y + scale * 0.5, z),
                rotation: Quat::from_rotation_y(rng.gen::<f32>() * std::f32::consts::TAU),
                scale: Vec3::splat(scale),
            };
            let body = self.physics.add_static_body_with_rotation(t.position, t.rotation);
            let half = t.scale * 0.5;
            let collider = self.physics.add_static_env_box_collider(body, half);
            let phys = DestructiblePhysics { body_handle: body, collider_handle: collider };
            let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color: rock_color, mesh_group: MESH_GROUP_ROCK };
            self.world.spawn((t, Destructible::new(40.0 + scale * 60.0, 6, 0.25), Rock, cached, phys));
        }

        // ---- Biome-specific decorations (trees, crystals, etc.) ----
        let prop_count = match primary {
            BiomeType::Jungle => rng.gen_range(180..320),      // Vietnam/Minecraft: dense lush canopy
            BiomeType::Swamp => rng.gen_range(90..160),        // Bayou: drowned trees, reeds
            BiomeType::Crystalline => rng.gen_range(70..130),   // Crystal forest: dense pillars
            BiomeType::Frozen => rng.gen_range(55..100),       // Arctic: ice spires, sparse conifers
            BiomeType::Toxic => rng.gen_range(60..110),        // Chernobyl: mutant growth everywhere
            BiomeType::Volcanic | BiomeType::Ashlands => rng.gen_range(35..70),  // Obsidian, ember mounds
            BiomeType::Desert => rng.gen_range(45..85),        // Sahara: cacti, scrub, rock formations
            BiomeType::HiveWorld => rng.gen_range(50..95),     // Organic hive structures
            BiomeType::Mountain => rng.gen_range(35..65),       // Alpine: boulders, stunted trees
            BiomeType::Badlands => rng.gen_range(40..75),       // Utah: mesas, spires
            BiomeType::Wasteland => rng.gen_range(25..50),      // Post-apocalyptic debris
            _ => rng.gen_range(25..50),
        };
        for _ in 0..prop_count {
            let x = (rng.gen::<f32>() - 0.5) * scatter_range;
            let z = (rng.gen::<f32>() - 0.5) * scatter_range;
            if x * x + z * z < clearance_sq { continue; }
            let y = self.chunk_manager.sample_height(x, z);
            let scale = 0.5 + rng.gen::<f32>() * 1.5;

            // Scale and shape vary by biome for distinctive look
            let prop_scale = match primary {
                BiomeType::Crystalline => {
                    let v = rng.gen::<f32>();
                    if v < 0.4 { Vec3::new(scale * 0.45, scale * 2.5, scale * 0.45) }      // tall pillars
                    else if v < 0.7 { Vec3::new(scale * 0.6, scale * 1.8, scale * 0.6) }  // mid crystals
                    else { Vec3::new(scale * 0.9, scale * 1.2, scale * 0.9) }             // cluster shards
                }
                BiomeType::Jungle => {
                    let variant = rng.gen::<f32>();
                    if variant < 0.35 { Vec3::new(scale * 0.7, scale * 3.2, scale * 0.7) }
                    else if variant < 0.6 { Vec3::new(scale * 0.9, scale * 2.6, scale * 0.9) }
                    else if variant < 0.85 { Vec3::new(scale * 1.3, scale * 1.4, scale * 1.3) }
                    else { Vec3::new(scale * 0.5, scale * 2.2, scale * 0.5) }
                }
                BiomeType::Swamp => {
                    let v = rng.gen::<f32>();
                    if v < 0.5 { Vec3::new(scale * 1.0, scale * 2.0, scale * 1.0) }      // drowned cypress
                    else if v < 0.8 { Vec3::new(scale * 0.6, scale * 1.4, scale * 0.6) }  // twisted snags
                    else { Vec3::new(scale * 1.3, scale * 1.0, scale * 1.3) }             // low stumps
                }
                BiomeType::Frozen => {
                    let v = rng.gen::<f32>();
                    if v < 0.4 { Vec3::new(scale * 0.5, scale * 2.0, scale * 0.5) }       // ice spires
                    else if v < 0.7 { Vec3::new(scale * 0.8, scale * 1.3, scale * 0.8) }  // squat formations
                    else { Vec3::new(scale * 0.6, scale * 1.8, scale * 0.6) }             // conifer-like
                }
                BiomeType::Toxic => {
                    let v = rng.gen::<f32>();
                    if v < 0.35 { Vec3::new(scale * 1.1, scale * 1.6, scale * 1.1) }     // bulbous growths
                    else if v < 0.7 { Vec3::new(scale * 0.8, scale * 1.3, scale * 0.8) }  // fungal stalks
                    else { Vec3::new(scale * 1.4, scale * 0.9, scale * 1.4) }             // flat caps
                }
                BiomeType::Desert => {
                    let v = rng.gen::<f32>();
                    if v < 0.5 { Vec3::new(scale * 0.35, scale * 2.2, scale * 0.35) }     // tall cacti
                    else if v < 0.8 { Vec3::new(scale * 1.2, scale * 0.8, scale * 1.2) }  // scrub bushes
                    else { Vec3::new(scale * 0.5, scale * 1.5, scale * 0.5) }             // yucca-like
                }
                BiomeType::Volcanic | BiomeType::Ashlands => {
                    let v = rng.gen::<f32>();
                    if v < 0.5 { Vec3::new(scale * 0.4, scale * 2.2, scale * 0.4) }      // obsidian spires
                    else if v < 0.8 { Vec3::new(scale * 0.9, scale * 1.2, scale * 0.9) }  // lava rock
                    else { Vec3::new(scale * 1.1, scale * 0.7, scale * 1.1) }             // ember mounds
                }
                BiomeType::Mountain => {
                    let v = rng.gen::<f32>();
                    if v < 0.5 { Vec3::new(scale * 1.2, scale * 0.9, scale * 1.2) }       // boulders
                    else if v < 0.8 { Vec3::new(scale * 0.5, scale * 1.6, scale * 0.5) }  // stunted trees
                    else { Vec3::new(scale * 0.7, scale * 1.4, scale * 0.7) }             // rock spires
                }
                BiomeType::Badlands => {
                    let v = rng.gen::<f32>();
                    if v < 0.5 { Vec3::new(scale * 0.6, scale * 1.9, scale * 0.6) }       // mesa spires
                    else if v < 0.8 { Vec3::new(scale * 1.0, scale * 1.2, scale * 1.0) }  // hoodoos
                    else { Vec3::new(scale * 0.8, scale * 1.5, scale * 0.8) }             // rock pillars
                }
                BiomeType::Wasteland => {
                    let v = rng.gen::<f32>();
                    if v < 0.4 { Vec3::new(scale * 1.5, scale * 0.6, scale * 1.0) }       // wreckage
                    else if v < 0.7 { Vec3::new(scale * 0.8, scale * 1.3, scale * 0.8) }  // rebar
                    else { Vec3::new(scale * 1.0, scale * 1.0, scale * 1.0) }             // debris
                }
                BiomeType::HiveWorld => {
                    let v = rng.gen::<f32>();
                    if v < 0.5 { Vec3::new(scale * 1.0, scale * 1.6, scale * 1.0) }       // resin nodes
                    else { Vec3::new(scale * 0.8, scale * 1.2, scale * 0.8) }             // organic stalks
                }
                _ => Vec3::splat(scale),
            };

            let t = Transform {
                position: Vec3::new(x, y + scale * 0.4, z),
                rotation: Quat::from_rotation_y(rng.gen::<f32>() * std::f32::consts::TAU),
                scale: prop_scale,
            };
            let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color: prop_color, mesh_group: MESH_GROUP_PROP_SPHERE };
            self.world.spawn((t, Destructible::new(60.0 + scale * 40.0, 4, 0.2), EnvironmentProp, cached));
        }

        // ---- Undergrowth: small vegetation / ground clutter per biome ----
        let undergrowth_count = match primary {
            BiomeType::Jungle => rng.gen_range(280..450),   // Vietnam lush: ferns, bushes, vines
            BiomeType::Swamp => rng.gen_range(120..200),    // Bayou: reeds, cattails, murky growth
            BiomeType::Toxic => rng.gen_range(90..160),     // Chernobyl: fungal mats, spores
            BiomeType::Desert => rng.gen_range(35..65),     // Sparse scrub, tumbleweed clusters
            BiomeType::Frozen => rng.gen_range(25..50),     // Tundra: lichen, frozen grass
            BiomeType::Crystalline => rng.gen_range(55..95), // Crystal shard clusters
            _ => 0,
        };
        for _ in 0..undergrowth_count {
            let x = (rng.gen::<f32>() - 0.5) * scatter_range;
            let z = (rng.gen::<f32>() - 0.5) * scatter_range;
            if x * x + z * z < clearance_sq { continue; }
            let y = self.chunk_manager.sample_height(x, z);
            let scale = 0.15 + rng.gen::<f32>() * 0.5;
            let (prop_scale, color) = match primary {
                BiomeType::Jungle => (
                    Vec3::new(scale * (0.8 + rng.gen::<f32>() * 0.6), scale * (1.4 + rng.gen::<f32>() * 1.8), scale * (0.8 + rng.gen::<f32>() * 0.6)),
                    [0.18, 0.42, 0.12, 0.95],
                ),
                BiomeType::Swamp => (
                    Vec3::new(scale * (0.7 + rng.gen::<f32>() * 0.5), scale * (1.2 + rng.gen::<f32>() * 1.4), scale * (0.7 + rng.gen::<f32>() * 0.5)),
                    [0.28, 0.32, 0.18, 0.9],
                ),
                BiomeType::Toxic => (
                    Vec3::new(scale * (0.8 + rng.gen::<f32>() * 0.6), scale * (0.7 + rng.gen::<f32>() * 1.0), scale * (0.8 + rng.gen::<f32>() * 0.6)),
                    [0.32, 0.45, 0.22, 0.9],
                ),
                BiomeType::Desert => (
                    Vec3::new(scale * (1.0 + rng.gen::<f32>() * 0.8), scale * (0.4 + rng.gen::<f32>() * 0.6), scale * (1.0 + rng.gen::<f32>() * 0.8)),
                    [0.45, 0.40, 0.28, 0.85],
                ),
                BiomeType::Frozen => (
                    Vec3::new(scale * (0.6 + rng.gen::<f32>() * 0.5), scale * (0.5 + rng.gen::<f32>() * 0.8), scale * (0.6 + rng.gen::<f32>() * 0.5)),
                    [0.55, 0.62, 0.68, 0.9],
                ),
                BiomeType::Crystalline => (
                    Vec3::new(scale * (0.4 + rng.gen::<f32>() * 0.4), scale * (0.8 + rng.gen::<f32>() * 1.2), scale * (0.4 + rng.gen::<f32>() * 0.4)),
                    [0.55, 0.42, 0.72, 0.9],
                ),
                _ => continue,
            };
            let t = Transform {
                position: Vec3::new(x, y + scale * 0.3, z),
                rotation: Quat::from_rotation_y(rng.gen::<f32>() * std::f32::consts::TAU),
                scale: prop_scale,
            };
            let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color, mesh_group: MESH_GROUP_PROP_SPHERE };
            self.world.spawn((t, Destructible::new(15.0 + scale * 20.0, 2, 0.1), EnvironmentProp, cached));
        }

        // ---- Crashed Federation ships / vehicle wreckage (rare, 1-3 per planet) ----
        let crash_count = rng.gen_range(1..4);
        for _ in 0..crash_count {
            let dist = 30.0 + rng.gen::<f32>() * (scatter_range * 0.4);
            let angle = rng.gen::<f32>() * std::f32::consts::TAU;
            let x = angle.cos() * dist;
            let z = angle.sin() * dist;
            let y = self.chunk_manager.sample_height(x, z);
            let scale = 1.5 + rng.gen::<f32>() * 2.0;
            // Crashed at an angle - partially buried
            let tilt_x = (rng.gen::<f32>() - 0.5) * 0.6;
            let tilt_z = (rng.gen::<f32>() - 0.5) * 0.6;
            let t = Transform {
                position: Vec3::new(x, y - scale * 0.3, z),
                rotation: Quat::from_rotation_y(rng.gen::<f32>() * std::f32::consts::TAU)
                    * Quat::from_rotation_x(tilt_x)
                    * Quat::from_rotation_z(tilt_z),
                scale: Vec3::new(scale * 2.0, scale * 0.6, scale * 1.2),
            };
            let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color: [0.25, 0.27, 0.30, 1.0], mesh_group: MESH_GROUP_ROCK };
            let body = self.physics.add_static_body_with_rotation(t.position, t.rotation);
            let collider = self.physics.add_static_env_box_collider(body, t.scale * 0.5);
            let phys = DestructiblePhysics { body_handle: body, collider_handle: collider };
            self.world.spawn((t, Destructible::new(500.0, 12, 0.4), CrashedShip, cached, phys));
        }

        // ---- Bone piles / skeleton heaps (biome-dependent) ----
        let bone_count = match primary {
            BiomeType::Desert | BiomeType::Badlands | BiomeType::Wasteland => rng.gen_range(8..20),
            BiomeType::Ashlands => rng.gen_range(5..15),
            BiomeType::HiveWorld => rng.gen_range(10..25), // lots of prey remains
            BiomeType::Toxic | BiomeType::Swamp => rng.gen_range(4..10),
            BiomeType::Frozen => rng.gen_range(3..8), // preserved in ice
            _ => rng.gen_range(2..6),
        };
        for _ in 0..bone_count {
            let x = (rng.gen::<f32>() - 0.5) * scatter_range;
            let z = (rng.gen::<f32>() - 0.5) * scatter_range;
            if x * x + z * z < clearance_sq { continue; }
            let y = self.chunk_manager.sample_height(x, z);
            let scale = 0.3 + rng.gen::<f32>() * 0.8;
            let t = Transform {
                position: Vec3::new(x, y + scale * 0.15, z),
                rotation: Quat::from_rotation_y(rng.gen::<f32>() * std::f32::consts::TAU),
                scale: Vec3::new(scale, scale * 0.3, scale),
            };
            let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color: [0.72, 0.68, 0.55, 1.0], mesh_group: MESH_GROUP_EGG_CLUSTER };
            self.world.spawn((t, Destructible::new(10.0, 4, 0.1), BonePile, cached));
        }

        // ---- Hazard pools: acid (Toxic/Swamp), lava (Volcanic/Ashlands), cryo (Frozen) ----
        let pool_count = match primary {
            BiomeType::Toxic | BiomeType::Swamp => rng.gen_range(8..22),   // More toxic/murky pools
            BiomeType::Volcanic | BiomeType::Ashlands => rng.gen_range(6..14),
            BiomeType::Frozen => rng.gen_range(4..10),   // Cryo pools, melt holes
            BiomeType::Crystalline => rng.gen_range(3..8),  // Prismatic mineral pools
            _ => 0,
        };
        for _ in 0..pool_count {
            let x = (rng.gen::<f32>() - 0.5) * scatter_range;
            let z = (rng.gen::<f32>() - 0.5) * scatter_range;
            if x * x + z * z < clearance_sq { continue; }
            let y = self.chunk_manager.sample_height(x, z);
            let scale = 1.5 + rng.gen::<f32>() * 3.0;
            let t = Transform {
                position: Vec3::new(x, y - scale * 0.1, z),
                rotation: Quat::from_rotation_y(rng.gen::<f32>() * std::f32::consts::TAU),
                scale: Vec3::new(scale, scale * 0.08, scale),
            };
            let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color: pool_color, mesh_group: MESH_GROUP_BUG_HOLE };
            self.world.spawn((t, Destructible::new(9999.0, 0, 0.0), HazardPool, cached));
        }

        // ---- Spore towers (HiveWorld + organic biomes) ----
        let spore_count = match primary {
            BiomeType::HiveWorld => rng.gen_range(18..35),
            BiomeType::Jungle => rng.gen_range(18..35),
            BiomeType::Swamp => rng.gen_range(8..18),    // Bayou: fungal growths
            BiomeType::Toxic => rng.gen_range(10..22),   // Toxic spore vents
            _ => 0,
        };
        for _ in 0..spore_count {
            let x = (rng.gen::<f32>() - 0.5) * scatter_range;
            let z = (rng.gen::<f32>() - 0.5) * scatter_range;
            if x * x + z * z < clearance_sq { continue; }
            let y = self.chunk_manager.sample_height(x, z);
            let scale = 0.8 + rng.gen::<f32>() * 1.5;
            let t = Transform {
                position: Vec3::new(x, y + scale * 1.5, z),
                rotation: Quat::from_rotation_y(rng.gen::<f32>() * std::f32::consts::TAU),
                scale: Vec3::new(scale * 0.5, scale * 3.0, scale * 0.5),
            };
            let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color: [0.22, 0.30, 0.15, 1.0], mesh_group: MESH_GROUP_HIVE_MOUND };
            self.world.spawn((t, Destructible::new(150.0 + scale * 50.0, 6, 0.3), SporeTower, cached));
        }

        // ---- Abandoned outposts / fortification ruins (0-4, more on frontier/abandoned worlds) ----
        let has_abandoned_structures = matches!(planet.classification,
            PlanetClassification::Abandoned | PlanetClassification::Frontier
            | PlanetClassification::WarZone | PlanetClassification::Research,
        );
        let outpost_count = if has_abandoned_structures {
            rng.gen_range(2..6)
        } else {
            rng.gen_range(0..3)
        };
        for _ in 0..outpost_count {
            let dist = 35.0 + rng.gen::<f32>() * (scatter_range * 0.35);
            let angle = rng.gen::<f32>() * std::f32::consts::TAU;
            let x = angle.cos() * dist;
            let z = angle.sin() * dist;
            if x * x + z * z < clearance_sq { continue; }
            let y = self.chunk_manager.sample_height(x, z);
            let scale = 2.0 + rng.gen::<f32>() * 1.5;
            let t = Transform {
                position: Vec3::new(x, y + scale * 0.3, z),
                rotation: Quat::from_rotation_y(rng.gen::<f32>() * std::f32::consts::TAU),
                scale: Vec3::new(scale * 1.5, scale * 0.8, scale * 1.5),
            };
            let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color: [0.40, 0.38, 0.36, 1.0], mesh_group: MESH_GROUP_CUBE };
            self.world.spawn((t, Destructible::new(800.0, 15, 0.5), AbandonedOutpost, cached));
        }

        // ---- Abandoned UCF research stations (Frontier, Abandoned, Research planets) ----
        let research_station_count = if has_abandoned_structures {
            match planet.classification {
                PlanetClassification::Research => rng.gen_range(3..8),
                PlanetClassification::Abandoned => rng.gen_range(2..6),
                PlanetClassification::Frontier | PlanetClassification::WarZone => rng.gen_range(1..4),
                _ => 0,
            }
        } else {
            0
        };
        for _ in 0..research_station_count {
            let x = (rng.gen::<f32>() - 0.5) * scatter_range;
            let z = (rng.gen::<f32>() - 0.5) * scatter_range;
            if x * x + z * z < clearance_sq { continue; }
            let y = self.chunk_manager.sample_height(x, z);
            let scale = 1.5 + rng.gen::<f32>() * 1.2;
            let t = Transform {
                position: Vec3::new(x, y + scale * 0.5, z),
                rotation: Quat::from_rotation_y(rng.gen::<f32>() * std::f32::consts::TAU),
                scale: Vec3::new(scale * 1.1, scale * 1.0, scale * 1.1),
            };
            let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color: [0.38, 0.40, 0.42, 1.0], mesh_group: MESH_GROUP_CUBE };
            self.world.spawn((
                t,
                Destructible::new(350.0 + scale * 60.0, 10, 0.4),
                BiomeLandmark { landmark_type: LandmarkType::AbandonedUCFResearchStation },
                cached,
            ));
        }

        // ---- Abandoned UCF bases (Abandoned, Frontier, WarZone — larger military ruins) ----
        let abandoned_base_count = if has_abandoned_structures {
            match planet.classification {
                PlanetClassification::Abandoned => rng.gen_range(2..5),
                PlanetClassification::WarZone => rng.gen_range(1..4),
                PlanetClassification::Frontier => rng.gen_range(1..3),
                _ => 0,
            }
        } else {
            0
        };
        for _ in 0..abandoned_base_count {
            let dist = 45.0 + rng.gen::<f32>() * (scatter_range * 0.4);
            let angle = rng.gen::<f32>() * std::f32::consts::TAU;
            let x = angle.cos() * dist;
            let z = angle.sin() * dist;
            if x * x + z * z < clearance_sq { continue; }
            let y = self.chunk_manager.sample_height(x, z);
            let scale = 2.0 + rng.gen::<f32>() * 1.5;
            let t = Transform {
                position: Vec3::new(x, y + scale * 0.5, z),
                rotation: Quat::from_rotation_y(rng.gen::<f32>() * std::f32::consts::TAU),
                scale: Vec3::new(scale * 1.2, scale * 1.2, scale * 1.2),
            };
            let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color: [0.30, 0.32, 0.34, 1.0], mesh_group: MESH_GROUP_CUBE };
            self.world.spawn((
                t,
                Destructible::new(500.0 + scale * 80.0, 12, 0.45),
                BiomeLandmark { landmark_type: LandmarkType::AbandonedUCFBase },
                cached,
            ));
        }

        // ---- UCF colonies / bases (Starship Troopers: Federation worlds) ----
        // Skip when base defense: we spawn our own base perimeter instead.
        let has_ucf = matches!(planet.classification,
            PlanetClassification::Colony | PlanetClassification::Outpost
            | PlanetClassification::Industrial | PlanetClassification::Research,
        );
        if has_ucf && !is_base_defense {
            let ucf_count = rng.gen_range(2..=5);
            for _ in 0..ucf_count {
                let x = (rng.gen::<f32>() - 0.5) * scatter_range;
                let z = (rng.gen::<f32>() - 0.5) * scatter_range;
                if x * x + z * z < clearance_sq { continue; }
                let y = self.chunk_manager.sample_height(x, z);
                let is_base = rng.gen_bool(0.4);
                let (landmark_type, scale_shape, scale_var, color) = if is_base {
                    (LandmarkType::UCFBase, Vec3::new(1.8, 2.2, 1.8), 0.3, [0.35, 0.38, 0.40, 1.0])
                } else {
                    (LandmarkType::UCFColony, Vec3::new(2.5, 1.8, 2.2), 0.25, [0.42, 0.44, 0.48, 1.0])
                };
                let mul = 1.0 + rng.gen::<f32>() * scale_var;
                let scale = scale_shape * mul;
                let t = Transform {
                    position: Vec3::new(x, y + scale.y * 0.5, z),
                    rotation: Quat::from_rotation_y(rng.gen::<f32>() * std::f32::consts::TAU),
                    scale,
                };
                let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color, mesh_group: MESH_GROUP_CUBE };
                let health = 200.0 + (scale.x + scale.y + scale.z) * 40.0;
                self.world.spawn((
                    t,
                    Destructible::new(health, 10, 0.4),
                    BiomeLandmark { landmark_type },
                    cached,
                ));
            }
        }

        // ---- Burn craters (Wasteland, Ashlands, any biome with small chance) ----
        let burn_count = match primary {
            BiomeType::Wasteland => rng.gen_range(6..15),
            BiomeType::Ashlands => rng.gen_range(4..10),
            BiomeType::Volcanic => rng.gen_range(3..8),
            BiomeType::Desert => rng.gen_range(1..4),
            _ => rng.gen_range(0..2),
        };
        for _ in 0..burn_count {
            let x = (rng.gen::<f32>() - 0.5) * scatter_range;
            let z = (rng.gen::<f32>() - 0.5) * scatter_range;
            if x * x + z * z < clearance_sq { continue; }
            let y = self.chunk_manager.sample_height(x, z);
            let scale = 1.0 + rng.gen::<f32>() * 2.5;
            let t = Transform {
                position: Vec3::new(x, y - scale * 0.15, z),
                rotation: Quat::from_rotation_y(rng.gen::<f32>() * std::f32::consts::TAU),
                scale: Vec3::new(scale, scale * 0.15, scale),
            };
            let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color: [0.08, 0.06, 0.05, 1.0], mesh_group: MESH_GROUP_BUG_HOLE };
            self.world.spawn((t, Destructible::new(9999.0, 0, 0.0), BurnCrater, cached));
        }

        // ---- Biome feature table: landmarks, hazards, destructibles ----
        let table = get_biome_feature_table(primary);

        // Landmark spawn: (type, min, max) -> spawn with CachedRenderData + Destructible + BiomeLandmark (distinct shapes per type)
        for (landmark_type, min_c, max_c) in &table.landmarks {
            let n = rng.gen_range(*min_c..=*max_c);
            for _ in 0..n {
                let x = (rng.gen::<f32>() - 0.5) * scatter_range;
                let z = (rng.gen::<f32>() - 0.5) * scatter_range;
                if x * x + z * z < clearance_sq { continue; }
                let y = self.chunk_manager.sample_height(x, z);
                let (scale_shape, scale_var, color, mesh_group) = landmark_visuals(*landmark_type, primary, &rock_color, &prop_color, &pool_color);
                let mul = 1.0 + rng.gen::<f32>() * scale_var;
                let scale = scale_shape * mul;
                let t = Transform {
                    position: Vec3::new(x, y + scale.y * 0.5, z),
                    rotation: Quat::from_rotation_y(rng.gen::<f32>() * std::f32::consts::TAU),
                    scale,
                };
                let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color, mesh_group };
                let health = 80.0 + (scale.x + scale.y + scale.z) * 25.0;
                let body = self.physics.add_static_body_with_rotation(t.position, t.rotation);
                let collider = self.physics.add_static_env_box_collider(body, t.scale * 0.5);
                let phys = DestructiblePhysics { body_handle: body, collider_handle: collider };
                self.world.spawn((
                    t,
                    Destructible::new(health, 5, 0.25),
                    BiomeLandmark { landmark_type: *landmark_type },
                    cached,
                    phys,
                ));
            }
        }

        // Hazard spawn: (type, min, max) -> spawn with EnvironmentalHazard + Transform + CachedRenderData
        for (hazard_type, min_c, max_c) in &table.hazards {
            let n = rng.gen_range(*min_c..=*max_c);
            for _ in 0..n {
                let x = (rng.gen::<f32>() - 0.5) * scatter_range;
                let z = (rng.gen::<f32>() - 0.5) * scatter_range;
                if x * x + z * z < clearance_sq { continue; }
                let y = self.chunk_manager.sample_height(x, z);
                let (radius, damage, interval) = hazard_params(*hazard_type);
                let hazard = EnvironmentalHazard {
                    hazard_type: *hazard_type,
                    radius,
                    damage,
                    timer: rng.gen::<f32>() * interval,
                    interval,
                    active: false,
                };
                let t = Transform {
                    position: Vec3::new(x, y, z),
                    rotation: Quat::IDENTITY,
                    scale: Vec3::new(radius * 2.0, 0.1, radius * 2.0),
                };
                let color = hazard_visual_color(*hazard_type);
                let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color, mesh_group: MESH_GROUP_HAZARD };
                self.world.spawn((t, hazard, cached));
            }
        }

        // Destructibles with chain reactions: (landmark_type, min, max) -> spawn with ChainReaction + BiomeDestructible (same distinct shapes)
        for (landmark_type, min_c, max_c) in &table.destructibles {
            let n = rng.gen_range(*min_c..=*max_c);
            for _ in 0..n {
                let x = (rng.gen::<f32>() - 0.5) * scatter_range;
                let z = (rng.gen::<f32>() - 0.5) * scatter_range;
                if x * x + z * z < clearance_sq { continue; }
                let y = self.chunk_manager.sample_height(x, z);
                let (scale_shape, scale_var, color, mesh_group) = landmark_visuals(*landmark_type, primary, &rock_color, &prop_color, &pool_color);
                let mul = 1.0 + rng.gen::<f32>() * scale_var;
                let scale = scale_shape * mul;
                let (chain_radius, chain_damage, chain_effect) = chain_reaction_params(*landmark_type);
                let t = Transform {
                    position: Vec3::new(x, y + scale.y * 0.5, z),
                    rotation: Quat::from_rotation_y(rng.gen::<f32>() * std::f32::consts::TAU),
                    scale,
                };
                let cached = CachedRenderData { matrix: t.to_matrix().to_cols_array_2d(), color, mesh_group };
                let health = 100.0 + (scale.x + scale.y + scale.z) * 35.0;
                let body = self.physics.add_static_body_with_rotation(t.position, t.rotation);
                let collider = self.physics.add_static_env_box_collider(body, t.scale * 0.5);
                let phys = DestructiblePhysics { body_handle: body, collider_handle: collider };
                self.world.spawn((
                    t,
                    Destructible::new(health, 8, 0.3),
                    BiomeDestructible { landmark_type: *landmark_type },
                    ChainReaction { radius: chain_radius, damage: chain_damage, effect: chain_effect },
                    cached,
                    phys,
                ));
            }
        }
    }

    /// Spawn UCF defense base: perimeter walls around origin (Starship Troopers Extermination style).
    /// Player and squad defend from inside; bugs spawn outside and come to you.
    fn spawn_defense_base(&mut self) {
        let base_y = self.chunk_manager.sample_height(0.0, 0.0);
        let half_extent = 14.0;
        let wall_segment_len = 4.0;
        let wall_scale = Vec3::new(4.0, 3.5, 2.0);
        let wall_color: [f32; 4] = [0.32, 0.35, 0.38, 1.0];
        let wall_health = 400.0;

        for side in 0..4 {
            let n_segments = (half_extent * 2.0 / wall_segment_len) as i32;
            for i in 0..n_segments {
                let t_along = (i as f32 + 0.5) / n_segments as f32 * 2.0 - 1.0;
                let (x, z, yaw) = match side {
                    0 => (t_along * half_extent, half_extent, 0.0),
                    1 => (t_along * half_extent, -half_extent, std::f32::consts::PI),
                    2 => (half_extent, t_along * half_extent, -std::f32::consts::FRAC_PI_2),
                    _ => (-half_extent, t_along * half_extent, std::f32::consts::FRAC_PI_2),
                };
                let pos = Vec3::new(x, base_y + wall_scale.y * 0.5, z);
                let t = Transform {
                    position: pos,
                    rotation: Quat::from_rotation_y(yaw),
                    scale: wall_scale,
                };
                let cached = CachedRenderData {
                    matrix: t.to_matrix().to_cols_array_2d(),
                    color: wall_color,
                    mesh_group: MESH_GROUP_CUBE,
                };
                self.world.spawn((
                    t,
                    Destructible::new(wall_health, 10, 0.4),
                    BiomeLandmark { landmark_type: LandmarkType::UCFBaseWall },
                    cached,
                ));
            }
        }

        self.defense_base = Some((Vec3::new(0.0, base_y, 0.0), half_extent));
    }

    /// Arrive at a new star system after warp.
    fn arrive_at_system(&mut self, system_idx: usize) {
        self.current_system_idx = system_idx;
        self.current_system = self.universe.generate_system(system_idx);
        // Randomize orbital phase so each system (and each visit) shows different planet positions
        let seed = self.current_system.seed;
        self.orbital_time = ((seed % 100000) as f64 * 0.123).rem_euclid(628.0); // ~0..100 orbits worth
        // Initialize war state for the new system
        self.war_state = GalacticWarState::new(self.current_system.bodies.len());

        self.game_messages.success(format!("Arrived at {} !", self.current_system.name));
        self.game_messages.info(format!(
            "Star: {} ({:?}) | {} planets",
            self.current_system.star.name,
            self.current_system.star.star_type,
            self.current_system.bodies.len()
        ));

        // Enter orbit of the first planet
        self.current_planet_idx = None;

        // Position player at edge of system, facing inward
        let entry_pos = Vec3::new(
            self.current_system.bodies[0].orbital_radius * 0.5,
            500.0,
            0.0,
        );
        self.camera.transform.position = entry_pos;
        self.universe_position = DVec3::new(entry_pos.x as f64, entry_pos.y as f64, entry_pos.z as f64);

        // Clear old terrain
        self.chunk_manager.clear_all(&mut self.physics);
        let all_entities: Vec<hecs::Entity> = self.world.iter().map(|e| e.entity()).collect();
        for entity in all_entities {
            let _ = self.world.despawn(entity);
        }
        self.effects = EffectsManager::new();
        self.rain_drops.clear();
        self.snow_particles.clear();
        self.tracer_projectiles.clear();
        self.last_player_track_pos = None;
        self.ground_track_bug_timer = 0.0;
        self.squad_track_last.clear();

        // Set planet to the first planet for sky/biome reference
        self.planet = self.current_system.bodies[0].planet.clone();
    }

    /// Build celestial body instances for rendering.
    /// When InShip, places star and planets in ship-local space so the view matches the bridge.
    fn build_celestial_instances(&self) -> Vec<CelestialBodyInstance> {
        let mut instances = Vec::new();
        let in_ship = self.phase == GamePhase::InShip;

        if in_ship && !self.current_system.bodies.is_empty() {
            let target_idx = self.ship_state.as_ref()
                .map(|s| s.target_planet_idx)
                .unwrap_or(0)
                .min(self.current_system.bodies.len().saturating_sub(1));
            let star = &self.current_system.star;
            let ot = self.orbital_time;

            // Realistic placement: use actual orbital positions, then orient so the Roger Young
            // has the targeted planet in front of the viewscreen (ship "points" at the target).
            let target_pos = self.current_system.bodies[target_idx].orbital_position(ot);
            let ship_pos = target_pos * 0.25; // Ship between star and target (so target is ahead)
            let target_rel = target_pos - ship_pos;
            let dist_target = target_rel.length();
            if dist_target < 1e-6 {
                // Fallback: target at origin (shouldn't happen)
                let star_pos = Vec3::new(0.0, 28.0, 150.0);
                let star_radius = (star.radius * 0.015).max(3.0).min(8.0);
                instances.push(CelestialBodyInstance {
                    position: star_pos.into(),
                    radius: star_radius,
                    color: [star.color.x, star.color.y, star.color.z, 1.0],
                    star_direction: [0.0, 0.0, 0.0, 0.0],
                    atmosphere_color: [0.0, 0.0, 0.0, 0.0],
                });
                return instances;
            }

            let scale = 70.0f32 / dist_target as f32; // Target will sit 70 units in front of the window
            let target_rel_f = Vec3::new(target_rel.x as f32, target_rel.y as f32, target_rel.z as f32);
            let forward = target_rel_f.normalize();
            // Rotation so "forward" (target direction) becomes +Z (viewscreen direction)
            let rot = Quat::from_rotation_arc(forward, Vec3::Z);

            // Star: behind the ship in system space; after rotation it's behind (-Z). Place in sky (above horizon).
            let star_rel = -ship_pos;
            let star_rel_f = Vec3::new(star_rel.x as f32, star_rel.y as f32, star_rel.z as f32) * scale;
            let star_view = rot * star_rel_f;
            let star_pos: Vec3 = star_view + Vec3::new(0.0, 28.0, 0.0); // Lift into sky so it's visible
            let star_radius = (star.radius * 0.015).max(3.0).min(8.0);
            instances.push(CelestialBodyInstance {
                position: star_pos.into(),
                radius: star_radius,
                color: [star.color.x, star.color.y, star.color.z, 1.0],
                star_direction: [0.0, 0.0, 0.0, 0.0],
                atmosphere_color: [0.0, 0.0, 0.0, 0.0],
            });

            let planet_scale = 0.04f32;
            let mut earth_view_pos: Option<Vec3> = None;

            // All planets: realistic orbital positions, rotated so target is in front
            for (i, body) in self.current_system.bodies.iter().enumerate() {
                let body_pos = body.orbital_position(ot);
                let rel = body_pos - ship_pos;
                let rel_f = Vec3::new(rel.x as f32, rel.y as f32, rel.z as f32) * scale;
                let view_pos = rot * rel_f;
                // Slight Y lift so target isn't on the floor (centered in viewscreen)
                let pos = view_pos + Vec3::new(0.0, 1.5, 0.0);

                let is_target = i == target_idx;
                if body.planet.name == "Earth" {
                    earth_view_pos = Some(pos);
                }

                let radius = if is_target {
                    (body.planet.visual_radius() * planet_scale).max(6.0).min(20.0)
                } else {
                    (body.planet.visual_radius() * planet_scale * 0.5).max(2.0).min(5.0)
                };

                let star_dir = (star_pos - pos).normalize();
                // Same surface and atmosphere color as in drop pod and on surface (Planet is single source)
                let surf = body.planet.surface_color();
                let atmo_rgb = body.planet.atmosphere_color_rgb();
                let planet_color = [surf[0], surf[1], surf[2], 0.3];
                let atmo_color = [
                    atmo_rgb[0],
                    atmo_rgb[1],
                    atmo_rgb[2],
                    if body.ring_system { 1.0 } else { 0.0 },
                ];
                instances.push(CelestialBodyInstance {
                    position: pos.into(),
                    radius,
                    color: planet_color,
                    star_direction: [star_dir.x, star_dir.y, star_dir.z, if body.planet.has_atmosphere { 1.0 } else { 0.0 }],
                    atmosphere_color: atmo_color,
                });
            }

            // Earth orbit: bustling spaceport — stations, MI corvettes, destroyers, dropships with own flight paths
            if let Some(earth_pos) = earth_view_pos {
                let ot_f = ot as f32;
                // Stations (slow orbit)
                let stations: &[(f32, f32, f32, [f32; 4])] = &[
                    (2.5, 0.0, 0.4, [0.55, 0.58, 0.62, 0.5]),
                    (3.0, 1.2, 0.35, [0.5, 0.52, 0.55, 0.5]),
                    (2.8, 2.8, 0.25, [0.6, 0.62, 0.65, 0.5]),
                    (3.2, 4.0, 0.3, [0.48, 0.5, 0.52, 0.5]),
                    (2.6, 5.1, 0.2, [0.58, 0.6, 0.62, 0.5]),
                ];
                for &(orbit_r, phase, rad, color) in stations {
                    let angle = phase + ot_f * 0.12;
                    let offset = Vec3::new(angle.cos() * orbit_r, (angle * 0.7).sin() * 0.3, angle.sin() * orbit_r);
                    let pos = earth_pos + offset;
                    let to_star = (star_pos - pos).normalize();
                    instances.push(CelestialBodyInstance {
                        position: pos.into(),
                        radius: rad,
                        color,
                        star_direction: [to_star.x, to_star.y, to_star.z, 0.0],
                        atmosphere_color: [0.0, 0.0, 0.0, 0.0],
                    });
                }
                // MI Corvettes (small, fast orbits — varied flight paths)
                let corvette_color = [0.14, 0.16, 0.22, 0.7];
                for &(orbit_r, phase, speed, rad) in &[
                    (2.2, 0.3, 0.28, 0.12),
                    (2.4, 1.8, 0.22, 0.14),
                    (2.7, 3.1, 0.25, 0.11),
                    (2.3, 4.5, 0.30, 0.13),
                    (2.9, 0.8, 0.20, 0.15),
                    (2.5, 2.2, 0.26, 0.12),
                    (2.6, 5.2, 0.24, 0.14),
                    (2.8, 1.0, 0.18, 0.13),
                    (2.4, 3.8, 0.28, 0.11),
                    (2.7, 4.9, 0.22, 0.14),
                ] {
                    let angle = phase + ot_f * speed;
                    let offset = Vec3::new(angle.cos() * orbit_r, (angle * 0.6).sin() * 0.25, angle.sin() * orbit_r);
                    let pos = earth_pos + offset;
                    let to_star = (star_pos - pos).normalize();
                    instances.push(CelestialBodyInstance {
                        position: pos.into(),
                        radius: rad,
                        color: corvette_color,
                        star_direction: [to_star.x, to_star.y, to_star.z, 0.0],
                        atmosphere_color: [0.0, 0.0, 0.0, 0.0],
                    });
                }
                // MI Destroyers (large, slower orbits)
                let destroyer_color = [0.11, 0.13, 0.18, 0.75];
                for &(orbit_r, phase, speed, rad) in &[
                    (3.2, 0.0, 0.10, 0.38),
                    (3.4, 2.1, 0.08, 0.42),
                    (3.0, 4.2, 0.11, 0.35),
                    (3.5, 1.0, 0.09, 0.40),
                    (3.3, 3.5, 0.07, 0.45),
                ] {
                    let angle = phase + ot_f * speed;
                    let offset = Vec3::new(angle.cos() * orbit_r, (angle * 0.5).sin() * 0.35, angle.sin() * orbit_r);
                    let pos = earth_pos + offset;
                    let to_star = (star_pos - pos).normalize();
                    instances.push(CelestialBodyInstance {
                        position: pos.into(),
                        radius: rad,
                        color: destroyer_color,
                        star_direction: [to_star.x, to_star.y, to_star.z, 0.0],
                        atmosphere_color: [0.0, 0.0, 0.0, 0.0],
                    });
                }
                // Dropships (medium, varied inclinations — traffic to/from surface)
                let dropship_color = [0.18, 0.20, 0.26, 0.65];
                for &(orbit_r, phase, speed, rad) in &[
                    (2.35, 0.6, 0.32, 0.18),
                    (2.55, 2.4, 0.26, 0.20),
                    (2.45, 4.0, 0.30, 0.17),
                    (2.65, 1.2, 0.24, 0.19),
                    (2.5, 3.3, 0.28, 0.18),
                    (2.4, 5.5, 0.34, 0.16),
                    (2.7, 0.2, 0.22, 0.21),
                    (2.6, 2.8, 0.26, 0.19),
                ] {
                    let angle = phase + ot_f * speed;
                    let offset = Vec3::new(angle.cos() * orbit_r, (angle * 0.8).sin() * 0.28, angle.sin() * orbit_r);
                    let pos = earth_pos + offset;
                    let to_star = (star_pos - pos).normalize();
                    instances.push(CelestialBodyInstance {
                        position: pos.into(),
                        radius: rad,
                        color: dropship_color,
                        star_direction: [to_star.x, to_star.y, to_star.z, 0.0],
                        atmosphere_color: [0.0, 0.0, 0.0, 0.0],
                    });
                }
            }

            return instances;
        }

        let cam_pos = self.camera.position();

        // On planet surface: camera is in planet-centered world space. Place sun and moons at the
        // far plane so they're not clipped (camera far = 1000) and use correct directions.
        if let Some(planet_idx) = self.current_planet_idx {
            let body = &self.current_system.bodies[planet_idx];
            let planet_pos = body.orbital_position(self.orbital_time);
            let sun_dir = (-planet_pos).normalize();
            let sun_dir_f = Vec3::new(sun_dir.x as f32, sun_dir.y as f32, sun_dir.z as f32);
            const FAR_PLANE: f32 = 999.0; // Just inside camera far=1000 so not clipped
            let star = &self.current_system.star;
            let sun_pos = cam_pos + sun_dir_f * FAR_PLANE;
            let sun_radius = 14.0; // ~0.8° angular radius — clearly visible disc
            instances.push(CelestialBodyInstance {
                position: sun_pos.into(),
                radius: sun_radius,
                color: [star.color.x, star.color.y, star.color.z, 1.0],
                star_direction: [0.0, 0.0, 0.0, 0.0],
                atmosphere_color: [0.0, 0.0, 0.0, 0.0],
            });
            for (m, moon) in body.moons.iter().enumerate() {
                if let Some(moon_pos) = body.moon_world_position(m, self.orbital_time) {
                    let moon_rel = moon_pos - planet_pos;
                    let moon_rel_f = Vec3::new(moon_rel.x as f32, moon_rel.y as f32, moon_rel.z as f32);
                    let to_moon = moon_rel_f - cam_pos;
                    let dist_sq = to_moon.length_squared();
                    if dist_sq < 1e-6 {
                        continue;
                    }
                    let moon_dir = to_moon.normalize();
                    let moon_pos_far = cam_pos + moon_dir * FAR_PLANE;
                    let moon_radius = 4.0; // ~0.23° angular
                    let moon_to_star = (-moon_pos).normalize();
                    let mts = Vec3::new(moon_to_star.x as f32, moon_to_star.y as f32, moon_to_star.z as f32);
                    let moon_cfg = moon.planet.get_biome_config();
                    let moon_color = moon_cfg.base_color;
                    instances.push(CelestialBodyInstance {
                        position: moon_pos_far.into(),
                        radius: moon_radius,
                        color: [moon_color.x, moon_color.y, moon_color.z, 0.3],
                        star_direction: [mts.x, mts.y, mts.z, 0.0],
                        atmosphere_color: [0.0, 0.0, 0.0, 0.0],
                    });
                }
            }
            return instances;
        }

        // Orbit/space view: camera in universe space
        let cam_dvec = DVec3::new(cam_pos.x as f64, cam_pos.y as f64, cam_pos.z as f64);

        // Star
        let star = &self.current_system.star;
        let star_pos = DVec3::ZERO;
        let rel = star_pos - cam_dvec;
        let rel_f = Vec3::new(rel.x as f32, rel.y as f32, rel.z as f32);
        if rel_f.length() < 200000.0 {
            instances.push(CelestialBodyInstance {
                position: rel_f.into(),
                radius: star.radius,
                color: [star.color.x, star.color.y, star.color.z, 1.0],
                star_direction: [0.0, 0.0, 0.0, 0.0],
                atmosphere_color: [0.0, 0.0, 0.0, 0.0],
            });
        }

        // Direction to star (for planet lighting)
        let star_dir_from_cam = (-cam_dvec).normalize();
        let star_dir_f = Vec3::new(star_dir_from_cam.x as f32, star_dir_from_cam.y as f32, star_dir_from_cam.z as f32);

        // Planets
        for (i, body) in self.current_system.bodies.iter().enumerate() {
            // Skip the planet we're currently on
            if self.current_planet_idx == Some(i) {
                continue;
            }

            let body_pos = body.orbital_position(self.orbital_time);
            let rel = body_pos - cam_dvec;
            let rel_f = Vec3::new(rel.x as f32, rel.y as f32, rel.z as f32);
            let dist = rel_f.length();

            if dist < 200000.0 && dist > body.planet.visual_radius() * 0.5 {
                // Direction from this body to the star (for lighting)
                let body_to_star = (-body_pos).normalize();
                let bts = Vec3::new(body_to_star.x as f32, body_to_star.y as f32, body_to_star.z as f32);

                let surf = body.planet.surface_color();
                let atmo_rgb = body.planet.atmosphere_color_rgb();
                let planet_color = [surf[0], surf[1], surf[2], 0.3];
                let atmo_color = [
                    atmo_rgb[0],
                    atmo_rgb[1],
                    atmo_rgb[2],
                    if body.ring_system { 1.0 } else { 0.0 },
                ];

                instances.push(CelestialBodyInstance {
                    position: rel_f.into(),
                    radius: body.planet.visual_radius(),
                    color: planet_color,
                    star_direction: [bts.x, bts.y, bts.z, if body.planet.has_atmosphere { 1.0 } else { 0.0 }],
                    atmosphere_color: atmo_color,
                });

                // Earth orbit from space: bustling spaceport — stations, MI corvettes, destroyers, dropships
                if body.planet.name == "Earth" {
                    let earth_center = rel_f;
                    let ot_f = self.orbital_time as f32;
                    // Stations (slow)
                    for &(orbit_r, phase, speed, rad, r, g, b) in &[
                        (800.0f32, 0.0, 0.015, 12.0, 0.55, 0.58, 0.62),
                        (950.0, 1.2, 0.012, 10.0, 0.5, 0.52, 0.55),
                        (880.0, 2.8, 0.018, 8.0, 0.6, 0.62, 0.65),
                        (1020.0, 4.0, 0.014, 9.0, 0.48, 0.5, 0.52),
                        (850.0, 5.1, 0.016, 6.0, 0.58, 0.6, 0.62),
                    ] {
                        let angle = phase + ot_f * speed;
                        let offset = Vec3::new(
                            angle.cos() * orbit_r,
                            (angle * 0.7).sin() * 80.0,
                            angle.sin() * orbit_r,
                        );
                        let pos = earth_center + offset;
                        let to_star = (-pos).normalize();
                        instances.push(CelestialBodyInstance {
                            position: pos.into(),
                            radius: rad,
                            color: [r, g, b, 0.5],
                            star_direction: [to_star.x, to_star.y, to_star.z, 0.0],
                            atmosphere_color: [0.0, 0.0, 0.0, 0.0],
                        });
                    }
                    // MI Corvettes (small, fast — varied flight paths)
                    for &(orbit_r, phase, speed, rad) in &[
                        (720.0f32, 0.3, 0.035, 5.0),
                        (780.0, 1.8, 0.030, 6.0),
                        (820.0, 3.1, 0.032, 5.5),
                        (750.0, 4.5, 0.038, 5.0),
                        (860.0, 0.8, 0.028, 6.5),
                        (790.0, 2.2, 0.033, 5.0),
                        (830.0, 5.2, 0.031, 5.5),
                        (760.0, 1.0, 0.026, 6.0),
                        (840.0, 3.8, 0.036, 5.0),
                        (770.0, 4.9, 0.029, 6.0),
                    ] {
                        let angle = phase + ot_f * speed;
                        let offset = Vec3::new(
                            angle.cos() * orbit_r,
                            (angle * 0.6).sin() * 60.0,
                            angle.sin() * orbit_r,
                        );
                        let pos = earth_center + offset;
                        let to_star = (-pos).normalize();
                        instances.push(CelestialBodyInstance {
                            position: pos.into(),
                            radius: rad,
                            color: [0.14, 0.16, 0.22, 0.7],
                            star_direction: [to_star.x, to_star.y, to_star.z, 0.0],
                            atmosphere_color: [0.0, 0.0, 0.0, 0.0],
                        });
                    }
                    // MI Destroyers (large, slower)
                    for &(orbit_r, phase, speed, rad) in &[
                        (1050.0f32, 0.0, 0.008, 20.0),
                        (1120.0, 2.1, 0.006, 22.0),
                        (1000.0, 4.2, 0.009, 18.0),
                        (1080.0, 1.0, 0.007, 21.0),
                        (1150.0, 3.5, 0.005, 24.0),
                    ] {
                        let angle = phase + ot_f * speed;
                        let offset = Vec3::new(
                            angle.cos() * orbit_r,
                            (angle * 0.5).sin() * 100.0,
                            angle.sin() * orbit_r,
                        );
                        let pos = earth_center + offset;
                        let to_star = (-pos).normalize();
                        instances.push(CelestialBodyInstance {
                            position: pos.into(),
                            radius: rad,
                            color: [0.11, 0.13, 0.18, 0.75],
                            star_direction: [to_star.x, to_star.y, to_star.z, 0.0],
                            atmosphere_color: [0.0, 0.0, 0.0, 0.0],
                        });
                    }
                    // Dropships (medium — traffic to/from surface)
                    for &(orbit_r, phase, speed, rad) in &[
                        (740.0f32, 0.6, 0.040, 8.0),
                        (810.0, 2.4, 0.034, 9.0),
                        (770.0, 4.0, 0.037, 7.5),
                        (800.0, 1.2, 0.031, 8.5),
                        (785.0, 3.3, 0.035, 8.0),
                        (755.0, 5.5, 0.042, 7.0),
                        (825.0, 0.2, 0.029, 9.5),
                        (795.0, 2.8, 0.033, 8.0),
                    ] {
                        let angle = phase + ot_f * speed;
                        let offset = Vec3::new(
                            angle.cos() * orbit_r,
                            (angle * 0.8).sin() * 70.0,
                            angle.sin() * orbit_r,
                        );
                        let pos = earth_center + offset;
                        let to_star = (-pos).normalize();
                        instances.push(CelestialBodyInstance {
                            position: pos.into(),
                            radius: rad,
                            color: [0.18, 0.20, 0.26, 0.65],
                            star_direction: [to_star.x, to_star.y, to_star.z, 0.0],
                            atmosphere_color: [0.0, 0.0, 0.0, 0.0],
                        });
                    }
                }
            }

            // Moons
            for (m, moon) in body.moons.iter().enumerate() {
                if let Some(moon_pos) = body.moon_world_position(m, self.orbital_time) {
                    let rel = moon_pos - cam_dvec;
                    let rel_f = Vec3::new(rel.x as f32, rel.y as f32, rel.z as f32);
                    let dist = rel_f.length();

                    if dist < 100000.0 && dist > moon.planet.visual_radius() * 0.5 {
                        let moon_to_star = (-moon_pos).normalize();
                        let mts = Vec3::new(moon_to_star.x as f32, moon_to_star.y as f32, moon_to_star.z as f32);

                        let moon_cfg = moon.planet.get_biome_config();
                        let moon_color = moon_cfg.base_color;

                        instances.push(CelestialBodyInstance {
                            position: rel_f.into(),
                            radius: moon.planet.visual_radius(),
                            color: [moon_color.x, moon_color.y, moon_color.z, 0.3],
                            star_direction: [mts.x, mts.y, mts.z, 0.0], // moons rarely have atmosphere
                            atmosphere_color: [0.0, 0.0, 0.0, 0.0],
                        });
                    }
                }
            }
        }

        instances
    }

    /// Real-time sun direction and time-of-day from star position and planet rotation.
    /// When on a planet: star is at system origin; planet position and rotation give unique day/night per system/planet.
    /// When in space: uses target planet if in ship, else procedural fallback from current time_of_day.
    fn compute_sun_direction_and_time_of_day(&self, planet_idx: Option<usize>) -> (Vec3, f32) {
        let tau_f64 = std::f64::consts::TAU;
        let tau_f32 = std::f32::consts::TAU;

        if let Some(idx) = planet_idx {
            if let Some(body) = self.current_system.bodies.get(idx) {
                let planet = &body.planet;
                // Star at origin; direction from planet to star (sun)
                let planet_pos = body.orbital_position(self.orbital_time);
                let to_star = Vec3::new(
                    -planet_pos.x as f32,
                    -planet_pos.y as f32,
                    -planet_pos.z as f32,
                );
                let len = to_star.length();
                if len > 1e-6 {
                    let to_star_n = to_star / len;
                    // Planet rotation: axis = world Y (spin), period = rotation_period_sec
                    let rotation_phase = (self.universe_time_sec / planet.rotation_period_sec as f64) * tau_f64
                        + planet.rotation_phase_0 as f64;
                    let c = rotation_phase.cos() as f32;
                    let s = rotation_phase.sin() as f32;
                    // R_y(phase) * to_star: rotate to_star around Y (sun moves across sky as planet spins)
                    let sun_dir = Vec3::new(
                        to_star_n.x * c + to_star_n.z * s,
                        to_star_n.y,
                        -to_star_n.x * s + to_star_n.z * c,
                    );
                    let time_of_day = Self::time_of_day_from_sun_direction(sun_dir);
                    return (sun_dir, time_of_day);
                }
            }
        }

        // In space or no body: use target planet when in ship for consistency
        if self.phase == GamePhase::InShip {
            let target_idx = self.ship_state.as_ref()
                .map(|s| s.target_planet_idx)
                .unwrap_or(0)
                .min(self.current_system.bodies.len().saturating_sub(1));
            let (sun_dir, tod) = self.compute_sun_direction_and_time_of_day(Some(target_idx));
            return (sun_dir, tod);
        }

        // Fallback: procedural arc from stored time_of_day (main menu, etc.)
        let t = self.time_of_day;
        let azimuth = t * tau_f32;
        let elev_raw = (t * tau_f32 - std::f32::consts::FRAC_PI_2).sin();
        let elevation = elev_raw * std::f32::consts::FRAC_PI_2 * 0.92;
        let sun_dir = Vec3::new(
            azimuth.cos() * elevation.cos(),
            elevation.sin(),
            azimuth.sin() * elevation.cos(),
        );
        (sun_dir, self.time_of_day)
    }

    /// Derive time_of_day (0=dawn, 0.25=noon, 0.5=dusk, 0.75=night) from sun direction in world space.
    fn time_of_day_from_sun_direction(sun_dir: Vec3) -> f32 {
        let elevation = sun_dir.y; // sin(elevation)
        let azimuth = sun_dir.x.atan2(sun_dir.z); // angle in XZ
        if elevation < -0.15 {
            // Below horizon: night (0.75) with smooth transition
            let blend = (elevation + 0.15) / -0.25;
            (0.5 + 0.25 * blend).max(0.65).min(0.85)
        } else {
            // Dawn = 0 when sun east (+X): azimuth π/2 -> (π/2 - π/2)/(2π) = 0
            (azimuth / (2.0 * std::f32::consts::PI) - 0.25).rem_euclid(1.0)
        }
    }

    fn sky_weather_params(&self) -> (Vec3, f32, f32, f32) {
        let (sun_dir, _) = self.compute_sun_direction_and_time_of_day(self.current_planet_idx);
        (sun_dir, self.weather.cloud_density, self.weather.dust, self.weather.fog_density)
    }

    fn update_rain(&mut self, dt: f32) {
        let (spawn_rate, fall_speed) = self.weather.rain_params();

        if spawn_rate > 0 && self.player.is_alive {
            let cam = self.camera.position();
            for _ in 0..(spawn_rate as f32 * dt) as usize {
                let x = cam.x + (rand::random::<f32>() - 0.5) * 40.0;
                let z = cam.z + (rand::random::<f32>() - 0.5) * 40.0;
                let y = cam.y + rand::random::<f32>() * 20.0;
                self.rain_drops.push(RainDrop {
                    position: Vec3::new(x, y, z),
                    velocity: Vec3::new(0.5, -fall_speed, 0.2),
                    life: 2.0,
                });
            }
        }

        for r in &mut self.rain_drops {
            r.position += r.velocity * dt;
            r.life -= dt;
        }
        // Just use lifetime to cull rain (avoids 800 sample_height calls per frame)
        self.rain_drops.retain(|r| r.life > 0.0);
        if self.rain_drops.len() > 400 {
            self.rain_drops.drain(0..(self.rain_drops.len() - 400));
        }
    }

    fn update_snow(&mut self, dt: f32) {
        let (spawn_rate, fall_speed) = self.weather.snow_params();
        if spawn_rate > 0 && self.player.is_alive {
            let cam = self.camera.position();
            for _ in 0..spawn_rate {
                let x = cam.x + (rand::random::<f32>() - 0.5) * 35.0;
                let z = cam.z + (rand::random::<f32>() - 0.5) * 35.0;
                let y = cam.y + rand::random::<f32>() * 18.0;
                let size = 0.04 + rand::random::<f32>() * 0.05;
                self.snow_particles.push(SnowParticle {
                    position: Vec3::new(x, y, z),
                    velocity: Vec3::new(
                        (rand::random::<f32>() - 0.5) * 1.5,
                        -fall_speed,
                        (rand::random::<f32>() - 0.5) * 1.5,
                    ),
                    life: 4.0,
                    size,
                });
            }
        }
        for s in &mut self.snow_particles {
            s.position += s.velocity * dt;
            s.life -= dt;
        }
        self.snow_particles.retain(|s| s.life > 0.0);
        if self.snow_particles.len() > 350 {
            self.snow_particles.drain(0..(self.snow_particles.len() - 350));
        }
    }

    fn execute_ability(&mut self) {
        match self.player.ability {
            fps::ClassAbility::JetpackBurst => {
                let forward = self.camera.forward();
                self.player.velocity = forward * 15.0 + Vec3::Y * 12.0;
                self.player.is_grounded = false;
            }
            fps::ClassAbility::ScanPulse => {
                let bug_count = self.world.query::<&Bug>().iter().count();
                let skinny_count = self.world.query::<&Skinny>().iter().count();
                self.game_messages.info(format!("SCAN: {} bugs, {} Skinnies detected in area!", bug_count, skinny_count));
            }
            fps::ClassAbility::DeployBarricade => {
                self.game_messages.info("Barricade deployed!");
            }
            fps::ClassAbility::AmmoStation => {
                for weapon in &mut self.player.weapons {
                    weapon.reserve_ammo += weapon.magazine_size * 2;
                }
                self.game_messages.info("Ammo resupplied!");
            }
            fps::ClassAbility::ShieldDome => {
                self.player.add_armor(25.0);
                self.game_messages.info("Shield dome activated! +25 armor");
            }
        }
    }

    fn cleanup_dead_bugs(&mut self) {
        // Helldivers 2 / Starship Troopers Extermination: corpses stay until player destroys them
        const MAX_CORPSES: usize = 800;
        const SETTLE_WINDOW: f32 = 2.0; // seconds of gravity settling after spawn

        // Convert fully dead bugs into corpse entities (lightweight, no physics)
        let mut to_convert: Vec<(hecs::Entity, Vec3, Quat, Vec3, [f32; 4], u8)> = Vec::new();
        for (entity, (transform, health, physics_bug, bug)) in
            self.world.query::<(&Transform, &Health, &PhysicsBug, &Bug)>().iter()
        {
            if health.is_dead() && physics_bug.death_phase == DeathPhase::Dead
                && physics_bug.ragdoll_time > 5.0
            {
                let type_idx = match bug.bug_type {
                    BugType::Warrior => 0,
                    BugType::Charger => 1,
                    BugType::Spitter => 2,
                    BugType::Tanker => 3,
                    BugType::Hopper => 4,
                };
                let mut color = bug.bug_type.color();
                if let Some(v) = bug.variant {
                    let t = v.color_tint();
                    color[0] *= t[0];
                    color[1] *= t[1];
                    color[2] *= t[2];
                }
                let (death_offset, death_rotation, _) = physics_bug.get_death_animation();
                let final_pos = transform.position + death_offset;
                let final_rot = transform.rotation * death_rotation;
                to_convert.push((entity, final_pos, final_rot, transform.scale, color, type_idx));
            }
        }

        // Dead Skinnies: despawn after ragdoll settled (no corpse entity for now)
        let mut skinny_to_despawn: Vec<hecs::Entity> = Vec::new();
        for (entity, (_, health, physics_bug, _)) in
            self.world.query::<(&Transform, &Health, &PhysicsBug, &Skinny)>().iter()
        {
            if health.is_dead() && physics_bug.death_phase == DeathPhase::Dead
                && physics_bug.ragdoll_time > 5.0
            {
                skinny_to_despawn.push(entity);
            }
        }
        for e in skinny_to_despawn {
            let _ = self.world.despawn(e);
        }

        // ── Collect all existing corpse data once (used for stacking & settling) ──
        // Build a spatial grid for O(1) average-case neighbor lookups.
        let mut corpse_positions: Vec<(hecs::Entity, Vec3, Vec3, bool)> = Vec::new();
        for (entity, (transform, corpse)) in self.world.query::<(&Transform, &BugCorpse)>().iter() {
            corpse_positions.push((entity, transform.position, transform.scale, corpse.settled));
        }

        // Spatial hash grid (cell size ~3m covers typical stack radius)
        const CELL: f32 = 3.0;
        let inv_cell = 1.0 / CELL;
        let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (i, &(_, pos, _, _)) in corpse_positions.iter().enumerate() {
            let cx = (pos.x * inv_cell).floor() as i32;
            let cz = (pos.z * inv_cell).floor() as i32;
            grid.entry((cx, cz)).or_default().push(i);
        }

        // Helper closure: find pile height at (x, z) using spatial grid
        let find_pile_height = |x: f32, z: f32, scale: &Vec3, grid: &HashMap<(i32, i32), Vec<usize>>, positions: &[(hecs::Entity, Vec3, Vec3, bool)], exclude: Option<hecs::Entity>| -> f32 {
            let cx = (x * inv_cell).floor() as i32;
            let cz = (z * inv_cell).floor() as i32;
            let stack_radius_sq = (scale.x.max(scale.z) * 1.2) * (scale.x.max(scale.z) * 1.2);
            let mut max_top = f32::NEG_INFINITY;
            for dcx in -1..=1 {
                for dcz in -1..=1 {
                    if let Some(indices) = grid.get(&(cx + dcx, cz + dcz)) {
                        for &idx in indices {
                            let (ent, opos, oscale, _) = positions[idx];
                            if Some(ent) == exclude { continue; }
                            let dx = x - opos.x;
                            let dz = z - opos.z;
                            if dx * dx + dz * dz < stack_radius_sq {
                                let other_top = opos.y + oscale.y * 0.4;
                                if other_top > max_top {
                                    max_top = other_top;
                                }
                            }
                        }
                    }
                }
            }
            max_top
        };

        // ── Spawn new corpses ──
        for (entity, pos, rot, scale, color, type_idx) in to_convert {
            self.world.despawn(entity).ok();

            let surface_y = self.chunk_manager.walkable_height(pos.x, pos.z);
            let corpse_half_height = scale.y * 0.3;
            let mut pile_height = surface_y + corpse_half_height;

            let stack_top = find_pile_height(pos.x, pos.z, &scale, &grid, &corpse_positions, None);
            if stack_top > pile_height {
                pile_height = stack_top;
            }

            let flat_rot = Quat::from_euler(
                glam::EulerRot::XYZ,
                (rand::random::<f32>() - 0.5) * 0.4,
                rot.to_euler(glam::EulerRot::XYZ).1,
                (rand::random::<f32>() - 0.5) * 0.3 + std::f32::consts::FRAC_PI_2 * 0.3,
            );

            let new_entity = self.world.spawn((
                Transform {
                    position: Vec3::new(pos.x, pile_height, pos.z),
                    rotation: flat_rot,
                    scale,
                },
                BugCorpse::new(color, type_idx, scale),
            ));

            // Add to spatial grid so subsequent spawns can stack on this one
            let new_pos = Vec3::new(pos.x, pile_height, pos.z);
            let idx = corpse_positions.len();
            corpse_positions.push((new_entity, new_pos, scale, false));
            let cx = (pos.x * inv_cell).floor() as i32;
            let cz = (pos.z * inv_cell).floor() as i32;
            grid.entry((cx, cz)).or_default().push(idx);
        }

        // ── Cap corpse count: remove oldest first ──
        let total_corpses = corpse_positions.len();
        if total_corpses > MAX_CORPSES {
            // Collect (entity, decay_timer) so we can remove the most-decayed first
            let mut by_decay: Vec<(hecs::Entity, f32)> = Vec::new();
            for (entity, (_, corpse)) in self.world.query::<(&Transform, &BugCorpse)>().iter() {
                by_decay.push((entity, corpse.decay_timer));
            }
            by_decay.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let excess = total_corpses - MAX_CORPSES;
            for i in 0..excess.min(by_decay.len()) {
                self.world.despawn(by_decay[i].0).ok();
            }
        }

        // ── Single pass: decay timer + gravity settle (only unsettled corpses) ──
        let dt = self.time.delta_seconds();
        let mut decayed: Vec<hecs::Entity> = Vec::new();
        // Rebuild positions after cap removal for settling
        let mut settle_data: Vec<(hecs::Entity, Vec3, Vec3)> = Vec::new();
        for (entity, (transform, corpse)) in self.world.query::<(&Transform, &BugCorpse)>().iter() {
            if !corpse.settled {
                settle_data.push((entity, transform.position, transform.scale));
            }
        }

        // Spatial grid for settling (only unsettled corpses need neighbor checks)
        let mut settle_grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        // Include ALL corpses for neighbor height data
        let mut all_corpse_data: Vec<(hecs::Entity, Vec3, Vec3)> = Vec::new();
        for (entity, (transform, _)) in self.world.query::<(&Transform, &BugCorpse)>().iter() {
            let idx = all_corpse_data.len();
            all_corpse_data.push((entity, transform.position, transform.scale));
            let cx = (transform.position.x * inv_cell).floor() as i32;
            let cz = (transform.position.z * inv_cell).floor() as i32;
            settle_grid.entry((cx, cz)).or_default().push(idx);
        }

        // Settle unsettled corpses (use walkable height so corpses float in water)
        for &(entity, pos, scale) in &settle_data {
            let surface_y = self.chunk_manager.walkable_height(pos.x, pos.z);
            let corpse_half_height = scale.y * 0.3;
            let mut target_y = surface_y + corpse_half_height;

            // Check neighbors via spatial grid
            let cx = (pos.x * inv_cell).floor() as i32;
            let cz = (pos.z * inv_cell).floor() as i32;
            let stack_radius_sq = (scale.x.max(scale.z) * 1.0).powi(2);
            for dcx in -1..=1 {
                for dcz in -1..=1 {
                    if let Some(indices) = settle_grid.get(&(cx + dcx, cz + dcz)) {
                        for &idx in indices {
                            let (other_entity, other_pos, other_scale) = all_corpse_data[idx];
                            if other_entity == entity { continue; }
                            let dx = pos.x - other_pos.x;
                            let dz = pos.z - other_pos.z;
                            if dx * dx + dz * dz < stack_radius_sq && other_pos.y < pos.y {
                                let other_top = other_pos.y + other_scale.y * 0.4;
                                if other_top > target_y {
                                    target_y = other_top;
                                }
                            }
                        }
                    }
                }
            }

            if let Ok(mut transform) = self.world.get::<&mut Transform>(entity) {
                if transform.position.y > target_y + 0.05 {
                    transform.position.y -= 12.0 * dt;
                    if transform.position.y < target_y {
                        transform.position.y = target_y;
                    }
                } else if transform.position.y < target_y - 0.05 {
                    transform.position.y = target_y;
                }
            }
        }

        // Single query_mut pass: update decay + settle timers
        for (entity, corpse) in self.world.query_mut::<&mut BugCorpse>() {
            corpse.decay_timer += dt;
            corpse.settle_timer += dt;
            if corpse.settle_timer >= SETTLE_WINDOW {
                corpse.settled = true;
            }
            let (_, _, _, is_done) = corpse.decay_state();
            if is_done {
                decayed.push(entity);
            }
        }
        for entity in decayed {
            self.world.despawn(entity).ok();
        }
    }

    fn render(&mut self) -> Result<()> {
        render::run(self)
    }
}

/// Application handler for winit.
struct App {
    state: Option<GameState>,
}

impl App {
    fn new() -> Self {
        Self { state: None }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            let config = config::GameConfig::load();
            let window_attrs = Window::default_attributes()
                .with_title("OpenSST")
                .with_inner_size(winit::dpi::LogicalSize::new(config.window_width, config.window_height));

            let window = match event_loop.create_window(window_attrs) {
                Ok(w) => Arc::new(w),
                Err(e) => {
                    log::error!("Failed to create window: {}", e);
                    event_loop.exit();
                    return;
                }
            };

            let state = pollster::block_on(GameState::new(window.clone()));
            match state {
                Ok(s) => {
                    self.state = Some(s);
                    window.request_redraw();
                }
                Err(e) => {
                    log::error!("Failed to initialize game: {}", e);
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let Some(state) = &mut self.state {
            if state.handle_window_event(event) || !state.running {
                event_loop.exit();
            }
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let Some(state) = &mut self.state {
            state.handle_device_event(event);
        }
    }
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║                            OpenSST                               ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  CONTROLS:                                                       ║");
    println!("║    WASD       - Move           │  Mouse      - Look around       ║");
    println!("║    Left Click - Fire weapon    │  Right Click - Aim down sights  ║");
    println!("║    Shift      - Sprint         │  Ctrl       - Crouch            ║");
    println!("║    Space      - Jump           │  R          - Reload            ║");
    println!("║    1/2/Scroll - Switch weapons │  Q          - Use ability       ║");
    println!("║    Tab        - Toggle HUD     │  Escape     - Release cursor    ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  FEATURES:                                                       ║");
    println!("║    - Euphoria-style physics-driven ragdoll death animations      ║");
    println!("║    - Procedurally generated arachnid bugs with bone hierarchies  ║");
    println!("║    - Dynamic gore and ichor effects                              ║");
    println!("║    - Infinite horde survival with escalating threat levels        ║");
    println!("║    - MI extraction dropship with LZ defense                     ║");
    println!("║    - 5 player classes with unique abilities                      ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  DEBUG:                                                          ║");
    println!("║    F1 - Spawn bugs │ F2 - Heal │ F3 - Ammo │ F4 - Kill all bugs  ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");

    log::info!("Starting OpenSST - Euphoria Physics Integration");

    let event_loop = EventLoop::new()?;
    // Poll continuously for lower input latency. Wait blocks until events arrive, which can delay
    // RedrawRequested and cause the "high FPS but laggy" feel. Poll ensures we process input
    // and redraw as fast as possible.
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}
