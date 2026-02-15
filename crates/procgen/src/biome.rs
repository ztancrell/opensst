//! Biome system for varied terrain types.

use glam::Vec3;
use noise::{NoiseFn, Perlin, Simplex};
use rand::prelude::*;

/// Types of biomes for Starship Troopers-style planets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BiomeType {
    /// Arid desert with sparse rocks.
    Desert,
    /// Rocky badlands with canyons.
    Badlands,
    /// Bug hive terrain with organic structures.
    HiveWorld,
    /// Volcanic with lava and ash.
    Volcanic,
    /// Frozen ice world.
    Frozen,
    /// Dead world with toxic atmosphere.
    Toxic,
    /// Rocky mountainous terrain.
    Mountain,
    /// Murky wetland with acidic pools.
    Swamp,
    /// Alien crystal formations, reflective sharp terrain.
    Crystalline,
    /// Post-eruption wasteland, flat grey with embers.
    Ashlands,
    /// Dense alien vegetation, tall spires.
    Jungle,
    /// Barren irradiated desert, cracked earth.
    Wasteland,
}

/// Biome configuration affecting terrain generation.
#[derive(Debug, Clone)]
pub struct BiomeConfig {
    pub biome_type: BiomeType,
    /// Base color for terrain tinting.
    pub base_color: Vec3,
    /// Secondary color for variation.
    pub secondary_color: Vec3,
    /// Height scale multiplier.
    pub height_scale: f32,
    /// Noise frequency multiplier.
    pub frequency_scale: f32,
    /// How rough/jagged the terrain is.
    pub roughness: f32,
    /// Density of props/decorations.
    pub prop_density: f32,
    /// Bug spawn rate multiplier.
    pub bug_density: f32,
}

impl BiomeConfig {
    /// Get configuration for a biome type.
    pub fn from_type(biome_type: BiomeType) -> Self {
        match biome_type {
            // Desert: warm sand (MIRO-style saturated, SST military palette)
            BiomeType::Desert => Self {
                biome_type,
                base_color: Vec3::new(0.82, 0.68, 0.48),
                secondary_color: Vec3::new(0.48, 0.38, 0.34),
                height_scale: 0.8,
                frequency_scale: 1.0,
                roughness: 0.3,
                prop_density: 0.2,
                bug_density: 1.0,
            },
            // Badlands: red-brown (bold SST wasteland)
            BiomeType::Badlands => Self {
                biome_type,
                base_color: Vec3::new(0.58, 0.38, 0.32),
                secondary_color: Vec3::new(0.45, 0.30, 0.26),
                height_scale: 1.5,
                frequency_scale: 1.2,
                roughness: 0.7,
                prop_density: 0.3,
                bug_density: 0.8,
            },
            // HiveWorld: dark organic #3d3228, resin #4a3d2e (ART_DIRECTION)
            BiomeType::HiveWorld => Self {
                biome_type,
                base_color: Vec3::new(0.24, 0.20, 0.16),
                secondary_color: Vec3::new(0.29, 0.24, 0.18),
                height_scale: 1.2,
                frequency_scale: 0.8,
                roughness: 0.5,
                prop_density: 0.8,
                bug_density: 2.0,
            },
            // Volcanic: black rock, ember (stylized fire/ash)
            BiomeType::Volcanic => Self {
                biome_type,
                base_color: Vec3::new(0.18, 0.15, 0.13),
                secondary_color: Vec3::new(0.65, 0.32, 0.10),
                height_scale: 1.8,
                frequency_scale: 0.9,
                roughness: 0.8,
                prop_density: 0.4,
                bug_density: 0.5,
            },
            // Frozen: pale blue-grey (MIRO-style cool saturated)
            BiomeType::Frozen => Self {
                biome_type,
                base_color: Vec3::new(0.58, 0.68, 0.75),
                secondary_color: Vec3::new(0.70, 0.82, 0.90),
                height_scale: 1.0,
                frequency_scale: 1.1,
                roughness: 0.4,
                prop_density: 0.15,
                bug_density: 0.6,
            },
            // Toxic wasteland: sickly yellow-green, chemical-stained earth
            BiomeType::Toxic => Self {
                biome_type,
                base_color: Vec3::new(0.28, 0.32, 0.20),
                secondary_color: Vec3::new(0.38, 0.45, 0.18),
                height_scale: 0.6,
                frequency_scale: 1.3,
                roughness: 0.35,
                prop_density: 0.5,
                bug_density: 1.5,
            },
            // Harsh alien mountains: grey-brown granite, exposed bedrock
            BiomeType::Mountain => Self {
                biome_type,
                base_color: Vec3::new(0.40, 0.38, 0.35),
                secondary_color: Vec3::new(0.50, 0.47, 0.44),
                height_scale: 2.5,
                frequency_scale: 0.7,
                roughness: 0.9,
                prop_density: 0.25,
                bug_density: 0.4,
            },
            // Murky swamp: dark green-brown, low terrain, waterlogged
            BiomeType::Swamp => Self {
                biome_type,
                base_color: Vec3::new(0.22, 0.28, 0.18),
                secondary_color: Vec3::new(0.18, 0.22, 0.12),
                height_scale: 0.4,
                frequency_scale: 1.4,
                roughness: 0.25,
                prop_density: 0.7,
                bug_density: 1.8,
            },
            // Alien crystal formations: bold blue-purple (stylized)
            BiomeType::Crystalline => Self {
                biome_type,
                base_color: Vec3::new(0.40, 0.30, 0.62),
                secondary_color: Vec3::new(0.55, 0.40, 0.75),
                height_scale: 1.6,
                frequency_scale: 1.0,
                roughness: 0.6,
                prop_density: 0.4,
                bug_density: 0.3,
            },
            // Post-eruption ashlands: flat grey powder, scattered embers
            BiomeType::Ashlands => Self {
                biome_type,
                base_color: Vec3::new(0.35, 0.33, 0.32),
                secondary_color: Vec3::new(0.28, 0.26, 0.25),
                height_scale: 0.5,
                frequency_scale: 1.1,
                roughness: 0.2,
                prop_density: 0.3,
                bug_density: 0.7,
            },
            // Dense alien jungle: rich green (MIRO-style vibrant)
            BiomeType::Jungle => Self {
                biome_type,
                base_color: Vec3::new(0.20, 0.38, 0.14),
                secondary_color: Vec3::new(0.28, 0.45, 0.18),
                height_scale: 1.3,
                frequency_scale: 0.9,
                roughness: 0.55,
                prop_density: 0.9,
                bug_density: 1.6,
            },
            // Wasteland: grey dust, cracked earth (ART_DIRECTION)
            BiomeType::Wasteland => Self {
                biome_type,
                base_color: Vec3::new(0.45, 0.42, 0.38),
                secondary_color: Vec3::new(0.38, 0.36, 0.32),
                height_scale: 0.7,
                frequency_scale: 1.2,
                roughness: 0.4,
                prop_density: 0.2,
                bug_density: 0.5,
            },
        }
    }

    /// Get a random biome type.
    pub fn random(seed: u64) -> BiomeType {
        let mut rng = StdRng::seed_from_u64(seed);
        ALL_BIOMES[rng.gen_range(0..ALL_BIOMES.len())]
    }

    /// Whether this biome typically has surface water (lakes, streams, ocean).
    pub fn has_water(&self) -> bool {
        !matches!(
            self.biome_type,
            BiomeType::Desert | BiomeType::Volcanic | BiomeType::Ashlands | BiomeType::Wasteland
        )
    }
}

/// All biome types for iteration.
pub const ALL_BIOMES: [BiomeType; 12] = [
    BiomeType::Desert,
    BiomeType::Badlands,
    BiomeType::HiveWorld,
    BiomeType::Volcanic,
    BiomeType::Frozen,
    BiomeType::Toxic,
    BiomeType::Mountain,
    BiomeType::Swamp,
    BiomeType::Crystalline,
    BiomeType::Ashlands,
    BiomeType::Jungle,
    BiomeType::Wasteland,
];

/// Noise-based biome sampler for a planet.
/// Uses large-scale noise to assign biome regions across the surface.
pub struct PlanetBiomes {
    /// The biome types present on this planet (2-4 types).
    pub biomes: Vec<BiomeType>,
    biome_noise: Perlin,
    _blend_noise: Simplex,
    /// Scale: lower = larger biome regions.
    pub region_scale: f64,
}

impl PlanetBiomes {
    /// Create a multi-biome planet from a seed.
    /// Picks 2-4 distinct biome types and builds noise for spatial selection.
    pub fn from_seed(seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);

        // Pick 2-4 distinct biome types
        let num_biomes = rng.gen_range(2..=4);
        let mut available = ALL_BIOMES.to_vec();
        // Shuffle and take
        for i in (1..available.len()).rev() {
            let j = rng.gen_range(0..=i);
            available.swap(i, j);
        }
        let biomes: Vec<BiomeType> = available.into_iter().take(num_biomes).collect();

        let biome_noise = Perlin::new(rng.gen());
        let blend_noise = Simplex::new(rng.gen());

        Self {
            biomes,
            biome_noise,
            _blend_noise: blend_noise,
            region_scale: 0.004 + rng.gen::<f64>() * 0.003, // 0.004..0.007
        }
    }

    /// Sample the biome at a world (x, z) position.
    /// Returns the biome config and a blend color (RGBA) for the vertex.
    pub fn sample_at(&self, x: f64, z: f64) -> (BiomeConfig, [f32; 4]) {
        let n = self.biomes.len();
        if n == 0 {
            let cfg = BiomeConfig::from_type(BiomeType::Desert);
            let c = cfg.base_color;
            return (cfg, [c.x, c.y, c.z, 1.0]);
        }

        // Primary biome selection noise (large-scale regions)
        let val = self.biome_noise.get([x * self.region_scale, z * self.region_scale]);
        // Map noise [-1, 1] to [0, n)
        let mapped = ((val * 0.5 + 0.5) * n as f64).clamp(0.0, (n - 1) as f64);

        let idx_a = mapped.floor() as usize;
        let idx_b = (idx_a + 1).min(n - 1);
        let frac = (mapped - idx_a as f64) as f32;

        let cfg_a = BiomeConfig::from_type(self.biomes[idx_a]);
        let cfg_b = BiomeConfig::from_type(self.biomes[idx_b]);

        // Sharpen the blend: most of the terrain is clearly one biome, with
        // smooth transitions at boundaries.
        let t = (frac * 2.0 - 1.0).clamp(-1.0, 1.0); // remap to [-1, 1]
        let blend = (t * t * t * 0.5 + 0.5).clamp(0.0, 1.0); // cubic ease

        // Blend colors
        let color = cfg_a.base_color * (1.0 - blend) + cfg_b.base_color * blend;

        // Return the dominant biome config (with blended colors in the vertex)
        let dominant = if blend < 0.5 { cfg_a } else { cfg_b };

        (dominant, [color.x, color.y, color.z, 1.0])
    }

    /// Sample just the height scale at a position (for terrain height variation by biome).
    pub fn height_scale_at(&self, x: f64, z: f64) -> f32 {
        let n = self.biomes.len();
        if n == 0 {
            return 1.0;
        }
        let val = self.biome_noise.get([x * self.region_scale, z * self.region_scale]);
        let mapped = ((val * 0.5 + 0.5) * n as f64).clamp(0.0, (n - 1) as f64);
        let idx_a = mapped.floor() as usize;
        let idx_b = (idx_a + 1).min(n - 1);
        let frac = mapped - idx_a as f64;

        let a = BiomeConfig::from_type(self.biomes[idx_a]).height_scale;
        let b = BiomeConfig::from_type(self.biomes[idx_b]).height_scale;
        a + (b - a) * frac as f32
    }
}

/// Blend between biomes based on position.
pub struct BiomeBlender {
    pub biomes: Vec<(BiomeConfig, Vec3, f32)>, // Config, center, radius
}

impl BiomeBlender {
    pub fn new() -> Self {
        Self { biomes: Vec::new() }
    }

    /// Add a biome region.
    pub fn add_biome(&mut self, config: BiomeConfig, center: Vec3, radius: f32) {
        self.biomes.push((config, center, radius));
    }

    /// Sample biome influence at a position.
    pub fn sample(&self, position: Vec3) -> BiomeConfig {
        if self.biomes.is_empty() {
            return BiomeConfig::from_type(BiomeType::Desert);
        }

        // Find dominant biome based on distance
        let mut best_config = &self.biomes[0].0;
        let mut best_weight = 0.0f32;

        for (config, center, radius) in &self.biomes {
            let dist = position.distance(*center);
            let weight = (1.0 - (dist / radius).min(1.0)).powi(2);

            if weight > best_weight {
                best_weight = weight;
                best_config = config;
            }
        }

        best_config.clone()
    }
}

impl Default for BiomeBlender {
    fn default() -> Self {
        Self::new()
    }
}
