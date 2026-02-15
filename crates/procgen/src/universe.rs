//! Universe generation: galaxies of star systems.

use crate::planet::Planet;
use crate::star_system::{StarSystem, StarType, Star};
use glam::DVec3;
use rand::prelude::*;

/// Entry in the galaxy map -- lightweight until fully generated.
#[derive(Debug, Clone)]
pub struct StarSystemEntry {
    pub seed: u64,
    pub name: String,
    pub position: DVec3,
    pub star_type: StarType,
    /// Whether the full StarSystem has been generated (on visit).
    pub visited: bool,
}

/// The entire procedural universe.
#[derive(Debug)]
pub struct Universe {
    pub seed: u64,
    pub systems: Vec<StarSystemEntry>,
}

impl Universe {
    /// Generate a universe with `system_count` star systems.
    pub fn generate(seed: u64, system_count: usize) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);

        let systems: Vec<StarSystemEntry> = (0..system_count)
            .map(|i| {
                let system_seed = seed.wrapping_add((i as u64 + 1) * 104729);
                let star = Star::generate(system_seed);

                // Place systems in a disc-like galaxy shape
                let angle = rng.gen::<f64>() * std::f64::consts::TAU;
                let arm_offset = (i % 4) as f64 * std::f64::consts::FRAC_PI_2;
                let spiral_angle = angle + arm_offset;

                // Radial distance with spiral arm clustering
                let r = 200.0 + rng.gen::<f64>() * 800.0;
                let spread = 80.0 * rng.gen::<f64>();

                let x = spiral_angle.cos() * r + rng.gen::<f64>() * spread - spread * 0.5;
                let z = spiral_angle.sin() * r + rng.gen::<f64>() * spread - spread * 0.5;
                let y = (rng.gen::<f64>() - 0.5) * 60.0; // thin disc

                let position = DVec3::new(x, y, z);

                // Generate name from syllable table
                let name = generate_system_name(system_seed);

                StarSystemEntry {
                    seed: system_seed,
                    name,
                    position,
                    star_type: star.star_type,
                    visited: false,
                }
            })
            .collect();

        Self { seed, systems }
    }

    /// Generate the full StarSystem for entry at `index`.
    /// System index 0 is Sol: first planet is Earth (homeworld, visitable; Starship Troopers aesthetic).
    pub fn generate_system(&mut self, index: usize) -> StarSystem {
        if let Some(entry) = self.systems.get_mut(index) {
            entry.visited = true;
            let mut system = StarSystem::generate(entry.seed);
            system.galaxy_position = entry.position;
            system.name = entry.name.clone();
            if index == 0 {
                system.name = "Sol System".to_string();
                if !system.bodies.is_empty() {
                    system.bodies[0].planet = Planet::earth();
                }
            }
            system
        } else {
            // Fallback: generate from universe seed
            StarSystem::generate(self.seed)
        }
    }

    /// Find the system entry nearest to a galaxy-space position.
    pub fn nearest_system(&self, pos: DVec3) -> Option<(usize, f64)> {
        self.systems
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let dist = (pos - entry.position).length();
                (i, dist)
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Get systems within a certain radius of a position (for the galaxy map).
    pub fn systems_near(&self, pos: DVec3, radius: f64) -> Vec<(usize, &StarSystemEntry, f64)> {
        self.systems
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| {
                let dist = (pos - entry.position).length();
                if dist <= radius {
                    Some((i, entry, dist))
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Generate a system name (Heinlein / Federation / Helldivers 2 style).
fn generate_system_name(seed: u64) -> String {
    let mut rng = StdRng::seed_from_u64(seed.wrapping_add(42));

    if rng.gen_bool(0.35) {
        // Federation sector style: "Sector 7", "UCF Block 12", "Outer Rim 4"
        let sector_style = [
            "Sector", "Block", "Zone", "Quadrant", "Region", "District",
            "Outer Rim", "Inner Rim", "Core", "Fringe", "Frontier",
        ];
        let style = sector_style[rng.gen_range(0..sector_style.len())];
        let num = rng.gen_range(1..=99);
        return format!("{} {}", style, num);
    }

    // Classical star name from syllable tables
    let prefixes = [
        "Sol", "Alp", "Bet", "Gam", "Del", "Eps", "Zet", "Eta",
        "The", "Iot", "Kap", "Lam", "Sig", "Tau", "Ups", "Phi",
        "Chi", "Psi", "Ome", "Rig", "Veg", "Pro", "Arc", "Sir",
        "Pol", "Den", "Alt", "Cap", "Ald", "Ant", "Spi", "For",
        "Cen", "Lac", "Pav", "Ind", "Ara", "Nor", "TrA", "Cru",
    ];
    let middles = [
        "ar", "el", "an", "or", "en", "al", "ir", "ul",
        "ax", "on", "is", "us", "em", "os", "in", "at",
    ];
    let suffixes = [
        "a", "us", "is", "i", "ae", "ix", "on", "um",
    ];

    let num_parts = rng.gen_range(2..=3usize);
    let mut name = String::new();

    name.push_str(prefixes[rng.gen_range(0..prefixes.len())]);
    if num_parts >= 2 {
        name.push_str(middles[rng.gen_range(0..middles.len())]);
    }
    if num_parts >= 3 {
        name.push_str(suffixes[rng.gen_range(0..suffixes.len())]);
    }

    if rng.gen_bool(0.25) {
        name.push_str(&format!("-{}", rng.gen_range(1..999)));
    }

    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn universe_generate_deterministic_system_count() {
        let u1 = Universe::generate(12345, 50);
        let u2 = Universe::generate(12345, 50);
        assert_eq!(u1.systems.len(), 50);
        assert_eq!(u2.systems.len(), 50);
    }

    #[test]
    fn universe_generate_same_seed_same_first_system_name() {
        let u1 = Universe::generate(999, 10);
        let u2 = Universe::generate(999, 10);
        assert_eq!(u1.systems[0].name, u2.systems[0].name);
        assert_eq!(u1.systems[0].seed, u2.systems[0].seed);
    }

    #[test]
    fn universe_different_seed_different_names() {
        let u1 = Universe::generate(1, 5);
        let u2 = Universe::generate(2, 5);
        // At least one system name should differ (extremely likely)
        let names1: Vec<_> = u1.systems.iter().map(|s| s.name.as_str()).collect();
        let names2: Vec<_> = u2.systems.iter().map(|s| s.name.as_str()).collect();
        assert_ne!(names1, names2);
    }
}
