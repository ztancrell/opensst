//! Input handling for keyboard, mouse, and gamepad.

use glam::Vec2;
use std::collections::HashSet;

/// Manages input state for the current frame.
#[derive(Debug, Default)]
pub struct InputState {
    /// Keys currently held down.
    keys_held: HashSet<KeyCode>,
    /// Keys pressed this frame.
    keys_pressed: HashSet<KeyCode>,
    /// Keys released this frame.
    keys_released: HashSet<KeyCode>,

    /// Mouse buttons currently held.
    mouse_held: HashSet<MouseButton>,
    /// Mouse buttons pressed this frame.
    mouse_pressed: HashSet<MouseButton>,
    /// Mouse buttons released this frame.
    mouse_released: HashSet<MouseButton>,

    /// Mouse position in window coordinates.
    mouse_position: Vec2,
    /// Mouse movement delta this frame.
    mouse_delta: Vec2,
    /// Accumulated mouse delta (for when cursor is locked).
    accumulated_delta: Vec2,

    /// Whether the cursor is captured/locked.
    cursor_locked: bool,

    /// Mouse scroll state
    scroll_up: bool,
    scroll_down: bool,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear per-frame state. Call at the start of each frame.
    pub fn begin_frame(&mut self) {
        self.keys_pressed.clear();
        self.keys_released.clear();
        self.mouse_pressed.clear();
        self.mouse_released.clear();
        self.mouse_delta = self.accumulated_delta;
        self.accumulated_delta = Vec2::ZERO;
        self.scroll_up = false;
        self.scroll_down = false;
    }

    /// Process a keyboard event.
    pub fn process_keyboard(&mut self, key: KeyCode, state: ElementState) {
        match state {
            ElementState::Pressed => {
                if !self.keys_held.contains(&key) {
                    self.keys_pressed.insert(key);
                }
                self.keys_held.insert(key);
            }
            ElementState::Released => {
                self.keys_held.remove(&key);
                self.keys_released.insert(key);
            }
        }
    }

    /// Process a mouse button event.
    pub fn process_mouse_button(&mut self, button: MouseButton, state: ElementState) {
        match state {
            ElementState::Pressed => {
                if !self.mouse_held.contains(&button) {
                    self.mouse_pressed.insert(button);
                }
                self.mouse_held.insert(button);
            }
            ElementState::Released => {
                self.mouse_held.remove(&button);
                self.mouse_released.insert(button);
            }
        }
    }

    /// Process mouse movement.
    pub fn process_mouse_motion(&mut self, delta: (f64, f64)) {
        self.accumulated_delta.x += delta.0 as f32;
        self.accumulated_delta.y += delta.1 as f32;
    }

    /// Process cursor position update.
    pub fn process_cursor_position(&mut self, position: (f64, f64)) {
        self.mouse_position = Vec2::new(position.0 as f32, position.1 as f32);
    }

    // Query methods

    /// Check if a key is currently held.
    pub fn is_key_held(&self, key: KeyCode) -> bool {
        self.keys_held.contains(&key)
    }

    /// Check if a key was pressed this frame.
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.keys_pressed.contains(&key)
    }

    /// Check if a key was released this frame.
    pub fn is_key_released(&self, key: KeyCode) -> bool {
        self.keys_released.contains(&key)
    }

    /// Check if a mouse button is held.
    pub fn is_mouse_held(&self, button: MouseButton) -> bool {
        self.mouse_held.contains(&button)
    }

    /// Check if a mouse button was pressed this frame.
    pub fn is_mouse_pressed(&self, button: MouseButton) -> bool {
        self.mouse_pressed.contains(&button)
    }

    /// Check if a mouse button was released this frame.
    pub fn is_mouse_released(&self, button: MouseButton) -> bool {
        self.mouse_released.contains(&button)
    }

    /// Get the mouse position in window coordinates.
    pub fn mouse_position(&self) -> Vec2 {
        self.mouse_position
    }

    /// Get the mouse movement delta for this frame.
    pub fn mouse_delta(&self) -> Vec2 {
        self.mouse_delta
    }

    /// Check if the cursor is locked.
    pub fn is_cursor_locked(&self) -> bool {
        self.cursor_locked
    }

    /// Set cursor lock state.
    pub fn set_cursor_locked(&mut self, locked: bool) {
        self.cursor_locked = locked;
    }

    /// Get movement input as a normalized vector (WASD).
    pub fn get_movement_input(&self) -> Vec2 {
        let mut movement = Vec2::ZERO;

        if self.is_key_held(KeyCode::KeyW) {
            movement.y += 1.0;
        }
        if self.is_key_held(KeyCode::KeyS) {
            movement.y -= 1.0;
        }
        if self.is_key_held(KeyCode::KeyA) {
            movement.x -= 1.0;
        }
        if self.is_key_held(KeyCode::KeyD) {
            movement.x += 1.0;
        }

        if movement.length_squared() > 0.0 {
            movement = movement.normalize();
        }

        movement
    }

    /// Check if sprint is held (Shift).
    pub fn is_sprinting(&self) -> bool {
        self.is_key_held(KeyCode::ShiftLeft) || self.is_key_held(KeyCode::ShiftRight)
    }

    /// Check if jump was pressed (Space).
    pub fn is_jump_pressed(&self) -> bool {
        self.is_key_pressed(KeyCode::Space)
    }

    /// Check if fire is held (Left mouse button).
    pub fn is_fire_held(&self) -> bool {
        self.is_mouse_held(MouseButton::Left)
    }

    /// Check if fire was pressed this frame (Left mouse button — one-shot per click).
    pub fn is_fire_pressed(&self) -> bool {
        self.is_mouse_pressed(MouseButton::Left)
    }

    /// Check if aim is held (Right mouse button).
    pub fn is_aim_held(&self) -> bool {
        self.is_mouse_held(MouseButton::Right)
    }

    /// Check if reload was pressed (R).
    pub fn is_reload_pressed(&self) -> bool {
        self.is_key_pressed(KeyCode::KeyR)
    }

    /// Check if aiming (right mouse).
    pub fn is_aiming(&self) -> bool {
        self.is_mouse_held(MouseButton::Right)
    }

    /// Check if crouching (Ctrl).
    pub fn is_crouching(&self) -> bool {
        self.is_key_held(KeyCode::ControlLeft) || self.is_key_held(KeyCode::ControlRight)
    }

    /// Check if ability key was pressed (Q).
    pub fn is_ability_pressed(&self) -> bool {
        self.is_key_pressed(KeyCode::KeyQ)
    }

    /// Check if a specific key was just pressed this frame.
    pub fn is_key_just_pressed(&self, key: KeyCode) -> bool {
        self.keys_pressed.contains(&key)
    }

    /// Set scroll up state.
    pub fn set_scroll_up(&mut self) {
        self.scroll_up = true;
    }

    /// Set scroll down state.
    pub fn set_scroll_down(&mut self) {
        self.scroll_down = true;
    }

    /// Check if scrolled up this frame.
    pub fn is_scroll_up(&self) -> bool {
        self.scroll_up
    }

    /// Check if scrolled down this frame.
    pub fn is_scroll_down(&self) -> bool {
        self.scroll_down
    }

    /// Check if interact was pressed (E).
    pub fn is_interact_pressed(&self) -> bool {
        self.is_key_pressed(KeyCode::KeyE)
    }

    /// Check if melee was pressed (V).
    pub fn is_melee_pressed(&self) -> bool {
        self.is_key_pressed(KeyCode::KeyV)
    }

    /// Check if grenade was pressed (G).
    pub fn is_grenade_pressed(&self) -> bool {
        self.is_key_pressed(KeyCode::KeyG)
    }
}

// Re-export for convenience
pub use winit::event::{ElementState, MouseButton};
pub use winit::keyboard::KeyCode;
