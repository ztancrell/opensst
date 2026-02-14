//! First-Person Shooter core gameplay systems
//! Similar to Starship Troopers: Extermination

use engine_core::{Health, Transform, Vec3};
use glam::Quat;
use hecs::{Entity, World};
use std::collections::HashMap;

use crate::bug::{Bug, BugType};
use crate::skinny::Skinny;
use crate::weapons::{Weapon, WeaponType};

/// Player class types (similar to STE)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayerClass {
    /// Hunter - Assault class with jetpack burst
    Hunter,
    /// Bastion - Heavy weapons and fortification
    Bastion,
    /// Operator - Support class with deployables
    Operator,
    /// Ranger - Long range specialist
    Ranger,
    /// Guardian - Defense and shields
    Guardian,
}

impl PlayerClass {
    pub fn loadout(&self) -> ClassLoadout {
        match self {
            PlayerClass::Hunter => ClassLoadout {
                primary: WeaponType::Rifle,
                secondary: WeaponType::Shotgun,
                tertiary: WeaponType::MachineGun,
                max_health: 100.0,
                move_speed: 8.0,
                sprint_multiplier: 1.6,
                ability: ClassAbility::JetpackBurst,
                ability_cooldown: 15.0,
            },
            PlayerClass::Bastion => ClassLoadout {
                primary: WeaponType::Rifle,
                secondary: WeaponType::Rocket,
                tertiary: WeaponType::MachineGun,
                max_health: 150.0,
                move_speed: 6.0,
                sprint_multiplier: 1.3,
                ability: ClassAbility::DeployBarricade,
                ability_cooldown: 30.0,
            },
            PlayerClass::Operator => ClassLoadout {
                primary: WeaponType::Rifle,
                secondary: WeaponType::Shotgun,
                tertiary: WeaponType::MachineGun,
                max_health: 100.0,
                move_speed: 7.0,
                sprint_multiplier: 1.5,
                ability: ClassAbility::AmmoStation,
                ability_cooldown: 45.0,
            },
            PlayerClass::Ranger => ClassLoadout {
                primary: WeaponType::Sniper,
                secondary: WeaponType::Rifle,
                tertiary: WeaponType::MachineGun,
                max_health: 80.0,
                move_speed: 7.5,
                sprint_multiplier: 1.5,
                ability: ClassAbility::ScanPulse,
                ability_cooldown: 20.0,
            },
            PlayerClass::Guardian => ClassLoadout {
                primary: WeaponType::Shotgun,
                secondary: WeaponType::Rifle,
                tertiary: WeaponType::MachineGun,
                max_health: 125.0,
                move_speed: 6.5,
                sprint_multiplier: 1.4,
                ability: ClassAbility::ShieldDome,
                ability_cooldown: 40.0,
            },
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            PlayerClass::Hunter => "Hunter",
            PlayerClass::Bastion => "Bastion",
            PlayerClass::Operator => "Operator",
            PlayerClass::Ranger => "Ranger",
            PlayerClass::Guardian => "Guardian",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClassLoadout {
    pub primary: WeaponType,
    pub secondary: WeaponType,
    pub tertiary: WeaponType,  // Slot 3: MachineGun for all classes
    pub max_health: f32,
    pub move_speed: f32,
    pub sprint_multiplier: f32,
    pub ability: ClassAbility,
    pub ability_cooldown: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassAbility {
    /// Jump boost in any direction
    JetpackBurst,
    /// Deploy a defensive wall
    DeployBarricade,
    /// Deploy ammo resupply
    AmmoStation,
    /// Reveal nearby bugs
    ScanPulse,
    /// Protective dome shield
    ShieldDome,
}

/// FPS Player state
#[derive(Debug)]
pub struct FPSPlayer {
    // Identity
    pub class: PlayerClass,
    pub callsign: String,

    // Health & Status
    pub health: f32,
    pub max_health: f32,
    pub armor: f32,
    pub max_armor: f32,
    pub is_alive: bool,
    pub respawn_timer: f32,

    // Movement
    pub position: Vec3,
    pub velocity: Vec3,
    pub look_direction: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub move_speed: f32,
    pub sprint_multiplier: f32,
    pub is_sprinting: bool,
    pub is_crouching: bool,
    pub is_prone: bool,
    pub is_grounded: bool,
    pub stamina: f32,
    pub max_stamina: f32,

    // Combat (slot 0=primary, 1=secondary, 2=machine gun, 3=entrenching shovel)
    pub weapons: [Weapon; 3],
    pub current_weapon_slot: usize,
    pub is_aiming: bool,
    pub aim_progress: f32, // 0 = hip, 1 = ADS
    pub last_damage_time: f32,
    pub damage_direction: Option<Vec3>,

    // Ability
    pub ability: ClassAbility,
    pub ability_cooldown: f32,
    pub ability_timer: f32,
    pub ability_active: bool,

    // Stats
    pub kills: u32,
    pub deaths: u32,
    pub damage_dealt: f32,
    pub damage_taken: f32,

    // Interaction
    pub interaction_target: Option<InteractionTarget>,
    pub carried_resources: u32,
}

#[derive(Debug, Clone)]
pub enum InteractionTarget {
    AmmoCrate(Vec3),
    HealthStation(Vec3),
    Objective(String, Vec3),
    Revivable(Entity),
}

impl FPSPlayer {
    pub fn new(class: PlayerClass, callsign: String, spawn_position: Vec3) -> Self {
        let loadout = class.loadout();

        Self {
            class,
            callsign,
            health: loadout.max_health,
            max_health: loadout.max_health,
            armor: 0.0,
            max_armor: 50.0,
            is_alive: true,
            respawn_timer: 0.0,

            position: spawn_position,
            velocity: Vec3::ZERO,
            look_direction: Vec3::NEG_Z,
            yaw: 0.0,
            pitch: 0.0,
            move_speed: loadout.move_speed,
            sprint_multiplier: loadout.sprint_multiplier,
            is_sprinting: false,
            is_crouching: false,
            is_prone: false,
            is_grounded: true,
            stamina: 100.0,
            max_stamina: 100.0,

            weapons: [
                Weapon::new(loadout.primary),
                Weapon::new(loadout.secondary),
                Weapon::new(loadout.tertiary),
            ],
            current_weapon_slot: 0,
            is_aiming: false,
            aim_progress: 0.0,
            last_damage_time: -10.0,
            damage_direction: None,

            ability: loadout.ability,
            ability_cooldown: loadout.ability_cooldown,
            ability_timer: 0.0,
            ability_active: false,

            kills: 0,
            deaths: 0,
            damage_dealt: 0.0,
            damage_taken: 0.0,

            interaction_target: None,
            carried_resources: 0,
        }
    }

    /// Slot index for the entrenching shovel (key 4).
    pub const SHOVEL_SLOT: usize = 3;
    /// Total equipment slots (3 weapons + shovel).
    pub const TOTAL_SLOTS: usize = 4;

    pub fn is_shovel_equipped(&self) -> bool {
        self.current_weapon_slot == Self::SHOVEL_SLOT
    }

    pub fn current_weapon(&self) -> &Weapon {
        &self.weapons[self.current_weapon_slot]
    }

    pub fn current_weapon_mut(&mut self) -> &mut Weapon {
        &mut self.weapons[self.current_weapon_slot]
    }

    pub fn switch_weapon(&mut self) {
        self.current_weapon_slot = (self.current_weapon_slot + 1) % Self::TOTAL_SLOTS;
        self.is_aiming = false;
        self.aim_progress = 0.0;
    }

    pub fn set_weapon_slot(&mut self, slot: usize) {
        if slot < Self::TOTAL_SLOTS {
            self.current_weapon_slot = slot;
            self.is_aiming = false;
            self.aim_progress = 0.0;
        }
    }

    pub fn take_damage(&mut self, amount: f32, from_direction: Option<Vec3>) {
        if !self.is_alive {
            return;
        }

        // Armor absorbs first
        let armor_absorbed = amount.min(self.armor);
        self.armor -= armor_absorbed;
        let remaining = amount - armor_absorbed;

        self.health -= remaining;
        self.damage_taken += amount;
        self.last_damage_time = 0.0;
        self.damage_direction = from_direction;

        if self.health <= 0.0 {
            self.health = 0.0;
            self.die();
        }
    }

    pub fn die(&mut self) {
        self.is_alive = false;
        self.deaths += 1;
        self.respawn_timer = 10.0; // 10 second respawn
        log::info!("{} was killed! Deaths: {}", self.callsign, self.deaths);
    }

    pub fn respawn(&mut self, position: Vec3) {
        self.is_alive = true;
        self.health = self.max_health;
        self.armor = 0.0;
        self.position = position;
        self.velocity = Vec3::ZERO;
        self.respawn_timer = 0.0;

        // Refill ammo
        for weapon in &mut self.weapons {
            weapon.current_ammo = weapon.magazine_size;
            weapon.reserve_ammo = weapon.magazine_size * 4;
        }
    }

    pub fn heal(&mut self, amount: f32) {
        self.health = (self.health + amount).min(self.max_health);
    }

    pub fn add_armor(&mut self, amount: f32) {
        self.armor = (self.armor + amount).min(self.max_armor);
    }

    pub fn add_ammo(&mut self, amount: u32) {
        self.current_weapon_mut().reserve_ammo += amount;
    }

    pub fn can_use_ability(&self) -> bool {
        self.ability_timer <= 0.0 && self.is_alive
    }

    pub fn use_ability(&mut self) -> bool {
        if !self.can_use_ability() {
            return false;
        }

        self.ability_timer = self.ability_cooldown;
        self.ability_active = true;
        true
    }

    pub fn update(&mut self, dt: f32) {
        // Update weapons
        for weapon in &mut self.weapons {
            weapon.update(dt);
        }

        // Update ability cooldown
        if self.ability_timer > 0.0 {
            self.ability_timer -= dt;
        }

        // Update respawn
        if !self.is_alive && self.respawn_timer > 0.0 {
            self.respawn_timer -= dt;
        }

        // Update stamina
        if self.is_sprinting && self.is_grounded {
            self.stamina -= 20.0 * dt;
            if self.stamina <= 0.0 {
                self.stamina = 0.0;
                self.is_sprinting = false;
            }
        } else {
            self.stamina = (self.stamina + 15.0 * dt).min(self.max_stamina);
        }

        // Update ADS — deliberate transition (Helldivers 2 / SST Extermination feel)
        let aim_in_speed = 5.0;   // slower raise for tactical feel
        let aim_out_speed = 7.0;  // slightly faster drop when releasing
        if self.is_aiming {
            self.aim_progress = (self.aim_progress + aim_in_speed * dt).min(1.0);
        } else {
            self.aim_progress = (self.aim_progress - aim_out_speed * dt).max(0.0);
        }

        // Update damage indicator timer
        self.last_damage_time += dt;

        // Clear damage direction after a bit
        if self.last_damage_time > 1.0 {
            self.damage_direction = None;
        }
    }

    pub fn health_percent(&self) -> f32 {
        self.health / self.max_health
    }

    pub fn armor_percent(&self) -> f32 {
        if self.max_armor > 0.0 {
            self.armor / self.max_armor
        } else {
            0.0
        }
    }

    pub fn stamina_percent(&self) -> f32 {
        self.stamina / self.max_stamina
    }

    pub fn ability_ready_percent(&self) -> f32 {
        if self.ability_cooldown > 0.0 {
            1.0 - (self.ability_timer / self.ability_cooldown).max(0.0)
        } else {
            1.0
        }
    }
}

/// Combat hit result
#[derive(Debug, Clone)]
pub struct HitResult {
    pub entity: Entity,
    pub position: Vec3,
    pub normal: Vec3,
    pub distance: f32,
    pub damage_dealt: f32,
    pub was_kill: bool,
    pub was_headshot: bool,
    pub bug_type: Option<BugType>,
}

/// Combat system for FPS gameplay
pub struct CombatSystem {
    /// Damage numbers to display
    pub damage_numbers: Vec<DamageNumber>,
    /// Hit markers
    pub hit_markers: Vec<HitMarker>,
    /// Kill feed entries
    pub kill_feed: Vec<KillFeedEntry>,
}

#[derive(Debug, Clone)]
pub struct DamageNumber {
    pub position: Vec3,
    pub damage: f32,
    pub is_critical: bool,
    pub lifetime: f32,
    pub velocity: Vec3,
}

#[derive(Debug, Clone)]
pub struct HitMarker {
    pub is_kill: bool,
    pub is_headshot: bool,
    pub lifetime: f32,
}

#[derive(Debug, Clone)]
pub struct KillFeedEntry {
    pub killer: String,
    pub victim: String,
    pub weapon: WeaponType,
    pub was_headshot: bool,
    pub lifetime: f32,
}

impl CombatSystem {
    pub fn new() -> Self {
        Self {
            damage_numbers: Vec::new(),
            hit_markers: Vec::new(),
            kill_feed: Vec::new(),
        }
    }

    /// Process a weapon hit against bugs
    pub fn process_hit(
        &mut self,
        world: &mut World,
        player: &mut FPSPlayer,
        hit_position: Vec3,
        hit_entity: Entity,
        weapon: &Weapon,
    ) -> Option<HitResult> {
        // Check if we hit a bug
        let bug_query = world.query_one::<(&Transform, &mut Health, &Bug)>(hit_entity);
        if let Ok(mut query) = bug_query {
            if let Some((transform, health, bug)) = query.get() {
                // Calculate damage (headshots for upper body hits)
                let hit_height = hit_position.y - transform.position.y;
                let bug_height = transform.scale.y;
                let is_headshot = hit_height > bug_height * 0.7;

                let mut damage = weapon.damage;
                if is_headshot {
                    damage *= 2.0; // Headshot multiplier
                }

                // Apply damage
                health.take_damage(damage);
                let was_kill = health.is_dead();
                let bug_type = bug.bug_type;

                // Track stats
                player.damage_dealt += damage;
                if was_kill {
                    player.kills += 1;
                }

                // Add damage number
                self.damage_numbers.push(DamageNumber {
                    position: hit_position + Vec3::Y * 0.5,
                    damage,
                    is_critical: is_headshot,
                    lifetime: 1.0,
                    velocity: Vec3::new(
                        rand::random::<f32>() * 2.0 - 1.0,
                        3.0,
                        rand::random::<f32>() * 2.0 - 1.0,
                    ),
                });

                // Add hit marker
                self.hit_markers.push(HitMarker {
                    is_kill: was_kill,
                    is_headshot,
                    lifetime: 0.3,
                });

                // Add kill feed entry
                if was_kill {
                    self.kill_feed.push(KillFeedEntry {
                        killer: player.callsign.clone(),
                        victim: format!("{:?}", bug_type),
                        weapon: weapon.weapon_type,
                        was_headshot: is_headshot,
                        lifetime: 5.0,
                    });
                }

                return Some(HitResult {
                    entity: hit_entity,
                    position: hit_position,
                    normal: Vec3::Y,
                    distance: (hit_position - player.position).length(),
                    damage_dealt: damage,
                    was_kill,
                    was_headshot: is_headshot,
                    bug_type: Some(bug_type),
                });
            }
        }

        // Check if we hit a Skinny (Heinlein humanoid enemy)
        let skinny_query = world.query_one::<(&Transform, &mut Health, &Skinny)>(hit_entity);
        if let Ok(mut query) = skinny_query {
            if let Some((transform, health, skinny)) = query.get() {
                let hit_height = hit_position.y - transform.position.y;
                let skinny_height = transform.scale.y;
                let is_headshot = hit_height > skinny_height * 0.7;

                let mut damage = weapon.damage;
                if is_headshot {
                    damage *= 2.0;
                }

                health.take_damage(damage);
                let was_kill = health.is_dead();
                let victim_name = format!("Skinny {:?}", skinny.skinny_type);

                player.damage_dealt += damage;
                if was_kill {
                    player.kills += 1;
                }

                self.damage_numbers.push(DamageNumber {
                    position: hit_position + Vec3::Y * 0.5,
                    damage,
                    is_critical: is_headshot,
                    lifetime: 1.0,
                    velocity: Vec3::new(
                        rand::random::<f32>() * 2.0 - 1.0,
                        3.0,
                        rand::random::<f32>() * 2.0 - 1.0,
                    ),
                });
                self.hit_markers.push(HitMarker {
                    is_kill: was_kill,
                    is_headshot,
                    lifetime: 0.3,
                });
                if was_kill {
                    self.kill_feed.push(KillFeedEntry {
                        killer: player.callsign.clone(),
                        victim: victim_name,
                        weapon: weapon.weapon_type,
                        was_headshot: is_headshot,
                        lifetime: 5.0,
                    });
                }

                return Some(HitResult {
                    entity: hit_entity,
                    position: hit_position,
                    normal: Vec3::Y,
                    distance: (hit_position - player.position).length(),
                    damage_dealt: damage,
                    was_kill,
                    was_headshot: is_headshot,
                    bug_type: None,
                });
            }
        }

        None
    }

    /// Update combat system (damage numbers, etc.)
    pub fn update(&mut self, dt: f32) {
        // Update damage numbers
        for dn in &mut self.damage_numbers {
            dn.lifetime -= dt;
            dn.position += dn.velocity * dt;
            dn.velocity.y -= 5.0 * dt; // Gravity
        }
        self.damage_numbers.retain(|dn| dn.lifetime > 0.0);

        // Update hit markers
        for hm in &mut self.hit_markers {
            hm.lifetime -= dt;
        }
        self.hit_markers.retain(|hm| hm.lifetime > 0.0);

        // Update kill feed
        for kf in &mut self.kill_feed {
            kf.lifetime -= dt;
        }
        self.kill_feed.retain(|kf| kf.lifetime > 0.0);
    }

    pub fn has_active_hit_marker(&self) -> bool {
        !self.hit_markers.is_empty()
    }

    pub fn latest_hit_marker(&self) -> Option<&HitMarker> {
        self.hit_markers.last()
    }
}

impl Default for CombatSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Bug or Skinny attack data
#[derive(Debug, Clone)]
pub struct BugAttack {
    pub bug_entity: Entity,
    pub bug_type: Option<BugType>,
    pub attack_range: f32,
    pub attack_damage: f32,
    pub attack_cooldown: f32,
    pub last_attack_time: f32,
}

/// System to handle bugs attacking the player
pub struct BugCombatSystem {
    attacks: HashMap<Entity, BugAttack>,
}

impl BugCombatSystem {
    pub fn new() -> Self {
        Self {
            attacks: HashMap::new(),
        }
    }

    /// Update bug attacks against player
    pub fn update(&mut self, world: &World, player: &mut FPSPlayer, dt: f32) {
        if !player.is_alive {
            return;
        }

        // Update attack timers
        for attack in self.attacks.values_mut() {
            attack.last_attack_time += dt;
        }

        // Check for bugs in attack range
        for (entity, (transform, bug)) in world.query::<(&Transform, &Bug)>().iter() {
            let distance = transform.position.distance(player.position);
            let attack_range = match bug.bug_type {
                BugType::Warrior => 2.5,
                BugType::Charger => 3.0,
                BugType::Tanker => 4.0,
                BugType::Hopper => 2.0,
                BugType::Spitter => 25.0, // Ranged
            };

            let attack = self.attacks.entry(entity).or_insert_with(|| BugAttack {
                bug_entity: entity,
                bug_type: Some(bug.bug_type),
                attack_range,
                attack_damage: bug.attack_damage,
                attack_cooldown: match bug.bug_type {
                    BugType::Warrior => 1.0,
                    BugType::Charger => 0.8,
                    BugType::Tanker => 2.0,
                    BugType::Hopper => 1.2,
                    BugType::Spitter => 3.0,
                },
                last_attack_time: 0.0,
            });

            if distance <= attack_range && attack.last_attack_time >= attack.attack_cooldown {
                let damage_direction = Some((transform.position - player.position).normalize());
                player.take_damage(attack.attack_damage, damage_direction);
                attack.last_attack_time = 0.0;
                log::debug!("{:?} attacked player for {} damage!", bug.bug_type, attack.attack_damage);
            }
        }

        // Skinnies (Heinlein): same chase/attack logic, different ranges
        for (entity, (transform, skinny)) in world.query::<(&Transform, &Skinny)>().iter() {
            let distance = transform.position.distance(player.position);
            let (attack_range, cooldown) = match skinny.skinny_type {
                crate::skinny::SkinnyType::Grunt => (2.5, 1.0),
                crate::skinny::SkinnyType::Sniper => (15.0, 2.5),
                crate::skinny::SkinnyType::Officer => (3.0, 1.2),
            };

            let attack = self.attacks.entry(entity).or_insert_with(|| BugAttack {
                bug_entity: entity,
                bug_type: None,
                attack_range,
                attack_damage: skinny.attack_damage,
                attack_cooldown: cooldown,
                last_attack_time: 0.0,
            });

            if distance <= attack_range && attack.last_attack_time >= attack.attack_cooldown {
                let damage_direction = Some((transform.position - player.position).normalize());
                player.take_damage(attack.attack_damage, damage_direction);
                attack.last_attack_time = 0.0;
                log::debug!("Skinny attacked player for {} damage!", attack.attack_damage);
            }
        }

        self.attacks.retain(|entity, _| world.contains(*entity));
    }
}

impl Default for BugCombatSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Mission state — combines Star Citizen scale, Helldivers 2 objectives, Starship Troopers tone.
#[derive(Debug, Clone)]
pub struct MissionState {
    pub mission_type: MissionType,
    /// Total bugs killed this deployment.
    pub bugs_killed: u32,
    /// Bugs currently alive on the field.
    pub bugs_remaining: u32,
    /// Time survived on-planet (seconds).
    pub time_elapsed: f32,
    /// Peak simultaneous bugs the trooper has faced.
    pub peak_bugs_alive: u32,
    /// Whether the trooper has fallen (all lives exhausted).
    pub is_failed: bool,
    /// Optional: kill this many bugs to complete (Bug Hunt).
    pub kill_target: Option<u32>,
    /// Optional: survive this many seconds to complete (Hold the Line).
    pub time_target_secs: Option<f32>,
    /// Set when objective is met; trooper can extract for full success.
    pub objective_complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionType {
    /// Infinite horde — survive and extract when ready (classic).
    Extermination,
    /// Bug Hunt: kill target count then extract (Helldivers 2 style).
    BugHunt,
    /// Hold the Line: survive for time then extract.
    HoldTheLine,
    /// Defend a location (future).
    Defense,
    /// Destroy bug hive (future).
    HiveDestruction,
}

impl MissionType {
    pub fn name(&self) -> &'static str {
        match self {
            MissionType::Extermination => "Extermination",
            MissionType::BugHunt => "Bug Hunt",
            MissionType::HoldTheLine => "Hold the Line",
            MissionType::Defense => "Defense",
            MissionType::HiveDestruction => "Hive Destruction",
        }
    }
}

impl MissionState {
    /// Create a new mission (default: Extermination, no target).
    pub fn new_horde() -> Self {
        Self {
            mission_type: MissionType::Extermination,
            bugs_killed: 0,
            bugs_remaining: 0,
            time_elapsed: 0.0,
            peak_bugs_alive: 0,
            is_failed: false,
            kill_target: None,
            time_target_secs: None,
            objective_complete: false,
        }
    }

    /// Create Bug Hunt: kill this many bugs then extract.
    pub fn new_bug_hunt(kill_target: u32) -> Self {
        Self {
            mission_type: MissionType::BugHunt,
            bugs_killed: 0,
            bugs_remaining: 0,
            time_elapsed: 0.0,
            peak_bugs_alive: 0,
            is_failed: false,
            kill_target: Some(kill_target),
            time_target_secs: None,
            objective_complete: false,
        }
    }

    /// Create Hold the Line: survive this many seconds then extract.
    pub fn new_hold_the_line(secs: f32) -> Self {
        Self {
            mission_type: MissionType::HoldTheLine,
            bugs_killed: 0,
            bugs_remaining: 0,
            time_elapsed: 0.0,
            peak_bugs_alive: 0,
            is_failed: false,
            kill_target: None,
            time_target_secs: Some(secs),
            objective_complete: false,
        }
    }

    /// Create Defense: hold the position for this many seconds (HD2 style).
    pub fn new_defense(secs: f32) -> Self {
        Self {
            mission_type: MissionType::Defense,
            bugs_killed: 0,
            bugs_remaining: 0,
            time_elapsed: 0.0,
            peak_bugs_alive: 0,
            is_failed: false,
            kill_target: None,
            time_target_secs: Some(secs),
            objective_complete: false,
        }
    }

    /// Create Hive Destruction: kill this many bugs (destroy hive presence).
    pub fn new_hive_destruction(kill_target: u32) -> Self {
        Self {
            mission_type: MissionType::HiveDestruction,
            bugs_killed: 0,
            bugs_remaining: 0,
            time_elapsed: 0.0,
            peak_bugs_alive: 0,
            is_failed: false,
            kill_target: Some(kill_target),
            time_target_secs: None,
            objective_complete: false,
        }
    }

    pub fn update(&mut self, dt: f32, _player_alive: bool) {
        self.time_elapsed += dt;

        if self.bugs_remaining > self.peak_bugs_alive {
            self.peak_bugs_alive = self.bugs_remaining;
        }

        // Check objectives
        if !self.objective_complete {
            match self.mission_type {
                MissionType::BugHunt | MissionType::HiveDestruction => {
                    if let Some(t) = self.kill_target {
                        if self.bugs_killed >= t {
                            self.objective_complete = true;
                        }
                    }
                }
                MissionType::HoldTheLine | MissionType::Defense => {
                    if let Some(t) = self.time_target_secs {
                        if self.time_elapsed >= t {
                            self.objective_complete = true;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Short objective string for HUD (e.g. "Kill 25 bugs" or "Survive 5:00").
    pub fn objective_text(&self) -> Option<String> {
        match self.mission_type {
            MissionType::BugHunt => self.kill_target.map(|t| format!("Kill {} bugs", t)),
            MissionType::HiveDestruction => self.kill_target.map(|t| format!("Destroy hive: {} kills", t)),
            MissionType::HoldTheLine => self.time_target_secs.map(|s| {
                let m = (s / 60.0) as u32;
                let sec = (s % 60.0) as u32;
                format!("Survive {:02}:{:02}", m, sec)
            }),
            MissionType::Defense => self.time_target_secs.map(|s| {
                let m = (s / 60.0) as u32;
                let sec = (s % 60.0) as u32;
                format!("Hold position {:02}:{:02}", m, sec)
            }),
            _ => None,
        }
    }

    /// Format time survived as MM:SS.
    pub fn time_survived_str(&self) -> String {
        let mins = (self.time_elapsed / 60.0) as u32;
        let secs = (self.time_elapsed % 60.0) as u32;
        format!("{:02}:{:02}", mins, secs)
    }
}
