//! Physics system using Rapier3D for OpenSST.

pub mod collision;
pub mod physics_world;
pub mod ragdoll;
pub mod raycast;

pub use collision::*;
pub use physics_world::*;
pub use ragdoll::*;
pub use raycast::*;

// Re-export Rapier for downstream crates
pub use rapier3d;

// Re-export common Rapier types
pub use rapier3d::prelude::{ColliderHandle, RigidBodyHandle, ImpulseJointSet};
