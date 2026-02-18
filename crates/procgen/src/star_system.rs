//! Star system generation: stars, orbital bodies, and system layout.
//!
//! Uses Keplerian mechanics: orbital periods from T² ∝ a³ / (G·M), elliptical orbits
//! via eccentricity and true anomaly. Star mass and semi-major axis drive period;
//! eccentricity and argument of periapsis shape the orbit.

use crate::planet::{Planet, PlanetSize};
use glam::{DVec3, Vec3};
use rand::prelude::*;

/// Gravitational constant × default star mass in game units so that a ≈ 10000 gives ~0.02 rad/s.
/// Kepler: ω = √(G·M / a³), so G·M = ω² · a³. With ω = 0.02, a = 10000: G·M = 4e8.
pub const GRAVITATIONAL_PARAM: f64 = 4e8;

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
    /// Mass in solar units (~1.0 = Sun-like). Drives orbital periods via Kepler.
    pub mass: f32,
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

        let (color, radius, luminosity, mass) = match star_type {
            StarType::RedDwarf => (
                Vec3::new(1.0, 0.4, 0.2),
                80.0 + rng.gen::<f32>() * 40.0,
                0.4,
                0.3 + rng.gen::<f32>() * 0.4, // 0.3–0.7 M_sun
            ),
            StarType::YellowMain => (
                Vec3::new(1.0, 0.95, 0.7),
                120.0 + rng.gen::<f32>() * 60.0,
                1.0,
                0.8 + rng.gen::<f32>() * 0.4, // 0.8–1.2
            ),
            StarType::BlueGiant => (
                Vec3::new(0.6, 0.7, 1.0),
                200.0 + rng.gen::<f32>() * 150.0,
                2.5,
                8.0 + rng.gen::<f32>() * 8.0, // 8–16
            ),
            StarType::WhiteDwarf => (
                Vec3::new(0.95, 0.95, 1.0),
                50.0 + rng.gen::<f32>() * 30.0,
                0.6,
                0.5 + rng.gen::<f32>() * 0.2, // 0.5–0.7
            ),
            StarType::BinaryStar => (
                Vec3::new(1.0, 0.85, 0.5),
                150.0 + rng.gen::<f32>() * 80.0,
                1.8,
                1.2 + rng.gen::<f32>() * 0.6, // 1.2–1.8
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
            mass,
            name,
        }
    }
}

/// An orbital body (planet or moon) in a star system.
#[derive(Debug, Clone)]
pub struct OrbitalBody {
    /// The planet data (biomes, terrain config, etc.).
    pub planet: Planet,
    /// Semi-major axis in game units (average distance from parent).
    pub orbital_radius: f32,
    /// Mean motion in radians per second (2π / period). From Kepler: ω = √(μ/a³).
    pub orbital_speed: f32,
    /// Mean anomaly at t=0 in radians (position along orbit at epoch).
    pub orbital_phase: f32,
    /// Orbital eccentricity (0 = circle, 0.2 = mild ellipse, 0.9 = very elongated).
    pub eccentricity: f32,
    /// Argument of periapsis in radians (angle from ascending node to periapsis).
    pub argument_of_periapsis: f32,
    /// Orbital inclination in radians (tilt out of reference plane).
    pub orbital_inclination: f32,
    /// Longitude of ascending node in radians.
    pub orbital_longitude: f32,
    /// Axial tilt in radians.
    pub axial_tilt: f32,
    /// Child moons.
    pub moons: Vec<OrbitalBody>,
    /// Whether this body has a ring system (visual only).
    pub ring_system: bool,
}

impl OrbitalBody {
    /// Solve Kepler's equation M = E - e*sin(E) for eccentric anomaly E (Newton iteration).
    #[inline]
    fn eccentric_anomaly(mean_anomaly: f64, e: f64) -> f64 {
        let mut e_anom = mean_anomaly;
        for _ in 0..20 {
            let delta = (e_anom - e * e_anom.sin() - mean_anomaly) / (1.0 - e * e_anom.cos());
            e_anom -= delta;
            if delta.abs() < 1e-12 {
                break;
            }
        }
        e_anom
    }

    /// Compute the world-space position of this body at a given time (Keplerian ellipse).
    /// Returns position relative to the parent. Uses eccentricity and argument of periapsis.
    pub fn orbital_position(&self, time: f64) -> DVec3 {
        let a = self.orbital_radius as f64;
        let e = self.eccentricity as f64;
        let omega = self.argument_of_periapsis as f64;
        let n = self.orbital_speed as f64; // mean motion

        let mean_anomaly = (self.orbital_phase as f64 + time * n).rem_euclid(std::f64::consts::TAU);
        let (x, z) = if e < 0.001 {
            // Circular: avoid Kepler iteration
            let r = a;
            let angle = mean_anomaly;
            (r * angle.cos(), r * angle.sin())
        } else {
            let e_anom = Self::eccentric_anomaly(mean_anomaly, e);
            // True anomaly: ν = 2 atan2(√(1+e) sin(E/2), √(1-e) cos(E/2))
            let half_e = e_anom / 2.0;
            let true_anomaly = 2.0 * ((1.0 + e).sqrt() * half_e.sin()).atan2((1.0 - e).max(0.01).sqrt() * half_e.cos());
            // Distance: r = a(1 - e²) / (1 + e*cos(ν))
            let r = a * (1.0 - e * e) / (1.0 + e * true_anomaly.cos());
            // Position in orbital plane (periapsis along +X)
            let x_orb = r * true_anomaly.cos();
            let z_orb = r * true_anomaly.sin();
            // Rotate by argument of periapsis
            let x = x_orb * omega.cos() - z_orb * omega.sin();
            let z = x_orb * omega.sin() + z_orb * omega.cos();
            (x, z)
        };

        // Apply longitude of ascending node (rotate in XZ)
        let lon = self.orbital_longitude as f64;
        let x1 = x * lon.cos() - z * lon.sin();
        let z1 = x * lon.sin() + z * lon.cos();
        // Apply inclination (tilt around X)
        let inc = self.orbital_inclination as f64;
        let y = z1 * inc.sin();
        let z_final = z1 * inc.cos();
        DVec3::new(x1, y, z_final)
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

        // Build orbit slots with randomized order so inner/outer isn't strictly by index
        let mut slot_factors: Vec<f32> = (0..num_planets)
            .map(|i| {
                let t = (i as f32 + 1.0) / (num_planets as f32 + 1.0);
                t * (0.7 + rng.gen::<f32>() * 0.6) // shuffle radius factor per slot
            })
            .collect();
        slot_factors.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let mut bodies = Vec::with_capacity(num_planets);
        for i in 0..num_planets {
            let planet_seed = seed.wrapping_add((i as u64 + 1) * 31337);
            let planet = Planet::generate(planet_seed);

            // Semi-major axis: randomized placement within star's range
            let slot = slot_factors[i];
            let orbital_radius = orbit_min + (orbit_max - orbit_min) * slot * (0.85 + rng.gen::<f32>() * 0.3);

            // Kepler: ω = √(μ/a³), period T = 2π/ω. Star mass drives period.
            let a = orbital_radius as f64;
            let mu = GRAVITATIONAL_PARAM * (star.mass as f64);
            let orbital_speed = (mu / (a * a * a)).sqrt() as f32 * (0.95 + rng.gen::<f32>() * 0.1);

            let orbital_phase = rng.gen::<f32>() * std::f32::consts::TAU;
            let eccentricity = rng.gen::<f32>() * 0.22f32; // 0..0.22 (realistic planet range)
            let argument_of_periapsis = rng.gen::<f32>() * std::f32::consts::TAU;
            let orbital_inclination = rng.gen::<f32>() * 0.4;
            let orbital_longitude = rng.gen::<f32>() * std::f32::consts::TAU;
            let axial_tilt = rng.gen::<f32>() * 0.5;

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
                moon_planet.size = PlanetSize::Small;
                moon_planet.name = format!("{} {}", planet.name, ["I", "II", "III"][m.min(2)]);

                let moon_orbit = planet.visual_radius() * (3.0 + m as f32 * 2.5)
                    + rng.gen::<f32>() * 200.0;
                // Moon period from Kepler with planet mass proxy (μ_planet ~ 1e6 in game units)
                const MOON_MU: f64 = 1e6;
                let moon_a = moon_orbit as f64;
                let moon_speed = (MOON_MU / (moon_a * moon_a * moon_a)).sqrt() as f32;

                moons.push(OrbitalBody {
                    planet: moon_planet,
                    orbital_radius: moon_orbit,
                    orbital_speed: moon_speed * (0.9 + rng.gen::<f32>() * 0.2),
                    orbital_phase: rng.gen::<f32>() * std::f32::consts::TAU,
                    eccentricity: rng.gen::<f32>() * 0.15,
                    argument_of_periapsis: rng.gen::<f32>() * std::f32::consts::TAU,
                    orbital_inclination: rng.gen::<f32>() * 0.25,
                    orbital_longitude: rng.gen::<f32>() * std::f32::consts::TAU,
                    axial_tilt: rng.gen::<f32>() * 0.3,
                    moons: Vec::new(),
                    ring_system: false,
                });
            }

            let ring_system = matches!(planet.size, PlanetSize::Large | PlanetSize::Massive)
                && rng.gen_bool(0.15);

            bodies.push(OrbitalBody {
                planet,
                orbital_radius,
                orbital_speed,
                orbital_phase,
                eccentricity,
                argument_of_periapsis,
                orbital_inclination,
                orbital_longitude,
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
