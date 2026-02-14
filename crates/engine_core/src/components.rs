//! Common ECS components used across the engine.

use glam::Vec3;

/// Velocity component for moving entities.
#[derive(Debug, Clone, Copy, Default)]
pub struct Velocity {
    pub linear: Vec3,
    pub angular: Vec3,
}

impl Velocity {
    pub fn new(linear: Vec3) -> Self {
        Self {
            linear,
            angular: Vec3::ZERO,
        }
    }

    pub fn with_angular(linear: Vec3, angular: Vec3) -> Self {
        Self { linear, angular }
    }
}

/// Health component for damageable entities.
#[derive(Debug, Clone, Copy)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { current: max, max }
    }

    pub fn take_damage(&mut self, amount: f32) {
        self.current = (self.current - amount).max(0.0);
    }

    pub fn heal(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.max);
    }

    pub fn is_dead(&self) -> bool {
        self.current <= 0.0
    }

    pub fn percentage(&self) -> f32 {
        self.current / self.max
    }
}

impl Default for Health {
    fn default() -> Self {
        Self::new(100.0)
    }
}

/// Tag component for the player entity.
#[derive(Debug, Clone, Copy, Default)]
pub struct Player;

/// Tag component for bug enemies.
#[derive(Debug, Clone, Copy, Default)]
pub struct Bug;

/// AI state for enemies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AIState {
    #[default]
    Idle,
    Chasing,
    Attacking,
    Fleeing,
    Dead,
}

/// Component storing AI behavior state.
#[derive(Debug, Clone, Copy, Default)]
pub struct AIComponent {
    pub state: AIState,
    pub target: Option<hecs::Entity>,
    pub aggro_range: f32,
    pub attack_range: f32,
    pub attack_cooldown: f32,
    pub current_cooldown: f32,
}

impl AIComponent {
    pub fn new(aggro_range: f32, attack_range: f32, attack_cooldown: f32) -> Self {
        Self {
            state: AIState::Idle,
            target: None,
            aggro_range,
            attack_range,
            attack_cooldown,
            current_cooldown: 0.0,
        }
    }

    pub fn can_attack(&self) -> bool {
        self.current_cooldown <= 0.0
    }

    pub fn trigger_attack(&mut self) {
        self.current_cooldown = self.attack_cooldown;
    }

    pub fn update_cooldown(&mut self, dt: f32) {
        self.current_cooldown = (self.current_cooldown - dt).max(0.0);
    }
}

/// Mesh reference component - links entity to a mesh for rendering.
#[derive(Debug, Clone, Copy)]
pub struct MeshInstance {
    pub mesh_id: u32,
    pub material_id: u32,
}

impl MeshInstance {
    pub fn new(mesh_id: u32, material_id: u32) -> Self {
        Self { mesh_id, material_id }
    }
}

impl Default for MeshInstance {
    fn default() -> Self {
        Self {
            mesh_id: 0,
            material_id: 0,
        }
    }
}

/// Lifetime component for temporary entities (debris, projectiles, effects).
#[derive(Debug, Clone, Copy)]
pub struct Lifetime {
    pub remaining: f32,
}

impl Lifetime {
    pub fn new(seconds: f32) -> Self {
        Self { remaining: seconds }
    }

    pub fn update(&mut self, dt: f32) -> bool {
        self.remaining -= dt;
        self.remaining <= 0.0
    }
}

/// Damage component for projectiles and explosions.
#[derive(Debug, Clone, Copy)]
pub struct Damage {
    pub amount: f32,
    pub damage_type: DamageType,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DamageType {
    #[default]
    Bullet,
    Explosion,
    Melee,
    Fire,
}

impl Damage {
    pub fn bullet(amount: f32) -> Self {
        Self {
            amount,
            damage_type: DamageType::Bullet,
        }
    }

    pub fn explosion(amount: f32) -> Self {
        Self {
            amount,
            damage_type: DamageType::Explosion,
        }
    }
}
