//! Weapon system with multiple weapon types.

use engine_core::{Damage, DamageType, Health, Lifetime, Transform, Velocity, Vec3};
use hecs::World;
use physics::{PhysicsWorld, RaycastHit};

/// Weapon types available to the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponType {
    /// Standard assault rifle - high fire rate, medium damage.
    Rifle,
    /// Shotgun - spread, high damage up close.
    Shotgun,
    /// Sniper - slow, high damage, penetration.
    Sniper,
    /// Rocket launcher - AOE explosive damage.
    Rocket,
    /// Flamethrower - continuous fire damage.
    Flamethrower,
    /// Morita MG - Starship Troopers machine gun: high rate, large magazine, horde shredder.
    MachineGun,
}

/// Weapon instance with current state.
#[derive(Debug, Clone)]
pub struct Weapon {
    pub weapon_type: WeaponType,
    pub damage: f32,
    pub fire_rate: f32, // Shots per second
    pub reload_time: f32,
    pub magazine_size: u32,
    pub current_ammo: u32,
    pub reserve_ammo: u32,
    pub range: f32,
    pub spread: f32, // In degrees
    pub projectile_count: u32, // For shotgun
    
    // State
    pub fire_cooldown: f32,
    pub reload_timer: f32,
    pub is_reloading: bool,
}

impl Weapon {
    pub fn new(weapon_type: WeaponType) -> Self {
        let (damage, fire_rate, reload_time, magazine_size, reserve, range, spread, projectiles) =
            match weapon_type {
                WeaponType::Rifle => (25.0, 10.0, 2.0, 30, 180, 100.0, 2.0, 1),
                WeaponType::Shotgun => (15.0, 1.5, 2.5, 8, 48, 30.0, 8.0, 8),
                WeaponType::Sniper => (150.0, 0.8, 3.0, 5, 30, 500.0, 0.5, 1),
                WeaponType::Rocket => (200.0, 0.5, 3.5, 1, 12, 200.0, 0.0, 1),
                WeaponType::Flamethrower => (5.0, 30.0, 0.0, 100, 300, 15.0, 10.0, 1),
                WeaponType::MachineGun => (18.0, 18.0, 4.0, 200, 600, 120.0, 3.0, 1), // Morita MG: shreds hordes
            };

        Self {
            weapon_type,
            damage,
            fire_rate,
            reload_time,
            magazine_size,
            current_ammo: magazine_size,
            reserve_ammo: reserve,
            range,
            spread,
            projectile_count: projectiles,
            fire_cooldown: 0.0,
            reload_timer: 0.0,
            is_reloading: false,
        }
    }

    /// Update weapon state.
    pub fn update(&mut self, dt: f32) {
        // Update cooldowns
        if self.fire_cooldown > 0.0 {
            self.fire_cooldown -= dt;
        }

        // Handle reloading
        if self.is_reloading {
            self.reload_timer -= dt;
            if self.reload_timer <= 0.0 {
                self.finish_reload();
            }
        }
    }

    /// Check if weapon can fire.
    pub fn can_fire(&self) -> bool {
        self.fire_cooldown <= 0.0 && self.current_ammo > 0 && !self.is_reloading
    }

    /// Fire the weapon, consuming ammo.
    pub fn fire(&mut self) -> bool {
        if !self.can_fire() {
            return false;
        }

        self.current_ammo -= 1;
        self.fire_cooldown = 1.0 / self.fire_rate;
        true
    }

    /// Start reloading.
    pub fn start_reload(&mut self) {
        if self.is_reloading || self.reserve_ammo == 0 || self.current_ammo == self.magazine_size {
            return;
        }

        self.is_reloading = true;
        self.reload_timer = self.reload_time;
    }

    /// Finish reloading.
    fn finish_reload(&mut self) {
        let needed = self.magazine_size - self.current_ammo;
        let available = needed.min(self.reserve_ammo);

        self.current_ammo += available;
        self.reserve_ammo -= available;
        self.is_reloading = false;
        self.reload_timer = 0.0;
    }

    /// Check if reloading.
    pub fn is_reloading(&self) -> bool {
        self.is_reloading
    }

    /// Get ammo display string.
    pub fn ammo_display(&self) -> String {
        if self.is_reloading {
            format!("RELOADING... {}", self.reserve_ammo)
        } else {
            format!("{} / {}", self.current_ammo, self.reserve_ammo)
        }
    }
}

/// Manages weapon firing and projectiles.
pub struct WeaponSystem {
    /// Projectile entities in flight.
    projectiles: Vec<hecs::Entity>,
}

impl WeaponSystem {
    pub fn new() -> Self {
        Self {
            projectiles: Vec::new(),
        }
    }

    /// Fire a weapon and return hit info if hitscan.
    pub fn fire(
        &mut self,
        weapon: &Weapon,
        origin: Vec3,
        direction: Vec3,
        physics: &PhysicsWorld,
    ) -> Option<RaycastHit> {
        match weapon.weapon_type {
            WeaponType::Rifle | WeaponType::Sniper | WeaponType::MachineGun => {
                // Hitscan weapons (MG = Morita machine gun)
                self.fire_hitscan(origin, direction, weapon.range, weapon.spread, physics)
            }
            WeaponType::Shotgun => {
                // Multiple pellets
                let mut closest_hit: Option<RaycastHit> = None;
                for _ in 0..weapon.projectile_count {
                    if let Some(hit) = self.fire_hitscan(origin, direction, weapon.range, weapon.spread, physics) {
                        let is_closer = match &closest_hit {
                            None => true,
                            Some(prev) => hit.distance < prev.distance,
                        };
                        if is_closer {
                            closest_hit = Some(hit);
                        }
                    }
                }
                closest_hit
            }
            WeaponType::Rocket | WeaponType::Flamethrower => {
                // Projectile weapons - would spawn projectile entity
                // For now, use simplified hitscan
                self.fire_hitscan(origin, direction, weapon.range, weapon.spread, physics)
            }
        }
    }

    /// Fire a hitscan ray with optional spread.
    fn fire_hitscan(
        &self,
        origin: Vec3,
        direction: Vec3,
        range: f32,
        spread: f32,
        physics: &PhysicsWorld,
    ) -> Option<RaycastHit> {
        // Apply spread
        let spread_rad = spread.to_radians();
        let mut rng = rand::thread_rng();
        
        let spread_x = (rand::Rng::gen_range(&mut rng, -spread_rad..spread_rad)) as f32;
        let spread_y = (rand::Rng::gen_range(&mut rng, -spread_rad..spread_rad)) as f32;

        let spread_rotation = glam::Quat::from_euler(glam::EulerRot::XYZ, spread_x, spread_y, 0.0);
        let spread_direction = spread_rotation * direction;

        physics.raycast(origin, spread_direction.normalize(), range)
    }

    /// Start reload on a weapon.
    pub fn reload(&self, weapon: &mut Weapon) {
        weapon.start_reload();
    }

    /// Update projectiles and damage.
    pub fn update(&mut self, world: &mut World, dt: f32) {
        // Update projectile lifetimes
        let mut expired = Vec::new();
        for (entity, lifetime) in world.query_mut::<&mut Lifetime>() {
            if lifetime.update(dt) {
                expired.push(entity);
            }
        }

        // Remove expired projectiles
        for entity in expired {
            world.despawn(entity).ok();
            self.projectiles.retain(|&e| e != entity);
        }
    }

    /// Apply damage to a target entity.
    pub fn apply_damage(world: &mut World, target: hecs::Entity, damage: &Damage) -> bool {
        if let Ok(mut health) = world.get::<&mut Health>(target) {
            health.take_damage(damage.amount);
            return health.is_dead();
        }
        false
    }

    /// Apply AOE damage around a point.
    pub fn apply_aoe_damage(
        world: &mut World,
        center: Vec3,
        radius: f32,
        damage: f32,
        _damage_type: DamageType,
    ) -> Vec<hecs::Entity> {
        let mut killed = Vec::new();

        // Collect entities in range
        let in_range: Vec<(hecs::Entity, Vec3)> = world
            .query::<&Transform>()
            .iter()
            .filter_map(|(entity, transform)| {
                let dist = transform.position.distance(center);
                if dist <= radius {
                    Some((entity, transform.position))
                } else {
                    None
                }
            })
            .collect();

        // Apply damage with falloff
        for (entity, pos) in in_range {
            let dist = pos.distance(center);
            let falloff = 1.0 - (dist / radius);
            let actual_damage = damage * falloff;

            if let Ok(mut health) = world.get::<&mut Health>(entity) {
                health.take_damage(actual_damage);
                if health.is_dead() {
                    killed.push(entity);
                }
            }
        }

        killed
    }
}

impl Default for WeaponSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Projectile component for non-hitscan weapons.
#[derive(Debug, Clone)]
pub struct Projectile {
    pub damage: f32,
    pub damage_type: DamageType,
    pub explosion_radius: Option<f32>,
    pub owner: Option<hecs::Entity>,
}
