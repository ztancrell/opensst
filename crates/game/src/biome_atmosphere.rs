//! Biome volumetric atmosphere system.
//! Per-biome cinematic particles: fog banks, embers, spores, crystals, etc.

use glam::Vec3;
use procgen::BiomeType;

/// Volumetric particle types for biome atmosphere.
#[derive(Clone, Copy, PartialEq)]
pub enum AtmoParticleKind {
    /// Large translucent fog/mist bank (billboard, very large)
    FogBank,
    /// Small glowing ember/spark (sphere, bright)
    Ember,
    /// Floating organic spore/pollen (billboard, medium)
    Spore,
    /// Drifting ash flake (billboard, medium, grey)
    Ash,
    /// Ice crystal shard (billboard, small, sparkly)
    IceCrystal,
    /// Toxic gas wisp (billboard, large, colored)
    ToxicGas,
    /// Firefly / bioluminescent orb (sphere, small, glowing)
    Firefly,
    /// Crystal sparkle / prismatic flash (sphere, tiny, bright)
    CrystalSparkle,
    /// Sand grain caught in wind (sphere, tiny)
    SandGrain,
    /// Electric arc / radiation spark (billboard, bright)
    RadiationSpark,
    /// Mist tendril / low fog (billboard, wide & low)
    MistTendril,
    /// God ray streak (billboard, tall & narrow, bright)
    GodRay,
}

pub struct AtmoParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub life: f32,
    pub max_life: f32,
    pub size: Vec3,          // Non-uniform scale (width, height, depth)
    pub kind: AtmoParticleKind,
    pub color: [f32; 4],
    pub spin: f32,
    pub phase: f32,          // Used for pulsing/flickering effects
}

/// Per-biome atmospheric particle configuration.
pub struct BiomeAtmoConfig {
    /// Which particle kinds this biome spawns
    pub particles: Vec<(AtmoParticleKind, f32)>, // (kind, relative_weight)
    /// Spawn rate (particles per second)
    pub spawn_rate: f32,
    /// Max particles alive
    pub max_particles: usize,
    /// Fog tint color for the biome
    pub fog_tint: [f32; 3],
    /// Extra fog density multiplier
    pub fog_density_mult: f32,
    /// Ambient light tint
    pub ambient_tint: [f32; 3],
}

impl BiomeAtmoConfig {
    pub fn for_biome(biome: BiomeType) -> Self {
        match biome {
            BiomeType::Desert => Self {
                particles: vec![
                    (AtmoParticleKind::SandGrain, 3.0),
                    (AtmoParticleKind::FogBank, 0.3),
                    (AtmoParticleKind::GodRay, 0.2),
                ],
                spawn_rate: 6.0,
                max_particles: 85,
                fog_tint: [0.85, 0.75, 0.55],
                fog_density_mult: 1.2,
                ambient_tint: [1.0, 0.9, 0.7],
            },
            BiomeType::Badlands => Self {
                particles: vec![
                    (AtmoParticleKind::SandGrain, 2.0),
                    (AtmoParticleKind::Ash, 1.0),
                    (AtmoParticleKind::FogBank, 0.4),
                ],
                spawn_rate: 4.0,
                max_particles: 70,
                fog_tint: [0.7, 0.5, 0.4],
                fog_density_mult: 1.3,
                ambient_tint: [0.9, 0.7, 0.6],
            },
            BiomeType::HiveWorld => Self {
                particles: vec![
                    (AtmoParticleKind::Spore, 3.0),
                    (AtmoParticleKind::ToxicGas, 1.5),
                    (AtmoParticleKind::Firefly, 1.0),
                    (AtmoParticleKind::FogBank, 0.5),
                    (AtmoParticleKind::MistTendril, 0.8),
                ],
                spawn_rate: 10.0,
                max_particles: 140,
                fog_tint: [0.4, 0.5, 0.3],
                fog_density_mult: 1.8,
                ambient_tint: [0.7, 0.8, 0.5],
            },
            BiomeType::Volcanic => Self {
                particles: vec![
                    (AtmoParticleKind::Ember, 4.0),
                    (AtmoParticleKind::Ash, 3.0),
                    (AtmoParticleKind::FogBank, 0.5),
                    (AtmoParticleKind::GodRay, 0.3),
                ],
                spawn_rate: 14.0,
                max_particles: 175,
                fog_tint: [0.6, 0.3, 0.15],
                fog_density_mult: 1.5,
                ambient_tint: [1.0, 0.6, 0.3],
            },
            BiomeType::Frozen => Self {
                particles: vec![
                    (AtmoParticleKind::IceCrystal, 3.0),
                    (AtmoParticleKind::FogBank, 0.6),
                    (AtmoParticleKind::MistTendril, 0.5),
                    (AtmoParticleKind::GodRay, 0.2),
                ],
                spawn_rate: 7.0,
                max_particles: 105,
                fog_tint: [0.7, 0.8, 0.95],
                fog_density_mult: 1.4,
                ambient_tint: [0.8, 0.85, 1.0],
            },
            BiomeType::Toxic => Self {
                particles: vec![
                    (AtmoParticleKind::ToxicGas, 4.0),
                    (AtmoParticleKind::Spore, 2.0),
                    (AtmoParticleKind::FogBank, 1.0),
                    (AtmoParticleKind::Firefly, 0.5),
                    (AtmoParticleKind::MistTendril, 1.0),
                ],
                spawn_rate: 12.0,
                max_particles: 155,
                fog_tint: [0.4, 0.55, 0.2],
                fog_density_mult: 2.0,
                ambient_tint: [0.6, 0.8, 0.3],
            },
            BiomeType::Mountain => Self {
                particles: vec![
                    (AtmoParticleKind::FogBank, 1.0),
                    (AtmoParticleKind::MistTendril, 0.8),
                    (AtmoParticleKind::GodRay, 0.5),
                ],
                spawn_rate: 4.0,
                max_particles: 55,
                fog_tint: [0.6, 0.65, 0.7],
                fog_density_mult: 1.1,
                ambient_tint: [0.85, 0.85, 0.9],
            },
            BiomeType::Swamp => Self {
                particles: vec![
                    (AtmoParticleKind::MistTendril, 3.0),
                    (AtmoParticleKind::FogBank, 2.0),
                    (AtmoParticleKind::Firefly, 2.5),
                    (AtmoParticleKind::Spore, 1.0),
                    (AtmoParticleKind::ToxicGas, 0.5),
                ],
                spawn_rate: 11.0,
                max_particles: 140,
                fog_tint: [0.35, 0.45, 0.3],
                fog_density_mult: 2.2,
                ambient_tint: [0.6, 0.7, 0.5],
            },
            BiomeType::Crystalline => Self {
                particles: vec![
                    (AtmoParticleKind::CrystalSparkle, 5.0),
                    (AtmoParticleKind::IceCrystal, 2.0),
                    (AtmoParticleKind::GodRay, 1.0),
                    (AtmoParticleKind::FogBank, 0.3),
                ],
                spawn_rate: 10.0,
                max_particles: 125,
                fog_tint: [0.6, 0.5, 0.8],
                fog_density_mult: 0.8,
                ambient_tint: [0.8, 0.7, 1.0],
            },
            BiomeType::Ashlands => Self {
                particles: vec![
                    (AtmoParticleKind::Ash, 5.0),
                    (AtmoParticleKind::Ember, 1.5),
                    (AtmoParticleKind::FogBank, 1.0),
                    (AtmoParticleKind::MistTendril, 0.5),
                ],
                spawn_rate: 12.0,
                max_particles: 155,
                fog_tint: [0.5, 0.48, 0.45],
                fog_density_mult: 2.0,
                ambient_tint: [0.7, 0.65, 0.6],
            },
            BiomeType::Jungle => Self {
                particles: vec![
                    (AtmoParticleKind::Firefly, 3.0),
                    (AtmoParticleKind::Spore, 3.0),
                    (AtmoParticleKind::MistTendril, 2.0),
                    (AtmoParticleKind::FogBank, 0.8),
                    (AtmoParticleKind::GodRay, 0.5),
                ],
                spawn_rate: 10.0,
                max_particles: 140,
                fog_tint: [0.3, 0.5, 0.25],
                fog_density_mult: 1.6,
                ambient_tint: [0.6, 0.85, 0.4],
            },
            BiomeType::Wasteland => Self {
                particles: vec![
                    (AtmoParticleKind::RadiationSpark, 2.0),
                    (AtmoParticleKind::SandGrain, 2.0),
                    (AtmoParticleKind::Ash, 1.5),
                    (AtmoParticleKind::FogBank, 0.5),
                ],
                spawn_rate: 7.0,
                max_particles: 100,
                fog_tint: [0.6, 0.55, 0.4],
                fog_density_mult: 1.5,
                ambient_tint: [0.8, 0.7, 0.5],
            },
            BiomeType::Tundra => Self {
                particles: vec![
                    (AtmoParticleKind::IceCrystal, 2.5),
                    (AtmoParticleKind::SandGrain, 1.5),
                    (AtmoParticleKind::FogBank, 0.8),
                    (AtmoParticleKind::MistTendril, 0.6),
                ],
                spawn_rate: 6.0,
                max_particles: 90,
                fog_tint: [0.68, 0.75, 0.82],
                fog_density_mult: 1.3,
                ambient_tint: [0.82, 0.86, 0.92],
            },
            BiomeType::SaltFlat => Self {
                particles: vec![
                    (AtmoParticleKind::SandGrain, 4.0),
                    (AtmoParticleKind::GodRay, 1.0),
                    (AtmoParticleKind::FogBank, 0.2),
                ],
                spawn_rate: 5.0,
                max_particles: 70,
                fog_tint: [0.9, 0.88, 0.85],
                fog_density_mult: 0.9,
                ambient_tint: [1.0, 0.98, 0.95],
            },
            BiomeType::Storm => Self {
                particles: vec![
                    (AtmoParticleKind::FogBank, 4.0),
                    (AtmoParticleKind::MistTendril, 3.0),
                    (AtmoParticleKind::RadiationSpark, 0.8),
                    (AtmoParticleKind::GodRay, 0.3),
                ],
                spawn_rate: 14.0,
                max_particles: 180,
                fog_tint: [0.35, 0.38, 0.42],
                fog_density_mult: 2.4,
                ambient_tint: [0.5, 0.52, 0.55],
            },
            BiomeType::Fungal => Self {
                particles: vec![
                    (AtmoParticleKind::Spore, 4.0),
                    (AtmoParticleKind::Firefly, 3.0),
                    (AtmoParticleKind::ToxicGas, 1.0),
                    (AtmoParticleKind::MistTendril, 1.5),
                ],
                spawn_rate: 11.0,
                max_particles: 150,
                fog_tint: [0.45, 0.35, 0.5],
                fog_density_mult: 1.7,
                ambient_tint: [0.65, 0.55, 0.75],
            },
            BiomeType::Scorched => Self {
                particles: vec![
                    (AtmoParticleKind::Ember, 5.0),
                    (AtmoParticleKind::Ash, 4.0),
                    (AtmoParticleKind::FogBank, 1.0),
                ],
                spawn_rate: 13.0,
                max_particles: 165,
                fog_tint: [0.4, 0.28, 0.2],
                fog_density_mult: 1.9,
                ambient_tint: [0.7, 0.45, 0.3],
            },
            BiomeType::Ruins => Self {
                particles: vec![
                    (AtmoParticleKind::SandGrain, 1.5),
                    (AtmoParticleKind::FogBank, 1.2),
                    (AtmoParticleKind::MistTendril, 0.8),
                    (AtmoParticleKind::RadiationSpark, 0.5),
                ],
                spawn_rate: 5.0,
                max_particles: 85,
                fog_tint: [0.5, 0.48, 0.46],
                fog_density_mult: 1.4,
                ambient_tint: [0.75, 0.72, 0.68],
            },
        }
    }
}

/// Biome volumetric atmosphere manager.
pub struct BiomeAtmosphere {
    pub particles: Vec<AtmoParticle>,
    pub config: BiomeAtmoConfig,
    pub spawn_accum: f32,
    pub biome: BiomeType,
}

impl BiomeAtmosphere {
    pub fn new(biome: BiomeType) -> Self {
        Self {
            particles: Vec::new(),
            config: BiomeAtmoConfig::for_biome(biome),
            spawn_accum: 0.0,
            biome,
        }
    }

    pub fn reset(&mut self, biome: BiomeType) {
        self.particles.clear();
        self.config = BiomeAtmoConfig::for_biome(biome);
        self.spawn_accum = 0.0;
        self.biome = biome;
    }

    pub fn update(&mut self, dt: f32, cam_pos: Vec3, time: f32) {
        // Spawn new particles
        self.spawn_accum += self.config.spawn_rate * dt;
        while self.spawn_accum >= 1.0 && self.particles.len() < self.config.max_particles {
            self.spawn_accum -= 1.0;
            self.spawn_particle(cam_pos, time);
        }

        // Update existing particles
        for p in &mut self.particles {
            p.position += p.velocity * dt;
            p.life -= dt;
            p.spin += dt * 0.3;
            p.phase += dt;

            // Kind-specific updates
            match p.kind {
                AtmoParticleKind::Firefly => {
                    let wander = Vec3::new(
                        (p.phase * 2.3 + p.spin * 5.0).sin() * 1.5,
                        (p.phase * 1.7).cos() * 0.8,
                        (p.phase * 3.1 + 1.0).sin() * 1.5,
                    );
                    p.position += wander * dt;
                    p.color[3] = (0.3 + (p.phase * 3.5).sin().abs() * 0.7).min(1.0);
                }
                AtmoParticleKind::Ember => {
                    p.velocity.y += 0.5 * dt;
                    p.color[3] = (p.life / p.max_life).powf(0.5) * (0.7 + (p.phase * 8.0).sin() * 0.3);
                }
                AtmoParticleKind::RadiationSpark => {
                    let t = 1.0 - p.life / p.max_life;
                    p.color[3] = if t < 0.1 { t / 0.1 } else { (1.0 - t).max(0.0) };
                    p.size = Vec3::splat(0.05 + t * 0.15);
                }
                AtmoParticleKind::CrystalSparkle => {
                    p.color[3] = ((p.phase * 12.0).sin() * 0.5 + 0.5).powf(3.0) * 0.9;
                }
                AtmoParticleKind::ToxicGas => {
                    let age = 1.0 - p.life / p.max_life;
                    let base_size = p.max_life * 0.15;
                    let expand = 1.0 + age * 0.8;
                    p.size = Vec3::splat((base_size * expand).min(2.5));
                    p.color[3] = (p.life / p.max_life) * 0.25;
                }
                AtmoParticleKind::FogBank => {
                    let life_frac = p.life / p.max_life;
                    let fade = if life_frac > 0.8 { (1.0 - life_frac) / 0.2 }
                        else if life_frac < 0.3 { life_frac / 0.3 }
                        else { 1.0 };
                    p.color[3] = fade * 0.12;
                }
                AtmoParticleKind::MistTendril => {
                    let life_frac = p.life / p.max_life;
                    let fade = if life_frac > 0.8 { (1.0 - life_frac) / 0.2 }
                        else if life_frac < 0.3 { life_frac / 0.3 }
                        else { 1.0 };
                    p.color[3] = fade * 0.15;
                    if p.position.y > cam_pos.y - 1.0 {
                        p.velocity.y -= 0.3 * dt;
                    }
                }
                AtmoParticleKind::GodRay => {
                    let life_frac = p.life / p.max_life;
                    let fade = if life_frac > 0.8 { (1.0 - life_frac) / 0.2 }
                        else if life_frac < 0.3 { life_frac / 0.3 }
                        else { 1.0 };
                    p.color[3] = fade * 0.1;
                }
                AtmoParticleKind::IceCrystal => {
                    p.velocity.x += (p.phase * 1.5).sin() * 0.1 * dt;
                    p.velocity.z += (p.phase * 2.1).cos() * 0.1 * dt;
                    p.color[3] = (p.life / p.max_life) * 0.6;
                }
                _ => {
                    p.color[3] = (p.life / p.max_life).min(p.color[3]);
                }
            }
        }

        // Remove dead particles
        self.particles.retain(|p| p.life > 0.0);
    }

    pub fn spawn_particle(&mut self, cam_pos: Vec3, time: f32) {
        // Weighted random selection of particle kind
        let total_weight: f32 = self.config.particles.iter().map(|(_, w)| w).sum();
        let mut roll = rand::random::<f32>() * total_weight;
        let mut kind = self.config.particles[0].0;
        for &(k, w) in &self.config.particles {
            roll -= w;
            if roll <= 0.0 {
                kind = k;
                break;
            }
        }

        let tint = &self.config.fog_tint;

        // Spawn based on kind
        let (pos, vel, life, size, color) = match kind {
            AtmoParticleKind::FogBank => {
                let dist = 25.0 + rand::random::<f32>() * 50.0;
                let angle = rand::random::<f32>() * std::f32::consts::TAU;
                let pos = cam_pos + Vec3::new(angle.cos() * dist, -2.0 + rand::random::<f32>() * 6.0, angle.sin() * dist);
                let vel = Vec3::new((rand::random::<f32>() - 0.5) * 0.6, 0.05, (rand::random::<f32>() - 0.5) * 0.6);
                let s = 2.5 + rand::random::<f32>() * 3.5;
                let color = [tint[0], tint[1], tint[2], 0.0];
                (pos, vel, 12.0 + rand::random::<f32>() * 12.0, Vec3::new(s, s * 0.4, s), color)
            }
            AtmoParticleKind::Ember => {
                let dist = 5.0 + rand::random::<f32>() * 25.0;
                let angle = rand::random::<f32>() * std::f32::consts::TAU;
                let pos = cam_pos + Vec3::new(angle.cos() * dist, -1.0 + rand::random::<f32>() * 3.0, angle.sin() * dist);
                let vel = Vec3::new((rand::random::<f32>() - 0.5) * 2.0, 1.0 + rand::random::<f32>() * 2.0, (rand::random::<f32>() - 0.5) * 2.0);
                let color = [1.0, 0.5 + rand::random::<f32>() * 0.4, 0.1, 0.9];
                (pos, vel, 2.0 + rand::random::<f32>() * 3.0, Vec3::splat(0.03 + rand::random::<f32>() * 0.05), color)
            }
            AtmoParticleKind::Spore => {
                let dist = 3.0 + rand::random::<f32>() * 20.0;
                let angle = rand::random::<f32>() * std::f32::consts::TAU;
                let pos = cam_pos + Vec3::new(angle.cos() * dist, rand::random::<f32>() * 6.0, angle.sin() * dist);
                let vel = Vec3::new((rand::random::<f32>() - 0.5) * 0.8, 0.2 + rand::random::<f32>() * 0.5, (rand::random::<f32>() - 0.5) * 0.8);
                let g = 0.6 + rand::random::<f32>() * 0.3;
                let color = [g * 0.7, g, g * 0.4, 0.5];
                (pos, vel, 5.0 + rand::random::<f32>() * 5.0, Vec3::splat(0.04 + rand::random::<f32>() * 0.08), color)
            }
            AtmoParticleKind::Ash => {
                let dist = 3.0 + rand::random::<f32>() * 30.0;
                let angle = rand::random::<f32>() * std::f32::consts::TAU;
                let pos = cam_pos + Vec3::new(angle.cos() * dist, 5.0 + rand::random::<f32>() * 15.0, angle.sin() * dist);
                let vel = Vec3::new((rand::random::<f32>() - 0.5) * 1.5, -0.5 - rand::random::<f32>() * 0.5, (rand::random::<f32>() - 0.5) * 1.5);
                let g = 0.4 + rand::random::<f32>() * 0.2;
                let color = [g, g, g, 0.6];
                (pos, vel, 4.0 + rand::random::<f32>() * 6.0, Vec3::splat(0.02 + rand::random::<f32>() * 0.04), color)
            }
            AtmoParticleKind::IceCrystal => {
                let dist = 3.0 + rand::random::<f32>() * 20.0;
                let angle = rand::random::<f32>() * std::f32::consts::TAU;
                let pos = cam_pos + Vec3::new(angle.cos() * dist, 2.0 + rand::random::<f32>() * 10.0, angle.sin() * dist);
                let vel = Vec3::new((rand::random::<f32>() - 0.5) * 0.6, -0.3, (rand::random::<f32>() - 0.5) * 0.6);
                let color = [0.8, 0.9, 1.0, 0.6];
                (pos, vel, 4.0 + rand::random::<f32>() * 4.0, Vec3::splat(0.02 + rand::random::<f32>() * 0.03), color)
            }
            AtmoParticleKind::ToxicGas => {
                let dist = 8.0 + rand::random::<f32>() * 25.0;
                let angle = rand::random::<f32>() * std::f32::consts::TAU;
                let pos = cam_pos + Vec3::new(angle.cos() * dist, -1.0 + rand::random::<f32>() * 3.0, angle.sin() * dist);
                let vel = Vec3::new((rand::random::<f32>() - 0.5) * 0.4, 0.1 + rand::random::<f32>() * 0.2, (rand::random::<f32>() - 0.5) * 0.4);
                let s = 0.8 + rand::random::<f32>() * 1.5;
                let color = [tint[0], tint[1] + 0.1, tint[2], 0.2];
                (pos, vel, 5.0 + rand::random::<f32>() * 6.0, Vec3::splat(s), color)
            }
            AtmoParticleKind::Firefly => {
                let dist = 3.0 + rand::random::<f32>() * 15.0;
                let angle = rand::random::<f32>() * std::f32::consts::TAU;
                let pos = cam_pos + Vec3::new(angle.cos() * dist, rand::random::<f32>() * 4.0, angle.sin() * dist);
                let vel = Vec3::new((rand::random::<f32>() - 0.5) * 0.5, 0.0, (rand::random::<f32>() - 0.5) * 0.5);
                let hue = rand::random::<f32>();
                let color = if hue < 0.5 {
                    [0.3, 1.0, 0.2, 0.8]
                } else {
                    [0.9, 0.8, 0.1, 0.8]
                };
                (pos, vel, 6.0 + rand::random::<f32>() * 8.0, Vec3::splat(0.04 + rand::random::<f32>() * 0.03), color)
            }
            AtmoParticleKind::CrystalSparkle => {
                let dist = 2.0 + rand::random::<f32>() * 18.0;
                let angle = rand::random::<f32>() * std::f32::consts::TAU;
                let pos = cam_pos + Vec3::new(angle.cos() * dist, 1.0 + rand::random::<f32>() * 8.0, angle.sin() * dist);
                let vel = Vec3::ZERO;
                let r = 0.5 + rand::random::<f32>() * 0.5;
                let g = 0.5 + rand::random::<f32>() * 0.5;
                let b = 0.5 + rand::random::<f32>() * 0.5;
                let color = [r, g, b, 0.0];
                (pos, vel, 3.0 + rand::random::<f32>() * 5.0, Vec3::splat(0.02 + rand::random::<f32>() * 0.02), color)
            }
            AtmoParticleKind::SandGrain => {
                let dist = 2.0 + rand::random::<f32>() * 20.0;
                let angle = rand::random::<f32>() * std::f32::consts::TAU;
                let pos = cam_pos + Vec3::new(angle.cos() * dist, rand::random::<f32>() * 5.0, angle.sin() * dist);
                let wind_x = (time * 0.3).sin() * 3.0 + 1.0;
                let wind_z = (time * 0.2).cos() * 2.0;
                let vel = Vec3::new(wind_x + (rand::random::<f32>() - 0.5) * 1.0, -0.2, wind_z + (rand::random::<f32>() - 0.5) * 1.0);
                let color = [0.85, 0.75, 0.55, 0.4];
                (pos, vel, 2.0 + rand::random::<f32>() * 3.0, Vec3::splat(0.01 + rand::random::<f32>() * 0.015), color)
            }
            AtmoParticleKind::RadiationSpark => {
                let dist = 5.0 + rand::random::<f32>() * 25.0;
                let angle = rand::random::<f32>() * std::f32::consts::TAU;
                let pos = cam_pos + Vec3::new(angle.cos() * dist, rand::random::<f32>() * 3.0, angle.sin() * dist);
                let vel = Vec3::new((rand::random::<f32>() - 0.5) * 4.0, rand::random::<f32>() * 2.0, (rand::random::<f32>() - 0.5) * 4.0);
                let color = [0.4, 0.9, 1.0, 0.0];
                (pos, vel, 0.5 + rand::random::<f32>() * 1.5, Vec3::splat(0.05), color)
            }
            AtmoParticleKind::MistTendril => {
                let dist = 8.0 + rand::random::<f32>() * 30.0;
                let angle = rand::random::<f32>() * std::f32::consts::TAU;
                let pos = cam_pos + Vec3::new(angle.cos() * dist, -1.5 + rand::random::<f32>() * 1.5, angle.sin() * dist);
                let vel = Vec3::new((rand::random::<f32>() - 0.5) * 0.5, 0.0, (rand::random::<f32>() - 0.5) * 0.5);
                let w = 1.5 + rand::random::<f32>() * 2.5;
                let color = [tint[0], tint[1], tint[2], 0.0];
                (pos, vel, 8.0 + rand::random::<f32>() * 8.0, Vec3::new(w, 0.6, w * 0.3), color)
            }
            AtmoParticleKind::GodRay => {
                let dist = 20.0 + rand::random::<f32>() * 35.0;
                let angle = rand::random::<f32>() * std::f32::consts::TAU;
                let pos = cam_pos + Vec3::new(angle.cos() * dist, 5.0 + rand::random::<f32>() * 15.0, angle.sin() * dist);
                let vel = Vec3::new(0.0, -0.05, 0.0);
                let color = [1.0, 0.95, 0.8, 0.0];
                (pos, vel, 6.0 + rand::random::<f32>() * 8.0, Vec3::new(0.15, 4.0 + rand::random::<f32>() * 3.0, 0.15), color)
            }
        };

        self.particles.push(AtmoParticle {
            position: pos,
            velocity: vel,
            life,
            max_life: life,
            size,
            kind,
            color,
            spin: rand::random::<f32>() * std::f32::consts::TAU,
            phase: rand::random::<f32>() * 10.0,
        });
    }
}
