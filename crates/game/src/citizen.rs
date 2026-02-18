//! Earth settlement citizens: Starship Troopers aesthetic — civilians walking to/from places,
//! schedule-driven AI, time-of-day and weather cycles.

use engine_core::{Transform, Velocity};
use glam::{Quat, Vec3};
use hecs::{Entity, World};
use rand::Rng;

use crate::state::{Weather, WeatherState};

/// Schedule phase for citizen AI (driven by time of day and weather).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CitizenSchedule {
    /// At home (dawn / night).
    Home,
    /// Walking to work.
    CommuteToWork,
    /// At work.
    Work,
    /// Break / recreation.
    Break,
    /// Walking home.
    CommuteHome,
    /// Evening recreation (plaza, etc.).
    Recreation,
    /// Bad weather — seeking shelter.
    SeekingShelter,
}

/// Settlement waypoints — bustling Federation Earth: cities, districts, plazas, shelters.
pub const SETTLEMENT_WAYPOINTS: &[(f32, f32)] = &[
    (-25.0, -20.0),  // 0: home sector NW
    (20.0, -22.0),   // 1: home sector NE
    (-28.0, 15.0),   // 2: home sector SW
    (22.0, 18.0),    // 3: home sector SE
    (0.0, -35.0),    // 4: work / factory north
    (5.0, 0.0),      // 5: plaza center
    (-15.0, 25.0),   // 6: recreation south
    (18.0, 28.0),    // 7: recreation south-east
    (-8.0, -10.0),   // 8: shelter
    (12.0, -8.0),    // 9: shelter
    (-40.0, -15.0),  // 10: city district west
    (38.0, -18.0),   // 11: city district east
    (-20.0, -45.0),  // 12: residential north-west
    (25.0, -48.0),   // 13: residential north-east
    (-35.0, 30.0),   // 14: market south-west
    (32.0, 35.0),   // 15: market south-east
    (0.0, 45.0),    // 16: civic center south
    (-12.0, -30.0),  // 17: housing
    (15.0, -25.0),   // 18: housing
    (-22.0, 0.0),   // 19: district plaza
    (28.0, 5.0),    // 20: district plaza
];

/// Waypoint index for the given schedule. When using territory waypoints (many points),
/// `waypoint_count` is the length of the waypoints slice; otherwise use legacy ranges.
fn waypoints_for_schedule(schedule: CitizenSchedule, rng: &mut impl Rng, waypoint_count: usize) -> usize {
    if waypoint_count == 0 {
        return 0;
    }
    if waypoint_count <= 21 {
        // Legacy small list
        let n = waypoint_count;
        match schedule {
            CitizenSchedule::Home => rng.gen_range(0..4.min(n).max(1)),
            CitizenSchedule::CommuteToWork | CitizenSchedule::Work => rng.gen_range(4.min(n - 1)..n.max(5)),
            CitizenSchedule::Break | CitizenSchedule::Recreation => rng.gen_range(5.min(n)..17.min(n).max(6)),
            CitizenSchedule::CommuteHome => rng.gen_range(0..4.min(n).max(1)),
            CitizenSchedule::SeekingShelter => rng.gen_range(8.min(n - 1)..10.min(n).max(9)),
        }
    } else {
        // Territory: many waypoints — pick by schedule "zone" (rough quarters of the list)
        let n = waypoint_count;
        match schedule {
            CitizenSchedule::Home | CitizenSchedule::CommuteHome => rng.gen_range(0..n / 4),
            CitizenSchedule::CommuteToWork | CitizenSchedule::Work => rng.gen_range(n / 4..n / 2),
            CitizenSchedule::Break | CitizenSchedule::Recreation => rng.gen_range(n / 2..(3 * n) / 4),
            CitizenSchedule::SeekingShelter => rng.gen_range((3 * n) / 4..n),
        }
    }
}

/// Citizen NPC — Federation civilian on Earth. Schedule-driven, simulated daily routine.
#[derive(Debug, Clone)]
pub struct Citizen {
    pub display_name: String,
    /// Index into dialogue content (dialogue.rs).
    pub dialogue_id: usize,
    pub schedule: CitizenSchedule,
    /// Current waypoint index (destination) — global index into waypoints list.
    pub waypoint_idx: usize,
    /// Start index of this citizen's district in the global waypoint list (territory) or 0 (legacy).
    pub waypoint_start: usize,
    /// Number of waypoints in this citizen's district (so they stay local).
    pub waypoint_count: usize,
    /// Time at current activity (for schedule transitions and re-pick).
    pub phase_timer: f32,
    /// Base walk speed (m/s). Modified by schedule (commute faster, recreation slower).
    pub walk_speed: f32,
    /// Personal clock offset (-0.08..0.08) so not everyone switches schedule at the same moment.
    pub schedule_offset: f32,
    /// Don't leave current spot until phase_timer exceeds this (simulated dwell at destination).
    pub dwell_until: f32,
    /// Next re-pick waypoint after this time (staggered so crowd doesn't move in sync).
    pub next_wander_at: f32,
}

impl Citizen {
    /// Create a citizen. For territory: pass waypoint_start and waypoint_count for their district so they stay local.
    /// For legacy single-settlement: pass waypoint_start = 0, waypoint_count = total waypoints.
    pub fn new(
        display_name: String,
        dialogue_id: usize,
        rng: &mut impl Rng,
        waypoint_start: usize,
        waypoint_count: usize,
    ) -> Self {
        let schedule = CitizenSchedule::Home;
        let local_idx = waypoints_for_schedule(schedule, rng, waypoint_count);
        let waypoint_idx = waypoint_start + local_idx;
        let phase_timer = rng.gen::<f32>() * 25.0; // Stagger so re-picks are spread out
        Self {
            display_name,
            dialogue_id,
            schedule,
            waypoint_idx,
            waypoint_start,
            waypoint_count,
            phase_timer,
            walk_speed: 1.1 + rng.gen::<f32>() * 0.5,
            schedule_offset: (rng.gen::<f32>() - 0.5) * 0.16, // -0.08..0.08
            dwell_until: 0.0,
            next_wander_at: phase_timer + 18.0 + rng.gen::<f32>() * 25.0,
        }
    }
}

/// Walk speed multiplier by schedule — commute faster, recreation slower (simulated world feel).
fn schedule_walk_mult(schedule: CitizenSchedule) -> f32 {
    match schedule {
        CitizenSchedule::CommuteToWork | CitizenSchedule::CommuteHome => 1.18,
        CitizenSchedule::Work => 0.92,
        CitizenSchedule::Break | CitizenSchedule::Recreation => 0.82,
        CitizenSchedule::Home => 0.88,
        CitizenSchedule::SeekingShelter => 1.25,
    }
}

/// Choose schedule from time of day (0 = dawn, 0.25 = noon, 0.5 = dusk, 0.75 = night). Deterministic so all citizens follow the same cycle.
fn schedule_from_time_of_day(t: f32) -> CitizenSchedule {
    if t < 0.12 {
        CitizenSchedule::Home
    } else if t < 0.22 {
        CitizenSchedule::CommuteToWork
    } else if t < 0.42 {
        CitizenSchedule::Work
    } else if t < 0.52 {
        CitizenSchedule::Break
    } else if t < 0.68 {
        CitizenSchedule::Work
    } else if t < 0.82 {
        CitizenSchedule::Recreation
    } else {
        CitizenSchedule::CommuteHome
    }
}

/// Spawn citizens at a single center with the legacy waypoint list (used if not using territory).
pub fn spawn_earth_citizens(
    world: &mut World,
    center: Vec3,
    sample_terrain_y: impl Fn(f32, f32) -> f32,
    count: usize,
) {
    let mut rng = rand::thread_rng();
    // Civilian names only — no overlap with Roger Young crew (Rico, Zim, Levy, etc.)
    let names = [
        "Carlos", "Maria", "Jake", "Yuki", "Hans", "Elena", "Dizzy",
        "Sanders", "Deladrier", "Shujumi", "Hendrick", "Nadia", "Viktor",
        "Anya", "Omar", "Lena", "Felix", "Irina", "Marcus", "Sofia",
    ];
    let n = SETTLEMENT_WAYPOINTS.len();
    for i in 0..count {
        let name = names[i % names.len()].to_string();
        let dialogue_id = i % 5;
        let (wx, wz) = SETTLEMENT_WAYPOINTS[i % n];
        let x = center.x + wx + rng.gen::<f32>() * 4.0;
        let z = center.z + wz + rng.gen::<f32>() * 4.0;
        let y = sample_terrain_y(x, z) + 0.5;
        let pos = Vec3::new(x, y, z);
        world.spawn((
            Transform { position: pos, rotation: Quat::IDENTITY, scale: Vec3::splat(1.0) },
            Velocity::default(),
            Citizen::new(name, dialogue_id, &mut rng, 0, n),
        ));
    }
}

/// Despawn all citizens (e.g. when leaving Earth).
pub fn despawn_citizens(world: &mut World) {
    let to_remove: Vec<Entity> = world
        .query::<&Citizen>()
        .iter()
        .map(|(e, _)| e)
        .collect();
    for e in to_remove {
        world.despawn(e).ok();
    }
}

/// Update citizen AI: schedule from time/weather, move toward destination.
/// `waypoints`: global (x,z) targets. If from territory, use global list with origin; else legacy offsets from settlement_center.
pub fn update_citizens(
    world: &mut World,
    time_of_day: f32,
    weather: &Weather,
    settlement_center: Vec3,
    waypoints: &[(f32, f32)],
    dt: f32,
    sample_terrain_y: impl Fn(f32, f32) -> f32,
) {
        let bad_weather = matches!(weather.current, WeatherState::Rain | WeatherState::Storm | WeatherState::Snow);
    let mut rng = rand::thread_rng();
    let wp_len = waypoints.len().max(1);
    let _legacy = waypoints.is_empty();

    for (_, (transform, velocity, citizen)) in
        world.query_mut::<(&mut Transform, &mut Velocity, &mut Citizen)>()
    {
        // Per-citizen effective time so not everyone switches schedule at once (simulated world).
        let effective_time = (time_of_day + citizen.schedule_offset).rem_euclid(1.0);
        let target_schedule = if bad_weather {
            CitizenSchedule::SeekingShelter
        } else {
            schedule_from_time_of_day(effective_time)
        };

        if target_schedule != citizen.schedule {
            citizen.schedule = target_schedule;
            let local = waypoints_for_schedule(target_schedule, &mut rng, citizen.waypoint_count);
            citizen.waypoint_idx = citizen.waypoint_start + local;
            citizen.phase_timer = 0.0;
            citizen.dwell_until = 0.0;
            citizen.next_wander_at = 18.0 + rng.gen::<f32>() * 25.0;
        }
        citizen.phase_timer += dt;

        // Staggered re-pick: only after dwell at destination and next_wander_at (no synchronized crowd).
        let may_repick = citizen.phase_timer > citizen.next_wander_at && citizen.phase_timer > citizen.dwell_until;
        if may_repick {
            let local = waypoints_for_schedule(citizen.schedule, &mut rng, citizen.waypoint_count);
            citizen.waypoint_idx = citizen.waypoint_start + local;
            citizen.next_wander_at = citizen.phase_timer + 20.0 + rng.gen::<f32>() * 28.0;
            citizen.dwell_until = 0.0; // will be set again when they arrive
        }

        let idx = citizen.waypoint_idx.min(wp_len.saturating_sub(1));
        let (target_x, target_z) = if waypoints.is_empty() {
            let (ow, oz) = SETTLEMENT_WAYPOINTS[idx.min(SETTLEMENT_WAYPOINTS.len().saturating_sub(1))];
            (settlement_center.x + ow, settlement_center.z + oz)
        } else {
            // Territory: waypoints are global (world x, z)
            waypoints[idx]
        };
        let dx = target_x - transform.position.x;
        let dz = target_z - transform.position.z;
        let dist_sq = dx * dx + dz * dz;
        let arrive_threshold_sq = 1.5 * 1.5;

        if dist_sq < arrive_threshold_sq {
            velocity.linear.x = 0.0;
            velocity.linear.z = 0.0;
            // Dwell at destination: stay 4–12 s before next re-pick can send them elsewhere.
            if citizen.phase_timer > citizen.dwell_until {
                citizen.dwell_until = citizen.phase_timer + 4.0 + rng.gen::<f32>() * 8.0;
            }
            continue;
        }

        let dist = dist_sq.sqrt();
        let speed = citizen.walk_speed * schedule_walk_mult(citizen.schedule);
        velocity.linear.x = (dx / dist) * speed;
        velocity.linear.z = (dz / dist) * speed;
        velocity.linear.y = 0.0;

        // Face movement direction
        let yaw = f32::atan2(-dx, -dz);
        transform.rotation = Quat::from_rotation_y(yaw);

        // Apply movement and snap to terrain
        transform.position.x += velocity.linear.x * dt;
        transform.position.z += velocity.linear.z * dt;
        let ground_y = sample_terrain_y(transform.position.x, transform.position.z);
        transform.position.y = ground_y + 0.5;
    }
}
