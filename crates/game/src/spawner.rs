//! Infinite horde bug spawning system.
//!
//! Planet danger level (1–10) sets base bug count and mix: higher danger =
//! more bugs allowed and nastier types from the start. Difficulty also
//! escalates over time survived — spawn rate and max bugs rise, and the
//! composition shifts toward more lethal variants the longer the trooper
//! holds the line.

use engine_core::Vec3;
use hecs::World;
use rand::prelude::*;

use crate::bug::{BugBundle, BugType, BugVariant};

/// Manages continuous, ever-escalating bug spawning.
pub struct BugSpawner {
    // ── Core spawn timing ───────────────────────────────────────────────
    /// Base spawn rate (bugs per second at difficulty 0).
    base_spawn_rate: f32,
    /// Current effective spawn rate (escalates over time).
    pub spawn_rate: f32,
    /// Accumulator for spawn timing.
    pub spawn_timer: f32,

    // ── Spawn geometry ──────────────────────────────────────────────────
    /// Minimum distance from player to spawn.
    pub min_spawn_distance: f32,
    /// Maximum distance from player to spawn.
    pub max_spawn_distance: f32,

    // ── Horde pressure ──────────────────────────────────────────────────
    /// Current max bugs alive at once (grows with difficulty).
    pub max_bugs: usize,
    /// Hard ceiling – max_bugs will never exceed this.
    pub max_bugs_cap: usize,
    /// Base max bugs at difficulty 0.
    base_max_bugs: usize,

    // ── Difficulty escalation ───────────────────────────────────────────
    /// Planet danger (1–10). Higher = more max bugs and nastier mix from the start.
    planet_danger: f32,
    /// Time survived on this planet (seconds).
    pub time_survived: f32,
    /// Current difficulty level (time_survived / 60). Drives time-based scaling.
    pub difficulty: f32,
    /// Threat level name for HUD display.
    pub threat_level: ThreatLevel,

    /// Biome-specific bug variant (set when landing on a planet).
    pub biome_variant: Option<BugVariant>,
    /// Probability (0.0–1.0) that a spawned bug is the biome variant.
    pub variant_chance: f32,

    /// Random number generator.
    rng: StdRng,
}

/// Named threat tiers shown on the HUD — pure flavour, driven by difficulty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreatLevel {
    /// 0-2 min: light resistance
    Minimal,
    /// 2-5 min: standard engagement
    Moderate,
    /// 5-8 min: heavy contact
    Elevated,
    /// 8-12 min: sustained assault
    Severe,
    /// 12-18 min: overwhelming force
    Critical,
    /// 18+ min: extinction-level horde
    Extinction,
}

impl ThreatLevel {
    pub fn from_difficulty(d: f32) -> Self {
        match d {
            d if d < 2.0 => ThreatLevel::Minimal,
            d if d < 5.0 => ThreatLevel::Moderate,
            d if d < 8.0 => ThreatLevel::Elevated,
            d if d < 12.0 => ThreatLevel::Severe,
            d if d < 18.0 => ThreatLevel::Critical,
            _ => ThreatLevel::Extinction,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ThreatLevel::Minimal => "MINIMAL",
            ThreatLevel::Moderate => "MODERATE",
            ThreatLevel::Elevated => "ELEVATED",
            ThreatLevel::Severe => "SEVERE",
            ThreatLevel::Critical => "CRITICAL",
            ThreatLevel::Extinction => "EXTINCTION",
        }
    }

    pub fn color(&self) -> [f32; 4] {
        match self {
            ThreatLevel::Minimal => [0.3, 1.0, 0.3, 1.0],    // green
            ThreatLevel::Moderate => [0.7, 1.0, 0.3, 1.0],   // yellow-green
            ThreatLevel::Elevated => [1.0, 1.0, 0.2, 1.0],   // yellow
            ThreatLevel::Severe => [1.0, 0.6, 0.1, 1.0],     // orange
            ThreatLevel::Critical => [1.0, 0.2, 0.1, 1.0],   // red
            ThreatLevel::Extinction => [0.8, 0.0, 0.0, 1.0], // deep red
        }
    }
}

impl BugSpawner {
    /// Create a spawner for a planet. `base_spawn_rate` is from `planet.bug_spawn_rate()` (already scales with danger).
    /// `danger_level` is planet danger 1–10: it sets how many bugs can be alive at once and how nasty the mix is from the start.
    /// Tuned for movie/2005 game horde scale + Starship Troopers Extermination intensity.
    pub fn new(base_spawn_rate: f32, danger_level: u32) -> Self {
        let danger = danger_level.clamp(1, 10) as f32;
        // Movie/2005 game scale: massive swarms (600–1500 base, 1300–4000 cap)
        let base_max_bugs = 500 + (danger_level as usize).min(10) * 100;   // danger 1 → 600, 10 → 1500
        let max_bugs_cap = 1000 + (danger_level as usize).min(10) * 300;   // danger 1 → 1300, 10 → 4000
        Self {
            base_spawn_rate,
            spawn_rate: base_spawn_rate,
            spawn_timer: 0.0,
            // Extermination intensity: tighter spawn ring = bugs appear closer, more immediate pressure
            min_spawn_distance: 18.0,
            max_spawn_distance: 55.0,
            max_bugs: base_max_bugs,
            max_bugs_cap,
            base_max_bugs,
            planet_danger: danger,
            time_survived: 0.0,
            difficulty: 0.0,
            threat_level: ThreatLevel::Minimal,
            biome_variant: None,
            variant_chance: 0.0,
            rng: StdRng::from_entropy(),
        }
    }

    /// Set biome variant and chance (call when landing on a planet).
    pub fn set_biome_variant(&mut self, variant: Option<BugVariant>, chance: f32) {
        self.biome_variant = variant;
        self.variant_chance = chance.clamp(0.0, 1.0);
    }

    /// Tick the difficulty clock and update derived values.
    pub fn update_difficulty(&mut self, dt: f32) {
        self.time_survived += dt;
        self.difficulty = self.time_survived / 60.0; // +1 per minute

        // Spawn rate: +20% per difficulty level (Extermination-style escalation)
        self.spawn_rate = self.base_spawn_rate * (1.0 + self.difficulty * 0.20);

        // Max bugs: base + 80 per difficulty level, capped (movie-scale horde growth)
        self.max_bugs = (self.base_max_bugs + (self.difficulty * 80.0) as usize)
            .min(self.max_bugs_cap);

        // Threat level (display only)
        self.threat_level = ThreatLevel::from_difficulty(self.difficulty);
    }

    /// Get a random bug type and optional biome variant, weighted by difficulty and variant_chance.
    /// Planet danger is added so high-danger planets get nastier mix from the start.
    pub fn random_bug_type(&mut self) -> (BugType, Option<BugVariant>) {
        let d = self.difficulty + self.planet_danger * 0.6; // e.g. danger 10 = +6 effective difficulty
        let roll = self.rng.gen::<f32>();

        let bug_type = if d < 2.0 {
            if roll < 0.85 { BugType::Warrior }
            else { BugType::Charger }
        } else if d < 5.0 {
            if roll < 0.55 { BugType::Warrior }
            else if roll < 0.80 { BugType::Charger }
            else { BugType::Hopper }
        } else if d < 8.0 {
            if roll < 0.35 { BugType::Warrior }
            else if roll < 0.55 { BugType::Charger }
            else if roll < 0.75 { BugType::Spitter }
            else { BugType::Hopper }
        } else if d < 12.0 {
            if roll < 0.25 { BugType::Warrior }
            else if roll < 0.45 { BugType::Charger }
            else if roll < 0.60 { BugType::Spitter }
            else if roll < 0.80 { BugType::Hopper }
            else { BugType::Tanker }
        } else {
            if roll < 0.15 { BugType::Warrior }
            else if roll < 0.30 { BugType::Charger }
            else if roll < 0.50 { BugType::Spitter }
            else if roll < 0.70 { BugType::Hopper }
            else { BugType::Tanker }
        };

        let variant = if self.rng.gen::<f32>() < self.variant_chance {
            self.biome_variant
        } else {
            None
        };
        (bug_type, variant)
    }

    /// Spawn a group of bugs at a position (for bug holes).
    pub fn spawn_group(&mut self, world: &mut World, center: Vec3, count: usize, bug_type: BugType) {
        for i in 0..count {
            let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
            let offset = Vec3::new(angle.cos() * 2.0, 0.0, angle.sin() * 2.0);
            BugBundle::new(bug_type, center + offset).spawn(world);
        }
    }

    /// Get a spawn interval scaling factor for bug holes (they speed up too).
    /// Returns a multiplier < 1.0 so holes spawn faster over time.
    pub fn hole_spawn_rate_multiplier(&self) -> f32 {
        // Holes speed up ~8% per difficulty level, floor at 0.25x interval
        (1.0 / (1.0 + self.difficulty * 0.08)).max(0.25)
    }

    /// Set base spawn rate (e.g. when changing planets).
    pub fn set_spawn_rate(&mut self, rate: f32) {
        self.base_spawn_rate = rate;
    }

    /// Format time survived as MM:SS.
    pub fn time_survived_str(&self) -> String {
        let mins = (self.time_survived / 60.0) as u32;
        let secs = (self.time_survived % 60.0) as u32;
        format!("{:02}:{:02}", mins, secs)
    }
}
