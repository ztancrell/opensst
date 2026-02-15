//! Planet and mission generation system.

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

        // Generate name from classification and biome (Starship Troopers / Federation style)
        let name = Self::generate_name(seed, classification, &mut rng);

        // Determine biomes
        let primary_biome = BiomeConfig::random(seed);
        let secondary_biome = if rng.gen_bool(0.6) {
            Some(BiomeConfig::random(seed.wrapping_add(12345)))
        } else {
            None
        };

        // Determine stats
        let danger_level = rng.gen_range(1..=10);
        let infestation = rng.gen_range(0.3..1.5);

        let size = match rng.gen_range(0..100) {
            0..=20 => PlanetSize::Small,
            21..=60 => PlanetSize::Medium,
            61..=85 => PlanetSize::Large,
            _ => PlanetSize::Massive,
        };

        // Galaxy position (for star map)
        let galaxy_position = Vec3::new(
            rng.gen_range(-100.0..100.0),
            rng.gen_range(-20.0..20.0),
            rng.gen_range(-100.0..100.0),
        );

        // Atmosphere: most planets have one, some small/dead ones don't
        let has_atmosphere = match primary_biome {
            BiomeType::Volcanic | BiomeType::Ashlands => rng.gen_bool(0.7),
            BiomeType::Toxic | BiomeType::Wasteland => rng.gen_bool(0.5),
            BiomeType::Crystalline => rng.gen_bool(0.6),
            _ => rng.gen_bool(0.9),
        };

        // Atmosphere color derived from biome
        let atmosphere_color = if has_atmosphere {
            let biome_cfg = BiomeConfig::from_type(primary_biome);
            let base = biome_cfg.base_color;
            // Blend biome color with sky blue for a tinted atmosphere
            Vec3::new(
                (base[0] * 0.3 + 0.4).min(1.0),
                (base[1] * 0.3 + 0.5).min(1.0),
                (base[2] * 0.3 + 0.7).min(1.0),
            )
        } else {
            Vec3::ZERO
        };

        // Visual radius scales with planet size
        let visual_radius_value = match size {
            PlanetSize::Small => 150.0 + rng.gen::<f32>() * 50.0,
            PlanetSize::Medium => 250.0 + rng.gen::<f32>() * 100.0,
            PlanetSize::Large => 400.0 + rng.gen::<f32>() * 150.0,
            PlanetSize::Massive => 600.0 + rng.gen::<f32>() * 200.0,
        };

        // Skinnies (Heinlein): humanoid aliens on contested/frontier/abandoned worlds
        let has_skinnies = match classification {
            PlanetClassification::HiveWorld => false,
            PlanetClassification::Frontier | PlanetClassification::WarZone | PlanetClassification::Abandoned => rng.gen_bool(0.65),
            PlanetClassification::Colony | PlanetClassification::Outpost => rng.gen_bool(0.4),
            PlanetClassification::Industrial | PlanetClassification::Research => rng.gen_bool(0.25),
        };

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

    /// Create a noise-based multi-biome sampler for this planet.
    pub fn biome_sampler(&self) -> PlanetBiomes {
        PlanetBiomes::from_seed(self.seed)
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
