//! Transform component and utilities for spatial positioning.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};

/// A 3D transform representing position, rotation, and scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    /// Create a new transform at the given position.
    pub fn from_position(position: Vec3) -> Self {
        Self {
            position,
            ..Default::default()
        }
    }

    /// Create a new transform with position and rotation.
    pub fn from_position_rotation(position: Vec3, rotation: Quat) -> Self {
        Self {
            position,
            rotation,
            ..Default::default()
        }
    }

    /// Create the model matrix for this transform.
    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }

    /// Get the forward direction (negative Z in right-handed coordinates).
    pub fn forward(&self) -> Vec3 {
        self.rotation * -Vec3::Z
    }

    /// Get the right direction (positive X).
    pub fn right(&self) -> Vec3 {
        self.rotation * Vec3::X
    }

    /// Get the up direction (positive Y).
    pub fn up(&self) -> Vec3 {
        self.rotation * Vec3::Y
    }

    /// Translate the transform by a delta.
    pub fn translate(&mut self, delta: Vec3) {
        self.position += delta;
    }

    /// Rotate around the Y axis (yaw).
    pub fn rotate_y(&mut self, angle: f32) {
        self.rotation = Quat::from_rotation_y(angle) * self.rotation;
    }

    /// Rotate around the local X axis (pitch).
    pub fn rotate_x(&mut self, angle: f32) {
        self.rotation = self.rotation * Quat::from_rotation_x(angle);
    }

    /// Look at a target position.
    pub fn look_at(&mut self, target: Vec3, up: Vec3) {
        let forward = (target - self.position).normalize();
        if forward.length_squared() > 0.0001 {
            self.rotation = Quat::from_mat4(&Mat4::look_at_rh(self.position, target, up)).inverse();
        }
    }
}

/// Raw transform data for GPU upload (instance data).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct TransformRaw {
    pub model: [[f32; 4]; 4],
}

impl From<&Transform> for TransformRaw {
    fn from(transform: &Transform) -> Self {
        Self {
            model: transform.to_matrix().to_cols_array_2d(),
        }
    }
}

impl From<Transform> for TransformRaw {
    fn from(transform: Transform) -> Self {
        Self::from(&transform)
    }
}
