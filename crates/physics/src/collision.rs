//! Collision groups and filtering.

use rapier3d::prelude::*;

/// Collision groups for different entity types.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionGroup {
    /// Static environment (terrain, walls)
    Environment = 1 << 0,
    /// Player character
    Player = 1 << 1,
    /// Enemy bugs
    Enemy = 1 << 2,
    /// Player projectiles
    PlayerProjectile = 1 << 3,
    /// Enemy projectiles
    EnemyProjectile = 1 << 4,
    /// Debris and physics objects
    Debris = 1 << 5,
    /// Triggers and sensors
    Trigger = 1 << 6,
}

impl CollisionGroup {
    /// Create a collision group that collides with everything.
    pub fn all() -> Group {
        Group::ALL
    }

    /// Create a collision group for environment.
    pub fn environment() -> (Group, Group) {
        let membership = Group::from_bits_retain(Self::Environment as u32);
        let filter = Group::ALL;
        (membership, filter)
    }

    /// Create a collision group for player.
    pub fn player() -> (Group, Group) {
        let membership = Group::from_bits_retain(Self::Player as u32);
        let filter = Group::from_bits_retain(
            Self::Environment as u32 | Self::Enemy as u32 | Self::EnemyProjectile as u32,
        );
        (membership, filter)
    }

    /// Create a collision group for enemies.
    pub fn enemy() -> (Group, Group) {
        let membership = Group::from_bits_retain(Self::Enemy as u32);
        let filter = Group::from_bits_retain(
            Self::Environment as u32
                | Self::Player as u32
                | Self::PlayerProjectile as u32
                | Self::Enemy as u32,
        );
        (membership, filter)
    }

    /// Create a collision group for player projectiles.
    pub fn player_projectile() -> (Group, Group) {
        let membership = Group::from_bits_retain(Self::PlayerProjectile as u32);
        let filter = Group::from_bits_retain(Self::Environment as u32 | Self::Enemy as u32);
        (membership, filter)
    }

    /// Create a collision group for debris.
    pub fn debris() -> (Group, Group) {
        let membership = Group::from_bits_retain(Self::Debris as u32);
        let filter = Group::from_bits_retain(Self::Environment as u32 | Self::Debris as u32);
        (membership, filter)
    }
}

/// Component linking an ECS entity to its physics handles.
#[derive(Debug, Clone, Copy)]
pub struct PhysicsBody {
    pub rigid_body: RigidBodyHandle,
    pub collider: Option<ColliderHandle>,
}

impl PhysicsBody {
    pub fn new(rigid_body: RigidBodyHandle) -> Self {
        Self {
            rigid_body,
            collider: None,
        }
    }

    pub fn with_collider(rigid_body: RigidBodyHandle, collider: ColliderHandle) -> Self {
        Self {
            rigid_body,
            collider: Some(collider),
        }
    }
}
