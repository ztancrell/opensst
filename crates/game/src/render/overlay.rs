//! Overlay rendering: HUD, debug info, game messages, war table UI.

use engine_core::{Health, Transform};
use glam::Vec3;
use procgen::StarType;
use renderer::OverlayTextBuilder;

use crate::earth_territory;
use crate::extraction::{self, ExtractionPhase};
use crate::roger_young_interior_npcs;
use crate::squad::SquadMate;
use crate::{DropPhase, GameMessage, GamePhase, GameState};

/// Build the screen-space overlay (debug info, HUD, game messages, war table, etc.).
pub fn build(state: &GameState, sw: f32, sh: f32) -> OverlayTextBuilder {
    let mut tb = OverlayTextBuilder::new(sw, sh);
    let scale = 2.0; // 2x scale for readability
    let line_h = 8.0 * scale + 4.0; // glyph height * scale + padding
    let bg = [0.0, 0.0, 0.0, 0.55]; // semi-transparent dark bg
    let white = [1.0, 1.0, 1.0, 1.0];
    let gray = [0.7, 0.7, 0.7, 1.0];
    let yellow = [1.0, 0.9, 0.3, 1.0];
    // STE-style tactical: green #00ff00, amber #ffaa00 (ART_DIRECTION)
    let tactical_green = [0.0, 1.0, 0.0, 1.0];
    let tactical_amber = [1.0, 0.67, 0.0, 1.0];

    // ---- Main menu: minimal (dark background + title + Play/Quit only) ----
    if state.phase == GamePhase::MainMenu {
        let title = "OpenSST";
        let title_scale = 1.8;
        let title_w = title.len() as f32 * 8.0 * title_scale;
        tb.add_text(sw * 0.5 - title_w * 0.5, sh * 0.35, title, title_scale, [0.9, 0.88, 0.75, 1.0]);

        let play_sel = state.main_menu_selected == 0;
        let quit_sel = state.main_menu_selected == 1;
        let menu_y = sh * 0.55;
        let menu_x = sw * 0.5 - 36.0;
        let item_h = 24.0;
        let item_scale = 1.4;
        let sel = [0.95, 0.9, 0.7, 1.0];
        let unsel = [0.6, 0.62, 0.68, 1.0];

        tb.add_text(menu_x, menu_y, "Play", item_scale, if play_sel { sel } else { unsel });
        tb.add_text(menu_x, menu_y + item_h, "Quit", item_scale, if quit_sel { sel } else { unsel });

        return tb;
    }

    // ---- Pause menu: full-screen dark overlay ----
    if state.phase == GamePhase::Paused {
        tb.add_rect(0.0, 0.0, sw, sh, [0.08, 0.08, 0.08, 1.0]); // Dark grey background

        let title = "PAUSED";
        let title_scale = 2.0;
        let title_w = title.len() as f32 * 8.0 * title_scale;
        tb.add_text(sw * 0.5 - title_w * 0.5, sh * 0.3, title, title_scale, [0.9, 0.88, 0.75, 1.0]);

        let resume_sel = state.pause_menu_selected == 0;
        let quit_sel = state.pause_menu_selected == 1;
        let menu_y = sh * 0.5;
        let menu_x = sw * 0.5 - 90.0;
        let item_h = 28.0;
        let item_scale = 1.5;
        let sel = [0.95, 0.9, 0.7, 1.0];
        let unsel = [0.6, 0.62, 0.68, 1.0];

        tb.add_text(menu_x, menu_y, "Resume", item_scale, if resume_sel { sel } else { unsel });
        tb.add_text(menu_x, menu_y + item_h, "Quit to main menu", item_scale, if quit_sel { sel } else { unsel });
        tb.add_text(sw * 0.5 - 120.0, menu_y + item_h * 2.5, "Escape / Enter to select", 1.0, gray);

        return tb;
    }

    let warp_active = state.warp_sequence.is_some();
    let approach_in_space = state.phase == GamePhase::ApproachPlanet && state.approach_flight_state.is_some();
    let ship_interior_visible = warp_active
        || ((state.phase == GamePhase::InShip || state.phase == GamePhase::ApproachPlanet)
            && !approach_in_space);

    // ---- Top-left: debug info (F3-style) — can be hidden via debug menu "Show Debug Overlay" ----
    let x = 4.0;
    if state.debug.show_debug_overlay {
        let mut y = 4.0;

        let fps_text = format!("FPS: {:.0}", state.time.fps());
        tb.add_text_with_bg(x, y, &fps_text, scale, tactical_green, bg);
        y += line_h;

        let pos = state.camera.position();
        let alt = pos.y.max(0.0);
        let zone = if alt > 450.0 { "SPACE" }
            else if alt > 100.0 { "Upper Atmo" }
            else if alt > 30.0 { "Low Atmo" }
            else { "Surface" };
        let pos_text = format!("XYZ: {:.1}/{:.1}/{:.1}  Alt: {:.0}m [{}]", pos.x, pos.y, pos.z, alt, zone);
        tb.add_text_with_bg(x, y, &pos_text, scale, white, bg);
        y += line_h;

        // System info
        let system_text = format!(
            "System: {} | Star: {} ({:?})",
            state.current_system.name,
            state.current_system.star.name,
            state.current_system.star.star_type,
        );
        tb.add_text_with_bg(x, y, &system_text, scale, [0.6, 0.8, 1.0, 1.0], bg);
        y += line_h;

        // Planet info
        let planet_text = if let Some(idx) = state.current_planet_idx {
            format!("Planet: {} [{}/{}] | {:?}", state.planet.name, idx + 1, state.current_system.bodies.len(), state.planet.primary_biome)
        } else {
            format!("In Space | {} planets in system", state.current_system.bodies.len())
        };
        tb.add_text_with_bg(x, y, &planet_text, scale, [0.8, 1.0, 0.6, 1.0], bg);
        y += line_h;

        // Earth territory: show current place (city, town, farm)
        if state.planet.name == "Earth" && state.settlement_center.is_some() {
            if let Some(place_name) = earth_territory::place_name_at(state.player.position.x, state.player.position.z) {
                tb.add_text_with_bg(x, y, &format!("Location: {}", place_name), scale, [0.4, 0.85, 0.6, 1.0], bg);
                y += line_h;
            }
        }

        if state.current_planet_idx.is_some() {
            let chunks_text = format!("Chunks: {}", state.chunk_manager.chunks.len());
            tb.add_text_with_bg(x, y, &chunks_text, scale, gray, bg);
            y += line_h;

            let bugs_alive = state.count_living_bugs();
            let (threat_name, threat_color) = if state.planet.name == "Earth" {
                ("Safe zone", [0.2, 0.7, 0.4, 1.0])
            } else {
                (state.spawner.threat_level.name(), state.spawner.threat_level.color())
            };
            let bugs_text = format!(
                "Bugs: {}  Kills: {}  Time: {}  Threat: {}",
                bugs_alive,
                state.mission.bugs_killed,
                state.spawner.time_survived_str(),
                threat_name,
            );
            tb.add_text_with_bg(x, y, &bugs_text, scale, threat_color, bg);
            y += line_h;

            let tod_name = if state.time_of_day < 0.125 { "Dawn" }
                else if state.time_of_day < 0.375 { "Day" }
                else if state.time_of_day < 0.625 { "Dusk" }
                else { "Night" };
            let weather_name = format!("{:?}", state.weather.current);
            let blend_info = if state.weather.current != state.weather.target {
                format!(" -> {:?} ({:.0}%)", state.weather.target, state.weather.blend * 100.0)
            } else {
                String::new()
            };
            let weather_text = format!("Weather: {}{} | {} ({:.2})", weather_name, blend_info, tod_name, state.time_of_day);
            tb.add_text_with_bg(x, y, &weather_text, scale, gray, bg);
            y += line_h;
        }

        // Player controller info (when on planet in FPS mode)
        if state.current_planet_idx.is_some() && !state.debug.noclip && state.debug.show_perf_stats {
            let speed = Vec3::new(state.player_velocity.x, 0.0, state.player_velocity.z).length();
            let ground_str = if state.player_grounded { "GROUNDED" } else { "AIRBORNE" };
            let ctrl_info = format!(
                "Speed: {:.1} m/s | {} | HP: {:.0}/{:.0} | Stamina: {:.0}%",
                speed, ground_str, state.player.health, state.player.max_health, state.player.stamina_percent() * 100.0
            );
            tb.add_text_with_bg(x, y, &ctrl_info, scale, [0.6, 0.9, 0.6, 1.0], bg);
            y += line_h;
        }

        // Controls hint
        let controls_text = if state.debug.noclip {
            if state.current_planet_idx.is_some() {
                "NOCLIP | R=next planet | M=galaxy map | F3=debug"
            } else {
                "NOCLIP | Approach planet to land | M=galaxy map | F3=debug"
            }
        } else if state.current_planet_idx.is_some() {
            "WASD=walk | LMB=shoot | RMB=aim | R=reload | 1/2=weapon | F3=debug"
        } else {
            "Approach planet to land | M=galaxy map | F3=debug"
        };
        tb.add_text_with_bg(x, y, controls_text, scale, tactical_amber, bg);
    }

    // ---- Galaxy map overlay (when M is pressed) ----
    if state.galaxy_map_open {
        tb.add_rect(sw * 0.1, sh * 0.1, sw * 0.8, sh * 0.8, [0.0, 0.0, 0.05, 0.85]);

        let title = format!("GALAXY MAP - {} systems | Viewing: {}", state.universe.systems.len(), state.current_system.name);
        tb.add_text(sw * 0.12, sh * 0.12, &title, scale, [0.6, 0.8, 1.0, 1.0]);

        let map_cx = sw * 0.5;
        let map_cy = sh * 0.5;
        let map_scale = sh * 0.3 / 1000.0;

        let current_pos = state.universe.systems[state.current_system_idx].position;

        for (i, entry) in state.universe.systems.iter().enumerate() {
            let rel = entry.position - current_pos;
            let sx = map_cx + rel.x as f32 * map_scale;
            let sy = map_cy - rel.z as f32 * map_scale;

            if sx < sw * 0.1 || sx > sw * 0.9 || sy < sh * 0.1 || sy > sh * 0.9 {
                continue;
            }

            let star_color = match entry.star_type {
                StarType::RedDwarf => [1.0, 0.3, 0.2, 0.9],
                StarType::YellowMain => [1.0, 0.95, 0.5, 0.9],
                StarType::BlueGiant => [0.5, 0.6, 1.0, 0.9],
                StarType::WhiteDwarf => [0.9, 0.9, 1.0, 0.8],
                StarType::BinaryStar => [1.0, 0.8, 0.4, 0.9],
            };

            let dot_size = if i == state.current_system_idx { 8.0 } else if i == state.galaxy_map_selected { 6.0 } else { 4.0 };

            if i == state.current_system_idx {
                tb.add_rect(sx - dot_size, sy - dot_size, dot_size * 2.0, dot_size * 2.0, [0.0, 1.0, 0.0, 0.4]);
            }
            if i == state.galaxy_map_selected {
                tb.add_rect(sx - dot_size - 1.0, sy - dot_size - 1.0, dot_size * 2.0 + 2.0, dot_size * 2.0 + 2.0, [1.0, 1.0, 0.0, 0.5]);
            }

            tb.add_rect(sx - dot_size * 0.5, sy - dot_size * 0.5, dot_size, dot_size, star_color);

            if i == state.current_system_idx || i == state.galaxy_map_selected || entry.visited {
                tb.add_text(sx + dot_size, sy - 4.0, &entry.name, 1.5, star_color);
            }
        }

        let selected = &state.universe.systems[state.galaxy_map_selected];
        let sel_info = format!(
            "Selected: {} ({:?}) | {}",
            selected.name,
            selected.star_type,
            if selected.visited { "VISITED" } else { "UNCHARTED" }
        );
        tb.add_text_with_bg(sw * 0.12, sh * 0.85, &sel_info, scale, yellow, bg);

        let help_text = "Arrow keys/scroll = select | Enter = warp | M = close";
        tb.add_text_with_bg(sw * 0.12, sh * 0.85 + line_h, help_text, scale, gray, bg);
    }

    // ---- Debug menu overlay (F3) ----
    if state.debug.menu_open {
        let menu_w = 380.0;
        let menu_x = sw - menu_w - 10.0;
        let menu_y = 10.0;
        let items = state.debug.menu_items();
        let item_h = line_h + 2.0;
        let header_h = 30.0;
        let menu_h = header_h + items.len() as f32 * item_h + 10.0;

        tb.add_rect(menu_x, menu_y, menu_w, menu_h, [0.02, 0.02, 0.05, 0.9]);
        tb.add_rect(menu_x, menu_y, menu_w, 2.0, [0.3, 0.5, 1.0, 0.8]);
        tb.add_rect(menu_x, menu_y + menu_h - 2.0, menu_w, 2.0, [0.3, 0.5, 1.0, 0.8]);
        tb.add_rect(menu_x, menu_y, 2.0, menu_h, [0.3, 0.5, 1.0, 0.8]);
        tb.add_rect(menu_x + menu_w - 2.0, menu_y, 2.0, menu_h, [0.3, 0.5, 1.0, 0.8]);

        tb.add_text(menu_x + 10.0, menu_y + 6.0, "DEBUG MENU [F3]", 2.5, [0.5, 0.8, 1.0, 1.0]);

        for (i, (name, enabled)) in items.iter().enumerate() {
            let iy = menu_y + header_h + i as f32 * item_h;
            let is_selected = i == state.debug.selected;

            if is_selected {
                tb.add_rect(menu_x + 4.0, iy, menu_w - 8.0, item_h - 2.0, [0.15, 0.25, 0.5, 0.7]);
            }

            if name.starts_with("--") {
                let action_color = if is_selected {
                    [1.0, 0.8, 0.3, 1.0]
                } else {
                    [0.7, 0.6, 0.3, 0.8]
                };
                tb.add_text(menu_x + 15.0, iy + 2.0, name, scale, action_color);
            } else {
                let name_color = if is_selected { white } else { gray };
                tb.add_text(menu_x + 15.0, iy + 2.0, name, scale, name_color);

                let (status_text, status_color) = if *enabled {
                    ("ON", [0.3, 1.0, 0.3, 1.0])
                } else {
                    ("OFF", [0.6, 0.4, 0.4, 0.8])
                };
                tb.add_text(menu_x + menu_w - 55.0, iy + 2.0, status_text, scale, status_color);
            }
        }

        let footer_y = menu_y + menu_h + 4.0;
        tb.add_text(menu_x, footer_y, "Up/Down=select  Enter=toggle", 1.5, [0.4, 0.4, 0.5, 0.7]);

        let mode_text = if state.debug.noclip { "Mode: NOCLIP" } else { "Mode: FPS" };
        let mode_color = if state.debug.noclip { [1.0, 0.7, 0.3, 1.0] } else { [0.3, 1.0, 0.5, 1.0] };
        tb.add_text(menu_x, footer_y + 14.0, mode_text, 1.5, mode_color);

        let ts_text = format!("Time Scale: {:.2}x", state.debug.time_scale);
        tb.add_text(menu_x + 150.0, footer_y + 14.0, &ts_text, 1.5, [0.6, 0.6, 0.8, 0.8]);
    }

    // ---- Ship interior HUD (InShip / Approach in bay / Warp) ----
    if ship_interior_visible {
        if warp_active {
            let header_text = "QUANTUM TRAVEL — Bridge view";
            let header_w = header_text.len() as f32 * 6.0 * 1.5;
            tb.add_rect(sw * 0.5 - header_w * 0.5 - 6.0, 4.0, header_w + 12.0, 22.0, [0.02, 0.03, 0.06, 0.7]);
            tb.add_text(sw * 0.5 - header_w * 0.5, 8.0, header_text, 1.5, [0.4, 0.6, 1.0, 0.9]);
            let cx = sw * 0.5;
            let cy = sh * 0.5;
            tb.add_rect(cx - 1.0, cy - 8.0, 2.0, 6.0, [0.5, 0.7, 1.0, 0.5]);
            tb.add_rect(cx - 1.0, cy + 2.0, 2.0, 6.0, [0.5, 0.7, 1.0, 0.5]);
            tb.add_rect(cx - 8.0, cy - 1.0, 6.0, 2.0, [0.5, 0.7, 1.0, 0.5]);
            tb.add_rect(cx + 2.0, cy - 1.0, 6.0, 2.0, [0.5, 0.7, 1.0, 0.5]);
        } else {
            let timer = state.ship_state.as_ref().map_or(0.0, |s| s.timer);
            let war_table_active = state.ship_state.as_ref().map_or(false, |s| s.war_table_active);
            let war_table_pos = state.ship_state.as_ref().map_or(Vec3::ZERO, |s| s.war_table_pos);
            let drop_bay_pos = state.ship_state.as_ref().map_or(Vec3::ZERO, |s| s.drop_bay_pos);
            let player_pos = state.camera.transform.position;

            let header_text = format!("FNS ROGER YOUNG — {} System", state.current_system.name);
            let header_w = header_text.len() as f32 * 6.0 * 1.5;
            tb.add_rect(sw * 0.5 - header_w * 0.5 - 6.0, 4.0, header_w + 12.0, 22.0, [0.02, 0.03, 0.06, 0.7]);
            tb.add_text(sw * 0.5 - header_w * 0.5, 8.0, &header_text, 1.5, [0.3, 0.5, 0.8, 0.9]);

            let cx = sw * 0.5;
            let cy = sh * 0.5;
            tb.add_rect(cx - 1.0, cy - 8.0, 2.0, 6.0, [0.5, 0.7, 1.0, 0.5]);
            tb.add_rect(cx - 1.0, cy + 2.0, 2.0, 6.0, [0.5, 0.7, 1.0, 0.5]);
            tb.add_rect(cx - 8.0, cy - 1.0, 6.0, 2.0, [0.5, 0.7, 1.0, 0.5]);
            tb.add_rect(cx + 2.0, cy - 1.0, 6.0, 2.0, [0.5, 0.7, 1.0, 0.5]);

            // NPC nametags
            const NAMETAG_MAX_DIST: f32 = 12.0;
            const NAMETAG_MIN_DOT: f32 = 0.4;
            let cam_pos = state.camera.position();
            let cam_fwd = state.camera.forward();
            let view_proj = state.camera.view_projection_matrix();
            for npc in roger_young_interior_npcs() {
                let head_pos = npc.position + Vec3::Y * 1.6;
                let to_npc = head_pos - cam_pos;
                let dist = to_npc.length();
                if dist > NAMETAG_MAX_DIST || dist < 0.1 {
                    continue;
                }
                let dir = to_npc / dist;
                if cam_fwd.dot(dir) < NAMETAG_MIN_DOT {
                    continue;
                }
                let clip = view_proj * glam::Vec4::new(head_pos.x, head_pos.y, head_pos.z, 1.0);
                if clip.w <= 0.01 {
                    continue;
                }
                let ndc_z = clip.z / clip.w;
                if ndc_z > 1.0 {
                    continue;
                }
                let sx = (clip.x / clip.w + 1.0) * 0.5 * sw;
                let sy = (1.0 - clip.y / clip.w) * 0.5 * sh;
                let name = npc.name;
                let scale = 1.5;
                let tw = name.len() as f32 * 6.0 * scale * 0.5;
                tb.add_text_with_bg(sx - tw, sy - 24.0, name, scale, [1.0, 1.0, 1.0, 0.95], [0.0, 0.0, 0.0, 0.6]);
            }

            if state.phase == GamePhase::ApproachPlanet {
                let msg = "APPROACHING PLANET — SPACE to launch drop pod";
                let mw = msg.len() as f32 * 6.0 * 1.8;
                tb.add_rect(sw * 0.5 - mw * 0.5 - 8.0, sh - 50.0, mw + 16.0, 28.0, [0.02, 0.05, 0.12, 0.8]);
                tb.add_text(sw * 0.5 - mw * 0.5, sh - 42.0, msg, 1.8, [0.4, 0.7, 1.0, 1.0]);
            } else {
                let dist_to_table = Vec3::new(
                    player_pos.x - war_table_pos.x, 0.0,
                    player_pos.z - war_table_pos.z,
                ).length();
                let dist_to_bay = Vec3::new(
                    player_pos.x - drop_bay_pos.x, 0.0,
                    player_pos.z - drop_bay_pos.z,
                ).length();


                if war_table_active {
                    tb.add_rect(sw * 0.05, sh * 0.05, sw * 0.9, sh * 0.9, [0.02, 0.03, 0.06, 0.85]);
                    let accent = [0.15, 0.25, 0.5, 0.6];
                    let bx = sw * 0.05;
                    let by = sh * 0.05;
                    let bw = sw * 0.9;
                    let bh = sh * 0.9;
                    tb.add_rect(bx, by, bw, 2.0, accent);
                    tb.add_rect(bx, by + bh - 2.0, bw, 2.0, accent);
                    tb.add_rect(bx, by, 2.0, bh, accent);
                    tb.add_rect(bx + bw - 2.0, by, 2.0, bh, accent);

                    let title = "GALACTIC WAR TABLE";
                    let title_w = title.len() as f32 * 6.0 * 2.5;
                    tb.add_text(sw * 0.5 - title_w * 0.5, by + 12.0, title, 2.5, [0.4, 0.65, 1.0, 1.0]);

                    let num_sys = state.universe.systems.len();
                    let sys_text = format!("SYSTEM: {}  ({}/{} — ↑/↓ or W/Q change)", state.current_system.name, state.current_system_idx + 1, num_sys);
                    let sys_w = sys_text.len() as f32 * 6.0 * 1.2;
                    tb.add_text(sw * 0.5 - sys_w * 0.5, by + 34.0, &sys_text, 1.2, [0.5, 0.7, 0.9, 1.0]);

                    let total_lib: f32 = state.war_state.planets.iter().map(|p| p.liberation).sum();
                    let avg_lib = if state.war_state.planets.is_empty() { 0.0 } else { total_lib / state.war_state.planets.len() as f32 };
                    let lib_text = format!("SECTOR LIBERATION: {:.0}%", avg_lib * 100.0);
                    let lib_tw = lib_text.len() as f32 * 6.0 * 1.5;
                    tb.add_text(sw * 0.5 - lib_tw * 0.5, by + 42.0, &lib_text, 1.5, [0.5, 0.7, 1.0, 1.0]);
                    let bar_x = sw * 0.25;
                    let bar_w = sw * 0.5;
                    tb.add_rect(bar_x, by + 60.0, bar_w, 6.0, [0.1, 0.1, 0.15, 1.0]);
                    let fill_col = if avg_lib > 0.7 { [0.2, 0.8, 0.3, 1.0] }
                        else if avg_lib > 0.3 { [0.8, 0.7, 0.2, 1.0] }
                        else { [0.8, 0.2, 0.2, 1.0] };
                    tb.add_rect(bar_x, by + 60.0, bar_w * avg_lib, 6.0, fill_col);

                    if let Some(order) = state.war_state.major_orders.iter().find(|o| !o.completed) {
                        let mo_y = by + 74.0;
                        tb.add_text(bx + 20.0, mo_y, "MAJOR ORDER", 1.2, [1.0, 0.85, 0.3, 1.0]);
                        tb.add_text(bx + 20.0, mo_y + 16.0, &order.title, 1.1, [0.9, 0.9, 0.95, 1.0]);
                        let trunc = 60.min(order.description.len());
                        let desc: String = if order.description.len() > 60 { format!("{}…", &order.description[..trunc]) } else { order.description.clone() };
                        tb.add_text(bx + 20.0, mo_y + 32.0, &desc, 0.95, [0.6, 0.65, 0.7, 1.0]);
                        let prog_w = bw - 40.0;
                        tb.add_rect(bx + 20.0, mo_y + 50.0, prog_w, 5.0, [0.1, 0.1, 0.15, 1.0]);
                        let prog_fill = (order.progress * prog_w).min(prog_w);
                        tb.add_rect(bx + 20.0, mo_y + 50.0, prog_fill, 5.0, [0.2, 0.6, 0.9, 1.0]);
                        tb.add_text(bx + 20.0, mo_y + 58.0, &format!("Reward: {}", order.reward), 0.9, [0.5, 0.8, 0.5, 0.9]);
                    }

                    let num_planets = state.current_system.bodies.len();
                    let selected = state.war_state.selected_planet;
                    let list_y = by + 148.0;
                    let list_h = bh - 240.0;

                    let holo_t = state.war_state.holo_rotation;
                    let grid_alpha = 0.06 + (holo_t * 2.0).sin().abs() * 0.03;
                    for i in 0..6 {
                        let gy = list_y + i as f32 * list_h / 5.0;
                        tb.add_rect(bx + 10.0, gy, bw - 20.0, 1.0, [0.1, 0.2, 0.4, grid_alpha]);
                    }

                    for (i, body) in state.current_system.bodies.iter().enumerate() {
                        let planet = &body.planet;
                        let war_status = state.war_state.planets.get(i);
                        let t_pos = if num_planets <= 1 { 0.5 } else { i as f32 / (num_planets - 1) as f32 };
                        let node_x = bx + 40.0 + t_pos * (bw - 80.0);
                        let node_y = list_y + list_h * 0.5 + (t_pos * std::f32::consts::PI).sin() * list_h * 0.25;
                        let is_sel = i == selected;

                        let node_size = if is_sel { 24.0 } else { 16.0 };
                        let is_earth = planet.name == "Earth";
                        let node_color = if is_earth {
                            [0.15, 0.45, 0.6, 0.85] // Safe zone: calm blue-green
                        } else if war_status.map_or(false, |s| s.liberated) {
                            [0.15, 0.5, 0.2, 0.8]
                        } else if is_sel {
                            [0.3, 0.5, 0.9, 0.9]
                        } else if planet.danger_level > 7 {
                            [0.7, 0.15, 0.1, 0.7]
                        } else if planet.danger_level > 4 {
                            [0.7, 0.5, 0.1, 0.7]
                        } else {
                            [0.2, 0.35, 0.5, 0.7]
                        };
                        tb.add_rect(node_x - node_size * 0.5, node_y - node_size * 0.5, node_size, node_size, node_color);

                        if is_sel {
                            let pulse = (timer * 3.0).sin() * 0.3 + 0.7;
                            let rs = node_size + 6.0;
                            let rc = [0.4 * pulse, 0.7 * pulse, 1.0 * pulse, 0.5];
                            tb.add_rect(node_x - rs * 0.5, node_y - rs * 0.5, rs, 2.0, rc);
                            tb.add_rect(node_x - rs * 0.5, node_y + rs * 0.5 - 2.0, rs, 2.0, rc);
                            tb.add_rect(node_x - rs * 0.5, node_y - rs * 0.5, 2.0, rs, rc);
                            tb.add_rect(node_x + rs * 0.5 - 2.0, node_y - rs * 0.5, 2.0, rs, rc);
                        }

                        let name = &planet.name;
                        let ns = if is_sel { 1.3 } else { 1.0 };
                        let nw = name.len() as f32 * 6.0 * ns;
                        let nc = if is_sel { [1.0, 1.0, 1.0, 1.0] } else { [0.5, 0.6, 0.7, 0.8] };
                        tb.add_text(node_x - nw * 0.5, node_y + node_size * 0.5 + 3.0, name, ns, nc);

                        let lib = war_status.map_or(0.0, |s| s.liberation);
                        let lb_w = 40.0;
                        tb.add_rect(node_x - lb_w * 0.5, node_y + node_size * 0.5 + 3.0 + ns * 10.0, lb_w, 3.0, [0.08, 0.08, 0.12, 0.8]);
                        let lbc = if lib > 0.8 { [0.2, 0.8, 0.3, 1.0] } else if lib > 0.4 { [0.7, 0.7, 0.2, 1.0] } else { [0.7, 0.2, 0.15, 1.0] };
                        tb.add_rect(node_x - lb_w * 0.5, node_y + node_size * 0.5 + 3.0 + ns * 10.0, lb_w * lib, 3.0, lbc);
                    }

                    let dp = &state.planet;
                    let dws = state.war_state.planets.get(selected);
                    let dx = bx + 20.0;
                    let mut dy = by + bh - 100.0;
                    let ds = 1.5;
                    let line_hd = 18.0;
                    tb.add_text(dx, dy, &format!("TARGET: {}", dp.name), ds, [1.0, 0.9, 0.5, 1.0]); dy += line_hd;
                    let is_earth = dp.name == "Earth";
                    if is_earth {
                        // Earth: no danger counter — safe zone, visit only
                        tb.add_text(dx, dy, "Mission: Visit | Biome: All", ds, [0.7, 0.7, 0.8, 1.0]); dy += line_hd;
                        tb.add_text(dx, dy, "Safe zone — no combat. Homeworld.", ds, [0.4, 0.7, 0.5, 0.9]); dy += line_hd;
                    } else {
                        let mission_str = state.next_mission_type.name().to_string();
                        let biome_str = format!("{:?}", dp.primary_biome);
                        let danger_str = format!("{}/10", dp.danger_level);
                        tb.add_text(dx, dy, &format!("Mission: {} | Biome: {} | Danger: {}", mission_str, biome_str, danger_str), ds, [0.7, 0.7, 0.8, 1.0]); dy += line_hd;
                        let lib_val = dws.map_or(0.0, |s| s.liberation);
                        tb.add_text(dx, dy, &format!("Liberation: {:.0}% | Kills: {} | Extractions: {}",
                            lib_val * 100.0,
                            dws.map_or(0, |s| s.total_kills),
                            dws.map_or(0, |s| s.successful_extractions),
                        ), ds, [0.5, 0.6, 0.7, 0.9]); dy += line_hd;
                        if dws.map_or(false, |s| s.defense_urgency > 0.1) {
                            let flash = (timer * 4.0).sin() * 0.3 + 0.7;
                            tb.add_text(dx, dy, "!! BUGS COUNTER-ATTACKING !!", ds, [1.0, 0.3 * flash, 0.1, flash]);
                        }
                    }

                    let ctrl = "[↑/↓ or W/Q] System   [A/D] Planet   [1-5] Mission   [E] Close   [SPACE] Deploy";
                    let ctrl_w = ctrl.len() as f32 * 6.0 * 1.5;
                    tb.add_text(sw * 0.5 - ctrl_w * 0.5, by + bh - 20.0, ctrl, 1.5, [0.5, 0.7, 1.0, 0.8]);

                    let ticker_texts = [
                        "FEDERAL NETWORK: \"The only good bug is a dead bug!\"",
                        "  ///  SERVICE GUARANTEES CITIZENSHIP  ///  ",
                        "FLEET COM: All troopers report to deployment bays.",
                    ];
                    let full_ticker: String = ticker_texts.join("");
                    let ticker_w = full_ticker.len() as f32 * 6.0 * 1.0;
                    let ticker_x = sw - (state.war_state.ticker_offset % (ticker_w + sw));
                    tb.add_text(ticker_x, by + bh - 6.0, &full_ticker, 1.0, [0.4, 0.5, 0.6, 0.5]);
                } else {
                    if dist_to_table < 4.0 {
                        let prompt = "[E] ACCESS WAR TABLE";
                        let pw = prompt.len() as f32 * 6.0 * 2.5;
                        tb.add_rect(cx - pw * 0.5 - 6.0, cy + 40.0, pw + 12.0, 30.0, [0.02, 0.03, 0.06, 0.7]);
                        let flash = (timer * 2.5).sin() * 0.3 + 0.7;
                        tb.add_text(cx - pw * 0.5, cy + 46.0, prompt, 2.5, [0.3 + flash * 0.3, 0.6 + flash * 0.2, 1.0, 1.0]);
                    }

                    if dist_to_bay < 4.0 {
                        let target_name = &state.planet.name;
                        let prompt = format!("[SPACE] DEPLOY TO {}", target_name);
                        let pw = prompt.len() as f32 * 6.0 * 2.5;
                        let flash = (timer * 3.0).sin() * 0.5 + 0.5;
                        tb.add_rect(cx - pw * 0.5 - 6.0, cy + 40.0, pw + 12.0, 30.0, [0.1, 0.02, 0.02, 0.7]);
                        tb.add_text(cx - pw * 0.5, cy + 46.0, &prompt, 2.5, [1.0, 0.5 * flash + 0.3, 0.1, 1.0]);
                    }

                    let hint = format!("Target: {} | WAR TABLE forward | DROP BAY aft", state.planet.name);
                    let hw = hint.len() as f32 * 6.0 * 1.3;
                    tb.add_rect(sw * 0.5 - hw * 0.5 - 6.0, sh - 28.0, hw + 12.0, 22.0, [0.02, 0.03, 0.06, 0.6]);
                    tb.add_text(sw * 0.5 - hw * 0.5, sh - 24.0, &hint, 1.3, [0.4, 0.5, 0.6, 0.8]);
                }
            }
        }
    } else if approach_in_space {
        let header_text = format!("FNS ROGER YOUNG — {} System", state.current_system.name);
        let header_w = header_text.len() as f32 * 6.0 * 1.5;
        tb.add_rect(sw * 0.5 - header_w * 0.5 - 6.0, 4.0, header_w + 12.0, 22.0, [0.02, 0.03, 0.06, 0.7]);
        tb.add_text(sw * 0.5 - header_w * 0.5, 8.0, &header_text, 1.5, [0.3, 0.5, 0.8, 0.9]);
        let cx = sw * 0.5;
        let cy = sh * 0.5;
        tb.add_rect(cx - 1.0, cy - 8.0, 2.0, 6.0, [0.5, 0.7, 1.0, 0.5]);
        tb.add_rect(cx - 1.0, cy + 2.0, 2.0, 6.0, [0.5, 0.7, 1.0, 0.5]);
        tb.add_rect(cx - 8.0, cy - 1.0, 6.0, 2.0, [0.5, 0.7, 1.0, 0.5]);
        tb.add_rect(cx + 2.0, cy - 1.0, 6.0, 2.0, [0.5, 0.7, 1.0, 0.5]);
        let msg = "PILOTING — W/S throttle  SPACE or E = launch drop pod";
        let mw = msg.len() as f32 * 6.0 * 1.4;
        tb.add_rect(sw * 0.5 - mw * 0.5 - 8.0, sh - 50.0, mw + 16.0, 28.0, [0.02, 0.05, 0.12, 0.8]);
        tb.add_text(sw * 0.5 - mw * 0.5, sh - 42.0, msg, 1.4, [0.4, 0.8, 1.0, 1.0]);
    }

    // ---- Drop pod sequence overlay ----
    if let Some(ref pod) = state.drop_pod {
        let alt = pod.altitude;
        let vel = pod.velocity;
        let t = pod.total_timer;

        if pod.terrain_fog > 0.01 {
            let fog_alpha = pod.terrain_fog * 0.7;
            let fog_color = match state.planet.primary_biome {
                procgen::BiomeType::Desert | procgen::BiomeType::Badlands => [0.6, 0.5, 0.35, fog_alpha],
                procgen::BiomeType::Volcanic | procgen::BiomeType::Ashlands => [0.3, 0.15, 0.1, fog_alpha],
                procgen::BiomeType::Frozen => [0.7, 0.75, 0.85, fog_alpha],
                procgen::BiomeType::Swamp | procgen::BiomeType::Jungle => [0.2, 0.3, 0.15, fog_alpha],
                procgen::BiomeType::Crystalline => [0.3, 0.2, 0.4, fog_alpha],
                _ => [0.5, 0.55, 0.45, fog_alpha],
            };
            tb.add_rect(0.0, 0.0, sw, sh, fog_color);
        }

        let frame_frac = (alt / 2500.0).clamp(0.0, 1.0);
        let frame = 12.0 + frame_frac * 20.0;
        tb.add_rect(0.0, 0.0, frame, sh, [0.06, 0.07, 0.09, 0.85]);
        tb.add_rect(sw - frame, 0.0, frame, sh, [0.06, 0.07, 0.09, 0.85]);
        tb.add_rect(0.0, 0.0, sw, frame * 0.35, [0.06, 0.07, 0.09, 0.75]);
        tb.add_rect(0.0, sh - frame * 0.35, sw, frame * 0.35, [0.06, 0.07, 0.09, 0.75]);

        for i in 0..6 {
            let ry = sh * 0.1 + i as f32 * sh * 0.15;
            tb.add_rect(frame - 4.0, ry, 3.0, 3.0, [0.12, 0.12, 0.14, 0.9]);
            tb.add_rect(sw - frame + 1.0, ry, 3.0, 3.0, [0.12, 0.12, 0.14, 0.9]);
        }

        match pod.phase {
            DropPhase::Detach => {
                let p = (pod.phase_timer / 3.0).min(1.0);
                let darkness = 0.75 * (1.0 - p * 0.7);
                tb.add_rect(0.0, 0.0, sw, sh, [0.02, 0.03, 0.06, darkness]);

                let flash_speed = 3.0 + p * 8.0;
                let flash = (t * flash_speed).sin() * 0.5 + 0.5;
                let warn_bar_h = 4.0;
                let warn_alpha = (1.0 - p) * flash;
                tb.add_rect(0.0, sh * 0.05, sw, warn_bar_h, [1.0, 0.5, 0.1, warn_alpha * 0.6]);
                tb.add_rect(0.0, sh * 0.95 - warn_bar_h, sw, warn_bar_h, [1.0, 0.5, 0.1, warn_alpha * 0.6]);

                let launch_text = "CORVETTE TRANSPORT — POD DETACHING";
                let tw = launch_text.len() as f32 * 6.0 * 2.5;
                tb.add_text(sw * 0.5 - tw * 0.5, sh * 0.10, launch_text, 2.5, [1.0, 0.4, 0.2, 1.0 - darkness * 0.5]);

                let sep_text = format!("SEPARATION: {:.0}m", pod.corvette_separation);
                let sw2 = sep_text.len() as f32 * 6.0 * 2.0;
                tb.add_text(sw * 0.5 - sw2 * 0.5, sh * 0.88, &sep_text, 2.0, [0.5, 0.8, 1.0, 1.0]);

                let bay_text = format!("BAY DOORS: {:.0}%", p * 100.0);
                let bw = bay_text.len() as f32 * 6.0 * 1.5;
                tb.add_text(sw * 0.5 - bw * 0.5, sh * 0.92, &bay_text, 1.5, [0.4, 0.6, 0.8, 0.8]);
            }
            DropPhase::SpaceFall => {
                let speed_frac = (vel / 300.0).min(1.0);
                let num_streaks = (speed_frac * 40.0) as usize + 5;
                for i in 0..num_streaks {
                    let sx = ((i as f32 * 73.7 + t * 30.0) % sw).abs();
                    let streak_h = 10.0 + speed_frac * 120.0;
                    let sy = ((i as f32 * 41.3 + t * 150.0 * (1.0 + speed_frac)) % (sh + streak_h)) - streak_h;
                    tb.add_rect(sx, sy, 1.5, streak_h, [0.8, 0.9, 1.0, 0.2 + speed_frac * 0.3]);
                }
            }
            DropPhase::AtmosphericEntry => {
                let entry_frac = (1.0 - alt / 800.0).clamp(0.0, 1.0);

                let heat = entry_frac * 0.85;
                let heat_color = [1.0, 0.3 + entry_frac * 0.2, 0.05, heat];
                let fire_h = sh * (0.1 + entry_frac * 0.25);
                tb.add_rect(0.0, 0.0, sw, fire_h, heat_color);
                tb.add_rect(0.0, sh - fire_h, sw, fire_h, heat_color);
                let fire_w = sw * (0.08 + entry_frac * 0.18);
                tb.add_rect(0.0, 0.0, fire_w, sh, heat_color);
                tb.add_rect(sw - fire_w, 0.0, fire_w, sh, heat_color);

                tb.add_rect(0.0, 0.0, sw, sh, [1.0, 0.5, 0.1, entry_frac * 0.18]);

                let num_streaks = (entry_frac * 30.0) as usize + 5;
                for i in 0..num_streaks {
                    let sx = ((i as f32 * 97.3 + t * 70.0) % (sw * 1.5)) - sw * 0.25;
                    let sy = ((i as f32 * 53.7 + t * 250.0) % (sh * 1.5)) - sh * 0.25;
                    let streak_len = 30.0 + entry_frac * 90.0;
                    tb.add_rect(sx, sy, 2.0, streak_len, [1.0, 0.6, 0.2, 0.35 * entry_frac]);
                }

                let flash = ((t * 4.0).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
                let warn_text = "!! ATMOSPHERIC ENTRY !!";
                let ww = warn_text.len() as f32 * 6.0 * 3.0;
                tb.add_text(sw * 0.5 - ww * 0.5, sh * 0.12, warn_text, 3.0, [1.0, 0.4 * flash, 0.1, 1.0]);
            }
            DropPhase::RetroBoost => {
                let fade = (alt / 200.0).min(1.0) * 0.25;
                tb.add_rect(0.0, 0.0, sw, sh * 0.06, [1.0, 0.4, 0.1, fade]);
                tb.add_rect(0.0, sh * 0.94, sw, sh * 0.06, [1.0, 0.4, 0.1, fade]);

                let retro_glow = (0.5 + (t * 12.0).sin() * 0.2).clamp(0.0, 1.0) * 0.4;
                tb.add_rect(0.0, sh * 0.85, sw, sh * 0.15, [0.2, 0.5, 1.0, retro_glow]);

                if alt < 50.0 {
                    let brace_flash = ((t * 6.0).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
                    let brace_text = "BRACE FOR IMPACT";
                    let bw = brace_text.len() as f32 * 6.0 * 4.0;
                    tb.add_text(sw * 0.5 - bw * 0.5, sh * 0.45, brace_text, 4.0, [1.0, 0.2, 0.1, brace_flash]);
                }
            }
            DropPhase::Impact => {
                let p = (pod.phase_timer / 1.5).min(1.0);
                let flash = (1.0 - p).powi(2);
                tb.add_rect(0.0, 0.0, sw, sh, [1.0, 0.95, 0.8, flash * 0.9]);
                let dust = p * 0.6;
                tb.add_rect(0.0, 0.0, sw, sh, [0.3, 0.2, 0.1, dust]);
            }
            DropPhase::Emerge => {
                let p = (pod.phase_timer / 2.0).min(1.0);
                let dust_alpha = (1.0 - p) * 0.45;
                tb.add_rect(0.0, 0.0, sw, sh, [0.3, 0.25, 0.15, dust_alpha]);

                let door_w = sw * 0.3 * (1.0 - p);
                if door_w > 2.0 {
                    tb.add_rect(0.0, 0.0, door_w, sh, [0.06, 0.06, 0.08, 0.9]);
                    tb.add_rect(sw - door_w, 0.0, door_w, sh, [0.06, 0.06, 0.08, 0.9]);
                }

                if p > 0.4 {
                    let emerge_text = "DROP POD DOWN - MOVE OUT, TROOPER!";
                    let ew = emerge_text.len() as f32 * 6.0 * 3.0;
                    let emerge_alpha = ((p - 0.4) / 0.6).min(1.0);
                    tb.add_text(sw * 0.5 - ew * 0.5, sh * 0.3, emerge_text, 3.0, [0.3, 1.0, 0.3, emerge_alpha]);
                }
            }
        }

        if pod.phase != DropPhase::Impact && pod.phase != DropPhase::Emerge {
            let alt_text = if alt > 1000.0 {
                format!("ALT: {:.1} km", alt / 1000.0)
            } else {
                format!("ALT: {:.0} m", alt)
            };
            let alt_color = if alt < 100.0 { [1.0, 0.3, 0.2, 1.0] }
                else if alt < 500.0 { [1.0, 0.8, 0.3, 1.0] }
                else { [0.5, 0.8, 1.0, 1.0] };
            tb.add_text_with_bg(sw * 0.72, sh * 0.08, &alt_text, 2.0, alt_color, [0.0, 0.0, 0.0, 0.5]);

            let vel_text = format!("VEL: {:.0} m/s", vel);
            let vel_color = if vel > 200.0 { [1.0, 0.4, 0.2, 1.0] } else { [0.5, 0.8, 1.0, 1.0] };
            tb.add_text_with_bg(sw * 0.72, sh * 0.08 + 22.0, &vel_text, 2.0, vel_color, [0.0, 0.0, 0.0, 0.5]);

            let phase_text = match pod.phase {
                DropPhase::Detach => "DETACHING",
                DropPhase::SpaceFall => "FREE FALL",
                DropPhase::AtmosphericEntry => "ATMO ENTRY",
                DropPhase::RetroBoost => "RETRO BURN",
                _ => "",
            };
            if !phase_text.is_empty() {
                tb.add_text_with_bg(sw * 0.72, sh * 0.08 + 44.0, phase_text, 2.0, [0.8, 0.8, 0.8, 1.0], [0.0, 0.0, 0.0, 0.5]);
            }

            if pod.retro_active {
                let g_force = vel / 10.0;
                let g_text = format!("G-FORCE: {:.1}G", g_force.min(15.0));
                let g_color = if g_force > 8.0 { [1.0, 0.2, 0.1, 1.0] } else { [1.0, 0.8, 0.3, 1.0] };
                tb.add_text_with_bg(sw * 0.72, sh * 0.08 + 66.0, &g_text, 2.0, g_color, [0.0, 0.0, 0.0, 0.5]);
            }
        }
    }

    // ---- Warp effect overlay ----
    if let Some(ref warp) = state.warp_sequence {
        let progress = warp.progress();
        let alpha = (progress * 2.0).min(1.0) * 0.7;
        tb.add_rect(0.0, 0.0, sw, sh, [0.0, 0.0, 0.1, alpha]);

        let warp_text = format!("WARPING... {:.0}%", progress * 100.0);
        let text_w = warp_text.len() as f32 * 6.0 * 3.0;
        tb.add_text(sw * 0.5 - text_w * 0.5, sh * 0.5, &warp_text, 3.0, [0.5, 0.8, 1.0, 1.0]);

        let num_lines = (progress * 20.0) as usize;
        for i in 0..num_lines {
            let ly = (i as f32 / num_lines as f32) * sh;
            let lw = 50.0 + progress * 200.0;
            let lx = ((i as f32 * 37.0) % sw) - lw * 0.5;
            tb.add_rect(lx, ly, lw, 2.0, [0.4, 0.6, 1.0, 0.3 * progress]);
        }
    }

    // ---- FPS HUD (crosshair, health, ammo) ----
    if !state.debug.noclip && state.current_planet_idx.is_some() && state.phase == GamePhase::Playing {
        let cx = sw * 0.5;
        let cy = sh * 0.5;

        let cross_size = 8.0;
        let cross_thick = 2.0;
        let cross_gap = 3.0;
        let cross_color = [1.0, 1.0, 1.0, 0.7];
        tb.add_rect(cx - cross_thick * 0.5, cy - cross_size - cross_gap, cross_thick, cross_size, cross_color);
        tb.add_rect(cx - cross_thick * 0.5, cy + cross_gap, cross_thick, cross_size, cross_color);
        tb.add_rect(cx - cross_size - cross_gap, cy - cross_thick * 0.5, cross_size, cross_thick, cross_color);
        tb.add_rect(cx + cross_gap, cy - cross_thick * 0.5, cross_size, cross_thick, cross_color);

        if let Some(hm) = state.combat.latest_hit_marker() {
            let hm_color = if hm.is_kill { [1.0, 0.3, 0.3, 1.0] } else { [1.0, 1.0, 1.0, 0.9] };
            let hm_size = if hm.is_kill { 12.0 } else { 10.0 };
            tb.add_rect(cx - hm_size, cy - 1.0, hm_size * 2.0, 2.0, hm_color);
            tb.add_rect(cx - 1.0, cy - hm_size, 2.0, hm_size * 2.0, hm_color);
        }

        const SQUAD_NAMETAG_MAX_DIST: f32 = 25.0;
        const SQUAD_NAMETAG_MIN_DOT: f32 = 0.4;
        let cam_pos = state.camera.position();
        let cam_fwd = state.camera.forward();
        let view_proj = state.camera.view_projection_matrix();
        for (_, (transform, squad, health)) in state.world.query::<(&Transform, &SquadMate, &Health)>().iter() {
            if health.current <= 0.0 {
                continue;
            }
            let head_pos = transform.position + Vec3::Y * 1.2;
            let to_squad = head_pos - cam_pos;
            let dist = to_squad.length();
            if dist > SQUAD_NAMETAG_MAX_DIST || dist < 0.1 {
                continue;
            }
            let dir = to_squad / dist;
            if cam_fwd.dot(dir) < SQUAD_NAMETAG_MIN_DOT {
                continue;
            }
            let clip = view_proj * glam::Vec4::new(head_pos.x, head_pos.y, head_pos.z, 1.0);
            if clip.w <= 0.01 {
                continue;
            }
            let ndc_z = clip.z / clip.w;
            if ndc_z > 1.0 {
                continue;
            }
            let sx = (clip.x / clip.w + 1.0) * 0.5 * sw;
            let sy = (1.0 - clip.y / clip.w) * 0.5 * sh;
            let name = squad.name;
            let scale = 1.5;
            let tw = name.len() as f32 * 6.0 * scale * 0.5;
            tb.add_text_with_bg(sx - tw, sy - 24.0, name, scale, [1.0, 1.0, 1.0, 0.95], [0.0, 0.0, 0.0, 0.6]);
        }

        let hbar_w = 200.0;
        let hbar_h = 12.0;
        let hbar_x = cx - 220.0;
        let hbar_y = sh - 50.0;
        let hp_pct = state.player.health_percent();
        let hp_color = if hp_pct > 0.5 { [0.2, 0.8, 0.2, 0.9] }
            else if hp_pct > 0.25 { [0.9, 0.7, 0.1, 0.9] }
            else { [1.0, 0.2, 0.1, 0.9] };

        tb.add_rect(hbar_x - 1.0, hbar_y - 1.0, hbar_w + 2.0, hbar_h + 2.0, [0.2, 0.2, 0.2, 0.8]);
        tb.add_rect(hbar_x, hbar_y, hbar_w * hp_pct, hbar_h, hp_color);

        let hp_text = format!("{:.0}/{:.0}", state.player.health, state.player.max_health);
        tb.add_text(hbar_x, hbar_y - 16.0, &hp_text, 1.8, white);
        tb.add_text(hbar_x + 70.0, hbar_y - 16.0, "HP", 1.8, gray);

        let stamina_pct = state.player.stamina_percent();
        let sbar_y = hbar_y + hbar_h + 4.0;
        tb.add_rect(hbar_x - 1.0, sbar_y - 1.0, hbar_w + 2.0, 6.0, [0.2, 0.2, 0.2, 0.6]);
        tb.add_rect(hbar_x, sbar_y, hbar_w * stamina_pct, 4.0, [0.3, 0.6, 1.0, 0.7]);

        let ammo_x = cx + 30.0;
        if state.player.is_shovel_equipped() {
            let shovel_hint = "LMB to dig".to_string();
            tb.add_text_with_bg(ammo_x, hbar_y - 4.0, &shovel_hint, 2.5, [0.6, 0.5, 0.3, 1.0], [0.0, 0.0, 0.0, 0.5]);
            tb.add_text(ammo_x, hbar_y + 22.0, "Entrenching Shovel", 1.5, gray);
        } else {
            let weapon = state.player.current_weapon();
            let ammo_text = format!("{} / {}", weapon.current_ammo, weapon.reserve_ammo);
            let ammo_color = if weapon.current_ammo == 0 { [1.0, 0.3, 0.2, 1.0] }
                else if weapon.current_ammo <= weapon.magazine_size / 4 { [1.0, 0.7, 0.2, 1.0] }
                else { white };
            tb.add_text_with_bg(ammo_x, hbar_y - 4.0, &ammo_text, 2.5, ammo_color, [0.0, 0.0, 0.0, 0.5]);

            let weapon_name = format!("{:?}", weapon.weapon_type);
            tb.add_text(ammo_x, hbar_y + 22.0, &weapon_name, 1.5, gray);

            if weapon.is_reloading {
                let reload_text = "RELOADING...";
                let rw = reload_text.len() as f32 * 6.0 * 2.0;
                tb.add_text(cx - rw * 0.5, cy + 30.0, reload_text, 2.0, [1.0, 0.8, 0.2, 0.8]);
            }

            if weapon.current_ammo == 0 && !weapon.is_reloading {
                let empty_text = if weapon.reserve_ammo > 0 { "PRESS R TO RELOAD" } else { "NO AMMO" };
                let ew = empty_text.len() as f32 * 6.0 * 2.0;
                let flash = (state.time.elapsed_seconds() * 4.0).sin() * 0.3 + 0.7;
                tb.add_text(cx - ew * 0.5, cy + 50.0, empty_text, 2.0, [1.0, 0.2, 0.1, flash]);
            }
        }

        let slot1_color = if state.player.current_weapon_slot == 0 { white } else { [0.4, 0.4, 0.4, 0.6] };
        let slot2_color = if state.player.current_weapon_slot == 1 { white } else { [0.4, 0.4, 0.4, 0.6] };
        let slot3_color = if state.player.current_weapon_slot == 2 { white } else { [0.4, 0.4, 0.4, 0.6] };
        let slot4_color = if state.player.current_weapon_slot == crate::fps::FPSPlayer::SHOVEL_SLOT { white } else { [0.4, 0.4, 0.4, 0.6] };
        let primary_name = format!("[1] {:?}", state.player.weapons[0].weapon_type);
        let secondary_name = format!("[2] {:?}", state.player.weapons[1].weapon_type);
        let tertiary_name = format!("[3] {:?}", state.player.weapons[2].weapon_type);
        tb.add_text(ammo_x, hbar_y + 36.0, &primary_name, 1.3, slot1_color);
        tb.add_text(ammo_x + 100.0, hbar_y + 36.0, &secondary_name, 1.3, slot2_color);
        tb.add_text(ammo_x + 200.0, hbar_y + 36.0, &tertiary_name, 1.3, slot3_color);
        tb.add_text(ammo_x + 300.0, hbar_y + 36.0, "[4] Shovel", 1.3, slot4_color);

        let smoke_text = if state.smoke_grenade_cooldown > 0.0 {
            format!("[G] SMOKE ({:.0}s)", state.smoke_grenade_cooldown)
        } else {
            "[G] SMOKE READY".to_string()
        };
        let smoke_color = if state.smoke_grenade_cooldown <= 0.0 {
            [0.9, 0.3, 0.3, 1.0]
        } else {
            [0.5, 0.5, 0.5, 0.7]
        };
        tb.add_text_with_bg(ammo_x - 160.0, hbar_y + 4.0, &smoke_text, 1.3, smoke_color, bg);

        let n = state.tac_fighters.len();
        let cas_text = if n > 0 {
            format!("CAS: {} ON STATION", n)
        } else if state.tac_fighter_cooldown > 0.0 {
            format!("[T] CAS ({:.0}s)", state.tac_fighter_cooldown)
        } else {
            "[T] TAC STRIKE READY".to_string()
        };
        let cas_color = if n > 0 {
            [1.0, 0.6, 0.2, 1.0]
        } else if state.tac_fighter_cooldown <= 0.0 {
            [0.3, 0.9, 0.3, 1.0]
        } else {
            [0.5, 0.5, 0.5, 0.7]
        };
        tb.add_text_with_bg(ammo_x - 160.0, hbar_y + 20.0, &cas_text, 1.3, cas_color, bg);

        let extract_text;
        let extract_color;
        if let Some(ref dropship) = state.extraction {
            match dropship.phase {
                ExtractionPhase::Called | ExtractionPhase::Inbound | ExtractionPhase::Landing => {
                    let eta = dropship.eta_to_touchdown();
                    let dist = dropship.distance_to_lz(state.player.position);
                    extract_text = format!("[V] EXTRACT ETA:{:.0}s  LZ:{:.0}m", eta, dist);
                    extract_color = [1.0, 0.8, 0.2, 1.0];
                }
                ExtractionPhase::Waiting => {
                    let remaining = dropship.time_until_dustoff();
                    let dist = dropship.distance_to_lz(state.player.position);
                    let flash = (state.time.elapsed_seconds() * 6.0).sin() * 0.3 + 0.7;
                    if dist <= extraction::BOARDING_RADIUS {
                        extract_text = "BOARDING...".to_string();
                        extract_color = [0.3, 1.0, 0.3, 1.0];
                    } else {
                        extract_text = format!("GET TO LZ! {:.0}m  DUSTOFF:{:.0}s", dist, remaining);
                        extract_color = [1.0 * flash, 0.3, 0.1, 1.0];
                    }
                }
                ExtractionPhase::Boarding => {
                    let pct = (dropship.boarding_progress * 100.0) as u32;
                    extract_text = format!("BOARDING {}%", pct);
                    extract_color = [0.3, 1.0, 0.3, 1.0];
                }
                ExtractionPhase::Departing => {
                    if dropship.player_aboard {
                        extract_text = "DEPARTING — HANG ON!".to_string();
                        extract_color = [0.3, 1.0, 0.3, 1.0];
                    } else {
                        extract_text = "EXTRACTION FAILED".to_string();
                        extract_color = [1.0, 0.2, 0.1, 1.0];
                    }
                }
                ExtractionPhase::Ascent => {
                    let flash = (state.time.elapsed_seconds() * 4.0).sin() * 0.2 + 0.8;
                    extract_text = "CLIMBING TO ROGER YOUNG...".to_string();
                    extract_color = [0.3 * flash, 0.9 * flash, 1.0 * flash, 1.0];
                }
            }
        } else if state.extraction_cooldown > 0.0 {
            extract_text = format!("[V] EXTRACT ({:.0}s)", state.extraction_cooldown);
            extract_color = [0.5, 0.5, 0.5, 0.7];
        } else if state.current_planet_idx.is_some() {
            extract_text = "[V] CALL EXTRACTION".to_string();
            extract_color = [0.3, 0.9, 0.3, 1.0];
        } else {
            extract_text = String::new();
            extract_color = [0.5, 0.5, 0.5, 0.5];
        };
        if !extract_text.is_empty() {
            tb.add_text_with_bg(ammo_x - 160.0, hbar_y + 36.0, &extract_text, 1.3, extract_color, bg);
        }

        if let Some(ref obj) = state.mission.objective_text() {
            let obj_y = hbar_y + 58.0;
            tb.add_text_with_bg(ammo_x - 160.0, obj_y, &format!("Mission: {}", obj), 1.0, [0.7, 0.8, 0.9, 1.0], bg);
            if state.mission.objective_complete {
                let complete_y = obj_y + 18.0;
                let pulse = (state.time.elapsed_seconds() * 2.0).sin() * 0.15 + 0.85;
                tb.add_text_with_bg(ammo_x - 200.0, complete_y, "MISSION COMPLETE — Extract when ready!", 1.2, [0.2 * pulse, 1.0 * pulse, 0.3 * pulse, 1.0], bg);
            }
        }

        let strat_y = hbar_y + 98.0;
        let b_ready = state.tac_fighters.len() + 4 <= 8 && state.tac_fighter_available && state.tac_fighter_cooldown <= 0.0;
        let strat_b = if b_ready { "[B] Orbital Strike" } else { "[B] Orbital Strike (cooldown)" };
        let strat_b_color = if b_ready { [0.9, 0.6, 0.2, 0.9] } else { [0.5, 0.5, 0.5, 0.7] };
        tb.add_text_with_bg(ammo_x - 160.0, strat_y, strat_b, 0.9, strat_b_color, bg);
        let supply_ready = state.supply_drop_cooldown <= 0.0;
        let strat_n: String = if supply_ready { "[N] Supply Drop".into() } else { format!("[N] Supply Drop ({:.0}s)", state.supply_drop_cooldown) };
        let strat_n_color = if supply_ready { [0.2, 0.8, 0.4, 0.9] } else { [0.5, 0.5, 0.5, 0.7] };
        tb.add_text_with_bg(ammo_x - 160.0, strat_y + 16.0, &strat_n, 0.9, strat_n_color, bg);
        let reinforce_ready = state.reinforce_cooldown <= 0.0;
        let strat_r: String = if reinforce_ready { "[R] Reinforce".into() } else { format!("[R] Reinforce ({:.0}s)", state.reinforce_cooldown) };
        let strat_r_color = if reinforce_ready { [0.9, 0.5, 0.2, 0.9] } else { [0.5, 0.5, 0.5, 0.7] };
        tb.add_text_with_bg(ammo_x - 160.0, strat_y + 32.0, &strat_r, 0.9, strat_r_color, bg);

        let mut kf_y = 60.0;
        for kf in state.combat.kill_feed.iter().rev().take(5) {
            let alpha = (kf.lifetime / 5.0).min(1.0);
            let kf_text = if kf.was_headshot {
                format!("{} [HEADSHOT] {} with {:?}", kf.killer, kf.victim, kf.weapon)
            } else {
                format!("{} killed {} with {:?}", kf.killer, kf.victim, kf.weapon)
            };
            let kf_color = if kf.was_headshot {
                [1.0, 0.8, 0.2, alpha]
            } else {
                [0.9, 0.9, 0.9, alpha]
            };
            let kf_w = kf_text.len() as f32 * 6.0 * 1.5;
            tb.add_text_with_bg(sw - kf_w - 10.0, kf_y, &kf_text, 1.5, kf_color, [0.0, 0.0, 0.0, 0.4 * alpha]);
            kf_y += line_h * 0.8;
        }

        if state.player.last_damage_time < 0.5 {
            let hit_alpha = (1.0 - state.player.last_damage_time * 2.0).max(0.0) * 0.4;
            let border = 40.0;
            tb.add_rect(0.0, 0.0, border, sh, [0.8, 0.0, 0.0, hit_alpha]);
            tb.add_rect(sw - border, 0.0, border, sh, [0.8, 0.0, 0.0, hit_alpha]);
            tb.add_rect(0.0, 0.0, sw, border, [0.8, 0.0, 0.0, hit_alpha]);
            tb.add_rect(0.0, sh - border, sw, border, [0.8, 0.0, 0.0, hit_alpha]);
        }

        if state.player.health < state.player.max_health * 0.25 && state.player.is_alive {
            let pulse = (state.time.elapsed_seconds() * 3.0).sin() * 0.15 + 0.15;
            tb.add_rect(0.0, 0.0, sw, sh, [0.5, 0.0, 0.0, pulse]);
        }

        if state.player.is_aiming && state.player.is_alive {
            let aim_alpha = state.player.aim_progress * 0.25;
            let v = 60.0;
            tb.add_rect(0.0, 0.0, v, sh, [0.0, 0.0, 0.0, aim_alpha]);
            tb.add_rect(sw - v, 0.0, v, sh, [0.0, 0.0, 0.0, aim_alpha]);
            tb.add_rect(0.0, 0.0, sw, v * 0.6, [0.0, 0.0, 0.0, aim_alpha]);
            tb.add_rect(0.0, sh - v * 0.6, sw, v * 0.6, [0.0, 0.0, 0.0, aim_alpha]);
        }

        if state.player.is_sprinting && state.player_grounded {
            let sprint_alpha = 0.08;
            let t = state.time.elapsed_seconds();
            for i in 0..12 {
                let angle = (i as f32 / 12.0) * std::f32::consts::TAU + t * 2.0;
                let dx = angle.cos();
                let dy = angle.sin();
                let start_r = 0.35;
                let end_r = 0.55;
                let lx = cx + dx * sw * start_r;
                let ly = cy + dy * sh * start_r;
                let ex = cx + dx * sw * end_r;
                let ey = cy + dy * sh * end_r;
                let len = ((ex - lx).powi(2) + (ey - ly).powi(2)).sqrt();
                let min_x = lx.min(ex);
                let min_y = ly.min(ey);
                if len > 5.0 {
                    tb.add_rect(min_x, min_y, (ex - lx).abs().max(2.0), (ey - ly).abs().max(2.0), [1.0, 1.0, 1.0, sprint_alpha]);
                }
            }
        }

        if let Some((ref text, time_left, color)) = state.kill_streaks.announcement {
            let alpha = (time_left / 0.5).min(1.0);
            let announce_scale = 4.0;
            let tw = text.len() as f32 * 6.0 * announce_scale;
            let mut c = color;
            c[3] *= alpha;

            let pulse = (state.time.elapsed_seconds() * 6.0).sin() * 0.15 + 0.85;
            let glow_w = tw + 40.0;
            let glow_h = 50.0;
            tb.add_rect(
                cx - glow_w * 0.5,
                sh * 0.3 - glow_h * 0.5,
                glow_w,
                glow_h,
                [c[0] * 0.3, c[1] * 0.3, c[2] * 0.3, alpha * 0.4 * pulse],
            );
            tb.add_text(cx - tw * 0.5, sh * 0.3 - 12.0, text, announce_scale, c);

            if state.kill_streaks.streak_count >= 2 {
                let streak_text = format!("{} kill streak!", state.kill_streaks.streak_count);
                let stw = streak_text.len() as f32 * 6.0 * 2.0;
                tb.add_text(cx - stw * 0.5, sh * 0.3 + 22.0, &streak_text, 2.0, [1.0, 1.0, 1.0, alpha * 0.8]);
            }
        }

        if state.current_planet_idx.is_some() {
            let ft = &state.biome_atmosphere.config.fog_tint;
            let density = state.biome_atmosphere.config.fog_density_mult;
            let tint_alpha = (density - 1.0).max(0.0) * 0.04;
            if tint_alpha > 0.001 {
                tb.add_rect(0.0, 0.0, sw, sh, [ft[0], ft[1], ft[2], tint_alpha]);
            }
            let vig_alpha = (density - 0.8).max(0.0) * 0.06;
            if vig_alpha > 0.001 {
                let v = 80.0;
                tb.add_rect(0.0, 0.0, v, sh, [ft[0] * 0.3, ft[1] * 0.3, ft[2] * 0.3, vig_alpha]);
                tb.add_rect(sw - v, 0.0, v, sh, [ft[0] * 0.3, ft[1] * 0.3, ft[2] * 0.3, vig_alpha]);
                tb.add_rect(0.0, 0.0, sw, v * 0.5, [ft[0] * 0.3, ft[1] * 0.3, ft[2] * 0.3, vig_alpha * 0.7]);
                tb.add_rect(0.0, sh - v * 0.5, sw, v * 0.5, [ft[0] * 0.3, ft[1] * 0.3, ft[2] * 0.3, vig_alpha]);
            }
        }
    }

    // ---- Dialogue box (Earth settlement — Starship Troopers style) ----
    if state.dialogue_state.is_open() {
        if let Some((line_text, choices)) = state.dialogue_state.current_line_and_choices() {
            let speaker_name = match &state.dialogue_state {
                crate::dialogue::DialogueState::Open { speaker_name, .. } => speaker_name.as_str(),
                _ => "",
            };
            let box_w = (line_text.len() as f32 * 6.0 * 1.2).max(280.0).min(sw - 40.0);
            let box_h = 24.0 + 28.0 + (choices.len() as f32 * 18.0);
            let box_x = sw * 0.5 - box_w * 0.5;
            let box_y = sh - box_h - 24.0;
            tb.add_rect(box_x - 4.0, box_y - 4.0, box_w + 8.0, box_h + 8.0, [0.06, 0.08, 0.12, 0.92]);
            tb.add_rect(box_x, box_y, box_w, 20.0, [0.25, 0.35, 0.45, 0.95]);
            tb.add_text(box_x + 6.0, box_y + 2.0, &format!("{}", speaker_name), 1.4, [0.9, 0.85, 0.7, 1.0]);
            let max_chars = 70;
            let line_trim: String = if line_text.chars().count() > max_chars {
                line_text.chars().take(max_chars).chain("...".chars()).collect()
            } else {
                line_text.clone()
            };
            tb.add_text(box_x + 6.0, box_y + 24.0, &line_trim, 1.1, [0.85, 0.88, 0.9, 1.0]);
            for (i, (choice_label, _)) in choices.iter().enumerate() {
                let key = (i + 1).to_string();
                tb.add_text(box_x + 6.0, box_y + 44.0 + i as f32 * 18.0, &format!("[{}] {}", key, choice_label), 1.0, [0.5, 0.75, 1.0, 1.0]);
            }
            tb.add_text(sw * 0.5 - 60.0, sh - 14.0, "1-4 = choose  Esc = close", 1.0, gray);
        }
    }

    // ---- Bottom-left: game messages ----
    let visible: Vec<&GameMessage> = state.game_messages.messages.iter()
        .rev()
        .take(state.game_messages.max_visible)
        .collect();
    let msg_count = visible.len();
    let msg_base_y = sh - 20.0 - (msg_count as f32 * line_h);
    for (i, msg) in visible.iter().rev().enumerate() {
        let alpha = if msg.time_remaining < 1.0 {
            msg.time_remaining
        } else {
            1.0
        };
        let mut color = msg.color;
        color[3] *= alpha;
        let mut msg_bg = bg;
        msg_bg[3] *= alpha;
        tb.add_text_with_bg(x, msg_base_y + i as f32 * line_h, &msg.text, scale, color, msg_bg);
    }

    tb
}
