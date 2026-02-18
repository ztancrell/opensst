//! First-person weapon viewmodel animation and shell casing physics.

use glam::{Quat, Vec3};
use physics::{ColliderHandle, RigidBodyHandle};

/// Shell casing type — matches weapon for persistent, weapon-appropriate shells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellCasingType {
    Rifle,      // 5.56mm brass
    Shotgun,    // 12ga red plastic hull
    Sniper,     // .308 / 7.62mm long brass
    MachineGun, // Belt-fed brass
    Rocket,     // Small ejector charge casing
    Flamethrower, // Igniter / fuel canister
}

/// Animation state for the first-person weapon viewmodel (M1A4 Morita Rifle).
pub struct ViewmodelAnimState {
    /// Time accumulator for idle sway oscillation.
    pub sway_time: f32,
    /// Fire recoil kick intensity (1.0 on fire, decays to 0).
    pub fire_kick: f32,
    /// Time since last weapon fire (for muzzle flash timing).
    pub fire_flash_timer: f32,
    /// Sprint lean amount (smoothly approaches 1 when sprinting, 0 otherwise).
    pub sprint_lean: f32,
    /// Walk/run bob accumulator.
    pub bob_time: f32,
    /// Weapon switch animation (1.0 = down, decays to 0 = ready).
    pub switch_anim: f32,
}

impl ViewmodelAnimState {
    pub fn new() -> Self {
        Self {
            sway_time: 0.0,
            fire_kick: 0.0,
            fire_flash_timer: 99.0,
            sprint_lean: 0.0,
            bob_time: 0.0,
            switch_anim: 0.5, // start with a quick raise animation
        }
    }

    pub fn update(&mut self, dt: f32, is_firing: bool, is_sprinting: bool, is_moving: bool, speed: f32) {
        // Idle sway (always running)
        self.sway_time += dt;

        // Fire recoil decay — fire_kick is set to 1.0 by the weapon system
        // when actually firing; fire_flash_timer is also reset there.
        // Here we only use is_firing to know if we should hold the kick.
        if is_firing {
            self.fire_kick = self.fire_kick.max(0.5);
        }
        self.fire_kick *= (1.0 - 18.0 * dt).max(0.0);
        if self.fire_kick < 0.001 { self.fire_kick = 0.0; }

        // Flash timer
        self.fire_flash_timer += dt;

        // Sprint lean
        let sprint_target = if is_sprinting { 1.0 } else { 0.0 };
        self.sprint_lean += (sprint_target - self.sprint_lean) * 6.0 * dt;

        // Walk bob
        if is_moving {
            let bob_speed = if is_sprinting { 12.0 } else { 8.0 };
            self.bob_time += dt * bob_speed * (speed / 5.0).min(1.5);
        } else {
            // Smoothly stop bobbing
            self.bob_time += dt * 2.0; // slow drift
        }

        // Switch animation decay
        self.switch_anim *= (1.0 - 6.0 * dt).max(0.0);
        if self.switch_anim < 0.001 { self.switch_anim = 0.0; }
    }

    /// Trigger weapon switch animation (weapon drops and raises).
    pub fn trigger_switch(&mut self) {
        self.switch_anim = 1.0;
    }

    /// Compute the animated base transform for the viewmodel.
    /// Returns (position_offset, rotation) to apply to the base viewmodel position.
    /// `base_pos`: hip-fire position in view space.
    /// `ads_target`: when ADS (aim_progress=1), the gun pivot should be here so the
    ///               rear sight aligns with screen center (crosshair). Computed from
    ///               weapon sight geometry: rear_sight at (0,0,-sight_dist) => pivot = -rear_sight_local.
    pub fn compute_transform(&self, aim_progress: f32, base_pos: Vec3, ads_target: Vec3) -> (Vec3, Quat) {
        // Idle sway (reduced when ADS for steadier aim)
        let sway_scale = 1.0 - aim_progress * 0.6;
        let sway_x = (self.sway_time * 0.7).sin() * 0.0025 * sway_scale;
        let sway_y = (self.sway_time * 1.1).cos() * 0.0018 * sway_scale;

        // Fire recoil
        let kick = self.fire_kick;
        let kick_back = kick * 0.025;       // push gun backward (+Z in view space)
        let kick_up = kick * 0.012;          // push gun up
        let kick_rot_x = -kick * 0.06;       // pitch up from recoil
        let kick_rot_z = kick * 0.015;        // slight roll

        // Sprint lean
        let sprint_tilt_z = self.sprint_lean * 0.35;    // roll to the side
        let sprint_lower_y = self.sprint_lean * 0.04;   // lower the gun
        let sprint_forward_z = self.sprint_lean * -0.03; // pull gun back slightly

        // Walk bob (reduced when ADS for steadier sight picture)
        let bob_scale = (1.0 - self.sprint_lean * 0.5) * (1.0 - aim_progress * 0.7);
        let bob_x = (self.bob_time).sin() * 0.004 * bob_scale;
        let bob_y = (self.bob_time * 2.0).sin().abs() * 0.003 * bob_scale;

        // ADS: interpolate from hip to sight-aligned position (rear sight on screen center)
        let ads_delta = (ads_target - base_pos) * aim_progress;

        // ADS tilt: slight pitch down so sight line aligns with view (cheek weld)
        let ads_tilt_x = -aim_progress * 0.04;

        // Weapon switch: drop gun down
        let switch_drop = self.switch_anim * self.switch_anim; // quadratic for smooth feel
        let switch_y = -switch_drop * 0.15;
        let switch_rot_x = switch_drop * 0.3;

        // Compose offset (ADS delta brings gun to sight-aligned position)
        let offset = Vec3::new(
            sway_x + bob_x + ads_delta.x,
            sway_y + kick_up - sprint_lower_y + bob_y + ads_delta.y + switch_y,
            kick_back + sprint_forward_z + ads_delta.z,
        );

        // Compose rotation
        let rotation = Quat::from_euler(
            glam::EulerRot::XYZ,
            kick_rot_x + switch_rot_x + ads_tilt_x,
            0.0,
            kick_rot_z + sprint_tilt_z,
        );

        (offset, rotation)
    }
}

/// An ejected shell casing (rigid body — flies then rests on ground, can roll when kicked).
pub struct ShellCasing {
    pub position: Vec3,
    pub rotation: Quat,
    pub body_handle: RigidBodyHandle,
    pub collider_handle: ColliderHandle,
    pub lifetime: f32,
    pub size: f32,
    pub shell_type: ShellCasingType,
}

/// A spent shell casing on the ground (same rigid body — persistent, can roll).
pub struct GroundedShellCasing {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub shell_type: ShellCasingType,
    pub body_handle: RigidBodyHandle,
    pub collider_handle: ColliderHandle,
}

impl GroundedShellCasing {
    /// Create from a flying casing when it has settled; keeps the same physics body.
    pub fn from_flying(casing: &ShellCasing) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let (base_scale, _tilt) = match casing.shell_type {
            ShellCasingType::Rifle => (Vec3::new(0.006, 0.018, 0.006), 0.2),
            ShellCasingType::Shotgun => (Vec3::new(0.012, 0.028, 0.012), 0.3),
            ShellCasingType::Sniper => (Vec3::new(0.008, 0.025, 0.008), 0.25),
            ShellCasingType::MachineGun => (Vec3::new(0.007, 0.020, 0.007), 0.22),
            ShellCasingType::Rocket => (Vec3::new(0.015, 0.035, 0.015), 0.35),
            ShellCasingType::Flamethrower => (Vec3::new(0.005, 0.012, 0.005), 0.15),
        };
        let jitter = 0.85 + rng.gen::<f32>() * 0.3;
        Self {
            position: casing.position,
            rotation: casing.rotation,
            scale: base_scale * jitter,
            shell_type: casing.shell_type,
            body_handle: casing.body_handle,
            collider_handle: casing.collider_handle,
        }
    }
}
