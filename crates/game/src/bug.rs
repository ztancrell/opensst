//! Bug enemy types and components.

use engine_core::{AIComponent, AIState, Health, Transform, Velocity, Vec3};

/// Biome-specific bug variant (one per biome). Affects stats, color, and on-death behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BugVariant {
    Burrower,      // Desert: emerges from ground
    AmbushWarrior, // Badlands: plays dead, springs
    BroodMother,   // HiveWorld: spawns mini-bugs on death
    MagmaBug,      // Volcanic: leaves fire trail
    FrostBug,      // Frozen: slows player on hit
    ToxicSpitter, // Toxic: enhanced, leaves acid pool
    CliffCrawler,  // Mountain: fast on slopes, armored
    SwampLurker,   // Swamp: cloaked until close
    ShardBug,      // Crystalline: reflects some damage
    AshStalker,    // Ashlands: camo in ash, fast
    JungleLeaper,  // Jungle: enhanced Hopper, web slow
    Irradiated,    // Wasteland: glows, explodes on death
}

/// On-death effect for variant bugs (used by game logic to spawn hazards/mini-bugs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariantDeathEffect {
    None,
    SpawnMiniBugs,   // BroodMother: 3–5 mini Warriors
    Explosion,       // Irradiated: AoE damage
    FireHazard,      // MagmaBug: fire at death position
    SlowZone,        // FrostBug: slow zone at death position
    AcidPool,        // ToxicSpitter: acid pool at death position
}

impl BugVariant {
    /// Health multiplier for this variant (e.g. 1.2 = 20% more health).
    pub fn health_mult(&self) -> f32 {
        match self {
            BugVariant::Burrower => 1.0,
            BugVariant::AmbushWarrior => 0.9,
            BugVariant::BroodMother => 1.5,
            BugVariant::MagmaBug => 1.1,
            BugVariant::FrostBug => 0.95,
            BugVariant::ToxicSpitter => 1.2,
            BugVariant::CliffCrawler => 1.3,
            BugVariant::SwampLurker => 0.9,
            BugVariant::ShardBug => 1.0,
            BugVariant::AshStalker => 0.85,
            BugVariant::JungleLeaper => 1.0,
            BugVariant::Irradiated => 1.1,
        }
    }

    /// Speed multiplier.
    pub fn speed_mult(&self) -> f32 {
        match self {
            BugVariant::Burrower => 0.9,
            BugVariant::AmbushWarrior => 1.2,
            BugVariant::BroodMother => 0.7,
            BugVariant::MagmaBug => 1.0,
            BugVariant::FrostBug => 1.0,
            BugVariant::ToxicSpitter => 1.1,
            BugVariant::CliffCrawler => 1.25,
            BugVariant::SwampLurker => 1.15,
            BugVariant::ShardBug => 1.0,
            BugVariant::AshStalker => 1.3,
            BugVariant::JungleLeaper => 1.2,
            BugVariant::Irradiated => 1.0,
        }
    }

    /// Damage multiplier.
    pub fn damage_mult(&self) -> f32 {
        match self {
            BugVariant::Burrower => 1.0,
            BugVariant::AmbushWarrior => 1.3,
            BugVariant::BroodMother => 0.8,
            BugVariant::MagmaBug => 1.2,
            BugVariant::FrostBug => 1.0,
            BugVariant::ToxicSpitter => 1.3,
            BugVariant::CliffCrawler => 1.1,
            BugVariant::SwampLurker => 1.2,
            BugVariant::ShardBug => 1.0,
            BugVariant::AshStalker => 1.0,
            BugVariant::JungleLeaper => 1.1,
            BugVariant::Irradiated => 1.0,
        }
    }

    /// Color tint [r, g, b] applied to base bug color (alpha unchanged). 1.0 = no change.
    pub fn color_tint(&self) -> [f32; 3] {
        match self {
            BugVariant::Burrower => [0.95, 0.9, 0.85],      // sandy
            BugVariant::AmbushWarrior => [0.9, 0.75, 0.7],  // red-brown
            BugVariant::BroodMother => [1.1, 0.85, 0.9],   // fleshy
            BugVariant::MagmaBug => [1.4, 0.6, 0.2],       // orange-red
            BugVariant::FrostBug => [0.6, 0.75, 1.2],      // blue
            BugVariant::ToxicSpitter => [0.6, 1.2, 0.5],   // green
            BugVariant::CliffCrawler => [0.85, 0.8, 0.85], // grey
            BugVariant::SwampLurker => [0.7, 0.8, 0.6],    // murky green
            BugVariant::ShardBug => [0.9, 0.95, 1.3],      // crystalline
            BugVariant::AshStalker => [0.6, 0.55, 0.5],   // ash grey
            BugVariant::JungleLeaper => [0.7, 0.9, 0.6],   // jungle green
            BugVariant::Irradiated => [0.4, 1.2, 0.3],     // sick green glow
        }
    }

    /// Effect when this variant is killed.
    pub fn death_effect(&self) -> VariantDeathEffect {
        match self {
            BugVariant::Burrower => VariantDeathEffect::None,
            BugVariant::AmbushWarrior => VariantDeathEffect::None,
            BugVariant::BroodMother => VariantDeathEffect::SpawnMiniBugs,
            BugVariant::MagmaBug => VariantDeathEffect::FireHazard,
            BugVariant::FrostBug => VariantDeathEffect::SlowZone,
            BugVariant::ToxicSpitter => VariantDeathEffect::AcidPool,
            BugVariant::CliffCrawler => VariantDeathEffect::None,
            BugVariant::SwampLurker => VariantDeathEffect::None,
            BugVariant::ShardBug => VariantDeathEffect::None,
            BugVariant::AshStalker => VariantDeathEffect::None,
            BugVariant::JungleLeaper => VariantDeathEffect::None,
            BugVariant::Irradiated => VariantDeathEffect::Explosion,
        }
    }
}

/// Bug enemy component.
#[derive(Debug, Clone)]
pub struct Bug {
    pub bug_type: BugType,
    pub attack_damage: f32,
    pub move_speed: f32,
    /// Biome-specific variant (affects stats, color, death effect).
    pub variant: Option<BugVariant>,
}

impl Bug {
    pub fn new(bug_type: BugType) -> Self {
        Self::new_with_variant(bug_type, None)
    }

    pub fn new_with_variant(bug_type: BugType, variant: Option<BugVariant>) -> Self {
        let (attack_damage, move_speed) = match bug_type {
            BugType::Warrior => (15.0, 6.0),
            BugType::Charger => (25.0, 12.0),
            BugType::Spitter => (10.0, 4.0),
            BugType::Tanker => (30.0, 3.0),
            BugType::Hopper => (10.0, 8.0),
        };

        let mult_damage = variant.map(|v| v.damage_mult()).unwrap_or(1.0);
        let mult_speed = variant.map(|v| v.speed_mult()).unwrap_or(1.0);

        Self {
            bug_type,
            attack_damage: attack_damage * mult_damage,
            move_speed: move_speed * mult_speed,
            variant,
        }
    }

    /// Effective max health (base type health × variant multiplier).
    pub fn effective_health(&self) -> f32 {
        let base = self.bug_type.health();
        let mult = self.variant.map(|v| v.health_mult()).unwrap_or(1.0);
        base * mult
    }
}

/// Types of bugs with different behaviors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BugType {
    /// Standard melee warrior bug.
    Warrior,
    /// Fast charging bug.
    Charger,
    /// Ranged acid spitter.
    Spitter,
    /// Heavily armored tank bug.
    Tanker,
    /// Flying/jumping bug.
    Hopper,
}

impl BugType {
    /// Get the color for this bug type (chitinous STE-style: dark carapace, readable silhouettes).
    /// Renderer applies health factor (0.5–1.0) and variant tint.
    pub fn color(&self) -> [f32; 4] {
        match self {
            BugType::Warrior => [0.35, 0.28, 0.22, 1.0],   // Dark brown, reddish undertones
            BugType::Charger => [0.30, 0.24, 0.18, 1.0],   // Darker, matte
            BugType::Spitter => [0.28, 0.38, 0.22, 1.0],   // Dark body, greenish sac tint
            BugType::Tanker => [0.38, 0.34, 0.30, 1.0],    // Grey-brown, heavy plates
            BugType::Hopper => [0.32, 0.26, 0.22, 1.0],    // Similar to Warrior, angular
        }
    }

    /// Get the scale for this bug type.
    pub fn scale(&self) -> Vec3 {
        match self {
            BugType::Warrior => Vec3::splat(1.0),
            BugType::Charger => Vec3::new(0.8, 0.7, 1.2),
            BugType::Spitter => Vec3::splat(0.9),
            BugType::Tanker => Vec3::splat(2.0),
            BugType::Hopper => Vec3::new(0.7, 0.6, 0.7),
        }
    }

    /// Get the health for this bug type.
    pub fn health(&self) -> f32 {
        match self {
            BugType::Warrior => 50.0,
            BugType::Charger => 30.0,
            BugType::Spitter => 40.0,
            BugType::Tanker => 200.0,
            BugType::Hopper => 25.0,
        }
    }
}

/// Bundle of components for spawning a bug.
pub struct BugBundle {
    pub transform: Transform,
    pub velocity: Velocity,
    pub health: Health,
    pub bug: Bug,
    pub ai: AIComponent,
}

impl BugBundle {
    pub fn new(bug_type: BugType, position: Vec3) -> Self {
        Self::new_with_variant(bug_type, None, position)
    }

    pub fn new_with_variant(bug_type: BugType, variant: Option<BugVariant>, position: Vec3) -> Self {
        let bug = Bug::new_with_variant(bug_type, variant);
        let health = bug.effective_health();

        Self {
            transform: Transform {
                position,
                scale: bug_type.scale(),
                ..Default::default()
            },
            velocity: Velocity::default(),
            health: Health::new(health),
            bug,
            ai: AIComponent::new(85.0, 2.5, 1.0),  // Extermination: large aggro
        }
    }

    /// Spawn into the ECS world.
    pub fn spawn(self, world: &mut hecs::World) -> hecs::Entity {
        world.spawn((
            self.transform,
            self.velocity,
            self.health,
            self.bug,
            self.ai,
        ))
    }
}
