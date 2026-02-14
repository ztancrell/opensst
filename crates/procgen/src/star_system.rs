//! Star system generation: stars, orbital bodies, and system layout.

use crate::planet::{Planet, PlanetSize};
use glam::{DVec3, Vec3};
use rand::prelude::*;

/// Types of stars.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StarType {
    RedDwarf,
    YellowMain,
    BlueGiant,
    WhiteDwarf,
    BinaryStar,
}

impl StarType {
    /// Number of planets this star type tends to have.
    pub fn planet_range(&self) -> (usize, usize) {
        match self {
            StarType::RedDwarf => (2, 4),
            StarType::YellowMain => (4, 8),
            StarType::BlueGiant => (3, 6),
            StarType::WhiteDwarf => (1, 3),
            StarType::BinaryStar => (2, 5),
        }
    }

    /// Base orbital radius range for planets (min, max in game units).
    pub fn orbital_range(&self) -> (f32, f32) {
        match self {
            StarType::RedDwarf => (1500.0, 20000.0),
            StarType::YellowMain => (3000.0, 50000.0),
            StarType::BlueGiant => (5000.0, 80000.0),
            StarType::WhiteDwarf => (1000.0, 15000.0),
            StarType::BinaryStar => (4000.0, 60000.0),
        }
    }
}

/// A star at the center of a system.
#[derive(Debug, Clone)]
pub struct Star {
    pub star_type: StarType,
    pub color: Vec3,
    pub radius: f32,
    pub luminosity: f32,
    pub name: String,
}

impl Star {
    /// Generate a star from a seed.
    pub fn generate(seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);

        let star_type = match rng.gen_range(0..100) {
            0..=35 => StarType::RedDwarf,
            36..=70 => StarType::YellowMain,
            71..=82 => StarType::BlueGiant,
            83..=92 => StarType::WhiteDwarf,
            _ => StarType::BinaryStar,
        };

        let (color, radius, luminosity) = match star_type {
            StarType::RedDwarf => (
                Vec3::new(1.0, 0.4, 0.2),
                80.0 + rng.gen::<f32>() * 40.0,
                0.4,
            ),
            StarType::YellowMain => (
                Vec3::new(1.0, 0.95, 0.7),
                120.0 + rng.gen::<f32>() * 60.0,
                1.0,
            ),
            StarType::BlueGiant => (
                Vec3::new(0.6, 0.7, 1.0),
                200.0 + rng.gen::<f32>() * 150.0,
                2.5,
            ),
            StarType::WhiteDwarf => (
                Vec3::new(0.95, 0.95, 1.0),
                50.0 + rng.gen::<f32>() * 30.0,
                0.6,
            ),
            StarType::BinaryStar => (
                Vec3::new(1.0, 0.85, 0.5),
                150.0 + rng.gen::<f32>() * 80.0,
                1.8,
            ),
        };

        // Generate star name from syllables
        let syllables = ["Sol", "Alp", "Bet", "Gam", "Sig", "Tau", "Rig", "Veg",
                         "Pro", "Arc", "Sir", "Pol", "Den", "Alt", "Cap", "Ald"];
        let suffixes = ["a", "us", "is", "ar", "el", "ix", "on", "ae"];
        let name = format!(
            "{}{}",
            syllables[rng.gen_range(0..syllables.len())],
            suffixes[rng.gen_range(0..suffixes.len())]
        );

        Self {
            star_type,
            color,
            radius,
            luminosity,
            name,
        }
    }
}

/// An orbital body (planet or moon) in a star system.
#[derive(Debug, Clone)]
pub struct OrbitalBody {
    /// The planet data (biomes, terrain config, etc.).
    pub planet: Planet,
    /// Distance from parent (star or planet) in game units.
    pub orbital_radius: f32,
    /// Orbital speed in radians per second.
    pub orbital_speed: f32,
    /// Starting orbital angle in radians.
    pub orbital_phase: f32,
    /// Axial tilt in radians.
    pub axial_tilt: f32,
    /// Child moons.
    pub moons: Vec<OrbitalBody>,
    /// Whether this body has a ring system (visual only).
    pub ring_system: bool,
}

impl OrbitalBody {
    /// Compute the world-space position of this body at a given time.
    /// Returns position relative to the parent (star center for planets, planet center for moons).
    pub fn orbital_position(&self, time: f64) -> DVec3 {
        let angle = self.orbital_phase as f64 + time * self.orbital_speed as f64;
        let r = self.orbital_radius as f64;
        DVec3::new(angle.cos() * r, 0.0, angle.sin() * r)
    }

    /// Compute the world-space position of a moon relative to the system origin.
    pub fn moon_world_position(&self, moon_idx: usize, time: f64) -> Option<DVec3> {
        let planet_pos = self.orbital_position(time);
        self.moons.get(moon_idx).map(|moon| {
            planet_pos + moon.orbital_position(time)
        })
    }
}

/// A complete star system with a star and orbiting bodies.
#[derive(Debug, Clone)]
pub struct StarSystem {
    pub seed: u64,
    pub name: String,
    pub star: Star,
    pub bodies: Vec<OrbitalBody>,
    pub galaxy_position: DVec3,
}

impl StarSystem {
    /// Generate a complete star system from a seed.
    pub fn generate(seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let star = Star::generate(seed);

        let (min_planets, max_planets) = star.star_type.planet_range();
        let num_planets = rng.gen_range(min_planets..=max_planets);
        let (orbit_min, orbit_max) = star.star_type.orbital_range();

        let mut bodies = Vec::with_capacity(num_planets);
        for i in 0..num_planets {
            let planet_seed = seed.wrapping_add((i as u64 + 1) * 31337);
            let planet = Planet::generate(planet_seed);

            // Orbital radius increases with index, with some randomness
            let t = (i as f32 + 1.0) / (num_planets as f32 + 1.0);
            let base_radius = orbit_min + (orbit_max - orbit_min) * t;
            let orbital_radius = base_radius * (0.8 + rng.gen::<f32>() * 0.4);

            // Slower orbits for farther planets (Kepler-ish)
            let orbital_speed = 0.02 / (t + 0.3).sqrt();

            let orbital_phase = rng.gen::<f32>() * std::f32::consts::TAU;
            let axial_tilt = rng.gen::<f32>() * 0.5; // up to ~28 degrees

            // Moons (0-3, more likely for larger planets)
            let max_moons = match planet.size {
                PlanetSize::Small => 0,
                PlanetSize::Medium => 1,
                PlanetSize::Large => 2,
                PlanetSize::Massive => 3,
            };
            let num_moons = if max_moons > 0 { rng.gen_range(0..=max_moons) } else { 0 };
            let mut moons = Vec::new();
            for m in 0..num_moons {
                let moon_seed = planet_seed.wrapping_add((m as u64 + 1) * 7777);
                let mut moon_planet = Planet::generate(moon_seed);
                // Moons are always small
                moon_planet.size = PlanetSize::Small;
                moon_planet.name = format!("{} {}", planet.name, ["I", "II", "III"][m.min(2)]);

                let moon_orbit = planet.visual_radius() * (3.0 + m as f32 * 2.5)
                    + rng.gen::<f32>() * 200.0;

                moons.push(OrbitalBody {
                    planet: moon_planet,
                    orbital_radius: moon_orbit,
                    orbital_speed: 0.1 + rng.gen::<f32>() * 0.15,
                    orbital_phase: rng.gen::<f32>() * std::f32::consts::TAU,
                    axial_tilt: rng.gen::<f32>() * 0.3,
                    moons: Vec::new(),
                    ring_system: false,
                });
            }

            // Ring systems: ~15% chance for large/massive planets
            let ring_system = matches!(planet.size, PlanetSize::Large | PlanetSize::Massive)
                && rng.gen_bool(0.15);

            bodies.push(OrbitalBody {
                planet,
                orbital_radius,
                orbital_speed,
                orbital_phase,
                axial_tilt,
                moons,
                ring_system,
            });
        }

        // System name from star
        let system_name = format!("{} System", star.name);

        // Galaxy position (will be overridden by Universe placement)
        let galaxy_position = DVec3::new(
            rng.gen_range(-500.0..500.0),
            rng.gen_range(-50.0..50.0),
            rng.gen_range(-500.0..500.0),
        );

        Self {
            seed,
            name: system_name,
            star,
            bodies,
            galaxy_position,
        }
    }

    /// Get all orbital body positions at a given time. Returns (planet_idx, position).
    pub fn body_positions(&self, time: f64) -> Vec<(usize, DVec3)> {
        self.bodies
            .iter()
            .enumerate()
            .map(|(i, body)| (i, body.orbital_position(time)))
            .collect()
    }

    /// Find the nearest planet to a position. Returns (index, distance).
    pub fn nearest_body(&self, pos: DVec3, time: f64) -> Option<(usize, f64)> {
        self.bodies
            .iter()
            .enumerate()
            .map(|(i, body)| {
                let body_pos = body.orbital_position(time);
                let dist = (pos - body_pos).length();
                (i, dist)
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    }
}
