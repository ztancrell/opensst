//! Orbital artillery barrages from corvettes and destroyers (Helldivers 2 style).
//! Red smoke designates target; ships fire 6 shells one after another; rearm like tac fighters.

use glam::{Quat, Vec3};
use rand::Rng;

/// Delay between each shell in a barrage (seconds).
pub const SHELL_FIRE_DELAY: f32 = 0.45;
/// Number of shells per barrage.
pub const SHELLS_PER_BARRAGE: usize = 6;

/// An artillery shell fired from orbit, arcing down to the target.
pub struct ArtilleryShell {
    pub position: Vec3,
    pub velocity: Vec3,
    /// Position where shell was fired from (for muzzle flash).
    pub from_pos: Vec3,
    /// Target impact position (XZ).
    pub target: Vec3,
    pub age: f32,
    pub detonated: bool,
}

impl ArtilleryShell {
    /// Create a shell fired from a corvette/destroyer toward the target.
    pub fn new(from_pos: Vec3, target: Vec3) -> Self {
        let to_target = target - from_pos;
        let horiz = Vec3::new(to_target.x, 0.0, to_target.z);
        let horiz_dist = horiz.length().max(1.0);
        let horiz_dir = horiz / horiz_dist;

        // Arc trajectory: orbital guns launch with high velocity — punchy impact
        let flight_time = 0.5 + rand::thread_rng().gen::<f32>() * 0.2; // ~0.5–0.7s
        let gravity = 90.0;
        let dy = to_target.y - from_pos.y;

        // Solve: y = vy*t - 0.5*g*t^2 => vy = (dy + 0.5*g*t^2) / t
        let vy = (dy + 0.5 * gravity * flight_time * flight_time) / flight_time;
        let horiz_speed = horiz_dist / flight_time;
        let velocity = horiz_dir * horiz_speed + Vec3::Y * vy;

        Self {
            position: from_pos,
            velocity,
            from_pos,
            target,
            age: 0.0,
            detonated: false,
        }
    }
}

/// Brief muzzle flash at a ship when it fires. Rendered for ~0.2s.
pub struct ArtilleryMuzzleFlash {
    pub position: Vec3,
    /// Direction ship is facing (for orienting the flash).
    pub facing: Vec3,
    pub age: f32,
    pub duration: f32,
}

impl ArtilleryMuzzleFlash {
    pub fn new(position: Vec3, facing: Vec3) -> Self {
        Self {
            position,
            facing,
            age: 0.0,
            duration: 0.6, // longer so player can see ships firing
        }
    }

    pub fn is_done(&self) -> bool {
        self.age >= self.duration
    }
}

/// Smoke/fire trail particle left behind by a streaking artillery shell.
pub struct ArtilleryTrailParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub life: f32,
    pub max_life: f32,
    pub size: f32,
    pub phase: f32,
}

impl ArtilleryTrailParticle {
    pub fn new(position: Vec3, shell_velocity: Vec3) -> Self {
        let mut rng = rand::thread_rng();
        // Drift opposite to travel (trail streams behind)
        let back = -shell_velocity.normalize_or_zero();
        let drift = back * (2.0 + rng.gen::<f32>() * 4.0)
            + Vec3::Y * (1.0 + rng.gen::<f32>() * 2.0) // smoke rises
            + Vec3::new(
                (rng.gen::<f32>() - 0.5) * 3.0,
                0.0,
                (rng.gen::<f32>() - 0.5) * 3.0,
            );
        let max_life = 1.2 + rng.gen::<f32>() * 0.6;
        Self {
            position,
            velocity: drift,
            life: max_life,
            max_life,
            size: 0.8 + rng.gen::<f32>() * 1.2,
            phase: rng.gen::<f32>() * std::f32::consts::TAU,
        }
    }
}

/// A spent artillery shell casing lying on the ground (Helldivers 2 style).
/// Large metallic cylinders scattered around impact craters.
pub struct GroundedArtilleryShell {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl GroundedArtilleryShell {
    pub fn new(position: Vec3) -> Self {
        let mut rng = rand::thread_rng();
        // Random rotation so shells lie at different angles
        let rot_y = rng.gen::<f32>() * std::f32::consts::TAU;
        let rot_x = (rng.gen::<f32>() - 0.5) * 0.4; // slight tilt
        let rot_z = (rng.gen::<f32>() - 0.5) * 0.3;
        let rotation = Quat::from_euler(
            glam::EulerRot::XYZ,
            rot_x,
            rot_y,
            rot_z,
        );
        // Elongated cylinder: ~1.2m long, 0.25m diameter (orbital artillery caliber)
        let scale = Vec3::new(
            0.25 * (0.9 + rng.gen::<f32>() * 0.2),
            0.6 * (0.9 + rng.gen::<f32>() * 0.2),
            0.25 * (0.9 + rng.gen::<f32>() * 0.2),
        );
        Self { position, rotation, scale }
    }
}

/// An active barrage: fires 6 shells one after another with delay between each.
pub struct ArtilleryBarrage {
    /// Base target position (from red smoke).
    pub target: Vec3,
    /// Shells left to fire.
    pub shells_remaining: usize,
    /// Time until next shell fires.
    pub fire_timer: f32,
    /// Index for alternating corvette/destroyer (0..6).
    pub fire_index: usize,
}
