//! Tac Fighter close air support: bombing runs and ordnance delivery.
//! Attack patterns and danger-close avoidance for cinematic CAS.

use glam::Vec3;
use rand::Rng;

/// Phase of a Tac Fighter bombing run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TacFighterPhase {
    /// Approaching from distance, warning given.
    Inbound,
    /// Buzzing the player at low altitude.
    BuzzPass,
    /// Climbing and circling for bomb run.
    ClimbForRun,
    /// Bombing run - dropping ordnance.
    BombingRun,
    /// Departing the area.
    Departing,
}

/// Attack pattern: different bombing run styles for variety.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttackPattern {
    /// Straight run: classic approach, drop in line.
    StraightRun,
    /// Strafe: flies parallel to target, drops along flank.
    Strafe,
    /// Pylon turn: orbits target, drops when clear.
    PylonTurn,
    /// Dive bomb: steep approach, pull up after release.
    DiveBomb,
}

/// Minimum distance from player for bomb impact when avoiding danger close.
const SAFE_DROP_RANGE: f32 = 55.0;
/// Chance (0–1) that a fighter will use danger-close strikes despite the risk.
const DANGER_CLOSE_CHANCE: f32 = 0.08;
/// Estimated bomb fall time for impact prediction.
const BOMB_FALL_TIME: f32 = 1.8;
/// Velocity smoothing: blend factor per second (higher = snappier, lower = more inertia).
const VELOCITY_SMOOTH: f32 = 3.0;

/// A Tac Fighter performing close air support.
pub struct TacFighter {
    pub position: Vec3,
    pub velocity: Vec3,
    /// Direction of approach (normalized XZ).
    pub approach_dir: Vec3,
    /// Current phase.
    pub phase: TacFighterPhase,
    /// Timer within current phase.
    pub phase_timer: f32,
    /// Bombs remaining.
    pub bombs_remaining: u32,
    /// Time since last bomb drop.
    pub bomb_interval_timer: f32,
    /// Player position at time of run.
    pub target_area: Vec3,
    /// Total age.
    pub age: f32,
    /// Attack pattern for this run.
    pub pattern: AttackPattern,
    /// Pattern-specific state (e.g. orbit angle, strafe offset).
    pub pattern_state: f32,
    /// If true, this fighter may drop danger-close (small chance at spawn).
    pub allow_danger_close: bool,
    /// Corvette index to return to when RTB (0–7). Used for Departing phase.
    pub home_corvette_index: u8,
}

impl TacFighter {
    /// Create a Tac Fighter approaching from a random direction.
    pub fn new(player_pos: Vec3, spawn_from_corvette: Option<(Vec3, u8)>) -> Self {
        let mut rng = rand::thread_rng();
        let angle = rng.gen::<f32>() * std::f32::consts::TAU;
        Self::new_with_angle(player_pos, angle, spawn_from_corvette)
    }

    /// Create a Tac Fighter approaching from a specific angle (radians). Used for fleet formation.
    /// If `spawn_from_corvette` is Some((pos, idx)), spawns at that corvette and RTBs to it.
    pub fn new_with_angle(
        player_pos: Vec3,
        angle: f32,
        spawn_from_corvette: Option<(Vec3, u8)>,
    ) -> Self {
        let mut rng = rand::thread_rng();
        let approach_dir = Vec3::new(angle.cos(), 0.0, angle.sin());

        // Pick attack pattern (weighted random for variety)
        let pattern = match rng.gen::<f32>() {
            x if x < 0.35 => AttackPattern::StraightRun,
            x if x < 0.65 => AttackPattern::Strafe,
            x if x < 0.85 => AttackPattern::PylonTurn,
            _ => AttackPattern::DiveBomb,
        };

        // Small chance to allow danger-close strikes (risky but sometimes they do it)
        let allow_danger_close = rng.gen::<f32>() < DANGER_CLOSE_CHANCE;

        // Spawn from corvette (launch from orbit) or fallback to distant approach
        let (start_pos, home_corvette_index) = if let Some((corvette_pos, idx)) = spawn_from_corvette
        {
            // Slightly below and behind corvette so fighter appears to launch from the ship
            let behind = -approach_dir;
            let launch_offset = behind * 25.0 + Vec3::Y * -15.0;
            (corvette_pos + launch_offset, idx)
        } else {
            let start_pos = player_pos - approach_dir * 300.0 + Vec3::Y * 80.0;
            (start_pos, 0)
        };

        Self {
            position: start_pos,
            velocity: approach_dir * 120.0 + Vec3::Y * -10.0, // fast approach, slight descent
            approach_dir,
            phase: TacFighterPhase::Inbound,
            phase_timer: 0.0,
            bombs_remaining: 5,
            bomb_interval_timer: 0.0,
            target_area: player_pos,
            age: 0.0,
            pattern,
            pattern_state: 0.0,
            allow_danger_close,
            home_corvette_index,
        }
    }

    /// Estimate where a bomb dropped now would impact (XZ plane).
    fn estimate_impact_xz(&self) -> Vec3 {
        let horiz = Vec3::new(self.velocity.x, 0.0, self.velocity.z);
        Vec3::new(
            self.position.x + horiz.x * BOMB_FALL_TIME,
            self.position.y,
            self.position.z + horiz.z * BOMB_FALL_TIME,
        )
    }

    /// Smoothly blend velocity toward target (realistic inertia, no instant direction changes).
    fn blend_velocity(&mut self, target: Vec3, dt: f32) {
        let t = (dt * VELOCITY_SMOOTH).min(1.0);
        self.velocity = self.velocity + (target - self.velocity) * t;
    }

    /// Check if dropping a bomb would be danger close to the player (impact within safe range).
    fn would_be_danger_close(&self, player_pos: Vec3) -> bool {
        let impact = self.estimate_impact_xz();
        let dist = Vec3::new(
            impact.x - player_pos.x,
            0.0,
            impact.z - player_pos.z,
        ).length();
        dist < SAFE_DROP_RANGE
    }

    /// Update fighter. Pass `corvette_positions` when on planet so Inbound/Departing use corvettes.
    pub fn update(
        &mut self,
        dt: f32,
        player_pos: Vec3,
        corvette_positions: Option<&[Vec3]>,
    ) -> Vec<Vec3> {
        self.age += dt;
        self.phase_timer += dt;
        let mut bomb_drops: Vec<Vec3> = Vec::new();

        match self.phase {
            TacFighterPhase::Inbound => {
                // Fly toward target area, descending
                let to_target = self.target_area - self.position;
                let dist = to_target.length();
                let speed = 120.0;
                let mut target_vel = to_target.normalize_or_zero() * speed;
                target_vel.y = -15.0 + (dist / 300.0) * 10.0; // descend as we approach
                self.blend_velocity(target_vel, dt);

                if dist < 80.0 {
                    self.phase = TacFighterPhase::BuzzPass;
                    self.phase_timer = 0.0;
                }
            }
            TacFighterPhase::BuzzPass => {
                // Fly low and fast over the target area
                let target_alt = self.target_area.y + 25.0;
                let alt_speed = (target_alt - self.position.y) * 2.0;
                let mut target_vel = self.approach_dir * 150.0;
                target_vel.y = alt_speed;
                self.blend_velocity(target_vel, dt);

                if self.phase_timer > 1.5 {
                    self.phase = TacFighterPhase::ClimbForRun;
                    self.phase_timer = 0.0;
                }
            }
            TacFighterPhase::ClimbForRun => {
                // Climb and turn around for bombing run — smooth banking turn
                let turn_rate = 1.2; // Slower turn for smoother arc
                let angle = self.phase_timer * turn_rate;
                let base_speed = 80.0;
                let mut target_vel = Vec3::new(
                    (-self.approach_dir.x * angle.cos() + self.approach_dir.z * angle.sin()) * base_speed,
                    30.0, // climb
                    (-self.approach_dir.z * angle.cos() - self.approach_dir.x * angle.sin()) * base_speed,
                );
                self.blend_velocity(target_vel, dt);

                if self.phase_timer > 2.8 {
                    self.phase = TacFighterPhase::BombingRun;
                    self.phase_timer = 0.0;
                    self.pattern_state = 0.0;
                    // Set up approach based on pattern
                    let to_target = (self.target_area - self.position).normalize_or_zero();
                    let to_xz = Vec3::new(to_target.x, 0.0, to_target.z).normalize_or_zero();
                    self.approach_dir = match self.pattern {
                        AttackPattern::Strafe => Vec3::new(-to_xz.z, 0.0, to_xz.x), // perpendicular for strafe
                        AttackPattern::PylonTurn => to_xz, // will orbit, dir used for initial velocity
                        _ => to_xz,
                    };
                }
            }
            TacFighterPhase::BombingRun => {
                self.pattern_state += dt;
                let to_target = self.target_area - self.position;
                let xz_dist = Vec3::new(to_target.x, 0.0, to_target.z).length();

                // Compute target velocity for each pattern, then smooth
                let target_vel = match self.pattern {
                    AttackPattern::StraightRun => {
                        let speed = 100.0;
                        let mut v = self.approach_dir * speed;
                        let target_alt = self.target_area.y + 50.0;
                        v.y = (target_alt - self.position.y) * 1.5;
                        v
                    }
                    AttackPattern::Strafe => {
                        let strafe_dir = Vec3::new(-self.approach_dir.z, 0.0, self.approach_dir.x);
                        let mut v = strafe_dir * 110.0;
                        let target_alt = self.target_area.y + 45.0;
                        v.y = (target_alt - self.position.y) * 1.8;
                        v
                    }
                    AttackPattern::PylonTurn => {
                        let orbit_radius = 60.0;
                        let orbit_speed = 0.9; // Slower orbit for smoother arc
                        let angle = self.pattern_state * orbit_speed;
                        let orbit_center = self.target_area + Vec3::Y * 55.0;
                        let desired_pos = orbit_center
                            + Vec3::new(angle.cos(), 0.0, angle.sin()) * orbit_radius;
                        let to_desired = desired_pos - self.position;
                        (to_desired * 2.5).clamp_length_max(90.0) // Softer pursuit
                    }
                    AttackPattern::DiveBomb => {
                        let dive_angle: f32 = 0.55; // Slightly shallower for smoother arc
                        let speed = 130.0;
                        self.approach_dir * speed * dive_angle.cos()
                            + Vec3::Y * -speed * dive_angle.sin()
                    }
                };
                self.blend_velocity(target_vel, dt);

                self.bomb_interval_timer += dt;
                let interval = match self.pattern {
                    AttackPattern::StraightRun => 0.4,
                    AttackPattern::Strafe => 0.35,
                    AttackPattern::PylonTurn => 0.5,
                    AttackPattern::DiveBomb => 0.3,
                };
                let in_drop_zone = match self.pattern {
                    AttackPattern::StraightRun | AttackPattern::DiveBomb => xz_dist < 90.0,
                    AttackPattern::Strafe => xz_dist < 100.0 && xz_dist > 30.0,
                    AttackPattern::PylonTurn => {
                        let to_center = Vec3::new(
                            self.target_area.x - self.position.x,
                            0.0,
                            self.target_area.z - self.position.z,
                        );
                        to_center.length() < 80.0
                    }
                };

                // Avoid danger close unless this fighter has the small chance to use it
                let safe_to_drop = !self.would_be_danger_close(player_pos) || self.allow_danger_close;
                if self.bombs_remaining > 0
                    && self.bomb_interval_timer > interval
                    && in_drop_zone
                    && safe_to_drop
                {
                    bomb_drops.push(self.position);
                    self.bombs_remaining -= 1;
                    self.bomb_interval_timer = 0.0;
                }

                let max_time = match self.pattern {
                    AttackPattern::PylonTurn => 8.0,
                    _ => 5.0,
                };
                if self.bombs_remaining == 0 || self.phase_timer > max_time {
                    self.phase = TacFighterPhase::Departing;
                    self.phase_timer = 0.0;
                }
            }
            TacFighterPhase::Departing => {
                // Fly toward home corvette (RTB) — smooth climb and approach
                let target_vel = if let Some(corvettes) = corvette_positions {
                    let idx = self.home_corvette_index as usize;
                    if idx < corvettes.len() {
                        let home = corvettes[idx];
                        let to_home = home - self.position;
                        let dist = to_home.length();
                        if dist > 20.0 {
                            to_home.normalize_or_zero() * 130.0
                        } else {
                            // Near corvette — gentle pull-up (dock / disappear)
                            (self.approach_dir * 150.0 + Vec3::Y * 45.0)
                        }
                    } else {
                        self.approach_dir * 150.0 + Vec3::Y * 40.0
                    }
                } else {
                    self.approach_dir * 150.0 + Vec3::Y * 40.0
                };
                self.blend_velocity(target_vel, dt);
            }
        }

        self.position += self.velocity * dt;
        bomb_drops
    }

    pub fn is_done(&self) -> bool {
        self.phase == TacFighterPhase::Departing && self.phase_timer > 5.0
    }
}

/// A falling bomb dropped by a Tac Fighter.
pub struct TacBomb {
    pub position: Vec3,
    pub velocity: Vec3,
    pub age: f32,
    pub detonated: bool,
}

impl TacBomb {
    pub fn new(drop_pos: Vec3, fighter_velocity: Vec3) -> Self {
        Self {
            position: drop_pos,
            velocity: Vec3::new(fighter_velocity.x * 0.5, -5.0, fighter_velocity.z * 0.5),
            age: 0.0,
            detonated: false,
        }
    }
}
