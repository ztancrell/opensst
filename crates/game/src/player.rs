//! Player controller and state.

use engine_core::{Transform, Vec3};
use input::InputState;
use renderer::Camera;

use crate::weapons::{Weapon, WeaponType};

/// Player controller handling movement and camera.
pub struct PlayerController {
    pub transform: Transform,
    pub velocity: Vec3,
    pub move_speed: f32,
    pub sprint_multiplier: f32,
    pub jump_force: f32,
    pub gravity: f32,
    pub is_grounded: bool,
    pub current_weapon: Weapon,
    pub health: f32,
    pub max_health: f32,
}

impl PlayerController {
    pub fn new(position: Vec3) -> Self {
        Self {
            transform: Transform::from_position(position),
            velocity: Vec3::ZERO,
            move_speed: 8.0,
            sprint_multiplier: 1.6,
            jump_force: 8.0,
            gravity: 20.0,
            is_grounded: false,
            current_weapon: Weapon::new(WeaponType::Rifle),
            health: 100.0,
            max_health: 100.0,
        }
    }

    /// Update player state based on input.
    pub fn update(&mut self, input: &InputState, camera: &mut Camera, dt: f32) {
        // Mouse look
        let mouse_delta = input.mouse_delta();
        if input.is_cursor_locked() {
            camera.process_mouse(mouse_delta.x, mouse_delta.y);
        }

        // Movement
        let movement = input.get_movement_input();
        let speed = if input.is_sprinting() {
            self.move_speed * self.sprint_multiplier
        } else {
            self.move_speed
        };

        // Apply vertical movement (gravity/jump)
        if self.is_grounded {
            self.velocity.y = 0.0;
            if input.is_jump_pressed() {
                self.velocity.y = self.jump_force;
                self.is_grounded = false;
            }
        } else {
            self.velocity.y -= self.gravity * dt;
        }

        // Move camera based on input
        let vertical = self.velocity.y * dt;
        camera.process_movement(movement, vertical, speed, dt);

        // Sync player position with camera
        self.transform.position = camera.transform.position;

        // Simple ground check (would use physics in full implementation)
        if self.transform.position.y <= 1.5 {
            self.transform.position.y = 1.5;
            camera.transform.position.y = 1.5;
            self.is_grounded = true;
        }

        // Update weapon
        self.current_weapon.update(dt);
    }

    /// Get player position.
    pub fn position(&self) -> Vec3 {
        self.transform.position
    }

    /// Get current weapon reference.
    pub fn current_weapon(&self) -> &Weapon {
        &self.current_weapon
    }

    /// Get mutable weapon reference.
    pub fn current_weapon_mut(&mut self) -> &mut Weapon {
        &mut self.current_weapon
    }

    /// Take damage.
    pub fn take_damage(&mut self, amount: f32) {
        self.health = (self.health - amount).max(0.0);
    }

    /// Check if player is dead.
    pub fn is_dead(&self) -> bool {
        self.health <= 0.0
    }

    /// Heal the player.
    pub fn heal(&mut self, amount: f32) {
        self.health = (self.health + amount).min(self.max_health);
    }
}
