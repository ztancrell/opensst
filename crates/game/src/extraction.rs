//! Extraction dropship system — MI Retrieval Boat pickup + return to the Roger Young.
//!
//! When the trooper calls for extraction the Federation Corvette dispatches a
//! Retrieval Boat (heavy dropship).  The boat flies in, lands, picks up the
//! player, then flies all the way back to the Roger Young in orbit — with the
//! player aboard watching the whole ride from inside the hold.

use glam::Vec3;
use rand::Rng;
use rapier3d::prelude::RigidBodyHandle;

// ── Extraction phases ───────────────────────────────────────────────────

/// Phase of the extraction sequence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExtractionPhase {
    /// Radio call made — fleet confirms, ETA counting down.
    Called,
    /// Retrieval boat visible in sky, flying toward LZ.
    Inbound,
    /// Descending to the LZ, engines roaring.
    Landing,
    /// On the ground, ramp open, waiting for the trooper.
    Waiting,
    /// Player reached the ramp — boarding in progress.
    Boarding,
    /// Lifting off from the surface (ramp closing, bugs receding)
    Departing,
    /// Climbing through atmosphere to corvette — real-time ascent, Roger Young in frame.
    Ascent,
}

// ── Main struct ─────────────────────────────────────────────────────────

/// A Retrieval Boat performing player extraction.
pub struct ExtractionDropship {
    // ── Spatial ──
    pub position: Vec3,
    pub velocity: Vec3,
    /// Direction the ship approaches from (normalized XZ).
    pub approach_dir: Vec3,
    /// Corvette we launched from — boat descends from here and returns here (real-time).
    pub home_corvette_pos: Vec3,

    // ── Phase machine ──
    pub phase: ExtractionPhase,
    pub phase_timer: f32,
    pub total_timer: f32,

    // ── Landing zone ──
    pub lz_position: Vec3,
    pub lz_ground_y: f32,

    // ── Boarding ──
    pub player_aboard: bool,
    pub boarding_progress: f32,

    // ── Comms message triggers ──
    pub msg_15s_sent: bool,
    pub msg_10s_sent: bool,
    pub msg_touchdown_sent: bool,
    pub msg_hurry_sent: bool,
    pub msg_dustoff_sent: bool,
    pub msg_orbit_sent: bool,
    pub msg_approach_sent: bool,
    pub msg_docking_sent: bool,

    // ── Thruster visual ──
    pub engine_intensity: f32,
    pub ramp_open: f32,

    // ── Door gunners ──
    pub gunner_left_timer: f32,
    pub gunner_right_timer: f32,
    pub gunner_left_target: Option<Vec3>,
    pub gunner_right_target: Option<Vec3>,

    // ── Physics collision ──
    pub hull_body: Option<RigidBodyHandle>,

    // ── Boarding camera ──
    pub boarding_start_pos: Option<Vec3>,
    /// Position when Ascent began (for smooth lerp to corvette).
    pub ascent_start_pos: Option<Vec3>,

    // ── Roger Young position (set when Ascent begins) ──
    /// Position of the Roger Young in world space (very high up).
    pub roger_young_pos: Vec3,
    /// Forward direction of the Roger Young.
    pub roger_young_fwd: Vec3,
}

// ── Timing constants ────────────────────────────────────────────────────

const CALLED_DURATION: f32 = 15.0;
const INBOUND_DURATION: f32 = 15.0;
const LANDING_DURATION: f32 = 6.0;
const WAITING_DURATION: f32 = 45.0;
const BOARDING_DURATION: f32 = 3.5;
const DEPARTING_DURATION: f32 = 6.0;
/// Ascent to Roger Young (smooth arc, real-time).
const ASCENT_DURATION: f32 = 28.0;

pub const BOARDING_RADIUS: f32 = 8.0;

const APPROACH_ALTITUDE: f32 = 250.0;
const HOVER_ALTITUDE: f32 = 4.0;
const LZ_FORWARD_OFFSET: f32 = 30.0;

/// How high orbit is (Roger Young altitude).
const ORBIT_ALTITUDE: f32 = 3000.0;
/// How far away from the LZ the Roger Young sits (XZ offset).
const RY_HORIZONTAL_OFFSET: f32 = 500.0;

impl ExtractionDropship {
    /// Create extraction. `corvette_spawn_pos` is the corvette above the player the boat launches from (real-time descent).
    pub fn new(player_pos: Vec3, player_forward: Vec3, ground_y_at_lz: f32, corvette_spawn_pos: Vec3) -> Self {
        let fwd_xz = Vec3::new(player_forward.x, 0.0, player_forward.z).normalize_or_zero();
        let lz_xz = Vec3::new(
            player_pos.x + fwd_xz.x * LZ_FORWARD_OFFSET,
            0.0,
            player_pos.z + fwd_xz.z * LZ_FORWARD_OFFSET,
        );
        let lz_position = Vec3::new(lz_xz.x, ground_y_at_lz, lz_xz.z);

        // Approach dir: from corvette toward LZ (boat descends from corvette)
        let to_lz = Vec3::new(lz_position.x - corvette_spawn_pos.x, 0.0, lz_position.z - corvette_spawn_pos.z);
        let approach_dir = if to_lz.length_squared() > 0.01 {
            to_lz.normalize()
        } else {
            Vec3::new(-fwd_xz.x, 0.0, -fwd_xz.z).normalize_or_zero()
        };

        // Boat spawns at the corvette (above player) — real-time descent
        let start_pos = corvette_spawn_pos;

        // Roger Young position: high orbit above the LZ (chase cam looks at it during climb)
        let ry_pos = Vec3::new(
            lz_position.x - approach_dir.x * RY_HORIZONTAL_OFFSET,
            ORBIT_ALTITUDE,
            lz_position.z - approach_dir.z * RY_HORIZONTAL_OFFSET,
        );
        let ry_fwd = approach_dir;

        Self {
            position: start_pos,
            home_corvette_pos: corvette_spawn_pos,
            velocity: Vec3::ZERO,
            approach_dir,
            phase: ExtractionPhase::Called,
            phase_timer: 0.0,
            total_timer: 0.0,
            lz_position,
            lz_ground_y: ground_y_at_lz,
            player_aboard: false,
            boarding_progress: 0.0,
            msg_15s_sent: false,
            msg_10s_sent: false,
            msg_touchdown_sent: false,
            msg_hurry_sent: false,
            msg_dustoff_sent: false,
            msg_orbit_sent: false,
            msg_approach_sent: false,
            msg_docking_sent: false,
            engine_intensity: 0.0,
            ramp_open: 0.0,
            gunner_left_timer: 0.0,
            gunner_right_timer: 0.0,
            gunner_left_target: None,
            gunner_right_target: None,
            hull_body: None,
            boarding_start_pos: None,
            ascent_start_pos: None,
            roger_young_pos: ry_pos,
            roger_young_fwd: ry_fwd,
        }
    }

    /// Tick the extraction sequence.  Returns comms messages.
    pub fn update(&mut self, dt: f32, player_pos: Vec3) -> Vec<ExtractionMessage> {
        self.total_timer += dt;
        self.phase_timer += dt;
        let mut messages: Vec<ExtractionMessage> = Vec::new();

        match self.phase {
            // ── Called ────────────────────────────────────────────────────
            ExtractionPhase::Called => {
                let eta = CALLED_DURATION + INBOUND_DURATION + LANDING_DURATION - self.phase_timer;
                if eta <= 20.0 && !self.msg_15s_sent {
                    self.msg_15s_sent = true;
                    messages.push(ExtractionMessage::Warning(
                        "FLEET COM: Retrieval boat 20 seconds out! Hold the line!".into(),
                    ));
                }
                if self.phase_timer >= CALLED_DURATION {
                    self.phase = ExtractionPhase::Inbound;
                    self.phase_timer = 0.0;
                }
            }

            // ── Inbound: real-time descent from corvette above player ────────
            ExtractionPhase::Inbound => {
                let t = (self.phase_timer / INBOUND_DURATION).clamp(0.0, 1.0);
                let start = self.home_corvette_pos; // Spawned at corvette
                let hover_pos = self.lz_position + Vec3::Y * (HOVER_ALTITUDE + 40.0);
                let ease = t * t * (3.0 - 2.0 * t);
                self.position = start.lerp(hover_pos, ease);
                let target_vel = (hover_pos - start).normalize_or_zero() * 80.0;
                self.velocity = target_vel * (1.0 - ease * 0.6);
                self.engine_intensity = 0.4 + t * 0.4;

                let eta_to_ground = (1.0 - t) * INBOUND_DURATION + LANDING_DURATION;
                if eta_to_ground <= 12.0 && !self.msg_10s_sent {
                    self.msg_10s_sent = true;
                    messages.push(ExtractionMessage::Warning(
                        "FLEET COM: Retrieval boat on final approach! Pop smoke!".into(),
                    ));
                }
                if self.phase_timer >= INBOUND_DURATION {
                    self.phase = ExtractionPhase::Landing;
                    self.phase_timer = 0.0;
                }
            }

            // ── Landing ──────────────────────────────────────────────────
            ExtractionPhase::Landing => {
                let t = (self.phase_timer / LANDING_DURATION).clamp(0.0, 1.0);
                let ease = t * t * (3.0 - 2.0 * t);
                let start_y = self.lz_ground_y + HOVER_ALTITUDE + 40.0;
                let end_y = self.lz_ground_y + HOVER_ALTITUDE;
                self.position = Vec3::new(
                    self.lz_position.x,
                    start_y + (end_y - start_y) * ease,
                    self.lz_position.z,
                );
                self.velocity = Vec3::new(0.0, -((start_y - end_y) / LANDING_DURATION), 0.0);
                self.engine_intensity = 0.8 + (self.total_timer * 4.0).sin() * 0.1;
                self.ramp_open = ease;

                if !self.msg_touchdown_sent && t > 0.9 {
                    self.msg_touchdown_sent = true;
                    messages.push(ExtractionMessage::Warning(
                        "RETRIEVAL BOAT ON DECK! GET TO THE RAMP, TROOPER!".into(),
                    ));
                }
                if self.phase_timer >= LANDING_DURATION {
                    self.phase = ExtractionPhase::Waiting;
                    self.phase_timer = 0.0;
                }
            }

            // ── Waiting ──────────────────────────────────────────────────
            ExtractionPhase::Waiting => {
                self.position = Vec3::new(
                    self.lz_position.x,
                    self.lz_ground_y + HOVER_ALTITUDE,
                    self.lz_position.z,
                );
                self.velocity = Vec3::ZERO;
                self.engine_intensity = 0.6 + (self.total_timer * 3.0).sin() * 0.15;
                self.ramp_open = 1.0;

                let dist_to_player = Vec3::new(
                    player_pos.x - self.lz_position.x, 0.0,
                    player_pos.z - self.lz_position.z,
                ).length();

                if dist_to_player <= BOARDING_RADIUS {
                    self.phase = ExtractionPhase::Boarding;
                    self.phase_timer = 0.0;
                    messages.push(ExtractionMessage::Success(
                        "BOARDING! HOLD ON, TROOPER!".into(),
                    ));
                }
                if self.phase_timer > WAITING_DURATION - 15.0 && !self.msg_hurry_sent {
                    self.msg_hurry_sent = true;
                    messages.push(ExtractionMessage::Warning(
                        "FLEET COM: Retrieval boat can't hold much longer! MOVE IT!".into(),
                    ));
                }
                if self.phase_timer >= WAITING_DURATION {
                    self.phase = ExtractionPhase::Departing;
                    self.phase_timer = 0.0;
                    self.player_aboard = false;
                    messages.push(ExtractionMessage::Warning(
                        "FLEET COM: Retrieval boat is dusting off! Too hot!".into(),
                    ));
                    messages.push(ExtractionMessage::Info(
                        "Extraction failed — the boat left without you.".into(),
                    ));
                }
            }

            // ── Boarding ─────────────────────────────────────────────────
            ExtractionPhase::Boarding => {
                self.boarding_progress = (self.phase_timer / BOARDING_DURATION).clamp(0.0, 1.0);
                self.position = Vec3::new(
                    self.lz_position.x,
                    self.lz_ground_y + HOVER_ALTITUDE,
                    self.lz_position.z,
                );
                self.engine_intensity = 0.7;
                self.ramp_open = 1.0 - self.boarding_progress * 0.3;

                if self.phase_timer >= BOARDING_DURATION {
                    self.phase = ExtractionPhase::Departing;
                    self.phase_timer = 0.0;
                    self.player_aboard = true;
                    messages.push(ExtractionMessage::Success(
                        "ALL ABOARD! Ramp closing — hang on!".into(),
                    ));
                    messages.push(ExtractionMessage::Info(
                        "\"The only good bug is a dead bug!\"".into(),
                    ));
                }
            }

            // ── Departing: lift off from the surface ─────────────────────
            ExtractionPhase::Departing => {
                let t = (self.phase_timer / DEPARTING_DURATION).clamp(0.0, 1.0);
                let climb_speed = 20.0 + t * 120.0;
                let forward_speed = t * 80.0;
                self.velocity = self.approach_dir * -forward_speed + Vec3::Y * climb_speed;
                self.position += self.velocity * dt;
                self.engine_intensity = 0.9 + t * 0.1;
                self.ramp_open = (self.ramp_open - dt * 0.5).max(0.0);

                if self.phase_timer >= DEPARTING_DURATION {
                    if self.player_aboard {
                        self.ascent_start_pos = Some(self.position);
                        self.phase = ExtractionPhase::Ascent;
                        self.phase_timer = 0.0;
                        messages.push(ExtractionMessage::Warning(
                            "FLEET COM: Retrieval boat clear of the AO. Climbing to orbit.".into(),
                        ));
                    }
                }
            }

            // ── Ascent: smooth arc to Roger Young (not snappy, ends at hangar) ──
            ExtractionPhase::Ascent => {
                let t = (self.phase_timer / ASCENT_DURATION).clamp(0.0, 1.0);
                // Smoother ease: 5th-order smoothstep — gentle accel/decel, no snap
                let ease = t * t * t * (t * (t * 6.0 - 15.0) + 10.0);

                let start = self.ascent_start_pos.unwrap_or(self.position);
                let end = self.hangar_entry_point(); // Always end at Roger Young

                // Curved path: quadratic bezier with apex above midpoint (arc, not straight line)
                let mid = start.lerp(end, 0.5);
                let apex = mid + Vec3::Y * 400.0; // Arc peaks ~400m above midpoint
                let one_minus_t = 1.0 - ease;
                let pos_curve = one_minus_t * one_minus_t * start
                    + 2.0 * one_minus_t * ease * apex
                    + ease * ease * end;
                self.position = pos_curve;

                // Velocity from analytical derivative (smooth, no snap)
                // Bezier derivative: 2(1-s)(P1-P0) + 2s(P2-P1); smoothstep5 derivative: 30*t^2*(t-1)^2
                let bezier_deriv = 2.0 * one_minus_t * (apex - start) + 2.0 * ease * (end - apex);
                let ease_deriv = 30.0 * t * t * (t - 1.0) * (t - 1.0);
                self.velocity = bezier_deriv * (ease_deriv / ASCENT_DURATION);

                self.engine_intensity = 0.7 + ease * 0.3;
                self.ramp_open = 0.0;

                if !self.msg_orbit_sent && t > 0.4 {
                    self.msg_orbit_sent = true;
                    messages.push(ExtractionMessage::Info(
                        "FLEET COM: Clearing atmosphere. Roger Young on scope.".into(),
                    ));
                }

                if self.phase_timer >= ASCENT_DURATION {
                    self.msg_docking_sent = true;
                    messages.push(ExtractionMessage::Success(
                        "EXTRACTION COMPLETE — Welcome aboard the Roger Young, trooper.".into(),
                    ));
                    messages.push(ExtractionMessage::Info(
                        "\"I'm from Buenos Aires, and I say kill 'em all!\"".into(),
                    ));
                }
            }
        }

        messages
    }

    /// Whether the entire extraction sequence is fully done.
    pub fn is_done(&self) -> bool {
        match self.phase {
            // Failed extraction: boat departed without player
            ExtractionPhase::Departing if !self.player_aboard => {
                self.phase_timer > DEPARTING_DURATION
            }
            // Successful: reached Roger Young hangar
            ExtractionPhase::Ascent if self.player_aboard => {
                self.phase_timer >= ASCENT_DURATION
            }
            _ => false,
        }
    }

    // ── Roger Young geometry helpers ─────────────────────────────────────

    /// The boat's starting position when it enters orbit (end of Ascent).
    fn position_at_orbit_start(&self) -> Vec3 {
        // ~200m behind the Roger Young, at its altitude
        self.roger_young_pos - self.roger_young_fwd * 400.0 + Vec3::Y * -20.0
    }

    /// Entry point of the Roger Young's hangar bay (just outside the stern).
    pub fn hangar_entry_point(&self) -> Vec3 {
        // Hangar is at the stern (behind the ship), ventral (below center)
        self.roger_young_pos - self.roger_young_fwd * 65.0 - Vec3::Y * 8.0
    }

    /// Dock position inside the hangar bay (center of the bay).
    fn hangar_dock_point(&self) -> Vec3 {
        self.roger_young_pos - self.roger_young_fwd * 30.0 - Vec3::Y * 8.0
    }

    /// Whether the Roger Young should be rendered in 3D (chase cam looks at it during climb).
    pub fn roger_young_visible(&self) -> bool {
        matches!(self.phase, ExtractionPhase::Departing | ExtractionPhase::Ascent)
    }

    /// Whether the player camera should be locked (chase cam: watch boat climb to corvette).
    pub fn player_camera_locked(&self) -> bool {
        self.player_aboard && matches!(self.phase, ExtractionPhase::Departing | ExtractionPhase::Ascent)
    }

    /// Camera position for the player inside the boat during the ride back.
    pub fn aboard_camera_pos(&self) -> Vec3 {
        // Seat position: center of the hold, slightly back from center, eye height
        self.position + self.ship_forward() * -2.0 + Vec3::Y * 0.8
    }

    /// Camera look direction during the ride back (changes per phase).
    pub fn aboard_look_target(&self) -> Vec3 {
        match self.phase {
            ExtractionPhase::Departing | ExtractionPhase::Ascent => {
                // Look at Roger Young — swap to destroyer (corvettes are higher than chase cam)
                self.roger_young_pos
            }
            _ => self.position + self.ship_forward() * 10.0,
        }
    }

    /// Third-person chase camera position: behind and above the retrieval boat.
    /// Slower, cinematic view of climb to corvette (Roger Young in sky).
    pub fn extraction_chase_camera_pos(&self) -> Vec3 {
        let fwd = self.ship_forward();
        // Farther back, higher — slower feel, Roger Young visible in frame
        self.position - fwd * 50.0 + Vec3::Y * 18.0
    }

    /// Chase cam always looks at Roger Young (corvettes higher — swap scene to destroyer).
    pub fn extraction_chase_look_target(&self) -> Vec3 {
        self.roger_young_pos
    }

    // ── ETA / distance helpers ───────────────────────────────────────────

    pub fn eta_to_touchdown(&self) -> f32 {
        match self.phase {
            ExtractionPhase::Called => {
                (CALLED_DURATION - self.phase_timer) + INBOUND_DURATION + LANDING_DURATION
            }
            ExtractionPhase::Inbound => {
                (INBOUND_DURATION - self.phase_timer) + LANDING_DURATION
            }
            ExtractionPhase::Landing => {
                (LANDING_DURATION - self.phase_timer).max(0.0)
            }
            _ => 0.0,
        }
    }

    pub fn time_until_dustoff(&self) -> f32 {
        if self.phase == ExtractionPhase::Waiting {
            (WAITING_DURATION - self.phase_timer).max(0.0)
        } else {
            0.0
        }
    }

    pub fn distance_to_lz(&self, pos: Vec3) -> f32 {
        Vec3::new(pos.x - self.lz_position.x, 0.0, pos.z - self.lz_position.z).length()
    }

    pub fn ramp_position(&self) -> Vec3 {
        self.position + self.approach_dir * 8.0 - Vec3::Y * (HOVER_ALTITUDE - 0.5)
    }

    pub fn ship_forward(&self) -> Vec3 {
        -self.approach_dir
    }

    pub fn ship_right(&self) -> Vec3 {
        let fwd = self.ship_forward();
        Vec3::new(-fwd.z, 0.0, fwd.x)
    }

    // ── Door gunner helpers ─────────────────────────────────────────────

    pub fn gunner_left_pos(&self) -> Vec3 {
        self.position + self.ship_right() * -5.5 + self.approach_dir * 2.0 - Vec3::Y * 0.5
    }

    pub fn gunner_right_pos(&self) -> Vec3 {
        self.position + self.ship_right() * 5.5 + self.approach_dir * 2.0 - Vec3::Y * 0.5
    }

    pub fn gunners_active(&self) -> bool {
        matches!(
            self.phase,
            ExtractionPhase::Landing
                | ExtractionPhase::Waiting
                | ExtractionPhase::Boarding
                | ExtractionPhase::Departing
        )
    }

    pub const GUNNER_FIRE_RATE: f32 = 8.0;
    pub const GUNNER_RANGE: f32 = 60.0;
    pub const GUNNER_DAMAGE: f32 = 15.0;

    pub fn update_gunners(&mut self, dt: f32) -> (u32, u32) {
        if !self.gunners_active() { return (0, 0); }
        let interval = 1.0 / Self::GUNNER_FIRE_RATE;
        self.gunner_left_timer += dt;
        self.gunner_right_timer += dt;

        let mut left_shots = 0u32;
        while self.gunner_left_timer >= interval && self.gunner_left_target.is_some() {
            self.gunner_left_timer -= interval;
            left_shots += 1;
        }
        if self.gunner_left_target.is_none() {
            self.gunner_left_timer = self.gunner_left_timer.min(interval);
        }

        let mut right_shots = 0u32;
        while self.gunner_right_timer >= interval && self.gunner_right_target.is_some() {
            self.gunner_right_timer -= interval;
            right_shots += 1;
        }
        if self.gunner_right_target.is_none() {
            self.gunner_right_timer = self.gunner_right_timer.min(interval);
        }
        (left_shots, right_shots)
    }

    // ── Physics / collision helpers ──────────────────────────────────────

    pub fn hull_half_extents(&self) -> Vec3 {
        Vec3::new(6.0, 2.0, 8.0)
    }

    pub fn needs_collider(&self) -> bool {
        matches!(
            self.phase,
            ExtractionPhase::Landing | ExtractionPhase::Waiting | ExtractionPhase::Boarding
        )
    }

    // ── Boarding camera interpolation ────────────────────────────────────

    pub fn boarding_interior_pos(&self) -> Vec3 {
        self.position + self.approach_dir * 2.0 + Vec3::Y * 0.5
    }

    pub fn boarding_camera_pos(&self, start: Vec3) -> Vec3 {
        let t = self.boarding_progress;
        let ease = t * t * (3.0 - 2.0 * t);
        start.lerp(self.boarding_interior_pos(), ease)
    }

    pub fn boarding_look_dir(&self) -> Vec3 {
        let fwd = self.ship_forward();
        Vec3::new(fwd.x, 0.15, fwd.z).normalize()
    }
}

// ── Roger Young rendering data ──────────────────────────────────────────

/// A single component mesh of the Roger Young destroyer.
pub struct RogerYoungPart {
    /// Offset from the ship center.
    pub offset: Vec3,
    /// Scale of this part.
    pub scale: Vec3,
    /// Color [R, G, B, A].
    pub color: [f32; 4],
    /// Which mesh to use: 0=rock(angular), 1=sphere, 2=flash(glow).
    pub mesh_type: u8,
}

/// Generate all the parts of the Roger Young Federation destroyer.
/// Returns a list of parts positioned relative to ship center.
/// The ship faces +Z (forward is positive Z in local space).
pub fn roger_young_parts() -> Vec<RogerYoungPart> {
    let hull_gray = [0.28, 0.30, 0.32, 1.0];
    let dark_gray = [0.18, 0.19, 0.22, 1.0];
    let accent = [0.22, 0.24, 0.28, 1.0];
    let window_blue = [0.4, 0.6, 1.0, 0.9];
    let engine_blue = [0.5, 0.7, 3.0, 1.0]; // emissive blue-white
    let nav_red = [2.0, 0.2, 0.1, 1.0];
    let nav_green = [0.1, 2.0, 0.2, 1.0];
    let hangar_interior = [0.12, 0.13, 0.15, 1.0];

    vec![
        // ── Main hull: long angular wedge ──
        RogerYoungPart { offset: Vec3::ZERO, scale: Vec3::new(12.0, 6.0, 55.0), color: hull_gray, mesh_type: 0 },
        // Upper hull (bridge deck)
        RogerYoungPart { offset: Vec3::new(0.0, 5.0, 15.0), scale: Vec3::new(8.0, 3.0, 20.0), color: hull_gray, mesh_type: 0 },
        // Lower hull keel
        RogerYoungPart { offset: Vec3::new(0.0, -4.5, -5.0), scale: Vec3::new(8.0, 2.0, 35.0), color: dark_gray, mesh_type: 0 },

        // ── Bridge tower ──
        RogerYoungPart { offset: Vec3::new(0.0, 9.0, 25.0), scale: Vec3::new(5.0, 4.0, 8.0), color: accent, mesh_type: 0 },
        // Bridge windows
        RogerYoungPart { offset: Vec3::new(0.0, 10.5, 29.5), scale: Vec3::new(4.0, 1.5, 1.0), color: window_blue, mesh_type: 1 },

        // ── Forward prow (tapered wedge) ──
        RogerYoungPart { offset: Vec3::new(0.0, 0.0, 50.0), scale: Vec3::new(5.0, 3.0, 12.0), color: hull_gray, mesh_type: 0 },
        RogerYoungPart { offset: Vec3::new(0.0, 1.5, 58.0), scale: Vec3::new(2.5, 1.5, 6.0), color: dark_gray, mesh_type: 0 },

        // ── Port engine nacelle ──
        RogerYoungPart { offset: Vec3::new(-16.0, -1.0, -20.0), scale: Vec3::new(4.0, 3.5, 18.0), color: dark_gray, mesh_type: 0 },
        // Port engine pylon
        RogerYoungPart { offset: Vec3::new(-12.0, -1.0, -15.0), scale: Vec3::new(3.0, 1.5, 8.0), color: accent, mesh_type: 0 },
        // Port engine glow
        RogerYoungPart { offset: Vec3::new(-16.0, -1.0, -39.0), scale: Vec3::new(3.5, 3.0, 2.0), color: engine_blue, mesh_type: 2 },

        // ── Starboard engine nacelle ──
        RogerYoungPart { offset: Vec3::new(16.0, -1.0, -20.0), scale: Vec3::new(4.0, 3.5, 18.0), color: dark_gray, mesh_type: 0 },
        // Starboard engine pylon
        RogerYoungPart { offset: Vec3::new(12.0, -1.0, -15.0), scale: Vec3::new(3.0, 1.5, 8.0), color: accent, mesh_type: 0 },
        // Starboard engine glow
        RogerYoungPart { offset: Vec3::new(16.0, -1.0, -39.0), scale: Vec3::new(3.5, 3.0, 2.0), color: engine_blue, mesh_type: 2 },

        // ── Central engine cluster (3 exhausts) ──
        RogerYoungPart { offset: Vec3::new(0.0, 0.0, -56.0), scale: Vec3::new(5.0, 4.0, 2.5), color: engine_blue, mesh_type: 2 },
        RogerYoungPart { offset: Vec3::new(-5.0, -2.0, -54.0), scale: Vec3::new(2.5, 2.5, 2.0), color: engine_blue, mesh_type: 2 },
        RogerYoungPart { offset: Vec3::new(5.0, -2.0, -54.0), scale: Vec3::new(2.5, 2.5, 2.0), color: engine_blue, mesh_type: 2 },

        // ── Weapons batteries (port and starboard) ──
        RogerYoungPart { offset: Vec3::new(-10.0, 4.0, 5.0), scale: Vec3::new(2.0, 1.0, 5.0), color: dark_gray, mesh_type: 0 },
        RogerYoungPart { offset: Vec3::new(10.0, 4.0, 5.0), scale: Vec3::new(2.0, 1.0, 5.0), color: dark_gray, mesh_type: 0 },
        RogerYoungPart { offset: Vec3::new(-10.0, 4.0, -10.0), scale: Vec3::new(2.0, 1.0, 5.0), color: dark_gray, mesh_type: 0 },
        RogerYoungPart { offset: Vec3::new(10.0, 4.0, -10.0), scale: Vec3::new(2.0, 1.0, 5.0), color: dark_gray, mesh_type: 0 },

        // ── Dorsal sensor array ──
        RogerYoungPart { offset: Vec3::new(0.0, 12.0, 10.0), scale: Vec3::new(1.0, 4.0, 1.0), color: accent, mesh_type: 1 },
        RogerYoungPart { offset: Vec3::new(0.0, 14.5, 10.0), scale: Vec3::splat(1.5), color: window_blue, mesh_type: 2 },

        // ── Ventral sensor array ──
        RogerYoungPart { offset: Vec3::new(0.0, -7.0, 20.0), scale: Vec3::new(1.0, 3.0, 1.0), color: accent, mesh_type: 1 },

        // ── Hangar bay (stern ventral — dark interior) ──
        RogerYoungPart { offset: Vec3::new(0.0, -5.0, -35.0), scale: Vec3::new(10.0, 5.0, 20.0), color: hangar_interior, mesh_type: 0 },
        // Hangar bay rim (lighter)
        RogerYoungPart { offset: Vec3::new(0.0, -3.0, -46.0), scale: Vec3::new(11.0, 6.0, 2.0), color: accent, mesh_type: 0 },

        // ── Hangar bay interior lights ──
        RogerYoungPart { offset: Vec3::new(-4.0, -2.0, -30.0), scale: Vec3::splat(0.8), color: [1.0, 0.9, 0.6, 1.0], mesh_type: 2 },
        RogerYoungPart { offset: Vec3::new(4.0, -2.0, -30.0), scale: Vec3::splat(0.8), color: [1.0, 0.9, 0.6, 1.0], mesh_type: 2 },
        RogerYoungPart { offset: Vec3::new(-4.0, -2.0, -40.0), scale: Vec3::splat(0.8), color: [1.0, 0.9, 0.6, 1.0], mesh_type: 2 },
        RogerYoungPart { offset: Vec3::new(4.0, -2.0, -40.0), scale: Vec3::splat(0.8), color: [1.0, 0.9, 0.6, 1.0], mesh_type: 2 },

        // ── Navigation lights ──
        RogerYoungPart { offset: Vec3::new(-13.0, 0.0, 0.0), scale: Vec3::splat(0.6), color: nav_red, mesh_type: 2 },
        RogerYoungPart { offset: Vec3::new(13.0, 0.0, 0.0), scale: Vec3::splat(0.6), color: nav_green, mesh_type: 2 },
        // Stern warning light
        RogerYoungPart { offset: Vec3::new(0.0, 6.0, -50.0), scale: Vec3::splat(0.8), color: nav_red, mesh_type: 2 },

        // ── Lateral armor plates ──
        RogerYoungPart { offset: Vec3::new(-12.5, 0.0, 10.0), scale: Vec3::new(1.0, 5.0, 25.0), color: hull_gray, mesh_type: 0 },
        RogerYoungPart { offset: Vec3::new(12.5, 0.0, 10.0), scale: Vec3::new(1.0, 5.0, 25.0), color: hull_gray, mesh_type: 0 },

        // ── Hull name plate glow ("ROGER YOUNG" implied by bright accent) ──
        RogerYoungPart { offset: Vec3::new(0.0, 3.5, 35.0), scale: Vec3::new(6.0, 0.5, 1.0), color: [0.8, 0.75, 0.5, 0.8], mesh_type: 2 },
    ]
}

// ── Message type ────────────────────────────────────────────────────────

pub enum ExtractionMessage {
    Info(String),
    Warning(String),
    Success(String),
}
