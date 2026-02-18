//! Game state types: debug settings, phases, sequences, weather, war table, etc.
//!
//! Extracted from main.rs for clearer separation of state types from application logic.

use glam::{Quat, Vec3};
use hecs::World;
use rand::Rng;

use crate::fps;
use crate::squad::{spawn_one_squad_mate, SQUAD_DROP_DATA};

// ── Debug & UI ─────────────────────────────────────────────────────────────

/// Developer debug settings, toggled via the in-game debug menu (F3).
pub(crate) struct DebugSettings {
    /// Show the debug menu overlay.
    pub menu_open: bool,
    /// Currently selected menu item index.
    pub selected: usize,
    /// Show the top-left debug text block (FPS, XYZ, system, planet, etc.). When false, hides it to save screen space.
    pub show_debug_overlay: bool,
    /// Noclip free-fly camera (no gravity/collision).
    pub noclip: bool,
    /// God mode: player takes no damage.
    pub god_mode: bool,
    /// Disable bug spawning entirely.
    pub no_bug_spawns: bool,
    /// Infinite ammo (no reload needed).
    pub infinite_ammo: bool,
    /// Show collision/physics debug info.
    pub show_physics_debug: bool,
    /// Show detailed FPS & performance stats.
    pub show_perf_stats: bool,
    /// Time scale multiplier (0.1 = slow-mo, 1.0 = normal, 2.0 = fast).
    pub time_scale: f32,
    /// Freeze time of day cycle.
    pub freeze_time_of_day: bool,
    /// Kill all living bugs (one-shot action).
    pub kill_all_bugs_requested: bool,
    /// Teleport player to surface origin.
    pub teleport_origin_requested: bool,
    /// Show chunk boundaries.
    pub show_chunk_debug: bool,
}

impl DebugSettings {
    pub fn new() -> Self {
        Self {
            menu_open: false,
            selected: 0,
            show_debug_overlay: true,
            noclip: false,
            god_mode: false,
            no_bug_spawns: false,
            infinite_ammo: false,
            show_physics_debug: false,
            show_perf_stats: true,
            time_scale: 1.0,
            freeze_time_of_day: false,
            kill_all_bugs_requested: false,
            teleport_origin_requested: false,
            show_chunk_debug: false,
        }
    }

    pub fn menu_items(&self) -> Vec<(&str, bool)> {
        vec![
            ("Show Debug Overlay", self.show_debug_overlay),
            ("Noclip (free-fly)", self.noclip),
            ("God Mode", self.god_mode),
            ("No Bug Spawns", self.no_bug_spawns),
            ("Infinite Ammo", self.infinite_ammo),
            ("Show Physics Debug", self.show_physics_debug),
            ("Show Perf Stats", self.show_perf_stats),
            ("Freeze Time of Day", self.freeze_time_of_day),
            ("Show Chunk Boundaries", self.show_chunk_debug),
            ("-- Kill All Bugs --", false),
            ("-- Teleport to Origin --", false),
            ("-- Time x0.25 --", false),
            ("-- Time x0.5 --", false),
            ("-- Time x1.0 --", false),
            ("-- Time x2.0 --", false),
        ]
    }

    pub fn menu_item_count(&self) -> usize {
        15
    }

    pub fn toggle_selected(&mut self) {
        match self.selected {
            0 => self.show_debug_overlay = !self.show_debug_overlay,
            1 => self.noclip = !self.noclip,
            2 => self.god_mode = !self.god_mode,
            3 => self.no_bug_spawns = !self.no_bug_spawns,
            4 => self.infinite_ammo = !self.infinite_ammo,
            5 => self.show_physics_debug = !self.show_physics_debug,
            6 => self.show_perf_stats = !self.show_perf_stats,
            7 => self.freeze_time_of_day = !self.freeze_time_of_day,
            8 => self.show_chunk_debug = !self.show_chunk_debug,
            9 => self.kill_all_bugs_requested = true,
            10 => self.teleport_origin_requested = true,
            11 => self.time_scale = 0.25,
            12 => self.time_scale = 0.5,
            13 => self.time_scale = 1.0,
            14 => self.time_scale = 2.0,
            _ => {}
        }
    }
}

// ── Interaction prompts (single source of truth for key labels; overlay renders dynamically) ──

/// Key label shown in prompts (e.g. "E", "SPACE"). Change here to update all interact prompts.
pub const INTERACT_KEY: &str = "E";
/// Key for deploy / launch actions.
pub const DEPLOY_KEY: &str = "SPACE";
/// Keys for dialogue choices (shown when dialogue is open).
pub const DIALOGUE_CHOICE_KEYS: &str = "1-4";
/// Key to close dialogue.
pub const DIALOGUE_CLOSE_KEY: &str = "Esc";

/// One on-screen interaction prompt: "[key] action" (e.g. "[E] Talk to Johnny Rico").
#[derive(Debug, Clone)]
pub struct InteractPrompt {
    pub key: &'static str,
    pub action: String,
}

impl InteractPrompt {
    /// Build the full prompt string for overlay (e.g. "[E] ACCESS WAR TABLE").
    pub fn display_text(&self) -> String {
        format!("[{}] {}", self.key, self.action)
    }
}

/// Game phase
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePhase {
    MainMenu,
    InShip,
    ApproachPlanet,
    DropSequence,
    Playing,
    Victory,
    Defeat,
    Paused,
}

/// Camera screen shake for cinematic impact.
pub(crate) struct ScreenShake {
    pub intensity: f32,
    pub decay_rate: f32,
    pub offset: Vec3,
    pub trauma: f32,
}

impl ScreenShake {
    pub fn new() -> Self {
        Self { intensity: 0.0, decay_rate: 5.0, offset: Vec3::ZERO, trauma: 0.0 }
    }

    pub fn add_trauma(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }

    pub fn update(&mut self, dt: f32) {
        self.intensity = self.trauma * self.trauma;
        if self.intensity > 0.001 {
            let max_offset = self.intensity * 0.4;
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f32();
            self.offset = Vec3::new(
                (t * 173.7).sin() * max_offset,
                (t * 259.3).cos() * max_offset,
                (t * 97.1).sin() * max_offset * 0.3,
            );
        } else {
            self.offset = Vec3::ZERO;
        }
        self.trauma = (self.trauma - self.decay_rate * dt).max(0.0);
    }
}

/// Kill streak tracker for cinematic announcements.
pub(crate) struct KillStreakTracker {
    pub streak_count: u32,
    pub time_since_kill: f32,
    pub streak_timeout: f32,
    pub announcement: Option<(String, f32, [f32; 4])>,
    pub total_multikills: u32,
}

impl KillStreakTracker {
    pub fn new() -> Self {
        Self {
            streak_count: 0,
            time_since_kill: 999.0,
            streak_timeout: 3.0,
            announcement: None,
            total_multikills: 0,
        }
    }

    pub fn register_kill(&mut self) {
        self.time_since_kill = 0.0;
        self.streak_count += 1;

        let (text, color) = match self.streak_count {
            2  => ("DOUBLE KILL!", [1.0, 0.9, 0.3, 1.0]),
            3  => ("TRIPLE KILL!", [1.0, 0.6, 0.1, 1.0]),
            4  => ("OVERKILL!", [1.0, 0.3, 0.1, 1.0]),
            5  => ("KILLIONAIRE!", [1.0, 0.1, 0.1, 1.0]),
            6  => ("EXTERMINATION!", [0.8, 0.0, 1.0, 1.0]),
            n if n >= 7 && n % 5 == 0 => ("UNSTOPPABLE!", [1.0, 0.0, 0.5, 1.0]),
            10 => ("KILLING FRENZY!", [1.0, 0.0, 0.0, 1.0]),
            _ => return,
        };
        self.total_multikills += 1;
        self.announcement = Some((text.to_string(), 2.5, color));
    }

    pub fn update(&mut self, dt: f32) {
        self.time_since_kill += dt;
        if self.time_since_kill > self.streak_timeout {
            self.streak_count = 0;
        }
        if let Some((_, ref mut t, _)) = self.announcement {
            *t -= dt;
            if *t <= 0.0 {
                self.announcement = None;
            }
        }
    }
}

// ── Weather ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub(crate) enum WeatherState {
    #[default]
    Clear,
    Cloudy,
    Rain,
    Storm,
    Snow,
}

/// Smooth weather that blends between states.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Weather {
    pub current: WeatherState,
    pub target: WeatherState,
    pub blend: f32,
    pub hold_timer: f32,
    pub cloud_density: f32,
    pub dust: f32,
    pub fog_density: f32,
}

impl Weather {
    pub fn new() -> Self {
        Self {
            current: WeatherState::Clear,
            target: WeatherState::Clear,
            blend: 0.0,
            hold_timer: 30.0,
            cloud_density: 0.2,
            dust: 0.05,
            fog_density: 0.0002,
        }
    }

    /// Create weather with a random initial state (for per-planet variety).
    pub fn random() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let states = [
            WeatherState::Clear,
            WeatherState::Cloudy,
            WeatherState::Rain,
            WeatherState::Storm,
            WeatherState::Snow,
        ];
        let idx = rng.gen_range(0..states.len());
        let state = states[idx];
        let (cloud_density, dust, fog_density) = Self::params_for(state);
        let hold_timer = 15.0 + rng.gen::<f32>() * 50.0;
        Self {
            current: state,
            target: state,
            blend: 0.0,
            hold_timer,
            cloud_density,
            dust,
            fog_density,
        }
    }

    fn params_for(state: WeatherState) -> (f32, f32, f32) {
        match state {
            WeatherState::Clear  => (0.15, 0.03, 0.0001),
            WeatherState::Cloudy => (0.55, 0.08, 0.0003),
            WeatherState::Rain   => (0.80, 0.15, 0.0008),
            WeatherState::Storm  => (0.95, 0.25, 0.0015),
            WeatherState::Snow   => (0.70, 0.12, 0.0005),
        }
    }

    /// Sky color tint for current weather (blended during transition). Multiply with planet atmosphere for moody sky.
    pub fn atmosphere_tint(&self) -> [f32; 3] {
        let tint_for = |s: WeatherState| -> [f32; 3] {
            match s {
                WeatherState::Clear  => [1.0, 1.0, 1.0],
                WeatherState::Cloudy => [0.82, 0.84, 0.88],
                WeatherState::Rain   => [0.55, 0.60, 0.72],
                WeatherState::Storm  => [0.40, 0.44, 0.52],
                WeatherState::Snow   => [0.78, 0.82, 0.92],
            }
        };
        let a = tint_for(self.current);
        let b = tint_for(self.target);
        let t = self.blend;
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
        ]
    }

    pub fn update(&mut self, dt: f32) {
        if self.current != self.target {
            self.blend += dt * 0.15;
            if self.blend >= 1.0 {
                self.blend = 0.0;
                self.current = self.target;
            }
        }

        let (c0, d0, f0) = Self::params_for(self.current);
        let (c1, d1, f1) = Self::params_for(self.target);
        let t = self.blend;
        let target_cloud = c0 + (c1 - c0) * t;
        let target_dust = d0 + (d1 - d0) * t;
        let target_fog = f0 + (f1 - f0) * t;

        let rate = dt * 2.0;
        self.cloud_density += (target_cloud - self.cloud_density) * rate;
        self.dust += (target_dust - self.dust) * rate;
        self.fog_density += (target_fog - self.fog_density) * rate;

        self.hold_timer -= dt;
        if self.hold_timer <= 0.0 && self.current == self.target {
            self.target = match self.current {
                WeatherState::Clear => {
                    if rand::random::<f32>() < 0.6 { WeatherState::Clear } else { WeatherState::Cloudy }
                }
                WeatherState::Cloudy => {
                    let r = rand::random::<f32>();
                    if r < 0.3 { WeatherState::Clear }
                    else if r < 0.65 { WeatherState::Cloudy }
                    else if r < 0.85 { WeatherState::Rain }
                    else { WeatherState::Snow }
                }
                WeatherState::Rain => {
                    let r = rand::random::<f32>();
                    if r < 0.4 { WeatherState::Cloudy }
                    else if r < 0.8 { WeatherState::Rain }
                    else { WeatherState::Storm }
                }
                WeatherState::Storm => {
                    if rand::random::<f32>() < 0.6 { WeatherState::Rain } else { WeatherState::Storm }
                }
                WeatherState::Snow => {
                    let r = rand::random::<f32>();
                    if r < 0.5 { WeatherState::Cloudy }
                    else if r < 0.8 { WeatherState::Snow }
                    else { WeatherState::Rain }
                }
            };
            self.hold_timer = 20.0 + rand::random::<f32>() * 40.0;
        }
    }

    /// Spawn rate (per frame) and fall speed for rain. Only active when current/target is Rain or Storm.
    pub fn rain_params(&self) -> (u32, f32) {
        let rain_amount = match (self.current, self.target) {
            (WeatherState::Rain, _) | (_, WeatherState::Rain) | (WeatherState::Storm, _) | (_, WeatherState::Storm) => {
                ((self.cloud_density - 0.5) / 0.5).clamp(0.0, 1.0)
            }
            _ => 0.0,
        };
        if rain_amount < 0.01 {
            (0, 0.0)
        } else {
            let spawn_rate = (rain_amount * 120.0) as u32;
            let fall_speed = 18.0 + rain_amount * 25.0;
            (spawn_rate, fall_speed)
        }
    }

    /// Spawn rate (per frame) and fall speed for snow. Only active when current/target is Snow.
    pub fn snow_params(&self) -> (u32, f32) {
        let snow_amount = match (self.current, self.target) {
            (WeatherState::Snow, _) | (_, WeatherState::Snow) => {
                ((self.cloud_density - 0.45) / 0.35).clamp(0.0, 1.0)
            }
            _ => 0.0,
        };
        if snow_amount < 0.01 {
            (0, 0.0)
        } else {
            let spawn_rate = (snow_amount * 80.0) as u32;
            let fall_speed = 3.0 + snow_amount * 4.0;
            (spawn_rate, fall_speed)
        }
    }
}

impl Default for Weather {
    fn default() -> Self {
        Self::new()
    }
}

// ── Warp & Approach ─────────────────────────────────────────────────────────

/// Warp jump sequence state.
pub(crate) struct WarpSequence {
    pub target_system_idx: usize,
    pub timer: f32,
    pub duration: f32,
}

impl WarpSequence {
    pub fn new(target_idx: usize) -> Self {
        Self {
            target_system_idx: target_idx,
            timer: 0.0,
            duration: 7.0,
        }
    }

    pub fn progress(&self) -> f32 {
        (self.timer / self.duration).clamp(0.0, 1.0)
    }

    pub fn is_complete(&self) -> bool {
        self.timer >= self.duration
    }
}

/// Approach craft state: flyable small craft toward planet.
pub(crate) struct ApproachFlightState {
    pub position: Vec3,
    pub velocity: Vec3,
}

// ── Drop Pod ────────────────────────────────────────────────────────────────

/// Drop pod descent phases — real-time continuous descent from Corvette to surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropPhase {
    Detach,
    SpaceFall,
    AtmosphericEntry,
    RetroBoost,
    Impact,
    Emerge,
}

/// Real-time drop pod simulation with continuous physics.
pub(crate) struct DropPodSequence {
    pub target_planet_idx: usize,
    pub phase: DropPhase,
    pub altitude: f32,
    pub velocity: f32,
    pub lateral_vel: Vec3,
    pub total_timer: f32,
    pub phase_timer: f32,
    pub shake_intensity: f32,
    pub shake_offset: Vec3,
    pub velocity_factor: f32,
    pub landing_pos: Vec3,
    pub terrain_ready: bool,
    pub camera_pitch: f32,
    pub camera_yaw: f32,
    pub camera_roll: f32,
    pub retro_active: bool,
    pub corvette_separation: f32,
    pub planet_visual_radius: f32,
    pub atmosphere_glow: f32,
    pub terrain_fog: f32,
}

impl DropPodSequence {
    pub fn new(planet_idx: usize) -> Self {
        Self {
            target_planet_idx: planet_idx,
            phase: DropPhase::Detach,
            altitude: 2500.0,
            velocity: 0.0,
            lateral_vel: Vec3::new(
                (rand::random::<f32>() - 0.5) * 2.0,
                0.0,
                (rand::random::<f32>() - 0.5) * 2.0,
            ),
            total_timer: 0.0,
            phase_timer: 0.0,
            shake_intensity: 0.0,
            shake_offset: Vec3::ZERO,
            velocity_factor: 0.0,
            landing_pos: Vec3::ZERO,
            terrain_ready: false,
            camera_pitch: -1.3,
            camera_yaw: 0.0,
            camera_roll: 0.0,
            retro_active: false,
            corvette_separation: 0.0,
            planet_visual_radius: 60.0,
            atmosphere_glow: 0.0,
            terrain_fog: 1.0,
        }
    }

    pub fn update(&mut self, dt: f32) -> bool {
        self.total_timer += dt;
        self.phase_timer += dt;
        let t = self.total_timer;

        match self.phase {
            DropPhase::Detach => {
                self.velocity += 18.0 * dt;
                self.altitude -= self.velocity * dt;
                self.corvette_separation += (8.0 + self.phase_timer * 15.0) * dt;
                self.shake_intensity = 0.06 * (1.0 - self.phase_timer / 3.0).max(0.0);
                self.camera_pitch = -1.3 + self.phase_timer * 0.05;
                self.camera_yaw += dt * 0.08;
                self.planet_visual_radius = 60.0 + self.phase_timer * 5.0;

                if self.phase_timer >= 3.0 {
                    self.phase = DropPhase::SpaceFall;
                    self.phase_timer = 0.0;
                }
            }
            DropPhase::SpaceFall => {
                let gravity = 45.0;
                self.velocity += gravity * dt;
                self.altitude -= self.velocity * dt;
                self.velocity_factor = (self.velocity / 400.0).min(1.0);
                self.shake_intensity = 0.005 + self.velocity_factor * 0.02;

                let target_pitch = -1.1 + self.velocity_factor * 0.4;
                self.camera_pitch += (target_pitch - self.camera_pitch) * dt * 0.5;
                self.camera_yaw += dt * 0.12;

                let alt_frac = (self.altitude / 2500.0).clamp(0.0, 1.0);
                self.planet_visual_radius = 60.0 + (1.0 - alt_frac) * 400.0;

                if self.altitude < 1200.0 {
                    self.atmosphere_glow = ((1200.0 - self.altitude) / 400.0).clamp(0.0, 1.0);
                }

                if self.altitude < 800.0 {
                    self.phase = DropPhase::AtmosphericEntry;
                    self.phase_timer = 0.0;
                }
            }
            DropPhase::AtmosphericEntry => {
                let gravity = 45.0;
                let drag = 0.004 * self.velocity * self.velocity;
                self.velocity += (gravity - drag) * dt;
                self.velocity = self.velocity.max(55.0);
                self.altitude -= self.velocity * dt;

                let entry_intensity = (1.0 - self.altitude / 800.0).clamp(0.0, 1.0);
                self.velocity_factor = (self.velocity / 300.0).min(1.0);

                self.shake_intensity = 0.1 + entry_intensity * 0.4;
                let target_pitch = -0.7 + entry_intensity * 0.25;
                self.camera_pitch += (target_pitch - self.camera_pitch) * dt * 2.0;
                self.camera_roll = (t * 8.0).sin() * 0.08 * entry_intensity
                    + (t * 13.0).cos() * 0.05 * entry_intensity;
                self.camera_yaw += (t * 3.0).sin() * 0.03 * dt;

                self.atmosphere_glow = (0.8 + entry_intensity * 0.2).min(1.0);

                if self.altitude < 600.0 {
                    self.planet_visual_radius = 0.0;
                }

                self.terrain_fog = (self.altitude / 500.0).clamp(0.0, 1.0);

                if self.altitude < 180.0 {
                    self.phase = DropPhase::RetroBoost;
                    self.phase_timer = 0.0;
                    self.retro_active = true;
                    self.atmosphere_glow = 0.0;
                }
            }
            DropPhase::RetroBoost => {
                let gravity = 45.0;
                let retro_thrust = 140.0 + (self.velocity - 40.0).max(0.0) * 2.0;
                let decel = retro_thrust - gravity;
                self.velocity -= decel * dt;
                self.velocity = self.velocity.max(12.0);
                self.altitude -= self.velocity * dt;

                let decel_g = decel / 10.0;
                self.shake_intensity = 0.18 + decel_g * 0.025;
                self.velocity_factor = (self.velocity / 200.0).min(1.0);

                let target_pitch = -0.35 - (decel_g * 0.015).min(0.15);
                self.camera_pitch += (target_pitch - self.camera_pitch) * dt * 3.0;
                self.camera_roll = (t * 5.0).sin() * 0.015;

                self.terrain_fog = (self.altitude / 120.0).clamp(0.0, 0.25);

                if self.altitude <= 0.0 {
                    self.altitude = 0.0;
                    self.phase = DropPhase::Impact;
                    self.phase_timer = 0.0;
                    self.shake_intensity = 2.0;
                    self.terrain_fog = 0.0;
                }
            }
            DropPhase::Impact => {
                let p = (self.phase_timer / 2.0).min(1.0);
                self.shake_intensity = 2.0 * (1.0 - p).powi(2);
                self.velocity = 0.0;
                self.velocity_factor = (1.0 - p) * 0.3;
                self.camera_pitch = -0.2 * (1.0 - p);
                self.camera_roll = (t * 25.0).sin() * 0.1 * (1.0 - p);

                if self.phase_timer >= 2.0 {
                    self.phase = DropPhase::Emerge;
                    self.phase_timer = 0.0;
                }
            }
            DropPhase::Emerge => {
                let p = (self.phase_timer / 2.0).min(1.0);
                let ease = p * p * (3.0 - 2.0 * p);
                self.shake_intensity = 0.02 * (1.0 - p);
                self.velocity_factor = 0.0;
                self.camera_pitch = self.camera_pitch * (1.0 - ease * 2.0).max(0.0);
                self.camera_roll = self.camera_roll * (1.0 - ease);

                if self.phase_timer >= 2.0 {
                    return true;
                }
            }
        }

        let shake_freq = 12.0 + self.velocity_factor * 25.0;
        self.shake_offset = Vec3::new(
            (t * shake_freq).sin() * self.shake_intensity * 0.7,
            (t * shake_freq * 1.3 + 1.0).cos() * self.shake_intensity,
            (t * shake_freq * 0.8 + 2.0).sin() * self.shake_intensity * 0.5,
        );

        false
    }
}

// ── Squad Drop ──────────────────────────────────────────────────────────────

/// One squad drop pod descending from orbit.
pub(crate) struct SquadPod {
    pub position: Vec3,
    pub velocity_y: f32,
    pub start_delay: f32,
    pub squad_index: usize,
    pub landed: bool,
}

/// Squad mates drop in separate pods from the fleet in orbit.
pub(crate) struct SquadDropSequence {
    pub landing_site: Vec3,
    pub terrain_y_center: f32,
    pub pods: Vec<SquadPod>,
    pub message_sent: bool,
}

impl SquadDropSequence {
    pub fn new(landing: Vec3, terrain_y: f32) -> Self {
        let mut rng = rand::thread_rng();
        let pods = (0..4)
            .map(|i| {
                let angle = (i as f32 * 1.2 + rng.gen::<f32>() * 0.5) * std::f32::consts::TAU / 4.0;
                let offset_dist = 8.0 + rng.gen::<f32>() * 12.0;
                let target_x = landing.x + angle.cos() * offset_dist;
                let target_z = landing.z + angle.sin() * offset_dist;
                SquadPod {
                    position: Vec3::new(target_x, 550.0 + rng.gen::<f32>() * 80.0, target_z),
                    velocity_y: 0.0,
                    start_delay: i as f32 * 2.5,
                    squad_index: i,
                    landed: false,
                }
            })
            .collect();
        Self {
            landing_site: landing,
            terrain_y_center: terrain_y,
            pods,
            message_sent: false,
        }
    }

    pub fn update(
        &mut self,
        dt: f32,
        sample_terrain: impl Fn(f32, f32) -> f32,
        world: &mut World,
    ) -> bool {
        const GRAVITY: f32 = 35.0;
        const MAX_VEL: f32 = 120.0;

        for pod in &mut self.pods {
            if pod.landed {
                continue;
            }
            if pod.start_delay > 0.0 {
                pod.start_delay -= dt;
                continue;
            }
            pod.velocity_y = (pod.velocity_y + GRAVITY * dt).min(MAX_VEL);
            pod.position.y -= pod.velocity_y * dt;
            let ground_y = sample_terrain(pod.position.x, pod.position.z);
            if pod.position.y <= ground_y + 2.5 {
                pod.landed = true;
                let (name, kind, formation) = SQUAD_DROP_DATA[pod.squad_index];
                spawn_one_squad_mate(
                    world,
                    pod.position,
                    ground_y,
                    name,
                    kind,
                    formation,
                );
            }
        }
        self.pods.iter().all(|p| p.landed)
    }

    pub fn pods_visible(&self) -> impl Iterator<Item = &SquadPod> {
        self.pods.iter().filter(|p| !p.landed && p.start_delay <= 0.0)
    }
}

// ── Supply Crate ───────────────────────────────────────────────────────────

/// Supply drop crate — stratagem call-in (Helldivers 2 style).
#[derive(Debug, Clone)]
pub struct SupplyCrate {
    pub position: Vec3,
    pub lifetime: f32,
    pub used: bool,
}

// ── Game Messages ──────────────────────────────────────────────────────────

/// On-screen message (Minecraft-style chat/event log).
pub struct GameMessage {
    pub text: String,
    pub color: [f32; 4],
    pub time_remaining: f32,
}

/// Manages the on-screen message log displayed over the game view.
pub struct GameMessages {
    pub messages: Vec<GameMessage>,
    pub max_visible: usize,
    default_duration: f32,
}

impl GameMessages {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            max_visible: 12,
            default_duration: 6.0,
        }
    }

    pub fn push(&mut self, text: impl Into<String>, color: [f32; 4]) {
        self.messages.push(GameMessage {
            text: text.into(),
            color,
            time_remaining: self.default_duration,
        });
        if self.messages.len() > 50 {
            self.messages.remove(0);
        }
    }

    pub fn info(&mut self, text: impl Into<String>) {
        self.push(text, [1.0, 1.0, 1.0, 1.0]);
    }

    pub fn success(&mut self, text: impl Into<String>) {
        self.push(text, [0.3, 1.0, 0.3, 1.0]);
    }

    pub fn warning(&mut self, text: impl Into<String>) {
        self.push(text, [1.0, 0.9, 0.3, 1.0]);
    }

    pub fn update(&mut self, dt: f32) {
        for msg in &mut self.messages {
            msg.time_remaining -= dt;
        }
        self.messages.retain(|m| m.time_remaining > 0.0);
    }
}
