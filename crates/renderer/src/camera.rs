//! Camera system for FPS view.

use bytemuck::{Pod, Zeroable};
use engine_core::Transform;
use glam::{Mat4, Vec3};

/// FPS camera with configurable FOV and clipping planes.
#[derive(Debug, Clone)]
pub struct Camera {
    /// Camera transform (position and rotation).
    pub transform: Transform,
    /// Field of view in degrees.
    pub fov_degrees: f32,
    /// Near clipping plane.
    pub near: f32,
    /// Far clipping plane.
    pub far: f32,
    /// Aspect ratio (width / height).
    pub aspect: f32,
    /// Mouse sensitivity for look controls.
    pub sensitivity: f32,
    /// Current pitch (up/down rotation) in radians.
    pitch: f32,
    /// Current yaw (left/right rotation) in radians.
    yaw: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            transform: Transform::default(),
            fov_degrees: 70.0,
            near: 0.1,
            far: 1000.0,
            aspect: 16.0 / 9.0,
            sensitivity: 0.002,
            pitch: 0.0,
            yaw: 0.0,
        }
    }
}

impl Camera {
    /// Create a new camera at the given position.
    pub fn new(position: Vec3) -> Self {
        Self {
            transform: Transform::from_position(position),
            ..Default::default()
        }
    }

    /// Update aspect ratio (call on window resize).
    pub fn set_aspect(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height.max(1) as f32;
    }

    /// Process mouse movement for FPS look controls.
    pub fn process_mouse(&mut self, delta_x: f32, delta_y: f32) {
        self.yaw -= delta_x * self.sensitivity;
        self.pitch -= delta_y * self.sensitivity;

        // Clamp pitch to prevent flipping
        let max_pitch = std::f32::consts::FRAC_PI_2 - 0.01;
        self.pitch = self.pitch.clamp(-max_pitch, max_pitch);

        // Update rotation from pitch and yaw
        self.transform.rotation =
            glam::Quat::from_rotation_y(self.yaw) * glam::Quat::from_rotation_x(self.pitch);
    }

    /// Move the camera in world space based on input (FPS style: horizontal plane + vertical).
    pub fn process_movement(&mut self, input: glam::Vec2, vertical: f32, speed: f32, dt: f32) {
        let forward = self.transform.forward();
        let right = self.transform.right();

        // Project forward onto horizontal plane for FPS movement
        let forward_flat = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
        let right_flat = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();

        let mut velocity = Vec3::ZERO;
        velocity += forward_flat * input.y;
        velocity += right_flat * input.x;
        velocity.y += vertical;

        if velocity.length_squared() > 0.0 {
            velocity = velocity.normalize() * speed * dt;
            self.transform.translate(velocity);
        }
    }

    /// Noclip free-fly: move in camera space (forward/right/up). No gravity or collision.
    /// - move_xy: x = strafe, y = forward/back (from WASD)
    /// - move_y: vertical (e.g. +1 space, -1 ctrl)
    pub fn process_fly(&mut self, move_xy: glam::Vec2, move_y: f32, speed: f32, dt: f32) {
        let forward = self.transform.forward();
        let right = self.transform.right();
        let up = self.transform.up();

        let mut velocity = Vec3::ZERO;
        velocity += forward * move_xy.y;
        velocity += right * move_xy.x;
        velocity += up * move_y;

        if velocity.length_squared() > 0.0 {
            velocity = velocity.normalize() * speed * dt;
            self.transform.translate(velocity);
        }
    }

    /// Get the view matrix.
    pub fn view_matrix(&self) -> Mat4 {
        let eye = self.transform.position;
        let target = eye + self.transform.forward();
        let up = Vec3::Y;
        Mat4::look_at_rh(eye, target, up)
    }

    /// Get the projection matrix.
    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_degrees.to_radians(), self.aspect, self.near, self.far)
    }

    /// Get the combined view-projection matrix.
    pub fn view_projection_matrix(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// View matrix for viewmodel pass: camera at origin, same rotation.
    /// Use so the gun is drawn in view space and no world geometry can appear in front of it.
    pub fn view_matrix_viewmodel(&self) -> Mat4 {
        Mat4::from_quat(self.transform.rotation).inverse()
    }

    /// View-projection for viewmodel pass (camera at origin).
    pub fn view_projection_matrix_viewmodel(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix_viewmodel()
    }

    /// Get camera position.
    pub fn position(&self) -> Vec3 {
        self.transform.position
    }

    /// Get camera forward direction.
    pub fn forward(&self) -> Vec3 {
        self.transform.forward()
    }

    /// Get current yaw (left/right rotation) in radians.
    pub fn yaw(&self) -> f32 {
        self.yaw
    }

    /// Get current pitch (up/down rotation) in radians.
    pub fn pitch(&self) -> f32 {
        self.pitch
    }

    /// Set yaw and pitch directly (in radians) and rebuild rotation.
    pub fn set_yaw_pitch(&mut self, yaw: f32, pitch: f32) {
        self.yaw = yaw;
        let max_pitch = std::f32::consts::FRAC_PI_2 - 0.01;
        self.pitch = pitch.clamp(-max_pitch, max_pitch);
        self.transform.rotation =
            glam::Quat::from_rotation_y(self.yaw) * glam::Quat::from_rotation_x(self.pitch);
    }

    /// Get camera right direction.
    pub fn right(&self) -> Vec3 {
        self.transform.right()
    }
}

/// Camera uniform data for GPU.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub view: [[f32; 4]; 4],
    pub proj: [[f32; 4]; 4],
    pub position: [f32; 4], // w unused, padding
    /// Planet radius for curvature (d^2/2R). 0 = no curvature.
    pub planet_radius: f32,
    /// Padding to match WGSL std140 layout (vec3 alignment + struct size multiple of 16).
    pub _pad: [f32; 7],
}

impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            view: Mat4::IDENTITY.to_cols_array_2d(),
            proj: Mat4::IDENTITY.to_cols_array_2d(),
            position: [0.0; 4],
            planet_radius: 0.0,
            _pad: [0.0; 7],
        }
    }

    pub fn update(&mut self, camera: &Camera, planet_radius: f32) {
        self.view = camera.view_matrix().to_cols_array_2d();
        self.proj = camera.projection_matrix().to_cols_array_2d();
        self.view_proj = camera.view_projection_matrix().to_cols_array_2d();
        let pos = camera.position();
        self.position = [pos.x, pos.y, pos.z, 1.0];
        self.planet_radius = planet_radius;
    }

    /// Set uniform for viewmodel pass: view/proj with camera at origin so viewmodel is in view space.
    pub fn update_viewmodel(&mut self, camera: &Camera) {
        self.view = camera.view_matrix_viewmodel().to_cols_array_2d();
        self.proj = camera.projection_matrix().to_cols_array_2d();
        self.view_proj = camera.view_projection_matrix_viewmodel().to_cols_array_2d();
        self.position = [0.0, 0.0, 0.0, 1.0];
        self.planet_radius = 0.0; // No curvature for viewmodel (in view space)
    }
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self::new()
    }
}
