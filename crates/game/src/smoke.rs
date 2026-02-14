//! Smoke grenade system: throwable red smoke marker grenades.

use glam::Vec3;
use rand::Rng;

/// A thrown smoke grenade projectile (in-flight, before detonation).
pub struct SmokeGrenade {
    pub position: Vec3,
    pub velocity: Vec3,
    /// Time since thrown.
    pub age: f32,
    /// Has it hit the ground and detonated?
    pub detonated: bool,
}

/// A single smoke particle in a smoke cloud.
pub struct SmokeParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    /// Current life remaining.
    pub life: f32,
    /// Maximum life.
    pub max_life: f32,
    /// Current size.
    pub size: f32,
    /// Phase offset for visual variation.
    pub phase: f32,
}

/// Active smoke cloud (spawned when grenade detonates).
pub struct SmokeCloud {
    pub origin: Vec3,
    pub particles: Vec<SmokeParticle>,
    /// Total age of this cloud.
    pub age: f32,
    /// Cloud lifetime (when all particles gone).
    pub duration: f32,
}

impl SmokeCloud {
    pub fn new(origin: Vec3) -> Self {
        let mut particles = Vec::with_capacity(200);
        let mut rng = rand::thread_rng();

        // Initial burst - dense core particles
        for _ in 0..120 {
            let angle = rng.gen::<f32>() * std::f32::consts::TAU;
            let dist = rng.gen::<f32>() * 2.0;
            let height = rng.gen::<f32>() * 3.0;
            let speed = 2.0 + rng.gen::<f32>() * 5.0;
            particles.push(SmokeParticle {
                position: origin + Vec3::new(angle.cos() * dist * 0.3, height * 0.2, angle.sin() * dist * 0.3),
                velocity: Vec3::new(
                    angle.cos() * speed,
                    1.5 + rng.gen::<f32>() * 4.0,
                    angle.sin() * speed,
                ),
                life: 8.0 + rng.gen::<f32>() * 7.0,
                max_life: 8.0 + rng.gen::<f32>() * 7.0,
                size: 0.4 + rng.gen::<f32>() * 0.8,
                phase: rng.gen::<f32>() * std::f32::consts::TAU,
            });
        }

        // Rising column particles
        for _ in 0..80 {
            let angle = rng.gen::<f32>() * std::f32::consts::TAU;
            let spread = rng.gen::<f32>() * 1.5;
            particles.push(SmokeParticle {
                position: origin + Vec3::new(angle.cos() * spread * 0.2, 0.0, angle.sin() * spread * 0.2),
                velocity: Vec3::new(
                    (rng.gen::<f32>() - 0.5) * 1.5,
                    3.0 + rng.gen::<f32>() * 5.0,
                    (rng.gen::<f32>() - 0.5) * 1.5,
                ),
                life: 6.0 + rng.gen::<f32>() * 8.0,
                max_life: 6.0 + rng.gen::<f32>() * 8.0,
                size: 0.6 + rng.gen::<f32>() * 1.2,
                phase: rng.gen::<f32>() * std::f32::consts::TAU,
            });
        }

        Self { origin, particles, age: 0.0, duration: 18.0 }
    }

    pub fn update(&mut self, dt: f32) {
        self.age += dt;
        let mut rng = rand::thread_rng();

        for p in &mut self.particles {
            p.life -= dt;

            // Slow down over time (drag)
            p.velocity *= 1.0 - 1.8 * dt;

            // Slight upward buoyancy (hot smoke rises)
            p.velocity.y += 0.5 * dt;

            // Wind drift
            p.velocity.x += (p.phase + self.age * 0.3).sin() * 0.3 * dt;
            p.velocity.z += (p.phase * 1.7 + self.age * 0.2).cos() * 0.3 * dt;

            // Update position
            p.position += p.velocity * dt;

            // Grow over time
            let age_frac = 1.0 - (p.life / p.max_life);
            p.size = (0.4 + age_frac * 2.5).min(3.0);

            // Don't let particles sink below ground
            if p.position.y < self.origin.y + 0.1 {
                p.position.y = self.origin.y + 0.1;
                p.velocity.y = p.velocity.y.abs() * 0.3;
            }
        }

        // Remove dead particles
        self.particles.retain(|p| p.life > 0.0);

        // Continuously spawn new particles near the origin for sustained smoke
        if self.age < self.duration * 0.6 {
            let spawn_rate = if self.age < 2.0 { 15 } else { 5 };
            for _ in 0..spawn_rate {
                if self.particles.len() >= 150 { break; }
                let angle = rng.gen::<f32>() * std::f32::consts::TAU;
                let spread = rng.gen::<f32>() * 3.0;
                self.particles.push(SmokeParticle {
                    position: self.origin + Vec3::new(
                        angle.cos() * spread + (rng.gen::<f32>() - 0.5) * 2.0,
                        rng.gen::<f32>() * 2.0,
                        angle.sin() * spread + (rng.gen::<f32>() - 0.5) * 2.0,
                    ),
                    velocity: Vec3::new(
                        (rng.gen::<f32>() - 0.5) * 2.0,
                        1.0 + rng.gen::<f32>() * 3.0,
                        (rng.gen::<f32>() - 0.5) * 2.0,
                    ),
                    life: 5.0 + rng.gen::<f32>() * 6.0,
                    max_life: 5.0 + rng.gen::<f32>() * 6.0,
                    size: 0.3 + rng.gen::<f32>() * 0.5,
                    phase: rng.gen::<f32>() * std::f32::consts::TAU,
                });
            }
        }
    }

    pub fn is_done(&self) -> bool {
        self.particles.is_empty() && self.age > 2.0
    }
}
