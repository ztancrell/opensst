//! Complete bug entity with physics, ragdoll, and rendering integration

use engine_core::{Health, Transform, Velocity, Vec3};
use glam::Quat;
use hecs::{Entity, World};
use physics::{PhysicsWorld, ColliderHandle, RigidBodyHandle};
use rand::prelude::*;

use crate::bug::{Bug, BugType};

/// Physics-enabled bug with ragdoll support
#[derive(Debug, Clone)]
pub struct PhysicsBug {
    /// Rigid body handle for main body
    pub body_handle: Option<RigidBodyHandle>,
    /// Collider handle
    pub collider_handle: Option<ColliderHandle>,
    /// Is this bug in ragdoll mode (dying/dead)
    pub is_ragdoll: bool,
    /// Ragdoll activation time
    pub ragdoll_time: f32,
    /// Death animation phase
    pub death_phase: DeathPhase,
    /// Hit impact velocity (for ragdoll launch)
    pub impact_velocity: Vec3,
    /// Limb damage for procedural death
    pub limb_damage: [f32; 6], // legs
    /// Gore spawned
    pub gore_spawned: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeathPhase {
    Alive,
    /// Just killed - apply impact force
    Launched,
    /// Ragdoll falling
    Falling,
    /// Twitching on ground
    Twitching,
    /// Curling up
    CurlingUp,
    /// Fully dead, settled
    Dead,
}

impl Default for PhysicsBug {
    fn default() -> Self {
        Self {
            body_handle: None,
            collider_handle: None,
            is_ragdoll: false,
            ragdoll_time: 0.0,
            death_phase: DeathPhase::Alive,
            impact_velocity: Vec3::ZERO,
            limb_damage: [0.0; 6],
            gore_spawned: false,
        }
    }
}

impl PhysicsBug {
    pub fn new() -> Self {
        Self::default()
    }

    /// Activate ragdoll with impact force — Euphoria-style violent launch
    pub fn activate_ragdoll(&mut self, impact_dir: Vec3, force: f32) {
        self.is_ragdoll = true;
        self.death_phase = DeathPhase::Launched;
        self.ragdoll_time = 0.0;
        self.impact_velocity = impact_dir * force * 0.6;
    }

    /// Update death animation phases
    pub fn update_death(&mut self, dt: f32) {
        if !self.is_ragdoll {
            return;
        }

        self.ragdoll_time += dt;

        match self.death_phase {
            DeathPhase::Launched => {
                if self.ragdoll_time > 0.1 {
                    self.death_phase = DeathPhase::Falling;
                }
            }
            DeathPhase::Falling => {
                if self.ragdoll_time > 0.5 {
                    self.death_phase = DeathPhase::Twitching;
                }
            }
            DeathPhase::Twitching => {
                if self.ragdoll_time > 2.0 {
                    self.death_phase = DeathPhase::CurlingUp;
                }
            }
            DeathPhase::CurlingUp => {
                if self.ragdoll_time > 3.5 {
                    self.death_phase = DeathPhase::Dead;
                }
            }
            _ => {}
        }
    }

    /// Get procedural death animation offset
    pub fn get_death_animation(&self) -> (Vec3, Quat, f32) {
        let mut offset = Vec3::ZERO;
        let mut rotation = Quat::IDENTITY;
        let mut scale = 1.0;

        match self.death_phase {
            DeathPhase::Launched => {
                // Apply launch velocity
                offset = self.impact_velocity * self.ragdoll_time;
                offset.y += 2.0 * self.ragdoll_time - 9.8 * self.ragdoll_time * self.ragdoll_time;
                // Tumble
                let tumble = self.ragdoll_time * 8.0;
                rotation = Quat::from_euler(glam::EulerRot::XYZ, tumble, tumble * 0.7, tumble * 0.3);
            }
            DeathPhase::Falling => {
                let t = self.ragdoll_time - 0.1;
                // Continue arc but slow down
                offset = self.impact_velocity * 0.1 + self.impact_velocity * t * 0.3;
                offset.y = (offset.y - 9.8 * t * t).max(0.0);
                // Settle rotation
                let tumble = 0.8 + t * 2.0;
                rotation = Quat::from_euler(glam::EulerRot::XYZ, tumble, 0.0, 1.5);
            }
            DeathPhase::Twitching => {
                // Euphoria-style: violent twitching, limbs spasming
                let twitch = ((self.ragdoll_time * 22.0).sin() * (3.0 - self.ragdoll_time).max(0.0) * 0.18) as f32;
                offset = self.impact_velocity.normalize_or_zero() * 0.5;
                offset.y = twitch;
                rotation = Quat::from_euler(glam::EulerRot::XYZ, 1.5, twitch * 2.0, 0.0);
            }
            DeathPhase::CurlingUp => {
                // Curl legs up (scale down to simulate)
                let curl_progress = ((self.ragdoll_time - 2.0) / 1.5).clamp(0.0, 1.0);
                scale = 1.0 - curl_progress * 0.3;
                offset = self.impact_velocity.normalize_or_zero() * 0.5;
                rotation = Quat::from_euler(glam::EulerRot::XYZ, 1.5 + curl_progress * 0.3, 0.0, 0.0);
            }
            DeathPhase::Dead => {
                scale = 0.7;
                offset = self.impact_velocity.normalize_or_zero() * 0.5;
                rotation = Quat::from_euler(glam::EulerRot::XYZ, 1.8, 0.0, 0.0);
            }
            _ => {}
        }

        (offset, rotation, scale)
    }
}

/// Gore splatter effect
#[derive(Debug, Clone)]
pub struct GoreSplatter {
    pub position: Vec3,
    pub normal: Vec3,
    pub size: f32,
    pub age: f32,
    pub splatter_type: GoreType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GoreType {
    Splat,
    Drip,
    Pool,
    Spray,
}

/// Bullet impact effect
#[derive(Debug, Clone)]
pub struct BulletImpact {
    pub position: Vec3,
    pub normal: Vec3,
    pub age: f32,
    pub is_blood: bool, // true for bug hit, false for terrain
}

/// Muzzle flash effect
#[derive(Debug, Clone)]
pub struct MuzzleFlash {
    pub position: Vec3,
    pub direction: Vec3,
    pub age: f32,
    pub intensity: f32,
}

/// Single explosion particle (Tac Fighter strikes: fire/smoke billboard)
#[derive(Debug, Clone)]
pub struct ExplosionParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub life: f32,
    pub max_life: f32,
    pub size: f32,
    pub phase: f32,
    /// 0 = fire core, 1 = orange, 2 = dark smoke
    pub kind: u8,
}

/// Ground track (footprint / trail) in snow or sand — Dune / Helldivers 2 style.
#[derive(Debug, Clone)]
pub struct GroundTrack {
    pub position: Vec3,
    /// Yaw in radians for footprint orientation (direction of travel).
    pub rotation_y: f32,
    pub age: f32,
    pub size: f32,
    pub kind: TrackKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    /// Trooper boot print (small oval).
    TrooperFoot,
    /// Bug leg / body trail (larger, elongated).
    BugFoot,
    /// Shovel dig mark (circular, for snow/sand).
    ShovelDig,
}

/// Complete bug spawn with all components
pub fn spawn_complete_bug(
    world: &mut World,
    physics: &mut PhysicsWorld,
    bug_type: BugType,
    position: Vec3,
) -> Entity {
    let bug = Bug::new(bug_type);
    let scale = bug_type.scale();

    // Create physics body
    let body_handle = physics.add_kinematic_body(position);
    let collider_handle = physics.add_capsule_collider(body_handle, scale.y * 0.5, scale.x * 0.5);

    let physics_bug = PhysicsBug {
        body_handle: Some(body_handle),
        collider_handle: Some(collider_handle),
        ..Default::default()
    };

    world.spawn((
        Transform {
            position,
            rotation: Quat::IDENTITY,
            scale,
        },
        Velocity::default(),
        Health::new(bug_type.health()),
        bug,
        physics_bug,
        engine_core::AIComponent::new(85.0, 2.5, 1.0),  // Extermination: large aggro
    ))
}

/// System to update bug physics and death animations
pub fn update_bug_physics(world: &mut World, physics: &mut PhysicsWorld, dt: f32) {
    // Collect bugs that need ragdoll activation
    let mut to_ragdoll: Vec<(Entity, Vec3)> = Vec::new();

    for (entity, (health, physics_bug, transform)) in
        world.query_mut::<(&Health, &mut PhysicsBug, &Transform)>()
    {
        if health.is_dead() && !physics_bug.is_ragdoll {
            // Random death direction if no impact recorded
            let impact_dir = if physics_bug.impact_velocity.length() > 0.1 {
                physics_bug.impact_velocity.normalize()
            } else {
                let mut rng = rand::thread_rng();
                Vec3::new(
                    rng.gen_range(-1.0..1.0),
                    0.5,
                    rng.gen_range(-1.0..1.0),
                ).normalize()
            };
            to_ragdoll.push((entity, impact_dir));
        }

        // Update death animation
        physics_bug.update_death(dt);
    }

    // Activate ragdolls
    for (entity, impact_dir) in to_ragdoll {
        if let Ok(mut physics_bug) = world.get::<&mut PhysicsBug>(entity) {
            physics_bug.activate_ragdoll(impact_dir, 8.0);

            // Convert to dynamic body for ragdoll — Euphoria-style violent death
            if let Some(handle) = physics_bug.body_handle {
                if let Some(body) = physics.rigid_body_set.get_mut(handle) {
                    use physics::rapier3d::prelude::RigidBodyType;
                    use physics::rapier3d::na::Vector3;
                    body.set_body_type(RigidBodyType::Dynamic, true);
                    // Euphoria-style: explosive death impulse — body flies back violently
                    let impulse = impact_dir * 18.0 + Vec3::Y * 6.0;
                    body.apply_impulse(
                        Vector3::new(impulse.x, impulse.y, impulse.z),
                        true,
                    );
                    // Chaotic tumble — limbs flying, dismemberment feel
                    body.apply_torque_impulse(
                        Vector3::new(
                            rand::random::<f32>() * 12.0 - 6.0,
                            rand::random::<f32>() * 12.0 - 6.0,
                            rand::random::<f32>() * 12.0 - 6.0,
                        ),
                        true,
                    );
                }
            }
        }
    }

    // Sync physics transforms
    for (_entity, (transform, physics_bug)) in
        world.query_mut::<(&mut Transform, &PhysicsBug)>()
    {
        if physics_bug.is_ragdoll {
            if let Some(handle) = physics_bug.body_handle {
                if let Some(body) = physics.rigid_body_set.get(handle) {
                    let pos = body.translation();
                    let rot = body.rotation();
                    transform.position = Vec3::new(pos.x, pos.y, pos.z);
                    transform.rotation = Quat::from_xyzw(rot.i, rot.j, rot.k, rot.w);
                }
            }
        }
    }
}

/// Effect manager for gore, impacts, muzzle flashes, explosions, and ground tracks
#[derive(Default)]
pub struct EffectsManager {
    pub gore_splatters: Vec<GoreSplatter>,
    pub bullet_impacts: Vec<BulletImpact>,
    pub muzzle_flashes: Vec<MuzzleFlash>,
    pub explosion_particles: Vec<ExplosionParticle>,
    /// Footprints and trails in snow/sand (Dune / Helldivers 2 style)
    pub ground_tracks: Vec<GroundTrack>,
    pub max_gore: usize,
    pub max_impacts: usize,
    pub max_explosion_particles: usize,
    pub max_ground_tracks: usize,
}

impl EffectsManager {
    pub fn new() -> Self {
        Self {
            gore_splatters: Vec::new(),
            bullet_impacts: Vec::new(),
            muzzle_flashes: Vec::new(),
            explosion_particles: Vec::new(),
            ground_tracks: Vec::new(),
            max_gore: 400,
            max_impacts: 100,
            max_explosion_particles: 800,
            max_ground_tracks: 450,
        }
    }

    /// Spawn a ground track (footprint / trail) at the given position. Used on snow/sand biomes.
    pub fn spawn_ground_track(&mut self, position: Vec3, rotation_y: f32, kind: TrackKind) {
        let size = match kind {
            TrackKind::TrooperFoot => 0.14,
            TrackKind::BugFoot => 0.28,
            TrackKind::ShovelDig => 0.5,
        };
        self.ground_tracks.push(GroundTrack {
            position,
            rotation_y,
            age: 0.0,
            size,
            kind,
        });
        while self.ground_tracks.len() > self.max_ground_tracks {
            self.ground_tracks.remove(0);
        }
    }

    pub fn spawn_gore(&mut self, position: Vec3, _normal: Vec3, size: f32) {
        // Euphoria-style: explosion of bug guts — dense splatters, sprays, pools
        let mut rng = rand::thread_rng();

        let ground_pos = Vec3::new(position.x, 0.02, position.z);

        // Main splat (larger, more visceral)
        self.gore_splatters.push(GoreSplatter {
            position: ground_pos,
            normal: Vec3::Y,
            size: size * 1.2,
            age: 0.0,
            splatter_type: GoreType::Splat,
        });

        // Dense spray — explosion of guts in all directions
        for _ in 0..12 {
            let offset = Vec3::new(
                rng.gen_range(-1.5..1.5) * size,
                0.0,
                rng.gen_range(-1.5..1.5) * size,
            );
            self.gore_splatters.push(GoreSplatter {
                position: ground_pos + offset,
                normal: Vec3::Y,
                size: size * rng.gen_range(0.25..0.7),
                age: 0.0,
                splatter_type: GoreType::Spray,
            });
        }

        // Drips and secondary splats
        for _ in 0..6 {
            let offset = Vec3::new(
                rng.gen_range(-1.0..1.0) * size,
                0.0,
                rng.gen_range(-1.0..1.0) * size,
            );
            self.gore_splatters.push(GoreSplatter {
                position: ground_pos + offset,
                normal: Vec3::Y,
                size: size * rng.gen_range(0.2..0.5),
                age: 0.0,
                splatter_type: GoreType::Drip,
            });
        }

        // Large ground pool (blood/ichor pool)
        self.gore_splatters.push(GoreSplatter {
            position: Vec3::new(position.x, 0.01, position.z),
            normal: Vec3::Y,
            size: size * 2.2,
            age: 0.0,
            splatter_type: GoreType::Pool,
        });

        // Limit total gore
        while self.gore_splatters.len() > self.max_gore {
            self.gore_splatters.remove(0);
        }
    }

    pub fn spawn_bullet_impact(&mut self, position: Vec3, normal: Vec3, is_blood: bool) {
        self.bullet_impacts.push(BulletImpact {
            position,
            normal,
            age: 0.0,
            is_blood,
        });

        while self.bullet_impacts.len() > self.max_impacts {
            self.bullet_impacts.remove(0);
        }
    }

    pub fn spawn_muzzle_flash(&mut self, position: Vec3, direction: Vec3) {
        self.muzzle_flashes.push(MuzzleFlash {
            position,
            direction,
            age: 0.0,
            intensity: 1.0,
        });
    }

    /// Spawn Tac Fighter impact explosion: fire/smoke billboard particles (flat billboard look).
    pub fn spawn_tac_explosion(&mut self, center: Vec3) {
        let mut rng = rand::thread_rng();
        let available = self.max_explosion_particles.saturating_sub(self.explosion_particles.len());
        let count = 120.min(available);

        for _ in 0..count {
            let angle = rng.gen::<f32>() * std::f32::consts::TAU;
            let dist = rng.gen::<f32>() * 6.0;
            let speed_out = 8.0 + rng.gen::<f32>() * 18.0;
            let up = 5.0 + rng.gen::<f32>() * 25.0;
            let pos = center + Vec3::new(angle.cos() * dist * 0.5, rng.gen::<f32>() * 2.0, angle.sin() * dist * 0.5);
            let vel = Vec3::new(
                angle.cos() * speed_out,
                up,
                angle.sin() * speed_out,
            );
            let kind = if rng.gen::<f32>() < 0.35 { 0 } else if rng.gen::<f32>() < 0.6 { 1 } else { 2 };
            let max_life = match kind {
                0 => 0.4 + rng.gen::<f32>() * 0.3,
                1 => 0.6 + rng.gen::<f32>() * 0.5,
                _ => 1.0 + rng.gen::<f32>() * 0.8,
            };
            self.explosion_particles.push(ExplosionParticle {
                position: pos,
                velocity: vel,
                life: max_life,
                max_life,
                size: 0.5 + rng.gen::<f32>() * 2.0,
                phase: rng.gen::<f32>() * std::f32::consts::TAU,
                kind,
            });
        }
    }

    pub fn update(&mut self, dt: f32) {
        // Update gore
        for gore in &mut self.gore_splatters {
            gore.age += dt;
        }
        // Remove old gore (keep for 30 seconds)
        self.gore_splatters.retain(|g| g.age < 30.0);

        // Update ground tracks (footprints in snow/sand)
        for track in &mut self.ground_tracks {
            track.age += dt;
        }
        self.ground_tracks.retain(|t| t.age < 120.0);

        // Update impacts
        for impact in &mut self.bullet_impacts {
            impact.age += dt;
        }
        self.bullet_impacts.retain(|i| i.age < 2.0);

        // Update muzzle flashes
        for flash in &mut self.muzzle_flashes {
            flash.age += dt;
            flash.intensity = (1.0 - flash.age * 20.0).max(0.0);
        }
        self.muzzle_flashes.retain(|f| f.age < 0.1);

        // Update explosion particles (Tac Fighter impacts)
        for p in &mut self.explosion_particles {
            p.life -= dt;
            p.velocity.y -= 15.0 * dt; // gravity
            p.velocity *= 1.0 - 2.0 * dt; // drag
            p.position += p.velocity * dt;
            let age_frac = 1.0 - (p.life / p.max_life);
            p.size *= 1.0 + dt * 2.0; // expand as it rises
        }
        self.explosion_particles.retain(|p| p.life > 0.0);
    }
}

