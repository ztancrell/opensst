//! Ambient particle effects: dust motes, rain, and tracer projectiles.

use glam::Vec3;

#[derive(Clone, Copy, PartialEq)]
pub enum DustShape {
    Billboard,
    Sphere,
}

/// Ambient dust particles floating in the air for atmosphere.
pub struct AmbientDust {
    pub particles: Vec<DustParticle>,
    pub spawn_timer: f32,
}

pub struct DustParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub life: f32,
    pub size: f32,
    pub shape: DustShape,
    /// Slow spin for billboard particles (radians)
    pub spin: f32,
}

impl AmbientDust {
    pub fn new() -> Self {
        Self { particles: Vec::new(), spawn_timer: 0.0 }
    }

    /// density_mult: 1.0 = normal; higher when cloudy/rain for more visible floating particles
    pub fn update(&mut self, dt: f32, cam_pos: Vec3, density_mult: f32) {
        self.spawn_timer += dt;
        let max_particles = (150.0 * density_mult.min(2.5)) as usize;
        let spawn_interval = 0.05 / density_mult.max(0.5);
        if self.spawn_timer > spawn_interval && self.particles.len() < max_particles {
            self.spawn_timer = 0.0;
            let px = cam_pos.x + (rand::random::<f32>() - 0.5) * 30.0;
            let py = cam_pos.y + (rand::random::<f32>() - 0.3) * 10.0;
            let pz = cam_pos.z + (rand::random::<f32>() - 0.5) * 30.0;
            // 70% billboard, 30% sphere for variety
            let shape = if rand::random::<f32>() < 0.7 {
                DustShape::Billboard
            } else {
                DustShape::Sphere
            };
            let base_size = if shape == DustShape::Billboard {
                0.03 + rand::random::<f32>() * 0.06 // billboard quads slightly larger
            } else {
                0.015 + rand::random::<f32>() * 0.03 // spheres smaller & denser
            };
            self.particles.push(DustParticle {
                position: Vec3::new(px, py, pz),
                velocity: Vec3::new(
                    (rand::random::<f32>() - 0.5) * 0.5,
                    (rand::random::<f32>() - 0.5) * 0.2,
                    (rand::random::<f32>() - 0.5) * 0.5,
                ),
                life: 4.0 + rand::random::<f32>() * 4.0,
                size: base_size,
                shape,
                spin: rand::random::<f32>() * std::f32::consts::TAU,
            });
        }
        for p in &mut self.particles {
            p.position += p.velocity * dt;
            p.life -= dt;
            p.spin += dt * 0.5; // gentle rotation
        }
        self.particles.retain(|p| p.life > 0.0);
    }
}

pub struct RainDrop {
    pub position: Vec3,
    pub velocity: Vec3,
    pub life: f32,
}

/// Snow particle: slower fall, slight drift, for Snow weather.
pub struct SnowParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub life: f32,
    pub size: f32,
}

/// Visual-only bullet tracer for first-person feedback
pub struct TracerProjectile {
    pub position: Vec3,
    pub velocity: Vec3,
    pub lifetime: f32,
}
