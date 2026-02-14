//! Skinny enemies (Heinlein Starship Troopers): humanoid aliens on some planets.

use engine_core::{AIComponent, Health, Transform, Vec3};

/// Skinny type — different stats and behavior (ranged vs melee).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkinnyType {
    /// Basic infantry, melee/close range.
    Grunt,
    /// Ranged, keeps distance.
    Sniper,
    /// Tougher, leads groups.
    Officer,
}

impl SkinnyType {
    pub fn health(&self) -> f32 {
        match self {
            SkinnyType::Grunt => 40.0,
            SkinnyType::Sniper => 30.0,
            SkinnyType::Officer => 70.0,
        }
    }

    pub fn attack_damage(&self) -> f32 {
        match self {
            SkinnyType::Grunt => 12.0,
            SkinnyType::Sniper => 8.0,
            SkinnyType::Officer => 18.0,
        }
    }

    pub fn move_speed(&self) -> f32 {
        match self {
            SkinnyType::Grunt => 5.0,
            SkinnyType::Sniper => 5.5,
            SkinnyType::Officer => 4.5,
        }
    }

    /// Scale for rendering (humanoid: taller than wide).
    pub fn scale(&self) -> Vec3 {
        match self {
            SkinnyType::Grunt => Vec3::new(0.35, 1.0, 0.25),
            SkinnyType::Sniper => Vec3::new(0.3, 0.95, 0.22),
            SkinnyType::Officer => Vec3::new(0.45, 1.15, 0.35),
        }
    }

    /// Color [r, g, b, a] — Skinnies are grey-green humanoids.
    pub fn color(&self) -> [f32; 4] {
        match self {
            SkinnyType::Grunt => [0.45, 0.52, 0.42, 1.0],
            SkinnyType::Sniper => [0.40, 0.48, 0.38, 1.0],
            SkinnyType::Officer => [0.38, 0.44, 0.35, 1.0],
        }
    }
}

/// Skinny enemy component (like Bug but for humanoid aliens).
#[derive(Debug, Clone)]
pub struct Skinny {
    pub skinny_type: SkinnyType,
    pub attack_damage: f32,
    pub move_speed: f32,
}

impl Skinny {
    pub fn new(skinny_type: SkinnyType) -> Self {
        Self {
            attack_damage: skinny_type.attack_damage(),
            move_speed: skinny_type.move_speed(),
            skinny_type,
        }
    }

    pub fn effective_health(&self) -> f32 {
        self.skinny_type.health()
    }
}
