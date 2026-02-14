//! Squad mates: AI troopers that deploy with the player, kill bugs, and call air strikes.
//!
//! Spawned when the drop pod lands; they follow the player, engage bugs in range,
//! and periodically request Tac Fighter CAS. Each trooper type has unique stats.

use engine_core::{Health, Transform, Velocity, Vec3};
use glam::Quat;
use hecs::{Entity, World};
use rand::Rng;

use crate::bug::Bug;

/// Kind of squad mate (affects visuals, behavior, and stats).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SquadMateKind {
    /// Fleet personnel — ship crew, lighter armor, support role.
    Fleet,
    /// Mobile Infantry — standard troopers, balanced stats.
    MobileInfantry,
    /// Marauder — powered suit, tanky, heavy firepower.
    Marauder,
    /// Tech — specialist, fast fire rate, lighter armor.
    Tech,
}

/// Per-trooper-type stats (health, damage, fire rate, move speed, etc.).
#[derive(Debug, Clone, Copy)]
pub struct SquadTrooperStats {
    pub health: f32,
    pub damage: f32,
    pub fire_interval: f32,
    pub move_speed: f32,
    pub fire_range_sq: f32,
    pub cas_cooldown_min: f32,
    pub cas_cooldown_max: f32,
}

impl SquadMateKind {
    /// Stats unique to each trooper type.
    pub fn stats(self) -> SquadTrooperStats {
        match self {
            SquadMateKind::Fleet => SquadTrooperStats {
                health: 65.0,
                damage: 14.0,
                fire_interval: 0.22,
                move_speed: 5.2,
                fire_range_sq: 42.0 * 42.0,
                cas_cooldown_min: 75.0,
                cas_cooldown_max: 110.0,
            },
            SquadMateKind::MobileInfantry => SquadTrooperStats {
                health: 85.0,
                damage: 18.0,
                fire_interval: 0.25,
                move_speed: 5.0,
                fire_range_sq: 45.0 * 45.0,
                cas_cooldown_min: 90.0,
                cas_cooldown_max: 120.0,
            },
            SquadMateKind::Marauder => SquadTrooperStats {
                health: 150.0,
                damage: 28.0,
                fire_interval: 0.35,
                move_speed: 4.0,
                fire_range_sq: 50.0 * 50.0,
                cas_cooldown_min: 100.0,
                cas_cooldown_max: 140.0,
            },
            SquadMateKind::Tech => SquadTrooperStats {
                health: 60.0,
                damage: 12.0,
                fire_interval: 0.16,
                move_speed: 5.5,
                fire_range_sq: 38.0 * 38.0,
                cas_cooldown_min: 70.0,
                cas_cooldown_max: 100.0,
            },
        }
    }
}

/// AI companion that deployed with the player.
#[derive(Debug, Clone)]
pub struct SquadMate {
    pub name: &'static str,
    pub kind: SquadMateKind,
    /// Time until can fire again.
    pub fire_cooldown: f32,
    /// Time until can call CAS again (so they don't spam).
    pub cas_call_cooldown: f32,
    /// Formation offset from player (XZ).
    pub formation_offset: Vec3,
}

impl SquadMate {
    pub fn new(name: &'static str, kind: SquadMateKind, formation_offset: Vec3) -> Self {
        Self {
            name,
            kind,
            fire_cooldown: 0.0,
            cas_call_cooldown: 30.0 + rand::thread_rng().gen::<f32>() * 20.0, // stagger first CAS calls
            formation_offset,
        }
    }
}

/// Despawn all squad mates (e.g. when returning to ship after extraction).
pub fn despawn_squad(world: &mut World) {
    let to_despawn: Vec<Entity> = world
        .query::<&SquadMate>()
        .iter()
        .map(|(e, _)| e)
        .collect();
    for e in to_despawn {
        world.despawn(e).ok();
    }
}

/// Squad data for the default drop bay crew (order matches drop pod indices).
pub const SQUAD_DROP_DATA: &[(&'static str, SquadMateKind, Vec3)] = &[
    ("Sgt. Zim", SquadMateKind::MobileInfantry, Vec3::new(-3.0, 0.0, -2.0)),
    ("Cpl. Higgins", SquadMateKind::MobileInfantry, Vec3::new(3.0, 0.0, -2.0)),
    ("Marauder Acevedo", SquadMateKind::Marauder, Vec3::new(0.0, 0.0, -4.0)),
    ("Tech Martinez", SquadMateKind::Tech, Vec3::new(-2.0, 0.0, -3.0)),
];

/// Spawn a single squad mate at a position (used when their drop pod lands).
pub fn spawn_one_squad_mate(
    world: &mut World,
    position: Vec3,
    ground_y: f32,
    name: &'static str,
    kind: SquadMateKind,
    formation_offset: Vec3,
) {
    let stats = kind.stats();
    let pos = Vec3::new(position.x, ground_y + 0.5, position.z);
    world.spawn((
        Transform {
            position: pos,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(1.0),
        },
        Velocity::default(),
        Health::new(stats.health),
        SquadMate::new(name, kind, formation_offset),
    ));
}

/// Spawn the full squad at the landing site (used if we skip the drop sequence).
pub fn spawn_squad(world: &mut World, landing: Vec3, terrain_y: f32) {
    let y = terrain_y + 0.5;
    let positions = [
        landing + Vec3::new(-2.0, 0.0, -3.0),
        landing + Vec3::new(2.0, 0.0, -3.0),
        landing + Vec3::new(0.0, 0.0, -5.0),
        landing + Vec3::new(-1.5, 0.0, -4.0),
    ];
    for (pos, (name, kind, formation)) in positions.iter().zip(SQUAD_DROP_DATA.iter()) {
        let position = Vec3::new(pos.x, y, pos.z);
        spawn_one_squad_mate(world, position, terrain_y, name, *kind, *formation);
    }
}

/// Sample terrain Y at (x, z). Caller passes a closure so we don't depend on ChunkManager type.
pub fn update_squad_movement(
    world: &mut World,
    player_pos: Vec3,
    dt: f32,
    sample_terrain_y: impl Fn(f32, f32) -> f32,
) {
    let follow_dist = 8.0;
    for (_, (transform, velocity, squad)) in world.query_mut::<(&mut Transform, &mut Velocity, &SquadMate)>() {
        let move_speed = squad.kind.stats().move_speed;
        let target = player_pos + Vec3::new(
            squad.formation_offset.x,
            0.0,
            squad.formation_offset.z,
        );
        let to_target = Vec3::new(target.x - transform.position.x, 0.0, target.z - transform.position.z);
        let dist_xz = (to_target.x * to_target.x + to_target.z * to_target.z).sqrt();
        if dist_xz < 0.1 {
            velocity.linear = Vec3::ZERO;
        } else {
            let dir = if dist_xz > follow_dist {
                to_target / dist_xz
            } else {
                to_target * (0.3 / dist_xz.max(0.1)) // slow down when close
            };
            velocity.linear = Vec3::new(dir.x * move_speed, 0.0, dir.z * move_speed);
            transform.position += velocity.linear * dt;
            if velocity.linear.length_squared() > 0.01 {
                transform.rotation = Quat::from_rotation_arc(Vec3::Z, velocity.linear.normalize());
            }
        }
        let ground_y = sample_terrain_y(transform.position.x, transform.position.z);
        transform.position.y = ground_y + 0.5;
    }
}

/// Find the nearest living bug to a position. Returns (entity, position, distance_sq).
pub fn nearest_bug(world: &World, from: Vec3) -> Option<(Entity, Vec3, f32)> {
    let mut best: Option<(Entity, Vec3, f32)> = None;
    for (entity, (transform, _bug, health)) in world.query::<(&Transform, &Bug, &Health)>().iter() {
        if health.current <= 0.0 {
            continue;
        }
        let dist_sq = transform.position.distance_squared(from);
        if best.as_ref().map_or(true, |(_, _, d)| dist_sq < *d) {
            best = Some((entity, transform.position, dist_sq));
        }
    }
    best
}

/// Find the nearest living enemy (bug or Skinny) to a position.
pub fn nearest_enemy(world: &World, from: Vec3) -> Option<(Entity, Vec3, f32)> {
    let mut best: Option<(Entity, Vec3, f32)> = None;
    for (entity, (transform, _bug, health)) in world.query::<(&Transform, &crate::bug::Bug, &Health)>().iter() {
        if health.current <= 0.0 { continue; }
        let dist_sq = transform.position.distance_squared(from);
        if best.as_ref().map_or(true, |(_, _, d)| dist_sq < *d) {
            best = Some((entity, transform.position, dist_sq));
        }
    }
    for (entity, (transform, _skinny, health)) in world.query::<(&Transform, &crate::skinny::Skinny, &Health)>().iter() {
        if health.current <= 0.0 { continue; }
        let dist_sq = transform.position.distance_squared(from);
        if best.as_ref().map_or(true, |(_, _, d)| dist_sq < *d) {
            best = Some((entity, transform.position, dist_sq));
        }
    }
    best
}

/// Update squad combat: shoot at nearest bug in range; optionally request CAS.
/// Returns the name of the squad mate who requested CAS this frame (caller should spawn TacFighter).
pub fn update_squad_combat(
    world: &mut World,
    dt: f32,
    tac_ready: bool,
) -> Option<&'static str> {
    // Pass 1: read-only — decide who fires and who calls CAS (avoids borrowing world mutably while querying).
    let mut decisions: Vec<(Entity, Option<Entity>, bool, bool, f32, f32)> = Vec::new();
    let mut first_cas_caller: Option<&'static str> = None;
    for (squad_entity, (transform, squad, health)) in world.query::<(&Transform, &SquadMate, &Health)>().iter() {
        if health.current <= 0.0 {
            continue;
        }
        let stats = squad.kind.stats();
        let fire_cooldown_after = (squad.fire_cooldown - dt).max(0.0);
        let cas_cooldown_after = (squad.cas_call_cooldown - dt).max(0.0);

        let mut cas_caller = false;
        if tac_ready && cas_cooldown_after <= 0.0 && first_cas_caller.is_none() {
            first_cas_caller = Some(squad.name);
            cas_caller = true;
        }

        let from = transform.position + Vec3::Y * 1.2;
        let (target_entity, should_fire) = match nearest_enemy(world, from) {
            Some((entity, _pos, dist_sq)) if dist_sq <= stats.fire_range_sq && fire_cooldown_after <= 0.0 => {
                (Some(entity), true)
            }
            _ => (None, false),
        };
        decisions.push((squad_entity, target_entity, should_fire, cas_caller, stats.fire_interval, stats.damage));
    }

    // Pass 2: apply cooldowns and damage.
    let mut rng = rand::thread_rng();
    for (squad_entity, target_entity, should_fire, cas_caller, fire_interval, damage) in decisions {
        if let Ok(mut squad) = world.get::<&mut SquadMate>(squad_entity) {
            squad.fire_cooldown = if should_fire { fire_interval } else { (squad.fire_cooldown - dt).max(0.0) };
            squad.cas_call_cooldown = if cas_caller {
                let s = squad.kind.stats();
                s.cas_cooldown_min + rng.gen::<f32>() * (s.cas_cooldown_max - s.cas_cooldown_min)
            } else {
                (squad.cas_call_cooldown - dt).max(0.0)
            };
        }
        if let Some(target_entity) = target_entity {
            if let Ok(mut h) = world.get::<&mut Health>(target_entity) {
                h.take_damage(damage);
            }
        }
    }
    first_cas_caller
}
