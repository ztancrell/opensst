//! Biome feature tables: landmarks, hazards, destructibles, and bug variants per biome.
//! Lookup by BiomeType (game holds tables; procgen stays dependency-free).

use procgen::BiomeType;

use crate::bug::BugVariant;
use crate::destruction::{HazardType, LandmarkType};

/// Per-biome spawn table for landmarks, hazards, destructibles, and bug variant.
#[derive(Debug, Clone)]
pub struct BiomeFeatureTable {
    /// (landmark_type, min_count, max_count) for decorative/destructible landmarks.
    pub landmarks: Vec<(LandmarkType, u32, u32)>,
    /// (hazard_type, min_count, max_count) for environmental hazards.
    pub hazards: Vec<(HazardType, u32, u32)>,
    /// (landmark_type, min_count, max_count) for destructibles with chain reactions.
    pub destructibles: Vec<(LandmarkType, u32, u32)>,
    /// This biome's unique bug variant.
    pub bug_variant: Option<BugVariant>,
    /// Probability a spawned bug is the variant (0.0–0.4).
    pub variant_chance: f32,
}

/// Return the feature table for a given biome type.
pub fn get_biome_feature_table(biome: BiomeType) -> BiomeFeatureTable {
    use HazardType::*;
    use LandmarkType::*;

    match biome {
        BiomeType::Desert => BiomeFeatureTable {
            landmarks: vec![
                (RockArch, 18, 38),        // Sahara/Mojave: wind-carved arches everywhere
                (SandDuneRidge, 28, 55),   // Rolling dunes, desert waves
                (OasisPool, 6, 14),        // Rare life-giving water
                (CaveEntrance, 0, 5),
            ],
            hazards: vec![(Sandstorm, 2, 5)],
            destructibles: vec![(RockArch, 4, 12)],
            bug_variant: Some(BugVariant::Burrower),
            variant_chance: 0.25,
        },
        BiomeType::Badlands => BiomeFeatureTable {
            landmarks: vec![
                (MesaPillar, 18, 38),      // Utah canyon country: towering mesas
                (CanyonWall, 10, 22),      // Steep canyon walls
                (DriedRavine, 8, 18),      // Cracked riverbeds
                (CaveEntrance, 6, 14),
            ],
            hazards: vec![(Rockslide, 4, 10)],
            destructibles: vec![(MesaPillar, 6, 14)],
            bug_variant: Some(BugVariant::AmbushWarrior),
            variant_chance: 0.22,
        },
        BiomeType::HiveWorld => BiomeFeatureTable {
            landmarks: vec![
                (ResinNode, 25, 50),         // Living hive: resin everywhere
                (PulsingEggWall, 14, 28),    // Egg clusters on walls
                (OrganicTunnel, 10, 22),     // Bug-carved passages
                (CaveEntrance, 6, 14),
                (HiveCaveEntrance, 12, 28),  // Big surface tunnel mouths (eggs, nests, holes)
            ],
            hazards: vec![(SporeBurst, 8, 18)],
            destructibles: vec![
                (ResinNode, 8, 18),
                (PulsingEggWall, 6, 14),     // Chain-pop egg goo
                (OrganicTunnel, 6, 12),     // Collapse
                (HiveCaveEntrance, 8, 18),  // Collapse
            ],
            bug_variant: Some(BugVariant::BroodMother),
            variant_chance: 0.2,
        },
        BiomeType::Volcanic => BiomeFeatureTable {
            landmarks: vec![
                (LavaRiver, 6, 14),        // Hawaii/Iceland: lava channels
                (ObsidianSpire, 14, 28),   // Jagged black glass spires
                (Geyser, 8, 18),           // Steam vents, boiling pools
                (CaveEntrance, 4, 12),
            ],
            hazards: vec![(GeyserEruption, 6, 14), (LavaFlow, 4, 12)],
            destructibles: vec![(ObsidianSpire, 6, 14)],
            bug_variant: Some(BugVariant::MagmaBug),
            variant_chance: 0.28,
        },
        BiomeType::Frozen => BiomeFeatureTable {
            landmarks: vec![
                (IcePillar, 35, 65),       // Arctic: ice formations everywhere
                (FrozenLake, 8, 18),       // Frozen tundra lakes
                (GlacialRidge, 16, 35),    // Wind-sculpted ice ridges
                (CaveEntrance, 0, 6),
            ],
            hazards: vec![(Blizzard, 2, 6), (IceCrack, 4, 10)],
            destructibles: vec![(IcePillar, 8, 18)],
            bug_variant: Some(BugVariant::FrostBug),
            variant_chance: 0.24,
        },
        BiomeType::Toxic => BiomeFeatureTable {
            landmarks: vec![
                (MutantGrowth, 30, 55),    // Chernobyl zone: twisted vegetation
                (GasVent, 16, 32),         // Toxic fumes everywhere
                (AcidGeyser, 10, 22),      // Corrosive geysers
                (CaveEntrance, 0, 5),
            ],
            hazards: vec![(PoisonGas, 8, 18)],
            destructibles: vec![(GasVent, 6, 14)],
            bug_variant: Some(BugVariant::ToxicSpitter),
            variant_chance: 0.26,
        },
        BiomeType::Mountain => BiomeFeatureTable {
            landmarks: vec![
                (BoulderField, 14, 28),    // Alpine: talus slopes, scree
                (CliffSpire, 12, 25),      // Jagged peaks
                (WaterfallCliff, 4, 12),   // Meltwater cascades
                (CaveEntrance, 6, 14),
            ],
            hazards: vec![(Avalanche, 4, 10)],
            destructibles: vec![(BoulderField, 8, 18)],
            bug_variant: Some(BugVariant::CliffCrawler),
            variant_chance: 0.2,
        },
        BiomeType::Swamp => BiomeFeatureTable {
            landmarks: vec![
                (DeadTree, 35, 65),        // Bayou/Everglades: drowned cypress everywhere
                (FogBank, 10, 22),         // Thick mist
                (MuddyPool, 16, 35),       // Murky water holes
                (CaveEntrance, 0, 5),
            ],
            hazards: vec![(Quicksand, 6, 14), (Leeches, 4, 10)],
            destructibles: vec![(DeadTree, 8, 18)],
            bug_variant: Some(BugVariant::SwampLurker),
            variant_chance: 0.22,
        },
        BiomeType::Crystalline => BiomeFeatureTable {
            landmarks: vec![
                (CrystalPillar, 40, 75),   // Alien crystal cave: dense forest
                (PrismaticPool, 10, 22),   // Reflective mineral pools
                (MirrorShard, 25, 50),     // Scattered reflective shards
                (CaveEntrance, 0, 6),
            ],
            hazards: vec![(CrystalResonance, 4, 10)],
            destructibles: vec![(CrystalPillar, 10, 22)],
            bug_variant: Some(BugVariant::ShardBug),
            variant_chance: 0.18,
        },
        BiomeType::Ashlands => BiomeFeatureTable {
            landmarks: vec![
                (EmberMound, 16, 32),      // Pompeii: smoldering ash, ruins
                (CollapsedRuin, 6, 16),     // Buried structures
                (AshDrift, 14, 28),        // Deep ash drifts
                (CaveEntrance, 0, 5),
            ],
            hazards: vec![(EmberStorm, 4, 12)],
            destructibles: vec![(EmberMound, 8, 18)],
            bug_variant: Some(BugVariant::AshStalker),
            variant_chance: 0.25,
        },
        BiomeType::Jungle => BiomeFeatureTable {
            landmarks: vec![
                (GiantAlienTree, 35, 65),   // Dense canopy — Vietnam/Minecraft jungle
                (VineWall, 55, 100),        // Vines everywhere
                (BioluminescentFlower, 70, 130),  // Lush understory
                (CaveEntrance, 0, 2),
            ],
            hazards: vec![(CarnivorousPlant, 10, 22)],  // More danger in dense jungle
            destructibles: vec![(GiantAlienTree, 8, 18)],
            bug_variant: Some(BugVariant::JungleLeaper),
            variant_chance: 0.24,
        },
        BiomeType::Wasteland => BiomeFeatureTable {
            landmarks: vec![
                (RustedVehicle, 10, 22),   // Post-apocalyptic: wreckage everywhere
                (RadiationCrater, 8, 18),  // Bomb craters, fallout zones
                (TwistedRebar, 14, 28),    // Collapsed structures
                (CaveEntrance, 4, 10),
            ],
            hazards: vec![(RadiationZone, 6, 14)],
            destructibles: vec![(RustedVehicle, 4, 12)],
            bug_variant: Some(BugVariant::Irradiated),
            variant_chance: 0.2,
        },
        BiomeType::Tundra => BiomeFeatureTable {
            landmarks: vec![
                (IcePillar, 22, 45),
                (FrozenLake, 10, 22),
                (GlacialRidge, 14, 30),
                (CaveEntrance, 0, 6),
            ],
            hazards: vec![(Blizzard, 3, 8), (IceCrack, 4, 10)],
            destructibles: vec![(IcePillar, 6, 16)],
            bug_variant: Some(BugVariant::FrostBug),
            variant_chance: 0.22,
        },
        BiomeType::SaltFlat => BiomeFeatureTable {
            landmarks: vec![
                (SandDuneRidge, 12, 28),
                (DriedRavine, 20, 45),
                (CanyonWall, 8, 18),
                (CaveEntrance, 0, 4),
            ],
            hazards: vec![(Sandstorm, 3, 8)],
            destructibles: vec![(RockArch, 2, 8)],
            bug_variant: Some(BugVariant::Burrower),
            variant_chance: 0.2,
        },
        BiomeType::Storm => BiomeFeatureTable {
            landmarks: vec![
                (FogBank, 25, 50),
                (WaterfallCliff, 8, 20),
                (MuddyPool, 12, 28),
                (CaveEntrance, 4, 12),
            ],
            hazards: vec![(Blizzard, 6, 14), (Quicksand, 4, 10)],
            destructibles: vec![(BoulderField, 4, 12)],
            bug_variant: Some(BugVariant::SwampLurker),
            variant_chance: 0.24,
        },
        BiomeType::Fungal => BiomeFeatureTable {
            landmarks: vec![
                (MutantGrowth, 40, 75),
                (VineWall, 35, 65),
                (BioluminescentFlower, 50, 95),
                (CaveEntrance, 2, 8),
            ],
            hazards: vec![(SporeBurst, 10, 22), (PoisonGas, 6, 14)],
            destructibles: vec![(MutantGrowth, 10, 22)],
            bug_variant: Some(BugVariant::JungleLeaper),
            variant_chance: 0.26,
        },
        BiomeType::Scorched => BiomeFeatureTable {
            landmarks: vec![
                (EmberMound, 20, 40),
                (AshDrift, 18, 36),
                (CollapsedRuin, 10, 22),
                (CaveEntrance, 2, 8),
            ],
            hazards: vec![(EmberStorm, 6, 14), (LavaFlow, 2, 8)],
            destructibles: vec![(EmberMound, 8, 18)],
            bug_variant: Some(BugVariant::AshStalker),
            variant_chance: 0.23,
        },
        BiomeType::Ruins => BiomeFeatureTable {
            landmarks: vec![
                (CollapsedRuin, 25, 50),
                (RockArch, 14, 30),
                (CaveEntrance, 12, 28),
            ],
            hazards: vec![(Rockslide, 5, 12), (RadiationZone, 3, 8)],
            destructibles: vec![(CollapsedRuin, 6, 16)],
            bug_variant: Some(BugVariant::AmbushWarrior),
            variant_chance: 0.22,
        },
    }
}
