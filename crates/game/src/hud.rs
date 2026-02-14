//! HUD (Heads-Up Display) system for FPS gameplay
//! Renders health, ammo, crosshair, damage indicators, etc.

use crate::fps::{CombatSystem, FPSPlayer, MissionState};
use crate::spawner::ThreatLevel;
use crate::weapons::WeaponType;
use glam::Vec3;

/// HUD configuration
#[derive(Debug, Clone)]
pub struct HUDConfig {
    pub show_crosshair: bool,
    pub show_health: bool,
    pub show_ammo: bool,
    pub show_minimap: bool,
    pub show_objective: bool,
    pub show_damage_indicators: bool,
    pub show_hit_markers: bool,
    pub show_kill_feed: bool,
    pub crosshair_style: CrosshairStyle,
    pub crosshair_color: [f32; 4],
    pub crosshair_size: f32,
    pub hud_scale: f32,
}

impl Default for HUDConfig {
    fn default() -> Self {
        Self {
            show_crosshair: true,
            show_health: true,
            show_ammo: true,
            show_minimap: true,
            show_objective: true,
            show_damage_indicators: true,
            show_hit_markers: true,
            show_kill_feed: true,
            crosshair_style: CrosshairStyle::Dynamic,
            crosshair_color: [1.0, 1.0, 1.0, 0.8],
            crosshair_size: 1.0,
            hud_scale: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrosshairStyle {
    /// Simple dot
    Dot,
    /// Classic cross
    Cross,
    /// Circle with cross
    CircleCross,
    /// Dynamic - changes based on movement/shooting
    Dynamic,
    /// No crosshair (iron sights only)
    None,
}

/// All HUD data for a frame
#[derive(Debug, Clone)]
pub struct HUDData {
    // Player stats
    pub health: f32,
    pub max_health: f32,
    pub armor: f32,
    pub max_armor: f32,
    pub stamina: f32,
    pub max_stamina: f32,

    // Weapon info
    pub weapon_name: String,
    pub weapon_icon: WeaponType,
    pub current_ammo: u32,
    pub reserve_ammo: u32,
    pub is_reloading: bool,
    pub reload_progress: f32,

    // Ability
    pub ability_name: String,
    pub ability_ready: bool,
    pub ability_cooldown_percent: f32,

    // Crosshair
    pub crosshair_spread: f32,
    pub is_aiming: bool,
    pub aim_progress: f32,

    // Combat feedback
    pub damage_direction: Option<f32>, // Angle in radians from forward
    pub recent_damage: f32,
    pub hit_marker: Option<HitMarkerData>,
    pub damage_numbers: Vec<DamageNumberData>,

    // Kill feed
    pub kill_feed: Vec<KillFeedData>,

    // Horde status
    pub bugs_alive: u32,
    pub kills: u32,
    pub time_survived: String,
    pub threat_level: ThreatLevel,
    pub peak_bugs: u32,

    // Status
    pub is_alive: bool,
    pub respawn_time: f32,
    pub fps: f32,
}

#[derive(Debug, Clone)]
pub struct HitMarkerData {
    pub is_kill: bool,
    pub is_headshot: bool,
    pub alpha: f32,
}

#[derive(Debug, Clone)]
pub struct DamageNumberData {
    pub screen_x: f32,
    pub screen_y: f32,
    pub damage: f32,
    pub is_critical: bool,
    pub alpha: f32,
}

#[derive(Debug, Clone)]
pub struct KillFeedData {
    pub killer: String,
    pub victim: String,
    pub weapon: String,
    pub is_headshot: bool,
    pub alpha: f32,
}

/// HUD system that generates display data
pub struct HUDSystem {
    pub config: HUDConfig,
}

impl HUDSystem {
    pub fn new() -> Self {
        Self {
            config: HUDConfig::default(),
        }
    }

    /// Generate HUD data from game state
    pub fn generate_hud_data(
        &self,
        player: &FPSPlayer,
        combat: &CombatSystem,
        mission: &MissionState,
        spawner: &crate::spawner::BugSpawner,
        bugs_alive: usize,
        fps: f32,
    ) -> HUDData {
        let (weapon_name, weapon_icon, current_ammo, reserve_ammo, is_reloading, reload_progress, base_spread) =
            if player.is_shovel_equipped() {
                (
                    "Entrenching Shovel".to_string(),
                    WeaponType::Rifle, // fallback icon
                    0,
                    0,
                    false,
                    1.0,
                    0.0,
                )
            } else {
                let weapon = player.current_weapon();
                let rp = if weapon.is_reloading {
                    1.0 - (weapon.reload_timer / weapon.reload_time)
                } else {
                    1.0
                };
                (
                    format!("{:?}", weapon.weapon_type),
                    weapon.weapon_type,
                    weapon.current_ammo,
                    weapon.reserve_ammo,
                    weapon.is_reloading,
                    rp,
                    // Bipod: machine gun gets 0.25x spread when prone
                    if player.is_prone && weapon.weapon_type == WeaponType::MachineGun {
                        weapon.spread * 0.25
                    } else {
                        weapon.spread
                    },
                )
            };

        // Calculate damage direction angle
        let damage_direction = player.damage_direction.map(|dir| {
            let forward = player.look_direction;
            let right = Vec3::Y.cross(forward).normalize();
            let forward_dot = forward.dot(dir);
            let right_dot = right.dot(dir);
            right_dot.atan2(-forward_dot)
        });

        // Calculate crosshair spread
        let movement_spread = if player.is_sprinting { 5.0 } else { 0.0 };
        let aim_reduction = player.aim_progress * 0.7;
        let crosshair_spread = (base_spread + movement_spread) * (1.0 - aim_reduction);

        // Hit marker
        let hit_marker = combat.latest_hit_marker().map(|hm| HitMarkerData {
            is_kill: hm.is_kill,
            is_headshot: hm.is_headshot,
            alpha: hm.lifetime / 0.3,
        });

        // Damage numbers (would need screen projection in real impl)
        let damage_numbers = combat
            .damage_numbers
            .iter()
            .map(|dn| DamageNumberData {
                screen_x: 0.5, // Would project to screen
                screen_y: 0.5,
                damage: dn.damage,
                is_critical: dn.is_critical,
                alpha: dn.lifetime,
            })
            .collect();

        // Kill feed
        let kill_feed = combat
            .kill_feed
            .iter()
            .map(|kf| KillFeedData {
                killer: kf.killer.clone(),
                victim: kf.victim.clone(),
                weapon: format!("{:?}", kf.weapon),
                is_headshot: kf.was_headshot,
                alpha: (kf.lifetime / 5.0).min(1.0),
            })
            .collect();

        HUDData {
            health: player.health,
            max_health: player.max_health,
            armor: player.armor,
            max_armor: player.max_armor,
            stamina: player.stamina,
            max_stamina: player.max_stamina,

            weapon_name,
            weapon_icon,
            current_ammo,
            reserve_ammo,
            is_reloading,
            reload_progress,

            ability_name: format!("{:?}", player.ability),
            ability_ready: player.can_use_ability(),
            ability_cooldown_percent: player.ability_ready_percent(),

            crosshair_spread,
            is_aiming: player.is_aiming,
            aim_progress: player.aim_progress,

            damage_direction,
            recent_damage: if player.last_damage_time < 0.5 {
                1.0 - player.last_damage_time * 2.0
            } else {
                0.0
            },
            hit_marker,
            damage_numbers,

            kill_feed,

            bugs_alive: bugs_alive as u32,
            kills: player.kills,
            time_survived: spawner.time_survived_str(),
            threat_level: spawner.threat_level,
            peak_bugs: mission.peak_bugs_alive,

            is_alive: player.is_alive,
            respawn_time: player.respawn_timer,
            fps,
        }
    }

    /// Generate ASCII representation of HUD for console output
    pub fn render_console_hud(&self, data: &HUDData) -> String {
        let mut output = String::new();

        // Top bar - Horde status
        output.push_str(&format!(
            "╔══════════════════════════════════════════════════════════════════════════╗\n"
        ));
        output.push_str(&format!(
            "║  THREAT: {:<10}  │  BUGS: {:3}  │  KILLS: {:4}  │  TIME: {}  │  FPS: {:4.0}  ║\n",
            data.threat_level.name(),
            data.bugs_alive,
            data.kills,
            data.time_survived,
            data.fps
        ));
        output.push_str(&format!(
            "╚══════════════════════════════════════════════════════════════════════════╝\n"
        ));

        output.push('\n');

        // Crosshair area (center of screen representation)
        if !data.is_alive {
            output.push_str(&format!(
                "\n                              YOU ARE DEAD\n"
            ));
            output.push_str(&format!(
                "                         Respawning in {:.1}s\n\n",
                data.respawn_time
            ));
        } else {
            // Hit marker
            if let Some(hm) = &data.hit_marker {
                let marker = if hm.is_kill {
                    if hm.is_headshot { ">>> HEADSHOT KILL <<<" } else { ">>> KILL <<<" }
                } else if hm.is_headshot {
                    ">> HEADSHOT <<"
                } else {
                    "× HIT ×"
                };
                output.push_str(&format!("                              {}\n", marker));
            }

            // Damage indicator
            if data.recent_damage > 0.0 {
                if let Some(angle) = data.damage_direction {
                    let direction = if angle.abs() < 0.5 {
                        "▼ FRONT ▼"
                    } else if angle > 0.0 {
                        "◄ LEFT ◄"
                    } else {
                        "► RIGHT ►"
                    };
                    output.push_str(&format!("                         !! {} !!\n", direction));
                }
            }
        }

        output.push('\n');

        // Bottom HUD
        output.push_str(&format!(
            "╔══════════════════════════════════════════════════════════════════════════╗\n"
        ));

        // Health and Armor
        let health_bar = self.health_bar(data.health / data.max_health, 15);
        let armor_bar = self.armor_bar(data.armor / data.max_armor.max(1.0), 10);
        let stamina_bar = self.stamina_bar(data.stamina / data.max_stamina, 10);

        output.push_str(&format!(
            "║  HP [{:>3.0}] {}  │  ARMOR {}  │  STAMINA {}  ",
            data.health, health_bar, armor_bar, stamina_bar
        ));

        // Ammo
        let ammo_str = if data.is_reloading {
            format!("RELOADING {}", self.progress_bar(data.reload_progress, 8))
        } else {
            format!("{:>3}/{:<4}", data.current_ammo, data.reserve_ammo)
        };
        output.push_str(&format!("│  {}  ║\n", ammo_str));

        // Weapon and Ability
        let ability_status = if data.ability_ready {
            "[READY]".to_string()
        } else {
            format!("[{:.0}%]", data.ability_cooldown_percent * 100.0)
        };

        output.push_str(&format!(
            "║  WEAPON: {:<12}  │  ABILITY: {:<15} {}               ║\n",
            data.weapon_name,
            data.ability_name,
            ability_status
        ));

        output.push_str(&format!(
            "╚══════════════════════════════════════════════════════════════════════════╝\n"
        ));

        // Kill feed (right side, last 3)
        if !data.kill_feed.is_empty() {
            output.push_str("\n  KILL FEED:\n");
            for kf in data.kill_feed.iter().take(5) {
                let hs = if kf.is_headshot { " [HS]" } else { "" };
                output.push_str(&format!(
                    "    {} killed {} with {}{}\n",
                    kf.killer, kf.victim, kf.weapon, hs
                ));
            }
        }

        output
    }

    fn health_bar(&self, percent: f32, width: usize) -> String {
        let filled = (percent * width as f32) as usize;
        let empty = width - filled;

        let color = if percent > 0.6 {
            "█" // Green would be here with ANSI
        } else if percent > 0.3 {
            "▓" // Yellow
        } else {
            "░" // Red
        };

        format!("{}{}", color.repeat(filled), "░".repeat(empty))
    }

    fn armor_bar(&self, percent: f32, width: usize) -> String {
        let filled = (percent * width as f32) as usize;
        let empty = width - filled;
        format!("{}{}", "▓".repeat(filled), "░".repeat(empty))
    }

    fn stamina_bar(&self, percent: f32, width: usize) -> String {
        let filled = (percent * width as f32) as usize;
        let empty = width - filled;
        format!("{}{}", "▒".repeat(filled), "░".repeat(empty))
    }

    fn progress_bar(&self, percent: f32, width: usize) -> String {
        let filled = (percent.clamp(0.0, 1.0) * width as f32) as usize;
        let empty = width - filled;
        format!("{}{}", "=".repeat(filled), "-".repeat(empty))
    }
}

impl Default for HUDSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Crosshair rendering data for GPU
#[derive(Debug, Clone, Copy)]
pub struct CrosshairRenderData {
    pub center_x: f32,
    pub center_y: f32,
    pub gap: f32,
    pub length: f32,
    pub thickness: f32,
    pub color: [f32; 4],
    pub dot_size: f32,
    pub style: CrosshairStyle,
    pub hit_marker_alpha: f32,
    pub hit_marker_is_kill: bool,
}

impl CrosshairRenderData {
    pub fn from_hud_data(data: &HUDData, config: &HUDConfig, screen_width: f32, screen_height: f32) -> Self {
        let center_x = screen_width / 2.0;
        let center_y = screen_height / 2.0;

        // Dynamic gap based on spread and ADS
        let base_gap = 4.0 * config.crosshair_size;
        let spread_gap = data.crosshair_spread * 2.0;
        let aim_reduction = data.aim_progress * 0.8;
        let gap = (base_gap + spread_gap) * (1.0 - aim_reduction);

        let (hit_marker_alpha, hit_marker_is_kill) = if let Some(hm) = &data.hit_marker {
            (hm.alpha, hm.is_kill)
        } else {
            (0.0, false)
        };

        Self {
            center_x,
            center_y,
            gap,
            length: 8.0 * config.crosshair_size,
            thickness: 2.0 * config.crosshair_size,
            color: config.crosshair_color,
            dot_size: 2.0 * config.crosshair_size,
            style: config.crosshair_style,
            hit_marker_alpha,
            hit_marker_is_kill,
        }
    }
}
