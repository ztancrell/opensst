//! Destruction system for destructible terrain and objects.

use engine_core::{Lifetime, Transform, Velocity, Vec3};
use hecs::World;
use physics::PhysicsWorld;
use rand::prelude::*;

/// A destructible object component.
#[derive(Debug, Clone)]
pub struct Destructible {
    /// Current health.
    pub health: f32,
    /// Maximum health.
    pub max_health: f32,
    /// Number of debris pieces to spawn on destruction.
    pub debris_count: u32,
    /// Size of debris pieces.
    pub debris_size: f32,
}

impl Destructible {
    pub fn new(health: f32, debris_count: u32, debris_size: f32) -> Self {
        Self {
            health,
            max_health: health,
            debris_count,
            debris_size,
        }
    }

    /// Apply damage and return true if destroyed.
    pub fn damage(&mut self, amount: f32) -> bool {
        self.health = (self.health - amount).max(0.0);
        self.health <= 0.0
    }

    /// Get health percentage.
    pub fn health_percent(&self) -> f32 {
        self.health / self.max_health
    }
}

/// Marker for destructible rock props on terrain.
#[derive(Debug, Clone, Copy)]
pub struct Rock;

/// Bug hole structure: spawns bugs periodically, can be destroyed.
#[derive(Debug, Clone)]
pub struct BugHole {
    /// Time since last bug spawn from this hole.
    pub spawn_timer: f32,
    /// How often this hole spawns a bug (seconds).
    pub spawn_interval: f32,
    /// Maximum bugs this hole can have alive at once.
    pub max_active_bugs: u32,
    /// Current count of alive bugs spawned by this hole.
    pub active_bugs: u32,
}

impl BugHole {
    pub fn new(spawn_interval: f32, max_active: u32) -> Self {
        Self {
            spawn_timer: 0.0,
            spawn_interval,
            max_active_bugs: max_active,
            active_bugs: 0,
        }
    }
}

/// Organic hive structure (decorative + destructible).
#[derive(Debug, Clone, Copy)]
pub struct HiveStructure;

/// Cluster of bug eggs (destructible, can hatch bugs when destroyed).
#[derive(Debug, Clone, Copy)]
pub struct EggCluster;

/// Generic biome-specific environment decoration.
#[derive(Debug, Clone, Copy)]
pub struct EnvironmentProp;

/// Crashed Federation dropship / vehicle wreckage.
#[derive(Debug, Clone, Copy)]
pub struct CrashedShip;

/// Acid / lava pool (hazard POI).
#[derive(Debug, Clone, Copy)]
pub struct HazardPool;

/// Bone pile / skeleton heap.
#[derive(Debug, Clone, Copy)]
pub struct BonePile;

/// Spore tower (tall organic growth on HiveWorlds).
#[derive(Debug, Clone, Copy)]
pub struct SporeTower;

/// Abandoned outpost / fortification ruin.
#[derive(Debug, Clone, Copy)]
pub struct AbandonedOutpost;

/// Plasma burn crater (Wasteland / combat aftermath).
#[derive(Debug, Clone, Copy)]
pub struct BurnCrater;

/// Dead bug corpse that decays over time (not an ECS bug entity).
#[derive(Debug, Clone)]
pub struct BugCorpse {
    /// Time since death.
    pub decay_timer: f32,
    /// Total time before full decay.
    pub decay_duration: f32,
    /// Original color.
    pub base_color: [f32; 4],
    /// Bug type index (for mesh selection).
    pub bug_type_idx: u8,
    /// Original scale.
    pub original_scale: Vec3,
    /// Time since spawned (used for gravity settling window).
    pub settle_timer: f32,
    /// Once settled, skip per-frame gravity work.
    pub settled: bool,
}

impl BugCorpse {
    pub fn new(base_color: [f32; 4], bug_type_idx: u8, scale: Vec3) -> Self {
        Self {
            decay_timer: 0.0,
            // Helldivers 2 / Starship Troopers Extermination: corpses persist indefinitely
            decay_duration: f32::MAX,
            base_color,
            bug_type_idx,
            original_scale: scale,
            settle_timer: 0.0,
            settled: false,
        }
    }

    /// Returns (color, scale_mult, sink_amount, is_fully_decayed).
    pub fn decay_state(&self) -> ([f32; 4], f32, f32, bool) {
        let t = (self.decay_timer / self.decay_duration).clamp(0.0, 1.0);

        // Phase 1 (0-30%): Fresh corpse, slight darkening
        // Phase 2 (30-70%): Darkening, shrinking starts
        // Phase 3 (70-100%): Sinking into ground, very dark, shrinking
        let darken = 1.0 - t * 0.8; // Goes from 1.0 to 0.2
        let color = [
            self.base_color[0] * darken * 0.4, // Already darkened from death
            self.base_color[1] * darken * 0.35,
            self.base_color[2] * darken * 0.3,
            self.base_color[3],
        ];

        let shrink = if t < 0.3 { 1.0 }
            else if t < 0.7 { 1.0 - (t - 0.3) * 0.5 } // shrink to 0.8
            else { 0.8 - (t - 0.7) * 1.5 }; // shrink faster to ~0.35
        let scale_mult = shrink.max(0.3);

        // Sinking starts at 50%
        let sink = if t < 0.5 { 0.0 }
            else { (t - 0.5) * 2.0 * 1.5 }; // up to 1.5 units of sinking

        (color, scale_mult, sink, t >= 1.0)
    }
}

// ── Biome landmarks and hazards (diversity overhaul) ───────────────────────

/// Landmark type: 3 per biome, 36 total. Used for spawning and mesh/color selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LandmarkType {
    // Desert
    RockArch,
    SandDuneRidge,
    OasisPool,
    // Badlands
    MesaPillar,
    CanyonWall,
    DriedRavine,
    // HiveWorld
    ResinNode,
    PulsingEggWall,
    OrganicTunnel,
    // Volcanic
    LavaRiver,
    ObsidianSpire,
    Geyser,
    // Frozen
    IcePillar,
    FrozenLake,
    GlacialRidge,
    // Toxic
    MutantGrowth,
    GasVent,
    AcidGeyser,
    // Mountain
    BoulderField,
    CliffSpire,
    WaterfallCliff,
    // Swamp
    DeadTree,
    FogBank,
    MuddyPool,
    // Crystalline
    CrystalPillar,
    PrismaticPool,
    MirrorShard,
    // Ashlands
    EmberMound,
    CollapsedRuin,
    AshDrift,
    // Jungle
    GiantAlienTree,
    VineWall,
    BioluminescentFlower,
    // Wasteland
    RustedVehicle,
    RadiationCrater,
    TwistedRebar,
    // UCF (Starship Troopers) — colonies and bases on Federation worlds
    UCFColony,
    UCFBase,
    /// Perimeter wall segment for base defense (Hold the Line / Defense on UCF planets).
    UCFBaseWall,
    // Caves and abandoned UCF structures (procgen surface variety)
    /// Cave entrance / tunnel mouth in rocky terrain.
    CaveEntrance,
    /// Abandoned UCF research station — derelict science outpost.
    AbandonedUCFResearchStation,
    /// Abandoned UCF base — ruined military installation.
    AbandonedUCFBase,
}

/// Environmental hazard type: 1–2 per biome. Drives update and damage behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HazardType {
    Sandstorm,       // Desert: visibility + slow
    Rockslide,       // Badlands: periodic boulder rain
    SporeBurst,      // HiveWorld: proximity gas cloud
    GeyserEruption,  // Volcanic: timed AoE
    LavaFlow,        // Volcanic: persistent damage
    Blizzard,        // Frozen: periodic slow + visibility
    IceCrack,        // Frozen: fall damage zones
    PoisonGas,       // Toxic: periodic damage zones
    Avalanche,       // Mountain: rolling boulders
    Quicksand,       // Swamp: slow zones
    Leeches,         // Swamp: swarm damage
    CrystalResonance, // Crystalline: chain trigger
    EmberStorm,      // Ashlands: periodic fire damage
    CarnivorousPlant, // Jungle: proximity snap damage
    RadiationZone,   // Wasteland: DoT areas
}

/// Marker for a biome-unique landmark (decorative or destructible).
#[derive(Debug, Clone, Copy)]
pub struct BiomeLandmark {
    pub landmark_type: LandmarkType,
}

/// Environmental hazard: timed, proximity, or persistent. Updated each frame.
#[derive(Debug, Clone)]
pub struct EnvironmentalHazard {
    pub hazard_type: HazardType,
    pub radius: f32,
    pub damage: f32,
    pub timer: f32,
    pub interval: f32,
    pub active: bool,
}

/// Marker for a destructible that participates in chain reactions.
#[derive(Debug, Clone, Copy)]
pub struct BiomeDestructible {
    pub landmark_type: LandmarkType,
}

/// Effect type when a chain reaction triggers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainEffect {
    Explosion,
    CrystalShatter,
    FireSpread,
    AcidSplash,
    Collapse,
    BoulderRoll,
}

/// When this destructible is destroyed, it triggers an effect in radius.
#[derive(Debug, Clone)]
pub struct ChainReaction {
    pub radius: f32,
    pub damage: f32,
    pub effect: ChainEffect,
}

/// Pre-computed render data for static environment entities.
/// Attached once at spawn time; avoids recomputing transform matrices and
/// per-entity color every frame in the render loop.
#[derive(Debug, Clone, Copy)]
pub struct CachedRenderData {
    /// Pre-computed model matrix (column-major 4×4).
    pub matrix: [[f32; 4]; 4],
    /// RGBA color.
    pub color: [f32; 4],
    /// Mesh group index (determines which shared mesh to draw with).
    /// 0=rock, 1=bug_hole, 2=hive_mound, 3=egg_cluster, 4=prop_sphere, 5=cube, 6=landmark, 7=hazard, 8=beveled_cube
    pub mesh_group: u8,
}

/// Number of distinct mesh groups for static environment entities.
pub const ENV_MESH_GROUP_COUNT: usize = 9;
pub const MESH_GROUP_ROCK: u8 = 0;
pub const MESH_GROUP_BUG_HOLE: u8 = 1;
pub const MESH_GROUP_HIVE_MOUND: u8 = 2;
pub const MESH_GROUP_EGG_CLUSTER: u8 = 3;
pub const MESH_GROUP_PROP_SPHERE: u8 = 4;
pub const MESH_GROUP_CUBE: u8 = 5;
pub const MESH_GROUP_LANDMARK: u8 = 6;
pub const MESH_GROUP_HAZARD: u8 = 7;
/// Beveled cube: UCF buildings (industrial, chamfered edges).
pub const MESH_GROUP_BEVELED_CUBE: u8 = 8;

/// Debris particle component.
#[derive(Debug, Clone, Copy)]
pub struct Debris {
    pub angular_velocity: Vec3,
}

/// Flying bug guts / dismembered chunks — Euphoria-style gore explosion.
#[derive(Debug, Clone, Copy)]
pub struct BugGoreChunk {
    pub color: [f32; 4],
    pub angular_velocity: Vec3,
}

/// Manages destruction effects and debris.
pub struct DestructionSystem {
    /// Maximum debris particles in the world.
    max_debris: usize,
    /// Debris lifetime in seconds.
    debris_lifetime: f32,
    /// Random number generator.
    rng: StdRng,
}

impl Default for DestructionSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl DestructionSystem {
    pub fn new() -> Self {
        Self {
            max_debris: 500,
            debris_lifetime: 5.0,
            rng: StdRng::from_entropy(),
        }
    }

    /// Spawn flying bug guts / dismembered chunks — Euphoria-style explosion of gore.
    pub fn spawn_bug_gore_debris(
        &mut self,
        world: &mut World,
        position: Vec3,
        impact_dir: glam::Vec3,
        bug_color: [f32; 4],
        bug_scale: f32,
        _physics: &mut PhysicsWorld,
    ) {
        let current = world.query::<&BugGoreChunk>().iter().count();
        let available = 200usize.saturating_sub(current);
        let count = (12 + (bug_scale * 8.0) as usize).min(available);

        for _ in 0..count {
            let offset = Vec3::new(
                self.rng.gen_range(-0.5..0.5) * bug_scale,
                self.rng.gen_range(0.0..0.5) * bug_scale,
                self.rng.gen_range(-0.5..0.5) * bug_scale,
            );
            let dir = if impact_dir.length_squared() > 0.01 {
                let spread = Vec3::new(
                    self.rng.gen_range(-0.5..0.5),
                    self.rng.gen_range(0.2..1.0),
                    self.rng.gen_range(-0.5..0.5),
                );
                (impact_dir + spread).normalize_or_zero()
            } else {
                glam::Vec3::new(
                    self.rng.gen_range(-1.0..1.0),
                    self.rng.gen_range(0.3..1.0),
                    self.rng.gen_range(-1.0..1.0),
                ).normalize_or_zero()
            };
            let speed = self.rng.gen_range(6.0..18.0) * (0.8 + bug_scale * 0.4);
            let velocity = dir * speed;
            let angular = glam::Vec3::new(
                self.rng.gen_range(-15.0..15.0),
                self.rng.gen_range(-15.0..15.0),
                self.rng.gen_range(-15.0..15.0),
            );
            let size = bug_scale * self.rng.gen_range(0.08..0.25);
            // Bug guts: mix exoskeleton color with green ichor (Starship Troopers style)
            let ichor = [0.25, 0.55, 0.18, 0.95];
            let mut color = [
                (bug_color[0] * 0.4 + ichor[0] * 0.6).min(1.0),
                (bug_color[1] * 0.3 + ichor[1] * 0.7).min(1.0),
                (bug_color[2] * 0.3 + ichor[2] * 0.7).min(1.0),
                0.95,
            ];

            world.spawn((
                Transform {
                    position: position + offset,
                    scale: glam::Vec3::splat(size),
                    ..Default::default()
                },
                Velocity::with_angular(velocity, angular),
                BugGoreChunk { color, angular_velocity: angular },
                Lifetime::new(4.0),
            ));
        }
    }

    /// Spawn debris particles when something is destroyed.
    pub fn spawn_debris(
        &mut self,
        world: &mut World,
        position: Vec3,
        count: u32,
        size: f32,
        _physics: &mut PhysicsWorld,
    ) {
        // Count current debris
        let current_debris = world.query::<&Debris>().iter().count();
        let available = self.max_debris.saturating_sub(current_debris);
        let spawn_count = (count as usize).min(available);

        for _ in 0..spawn_count {
            // Random offset
            let offset = Vec3::new(
                self.rng.gen_range(-1.0..1.0),
                self.rng.gen_range(0.0..1.0),
                self.rng.gen_range(-1.0..1.0),
            );

            // Random velocity (explosion outward)
            let velocity = Vec3::new(
                self.rng.gen_range(-10.0..10.0),
                self.rng.gen_range(5.0..15.0),
                self.rng.gen_range(-10.0..10.0),
            );

            // Random angular velocity
            let angular = Vec3::new(
                self.rng.gen_range(-5.0..5.0),
                self.rng.gen_range(-5.0..5.0),
                self.rng.gen_range(-5.0..5.0),
            );

            // Random size variation
            let actual_size = size * self.rng.gen_range(0.5..1.5);

            world.spawn((
                Transform {
                    position: position + offset,
                    scale: Vec3::splat(actual_size),
                    ..Default::default()
                },
                Velocity::with_angular(velocity, angular),
                Debris { angular_velocity: angular },
                Lifetime::new(self.debris_lifetime),
            ));
        }
    }

    /// Update debris physics (simple simulation without full physics).
    /// `surface_fn` returns (ground_y, water_level): ground for collision, Some(water_y) when in water for buoyancy.
    pub fn update_debris<S>(&self, world: &mut World, dt: f32, surface_fn: S)
    where
        S: Fn(f32, f32) -> (f32, Option<f32>),
    {
        let gravity = Vec3::new(0.0, -20.0, 0.0);

        for (_, (transform, velocity, debris)) in
            world.query_mut::<(&mut Transform, &mut Velocity, &Debris)>()
        {
            let (ground_y, water_level) = surface_fn(transform.position.x, transform.position.z);
            if let Some(wl) = water_level {
                let depth = wl - transform.position.y;
                if depth > 0.0 {
                    velocity.linear.y += 10.0 * dt; // float up
                }
                velocity.linear *= 1.0 - 3.0 * dt; // water drag
            } else {
                velocity.linear += gravity * dt;
            }

            // Update position
            transform.position += velocity.linear * dt;

            // Update rotation
            let rotation_delta = glam::Quat::from_scaled_axis(debris.angular_velocity * dt);
            transform.rotation = rotation_delta * transform.rotation;

            // Ground/water surface collision
            let surface = ground_y.max(transform.scale.x);
            if transform.position.y < surface {
                transform.position.y = surface;
                velocity.linear.y = -velocity.linear.y * 0.3; // Bounce
                velocity.linear.x *= 0.8; // Friction
                velocity.linear.z *= 0.8;
            }

            // Damping
            velocity.linear *= 0.99;
        }
    }

    /// Update bug gore chunk physics (same as debris — flying, bouncing).
    pub fn update_bug_gore<S>(&self, world: &mut World, dt: f32, surface_fn: S)
    where
        S: Fn(f32, f32) -> (f32, Option<f32>),
    {
        let gravity = Vec3::new(0.0, -20.0, 0.0);

        for (_, (transform, velocity, chunk)) in
            world.query_mut::<(&mut Transform, &mut Velocity, &BugGoreChunk)>()
        {
            let (ground_y, water_level) = surface_fn(transform.position.x, transform.position.z);
            if let Some(wl) = water_level {
                let depth = wl - transform.position.y;
                if depth > 0.0 {
                    velocity.linear.y += 10.0 * dt;
                }
                velocity.linear *= 1.0 - 3.0 * dt;
            } else {
                velocity.linear += gravity * dt;
            }

            transform.position += velocity.linear * dt;

            let rotation_delta = glam::Quat::from_scaled_axis(chunk.angular_velocity * dt);
            transform.rotation = rotation_delta * transform.rotation;

            let surface = ground_y.max(transform.scale.x);
            if transform.position.y < surface {
                transform.position.y = surface;
                velocity.linear.y = -velocity.linear.y * 0.3;
                velocity.linear.x *= 0.8;
                velocity.linear.z *= 0.8;
            }

            velocity.linear *= 0.99;
        }
    }

    /// Apply explosion damage to destructibles.
    pub fn apply_explosion(
        &mut self,
        world: &mut World,
        physics: &mut PhysicsWorld,
        center: Vec3,
        radius: f32,
        damage: f32,
    ) {
        // Collect destructibles in range
        let in_range: Vec<(hecs::Entity, Vec3, u32, f32)> = world
            .query::<(&Transform, &Destructible)>()
            .iter()
            .filter_map(|(entity, (transform, destructible))| {
                let dist = transform.position.distance(center);
                if dist <= radius {
                    Some((entity, transform.position, destructible.debris_count, destructible.debris_size))
                } else {
                    None
                }
            })
            .collect();

        // Apply damage and collect destroyed entities
        let mut to_spawn_debris: Vec<(Vec3, u32, f32)> = Vec::new();
        
        for (entity, pos, debris_count, debris_size) in in_range {
            let dist = pos.distance(center);
            let falloff = 1.0 - (dist / radius);
            let actual_damage = damage * falloff;

            if let Ok(mut destructible) = world.get::<&mut Destructible>(entity) {
                if destructible.damage(actual_damage) {
                    // Mark for debris spawn after borrow ends
                    to_spawn_debris.push((pos, debris_count, debris_size));
                }
            }
        }
        
        // Spawn debris for destroyed objects (after releasing borrows)
        for (pos, debris_count, debris_size) in to_spawn_debris {
            self.spawn_debris(world, pos, debris_count, debris_size, physics);
        }

        // Remove destroyed entities
        let destroyed: Vec<hecs::Entity> = world
            .query::<&Destructible>()
            .iter()
            .filter(|(_, d)| d.health <= 0.0)
            .map(|(e, _)| e)
            .collect();

        for entity in destroyed {
            world.despawn(entity).ok();
        }
    }

    /// Create a chunk of terrain that can be destroyed.
    pub fn create_destructible_terrain(
        world: &mut World,
        position: Vec3,
        size: Vec3,
        health: f32,
    ) -> hecs::Entity {
        world.spawn((
            Transform {
                position,
                scale: size,
                ..Default::default()
            },
            Destructible::new(health, (size.x * size.z) as u32, 0.3),
        ))
    }
}
