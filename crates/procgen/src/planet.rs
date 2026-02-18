//! Planet and mission generation system.
//!
//! **Seed-based replayability:** `Planet::generate(seed)` is fully deterministic: the same
//! seed always produces the same planet (name, biomes, size, stats). Terrain and biome
//! layout are derived from this seed so the same planet seed yields the same world
//! everywhere (Minecraft-style).

use crate::biome::{BiomeConfig, BiomeType, PlanetBiomes};
use glam::Vec3;
use rand::prelude::*;

/// Planet classification (Heinlein / Helldivers 2 style) — affects naming and flavor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanetClassification {
    /// Arachnid-held world (Klendathu-style).
    HiveWorld,
    /// Federation colony or terraformed world.
    Colony,
    /// Military outpost or forward base.
    Outpost,
    /// Frontier / uncharted.
    Frontier,
    /// Mining or industrial.
    Industrial,
    /// Research or science station.
    Research,
    /// War zone / contested.
    WarZone,
    /// Abandoned or lost colony.
    Abandoned,
}

/// A procedurally generated planet.
#[derive(Debug, Clone)]
pub struct Planet {
    /// Unique seed for this planet.
    pub seed: u64,
    /// Planet name.
    pub name: String,
    /// Classification for flavor (colony, hive, outpost, etc.).
    pub classification: PlanetClassification,
    /// Primary biome.
    pub primary_biome: BiomeType,
    /// Secondary biome (for variety).
    pub secondary_biome: Option<BiomeType>,
    /// Danger level 1-10.
    pub danger_level: u32,
    /// Bug infestation level (affects spawn rates).
    pub infestation: f32,
    /// Size category.
    pub size: PlanetSize,
    /// Position in galaxy (for map display).
    pub galaxy_position: Vec3,
    /// Whether this planet has been liberated.
    pub liberated: bool,
    /// Whether this planet has an atmosphere.
    pub has_atmosphere: bool,
    /// Atmosphere tint color.
    pub atmosphere_color: Vec3,
    /// Visual radius when seen from space (game units).
    pub visual_radius_value: f32,
    /// Whether Skinnies (Heinlein alien faction) are present — adds humanoid enemies on some worlds.
    pub has_skinnies: bool,
    /// If true, war table shows "???" for biomes/danger until drop — troopers have no intel.
    pub has_unknown_intel: bool,
    /// Gravity multiplier (0.85–1.2). Affects jump/fall feel.
    pub gravity_mult: f32,
    /// Day length multiplier (0.5–2.0). Affects time-of-day cycle.
    pub day_length_mult: f32,
}

/// Planet size categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanetSize {
    Small,
    Medium,
    Large,
    Massive,
}

impl PlanetSize {
    /// Get terrain size in world units.
    pub fn terrain_size(&self) -> f32 {
        match self {
            PlanetSize::Small => 256.0,
            PlanetSize::Medium => 512.0,
            PlanetSize::Large => 1024.0,
            PlanetSize::Massive => 2048.0,
        }
    }

    /// Get recommended chunk count.
    pub fn chunk_count(&self) -> u32 {
        match self {
            PlanetSize::Small => 4,
            PlanetSize::Medium => 9,
            PlanetSize::Large => 16,
            PlanetSize::Massive => 25,
        }
    }
}

impl Planet {
    /// Earth — homeworld, UCF safe zone; all biomes, no bugs. Starship Troopers aesthetic.
    pub fn earth() -> Self {
        let seed = 0x_E4_77_00_00; // fixed seed for deterministic Earth terrain
        Self {
            seed,
            name: "Earth".to_string(),
            classification: PlanetClassification::Colony,
            primary_biome: BiomeType::Mountain, // Neutral for UI; terrain uses all biomes via biome_sampler()
            secondary_biome: None,
            danger_level: 0,   // Safe zone — no danger counter on War Table
            infestation: 0.0, // No bugs on the homeworld
            size: PlanetSize::Large,
            galaxy_position: Vec3::ZERO,
            liberated: false,
            has_atmosphere: true,
            atmosphere_color: Vec3::new(0.35, 0.55, 0.92), // Earth: natural blue atmosphere
            visual_radius_value: 420.0,
            has_skinnies: false,
            has_unknown_intel: false,
            gravity_mult: 1.0,
            day_length_mult: 1.0,
        }
    }

    /// Generate a random planet from a seed.
    pub fn generate(seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);

        // Classification (Heinlein / Helldivers 2 style) — influences naming
        let classification = match rng.gen_range(0..100) {
            0..=18 => PlanetClassification::HiveWorld,
            19..=38 => PlanetClassification::Colony,
            39..=52 => PlanetClassification::Outpost,
            53..=65 => PlanetClassification::Frontier,
            66..=75 => PlanetClassification::Industrial,
            76..=84 => PlanetClassification::Research,
            85..=91 => PlanetClassification::WarZone,
            _ => PlanetClassification::Abandoned,
        };

        // Generate name from classification (Starship Troopers / Federation style)
        let name = Self::generate_name(seed, classification, &mut rng);

        // Determine biomes — fully random so every world is a surprise
        let primary_biome = BiomeConfig::random(seed);
        let secondary_biome = if rng.gen_bool(0.65) {
            Some(BiomeConfig::random(seed.wrapping_add(12345)))
        } else {
            None
        };

        // Stats: wide ranges so danger and infestation feel unpredictable
        let danger_level = rng.gen_range(1..=10);
        let infestation = rng.gen_range(0.2..1.8);

        let size = match rng.gen_range(0..100) {
            0..=25 => PlanetSize::Small,
            26..=55 => PlanetSize::Medium,
            56..=82 => PlanetSize::Large,
            _ => PlanetSize::Massive,
        };

        // Galaxy position (for star map)
        let galaxy_position = Vec3::new(
            rng.gen_range(-120.0..120.0),
            rng.gen_range(-25.0..25.0),
            rng.gen_range(-120.0..120.0),
        );

        // Atmosphere: random per planet type
        let has_atmosphere = match primary_biome {
            BiomeType::Volcanic | BiomeType::Ashlands | BiomeType::Scorched => rng.gen_bool(0.65),
            BiomeType::Toxic | BiomeType::Wasteland => rng.gen_bool(0.5),
            BiomeType::Crystalline | BiomeType::SaltFlat => rng.gen_bool(0.55),
            BiomeType::Storm => true, // Storm worlds always have thick atmosphere
            _ => rng.gen_bool(0.88),
        };

        // Atmosphere color: derive from biome then randomize tint so no two planets look the same
        let atmosphere_color = if has_atmosphere {
            let biome_cfg = BiomeConfig::from_type(primary_biome);
            let base = biome_cfg.base_color;
            let tint = (rng.gen::<f32>() * 0.25 + 0.85, rng.gen::<f32>() * 0.2 + 0.9, rng.gen::<f32>() * 0.2 + 0.85);
            Vec3::new(
                (base[0] * 0.35 + 0.35) * tint.0,
                (base[1] * 0.35 + 0.45) * tint.1,
                (base[2] * 0.35 + 0.6) * tint.2,
            ).min(Vec3::ONE)
        } else {
            Vec3::ZERO
        };

        // Visual radius: more variance per size class
        let visual_radius_value = match size {
            PlanetSize::Small => 140.0 + rng.gen::<f32>() * 70.0,
            PlanetSize::Medium => 230.0 + rng.gen::<f32>() * 120.0,
            PlanetSize::Large => 380.0 + rng.gen::<f32>() * 180.0,
            PlanetSize::Massive => 550.0 + rng.gen::<f32>() * 250.0,
        };

        // Skinnies: humanoid aliens on contested/frontier/abandoned worlds
        let has_skinnies = match classification {
            PlanetClassification::HiveWorld => false,
            PlanetClassification::Frontier | PlanetClassification::WarZone | PlanetClassification::Abandoned => rng.gen_bool(0.7),
            PlanetClassification::Colony | PlanetClassification::Outpost => rng.gen_bool(0.45),
            PlanetClassification::Industrial | PlanetClassification::Research => rng.gen_bool(0.28),
        };

        // Intel unknown: troopers have no idea what biomes/danger they're dropping into
        let has_unknown_intel = rng.gen_bool(0.45);

        // Gravity and day length: random so each world feels different
        let gravity_mult = 0.85 + rng.gen::<f32>() * 0.35;
        let day_length_mult = 0.5 + rng.gen::<f32>() * 1.5;

        Self {
            seed,
            name,
            classification,
            primary_biome,
            secondary_biome,
            danger_level,
            infestation,
            size,
            galaxy_position,
            liberated: false,
            has_atmosphere,
            atmosphere_color,
            visual_radius_value,
            has_skinnies,
            has_unknown_intel,
            gravity_mult,
            day_length_mult,
        }
    }

    /// Get the visual radius of this planet (for rendering from space).
    pub fn visual_radius(&self) -> f32 {
        self.visual_radius_value
    }

    /// Generate planet name (Heinlein / Federation / Helldivers 2 style).
    fn generate_name(_seed: u64, classification: PlanetClassification, rng: &mut StdRng) -> String {
        let (prefixes, suffixes) = match classification {
            PlanetClassification::HiveWorld => (
                vec![
                    "Klendathu", "Tango", "Zegema", "Arachnid", "Hive", "Xenomorph",
                    "Bug", "Chitin", "Spinner", "Crawler", "Nest", "Brood",
                ],
                vec!["Prime", "Secundus", "IV", "VII", "Hive", "Nest", "Alpha", "Omega"],
            ),
            PlanetClassification::Colony => (
                vec![
                    "New", "Port", "Fort", "Camp", "Federation", "UCF", "Citizen",
                    "Liberty", "Hope", "Sanctuary", "Pioneer", "Settlement",
                ],
                vec!["Colony", "Station", "Prime", "IV", "Outpost", "Base", "One", "Two"],
            ),
            PlanetClassification::Outpost => (
                vec![
                    "Outpost", "Forward", "Firebase", "MI", "Drop", "Strike",
                    "Watch", "Guard", "Sentinel", "Bastion", "Redoubt",
                ],
                vec!["Alpha", "Beta", "7", "12", "North", "One", "Prime"],
            ),
            PlanetClassification::Frontier => (
                vec![
                    "Rim", "Far", "Edge", "Deep", "Wild", "Unknown", "Uncharted",
                    "Distant", "Lost", "Shadow", "Outer", "Fringe",
                ],
                vec!["Space", "Reach", "IV", "Minor", "Drift", "Expanse"],
            ),
            PlanetClassification::Industrial => (
                vec![
                    "Mining", "Refinery", "Ore", "Smelter", "Works", "Depot",
                    "Drill", "Strike", "Quarry", "Mine", "Extraction",
                ],
                vec!["Prime", "Alpha", "7", "Colony", "Station", "Base"],
            ),
            PlanetClassification::Research => (
                vec![
                    "Labs", "Science", "Survey", "Probe", "Study", "Archive",
                    "Observatory", "Institute", "Facility", "Station",
                ],
                vec!["One", "Alpha", "7", "Prime", "Outpost"],
            ),
            PlanetClassification::WarZone => (
                vec![
                    "Conflict", "Hot", "Contested", "War", "Battle", "Front",
                    "Siege", "Strike", "Kill", "Fire", "No-Man's",
                ],
                vec!["Zone", "Sector", "Prime", "IV", "Alpha", "One"],
            ),
            PlanetClassification::Abandoned => (
                vec![
                    "Old", "Dead", "Lost", "Ruin", "Ghost", "Abandoned",
                    "Forsaken", "Derelict", "Empty", "Silent", "Forgotten",
                ],
                vec!["Colony", "Outpost", "Station", "Prime", "IV", "Ruin"],
            ),
        };
        let p = prefixes[rng.gen_range(0..prefixes.len())];
        let s = suffixes[rng.gen_range(0..suffixes.len())];
        format!("{} {}", p, s)
    }

    /// Get the biome configuration for this planet's primary biome.
    pub fn get_biome_config(&self) -> BiomeConfig {
        BiomeConfig::from_type(self.primary_biome)
    }

    /// Canonical planet surface/terrain color for consistent display in orbit, drop pod, and on surface.
    pub fn surface_color(&self) -> [f32; 3] {
        if self.name == "Earth" {
            return [0.22, 0.48, 0.32]; // Green earth + blue tint for horizon
        }
        let primary = BiomeConfig::from_type(self.primary_biome).base_color;
        let blended = match self.secondary_biome {
            Some(sec) => {
                let secondary = BiomeConfig::from_type(sec).base_color;
                primary * 0.7 + secondary * 0.3
            }
            None => primary,
        };
        [blended.x, blended.y, blended.z]
    }

    /// Atmosphere color as [r, g, b] for sky, drop-pod halo, and horizon. Same everywhere.
    pub fn atmosphere_color_rgb(&self) -> [f32; 3] {
        [
            self.atmosphere_color.x,
            self.atmosphere_color.y,
            self.atmosphere_color.z,
        ]
    }

    /// Create a noise-based multi-biome sampler for this planet.
    /// Biome sampler for terrain: Earth gets all biomes; other planets get 2–4 from seed.
    pub fn biome_sampler(&self) -> PlanetBiomes {
        if self.name == "Earth" {
            PlanetBiomes::earth(self.seed)
        } else {
            PlanetBiomes::from_seed(self.seed)
        }
    }

    /// Calculate bug spawn rate for this planet.
    /// Tuned for Extermination intensity: ~2x base rate so hordes feel relentless.
    pub fn bug_spawn_rate(&self) -> f32 {
        let base_rate = self.get_biome_config().bug_density;
        let raw = base_rate * self.infestation * (self.danger_level as f32 / 5.0);
        raw * 2.0  // Movie/Extermination scale: constant pressure
    }
}

/// Mission objective types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectiveType {
    /// Kill a certain number of bugs.
    Extermination,
    /// Destroy bug holes/nests.
    NestDestruction,
    /// Reach extraction point.
    Extraction,
    /// Defend a position for a time.
    Defense,
    /// Retrieve an item.
    Retrieval,
    /// Eliminate a boss bug.
    Assassination,
}

/// A mission on a planet.
#[derive(Debug, Clone)]
pub struct Mission {
    pub planet: Planet,
    pub objectives: Vec<Objective>,
    pub time_limit: Option<f32>, // In seconds
    pub reward: u32,
}

/// A single mission objective.
#[derive(Debug, Clone)]
pub struct Objective {
    pub objective_type: ObjectiveType,
    pub description: String,
    pub target_count: u32,
    pub current_count: u32,
    pub position: Option<Vec3>,
    pub completed: bool,
}

impl Mission {
    /// Generate a mission for a planet.
    pub fn generate(planet: Planet) -> Self {
        let mut rng = StdRng::seed_from_u64(planet.seed.wrapping_add(999));

        let mut objectives = Vec::new();

        // Primary objective
        let primary_type = match rng.gen_range(0..6) {
            0 => ObjectiveType::Extermination,
            1 => ObjectiveType::NestDestruction,
            2 => ObjectiveType::Defense,
            3 => ObjectiveType::Retrieval,
            4 => ObjectiveType::Assassination,
            _ => ObjectiveType::Extraction,
        };

        objectives.push(Objective::new(primary_type, planet.danger_level, &mut rng));

        // Secondary objectives based on danger level
        if planet.danger_level >= 4 && rng.gen_bool(0.5) {
            let secondary_type = match rng.gen_range(0..3) {
                0 => ObjectiveType::Extermination,
                1 => ObjectiveType::NestDestruction,
                _ => ObjectiveType::Retrieval,
            };
            objectives.push(Objective::new(secondary_type, planet.danger_level / 2, &mut rng));
        }

        // Always add extraction
        if primary_type != ObjectiveType::Extraction {
            objectives.push(Objective {
                objective_type: ObjectiveType::Extraction,
                description: "Reach extraction point".to_string(),
                target_count: 1,
                current_count: 0,
                position: None,
                completed: false,
            });
        }

        let time_limit = if primary_type == ObjectiveType::Defense {
            Some(180.0 + (planet.danger_level as f32 * 30.0))
        } else {
            None
        };

        let reward = planet.danger_level * 100 + objectives.len() as u32 * 50;

        Self {
            planet,
            objectives,
            time_limit,
            reward,
        }
    }

    /// Check if all objectives are complete.
    pub fn is_complete(&self) -> bool {
        self.objectives.iter().all(|o| o.completed)
    }

    /// Update objective progress.
    pub fn update_objective(&mut self, objective_type: ObjectiveType, increment: u32) {
        for objective in &mut self.objectives {
            if objective.objective_type == objective_type && !objective.completed {
                objective.current_count = (objective.current_count + increment).min(objective.target_count);
                if objective.current_count >= objective.target_count {
                    objective.completed = true;
                }
                break;
            }
        }
    }
}

impl Objective {
    fn new(objective_type: ObjectiveType, danger_level: u32, rng: &mut StdRng) -> Self {
        let (description, target_count) = match objective_type {
            ObjectiveType::Extermination => (
                "Eliminate hostile bugs".to_string(),
                50 + danger_level * 20 + rng.gen_range(0..30),
            ),
            ObjectiveType::NestDestruction => (
                "Destroy bug nests".to_string(),
                2 + danger_level / 3,
            ),
            ObjectiveType::Defense => (
                "Defend the position".to_string(),
                1,
            ),
            ObjectiveType::Retrieval => (
                "Retrieve intelligence".to_string(),
                1 + danger_level / 4,
            ),
            ObjectiveType::Assassination => (
                "Eliminate the hive leader".to_string(),
                1,
            ),
            ObjectiveType::Extraction => (
                "Reach extraction".to_string(),
                1,
            ),
        };

        Self {
            objective_type,
            description,
            target_count,
            current_count: 0,
            position: None,
            completed: false,
        }
    }
}

/// Galaxy containing multiple planets.
#[derive(Debug)]
pub struct Galaxy {
    pub planets: Vec<Planet>,
    pub current_planet: Option<usize>,
}

impl Galaxy {
    /// Generate a galaxy with multiple planets.
    pub fn generate(planet_count: usize, base_seed: u64) -> Self {
        let planets = (0..planet_count)
            .map(|i| Planet::generate(base_seed.wrapping_add(i as u64 * 7919)))
            .collect();

        Self {
            planets,
            current_planet: None,
        }
    }

    /// Get available missions (non-liberated planets).
    pub fn available_planets(&self) -> Vec<&Planet> {
        self.planets.iter().filter(|p| !p.liberated).collect()
    }

    /// Mark a planet as liberated.
    pub fn liberate_planet(&mut self, index: usize) {
        if let Some(planet) = self.planets.get_mut(index) {
            planet.liberated = true;
        }
    }
}
