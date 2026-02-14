//! Core engine types and utilities for OpenSST.
//!
//! This crate provides the foundational types used across all engine systems:
//! - Transform and spatial components
//! - Time management
//! - Common component types for ECS

pub mod components;
pub mod time;
pub mod transform;

pub use components::*;
pub use time::*;
pub use transform::*;

// Re-export commonly used types
pub use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
pub use hecs::{Entity, World};
