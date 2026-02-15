//! Gameplay update logic: all per-frame game state advancement.
//!
//! Extracted from main.rs to keep the game loop modular and maintainable.

use glam::{DVec3, Vec3};
use hecs::Entity;
use procgen::PlanetSize;
use winit::keyboard::KeyCode;

use crate::bug::Bug;
use crate::fps::FPSPlayer;
use crate::bug_entity::{PhysicsBug, update_bug_physics};
use crate::destruction::{BugCorpse, BugGoreChunk, Debris};
use crate::effects::TracerProjectile;
use crate::extraction::{ExtractionDropship, ExtractionMessage, ExtractionPhase};
use crate::viewmodel::GroundedShellCasing;
use crate::horde_ai::apply_separation;
use crate::skinny::Skinny;
use crate::smoke::{SmokeCloud, SmokeGrenade};
use crate::squad::{despawn_squad, update_squad_combat, update_squad_movement, SquadMate};
use crate::fleet::{self, surface_corvette_positions};
use crate::artillery::{ArtilleryBarrage, ArtilleryMuzzleFlash, ArtilleryShell, ArtilleryTrailParticle, GroundedArtilleryShell, SHELL_FIRE_DELAY, SHELLS_PER_BARRAGE};
use crate::tac_fighter::{TacBomb, TacFighter, TacFighterPhase};
use engine_core::{Health, Lifetime, Transform};

use crate::{GamePhase, GameState, SupplyCrate};

/// Run one frame of gameplay update. Called from `GameState::update_gameplay()`.
pub fn gameplay(state: &mut GameState, dt: f32) {
    const MAX_TAC_FIGHTERS: usize = 8;

    // Handle warp / quantum travel sequence (first-person bridge view — Star Citizen style)
    if let Some(ref mut warp) = state.warp_sequence {
        warp.timer += dt;
        // First-person bridge view during warp (no planet curvature)
        state.camera.transform.position = Vec3::new(0.0, 2.2, 14.0);
        state.camera.set_yaw_pitch(0.0, 0.0);
        state.renderer.update_camera(&state.camera, 0.0);
        if warp.is_complete() {
            let target_idx = warp.target_system_idx;
            state.warp_sequence = None;
            state.arrive_at_system(target_idx);
        }
        return; // Skip normal gameplay during warp
    }

    // Environmental hazards: damage/slow when in radius (only on planet, when alive)
    if state.current_planet_idx.is_some() && state.player.is_alive {
        state.update_environmental_hazards(dt);
    }

    // Player movement (FPS walking or noclip based on debug settings)
    state.handle_player_input(dt);

    // Advance orbital time (planets orbit)
    state.orbital_time += dt as f64 * 0.1; // slow orbit

    // Update universe position based on camera
    if let Some(planet_idx) = state.current_planet_idx {
        // On a planet: track planet-local position
        let planet_pos = state.current_system.bodies[planet_idx].orbital_position(state.orbital_time);
        state.universe_position = planet_pos + DVec3::new(
            state.camera.position().x as f64,
            state.camera.position().y as f64,
            state.camera.position().z as f64,
        );

        // Check if leaving planet (altitude > atmo_height * 2)
        let altitude = state.camera.position().y;
        let planet_radius: f32 = match state.planet.size {
            PlanetSize::Small => 2000.0,
            PlanetSize::Medium => 3000.0,
            PlanetSize::Large => 5000.0,
            PlanetSize::Massive => 8000.0,
        };
        let atmo_height = planet_radius * 0.15;
        if altitude > atmo_height * 2.0 {
            state.leave_planet();
        }
    } else {
        // In space: track absolute position
        state.universe_position = DVec3::new(
            state.camera.position().x as f64,
            state.camera.position().y as f64,
            state.camera.position().z as f64,
        );

        // Check if approaching a planet
        state.check_planet_approach();
    }

    // Stream terrain chunks around camera (only when on a planet)
    let cam_pos = state.camera.position();
    if state.current_planet_idx.is_some() {
        state.chunk_manager.update(cam_pos, state.renderer.device(), &mut state.physics);
    }

    // Update flow field target to player position (for AI)
    state.horde_ai.update_target(state.player.position);

    // Spawn bugs with physics integration (skip if debug disabled)
    if !state.debug.no_bug_spawns {
        state.spawn_physics_bugs(dt);

        // Bug holes spawn bugs near themselves
        state.update_bug_holes(dt);
    }

    // Update bugs (AI + movement)
    state.horde_ai.update(&mut state.world, dt);

    // Apply separation force so bugs don't stack on each other
    // Runs on frame_count % 4 == 0 (staggered with other expensive systems)
    if state.time.frame_count() % 4 == 0 {
        apply_separation(&mut state.world, 2.5, 8.0);
    }

    // Squad drop pods: descend from orbit and spawn squad when they land (only while on planet, Playing)
    if state.current_planet_idx.is_some() && state.phase == GamePhase::Playing {
        if let Some(ref mut squad_drop) = state.squad_drop_pods {
            let all_landed = squad_drop.update(
                dt,
                |x, z| state.chunk_manager.sample_height(x, z),
                &mut state.world,
            );
            if all_landed {
                state.squad_drop_pods = None;
                state.game_messages.success("Squad has landed. Move out!".to_string());
            }
        }
    }

    // Squad mates: follow player and stick to terrain/water surface
    // When extraction is waiting: squadmates run toward the LZ (NO TROOPER LEFT BEHIND!)
    let squad_target = if let Some(ref dropship) = state.extraction {
        if dropship.phase == ExtractionPhase::Waiting {
            dropship.lz_position
        } else {
            state.player.position
        }
    } else {
        state.player.position
    };
    update_squad_movement(
        &mut state.world,
        squad_target,
        dt,
        |x, z| state.chunk_manager.walkable_height(x, z),
    );

    // Snap living bugs to terrain/water surface and sync kinematic physics bodies
    // Only snap bugs within 160m of the player – distant bugs are culled anyway
    let player_snap_pos = state.player.position;
    for (_, (transform, health, physics_bug)) in
        state.world.query_mut::<(&mut Transform, &Health, &PhysicsBug)>()
    {
        if !health.is_dead() && !physics_bug.is_ragdoll {
            let dist_sq = (transform.position.x - player_snap_pos.x).powi(2)
                + (transform.position.z - player_snap_pos.z).powi(2);
            if dist_sq > 160.0 * 160.0 {
                continue; // Too far, skip expensive terrain sample
            }
            let surface_y = state.chunk_manager.walkable_height(
                transform.position.x,
                transform.position.z,
            );
            // Place bug center above terrain/water; extra clearance prevents slope clipping
            let half_height = transform.scale.y * 0.6 + 0.15;
            transform.position.y = surface_y + half_height;
            // Keep kinematic body in sync so collisions work
            if let Some(handle) = physics_bug.body_handle {
                state.physics.set_kinematic_position(handle, transform.position);
            }
        }
    }

    // Bug physics and ragdolls (Euphoria-style)
    update_bug_physics(&mut state.world, &mut state.physics, dt);

    // Process dying bugs - spawn gore
    state.process_dying_bugs();

    // ---- Ground tracks (footprints in snow/sand — Dune / Helldivers 2 style) ----
    if state.current_planet_idx.is_some()
        && state.phase == GamePhase::Playing
        && GameState::biome_has_snow_or_sand(state.planet.primary_biome)
    {
        state.emit_ground_tracks(dt);
    }

    // ---- Weapon fire, reload, aiming, and combat ----
    if state.current_planet_idx.is_some() && state.player.is_alive {
        // Weapon firing (left mouse button)
        state.handle_weapon_fire();

        // Manual reload (R key, only when weapon equipped — not shovel)
        if state.input.is_reload_pressed() && !state.debug.noclip && !state.player.is_shovel_equipped() {
            let weapon = state.player.current_weapon();
            if !weapon.is_reloading && weapon.current_ammo < weapon.magazine_size && weapon.reserve_ammo > 0 {
                state.player.current_weapon_mut().start_reload();
                state.viewmodel_anim.trigger_switch(); // reload uses same drop/raise animation
                state.game_messages.info("Reloading...");
            }
        }

        // Weapon/tool switching (1/2/3/4 keys or scroll wheel)
        let mut switch_to: Option<usize> = None;
        if state.input.is_key_pressed(KeyCode::Digit1) {
            switch_to = Some(0);
        } else if state.input.is_key_pressed(KeyCode::Digit2) {
            switch_to = Some(1);
        } else if state.input.is_key_pressed(KeyCode::Digit3) {
            switch_to = Some(2);
        } else if state.input.is_key_pressed(KeyCode::Digit4) {
            switch_to = Some(FPSPlayer::SHOVEL_SLOT);
        } else if state.input.is_scroll_up() {
            switch_to = Some((state.player.current_weapon_slot + 1) % FPSPlayer::TOTAL_SLOTS);
        } else if state.input.is_scroll_down() {
            switch_to = Some((state.player.current_weapon_slot + FPSPlayer::TOTAL_SLOTS - 1) % FPSPlayer::TOTAL_SLOTS);
        }
        if let Some(slot) = switch_to {
            if state.player.current_weapon_slot != slot {
                state.player.set_weapon_slot(slot);
                state.viewmodel_anim.trigger_switch();
                let name = if slot == FPSPlayer::SHOVEL_SLOT {
                    "Entrenching Shovel".to_string()
                } else {
                    format!("{:?}", state.player.weapons[slot].weapon_type)
                };
                state.game_messages.info(format!("Switched to {}", name));
            }
        }

        // ADS (aim down sights) - right mouse button
        state.player.is_aiming = state.input.is_aiming();

        // Update bug combat (bugs attacking player)
        let hp_before = state.player.health;
        state.bug_combat.update(&state.world, &mut state.player, dt);
        // Cinematic: screen shake when taking damage
        if state.player.health < hp_before {
            let damage_taken = hp_before - state.player.health;
            state.screen_shake.add_trauma((damage_taken / 30.0).min(0.5));
        }
    }

    // Update player state (weapon cooldowns, reload timers, stamina, ADS)
    state.player.update(dt);

    // ADS FOV zoom (smooth transition between 70 normal and 45 ADS)
    let target_fov = if state.player.is_aiming && state.player.is_alive && !state.debug.noclip {
        45.0
    } else {
        70.0
    };
    let fov_speed = 10.0;
    if (state.camera.fov_degrees - target_fov).abs() > 0.1 {
        state.camera.fov_degrees += (target_fov - state.camera.fov_degrees) * fov_speed * dt;
    } else {
        state.camera.fov_degrees = target_fov;
    }

    // Update combat system (damage numbers, hit markers, kill feed)
    state.combat.update(dt);

    // Update effects (gore, particles)
    state.effects.update(dt);

    // ---- Cinematic effects ----
    // Screen shake decay
    state.screen_shake.update(dt);

    // Camera recoil decay (smooth return)
    if state.camera_recoil.abs() > 0.001 {
        state.camera_recoil *= (1.0 - 12.0 * dt).max(0.0); // fast decay
    } else {
        state.camera_recoil = 0.0;
    }

    // Kill streak tracking
    state.kill_streaks.update(dt);

    // Ambient dust particles (only on planet surface)
    if state.current_planet_idx.is_some() && state.player.is_alive {
        state.ambient_dust.update(dt, state.camera.position());
        // Biome-specific volumetric atmosphere (fog banks, embers, spores, etc.)
        state.biome_atmosphere.update(dt, state.camera.position(), state.time.elapsed_seconds());
    }


    // ---- Smoke grenades ----
    state.smoke_grenade_cooldown = (state.smoke_grenade_cooldown - dt).max(0.0);
    // G key throws smoke grenade
    if state.input.is_key_pressed(KeyCode::KeyG) && state.phase == GamePhase::Playing
        && state.player.is_alive && state.smoke_grenade_cooldown <= 0.0
    {
        let throw_pos = state.camera.position() + state.camera.forward() * 1.0;
        let throw_vel = state.camera.forward() * 25.0 + Vec3::Y * 12.0;
        state.smoke_grenades.push(SmokeGrenade {
            position: throw_pos,
            velocity: throw_vel,
            age: 0.0,
            detonated: false,
        });
        state.smoke_grenade_cooldown = 5.0; // 5 second cooldown
        state.game_messages.info("SMOKE OUT!");
    }

    // Update in-flight grenades
    let gravity = Vec3::new(0.0, -20.0, 0.0);
    for grenade in &mut state.smoke_grenades {
        grenade.age += dt;
        let is_in_water = state.chunk_manager.is_in_water(grenade.position.x, grenade.position.z);
        let water_level = state.chunk_manager.water_level();
        if let Some(wl) = water_level.filter(|_| is_in_water) {
            // Buoyancy: float toward surface. Grenades are light.
            let depth = wl - grenade.position.y;
            if depth > 0.0 {
                grenade.velocity.y += 12.0 * dt; // float up
            }
            grenade.velocity *= 1.0 - 3.0 * dt; // water drag
        } else {
            grenade.velocity += gravity * dt;
        }
        grenade.position += grenade.velocity * dt;

        // Check ground/water surface collision
        let surface_y = state.chunk_manager.walkable_height(grenade.position.x, grenade.position.z);
        if grenade.position.y <= surface_y + 0.2 {
            grenade.position.y = surface_y + 0.2;
            grenade.detonated = true;
        }
        // Auto-detonate after 3 seconds
        if grenade.age > 3.0 {
            grenade.detonated = true;
        }
    }

    // Convert detonated grenades into smoke clouds; red smoke designates CAS target (no T key)
    let detonated: Vec<Vec3> = state.smoke_grenades.iter()
        .filter(|g| g.detonated)
        .map(|g| g.position)
        .collect();
    state.smoke_grenades.retain(|g| !g.detonated);
    for pos in &detonated {
        state.smoke_clouds.push(SmokeCloud::new(*pos));
        state.screen_shake.add_trauma(0.08);
        state.game_messages.info("RED SMOKE DEPLOYED - MARKING POSITION");
        // Red smoke = artillery designator: start staggered barrage (6 shells, one after another)
        if state.current_planet_idx.is_some() && state.phase == GamePhase::Playing
            && state.artillery_cooldown <= 0.0
            && state.artillery_barrage.is_none()
        {
            state.artillery_barrage = Some(ArtilleryBarrage {
                target: *pos,
                shells_remaining: SHELLS_PER_BARRAGE,
                fire_timer: 0.0, // fire first shell immediately
                fire_index: 0,
            });
            state.game_messages.warning("ORBITAL ARTILLERY INBOUND — DANGER CLOSE!".to_string());
            state.game_messages.info("FLEET COM: Roger, red smoke acquired. Barrage firing.");
            state.game_messages.info("Look up to see the ships fire!".to_string());
        }
    }

    // Update active smoke clouds (staggered: frame_count % 4 == 2)
    if state.time.frame_count() % 4 == 2 || !state.smoke_clouds.is_empty() {
        // Always update if there are active clouds (for visual consistency),
        // but only do the expensive retain/cleanup on staggered frames
        for cloud in &mut state.smoke_clouds {
            cloud.update(dt);
        }
        if state.time.frame_count() % 4 == 2 {
            state.smoke_clouds.retain(|c| !c.is_done());
        }
    }

    // Update stratagem smoke (supply drop, reinforce, orbital strike)
    if state.current_planet_idx.is_some() && state.phase == GamePhase::Playing {
        for cloud in &mut state.supply_drop_smoke {
            cloud.update(dt);
        }
        if state.time.frame_count() % 4 == 2 {
            state.supply_drop_smoke.retain(|c| !c.is_done());
        }
        if let Some(ref mut s) = state.reinforce_smoke {
            s.update(dt);
            if s.is_done() {
                state.reinforce_smoke = None;
            }
        }
        if let Some(ref mut s) = state.orbital_strike_smoke {
            s.update(dt);
            if s.is_done() {
                state.orbital_strike_smoke = None;
            }
        }
    }

    // ---- Tac Fighter fleet — multiple CAS runs (Starship Troopers style) ----
    if state.current_planet_idx.is_some() && state.phase == GamePhase::Playing {
        state.tac_fighter_cooldown -= dt;

        // Squad mates can request CAS when fleet has room and cooldown is up
        let tac_ready = state.tac_fighters.len() + 4 <= MAX_TAC_FIGHTERS
            && state.tac_fighter_available
            && state.tac_fighter_cooldown <= 0.0;
        if let Some(caller) = update_squad_combat(&mut state.world, dt, tac_ready) {
            let cam_pos = state.camera.transform.position;
            let corvettes = surface_corvette_positions(
                cam_pos,
                state.orbital_time,
                state.time.elapsed_seconds(),
            );
            let base_angle = rand::random::<f32>() * std::f32::consts::TAU;
            for i in 0..4 {
                let angle = base_angle + (i as f32) * std::f32::consts::FRAC_PI_2;
                let approach_dir = Vec3::new(angle.cos(), 0.0, angle.sin());
                let idx = fleet::corvette_index_for_direction(&corvettes, state.player.position, -approach_dir);
                let spawn = Some((corvettes[idx], idx as u8));
                state.tac_fighters.push(TacFighter::new_with_angle(state.player.position, angle, spawn));
            }
            state.tac_fighter_cooldown = 25.0 + rand::random::<f32>() * 20.0;
            state.game_messages.warning("TAC FIGHTER FLEET INBOUND - DANGER CLOSE!".to_string());
            state.game_messages.info(format!("{}: Roger, four birds on station! Ordnance away.", caller));
        }

        // Stratagem B = Orbital Strike (tac fighter fleet on your position — Helldivers 2 style)
        if state.input.is_key_pressed(KeyCode::KeyB) && tac_ready {
            let cam_pos = state.camera.transform.position;
            let corvettes = surface_corvette_positions(
                cam_pos,
                state.orbital_time,
                state.time.elapsed_seconds(),
            );
            let base_angle = rand::random::<f32>() * std::f32::consts::TAU;
            for i in 0..4 {
                let angle = base_angle + (i as f32) * std::f32::consts::FRAC_PI_2;
                let approach_dir = Vec3::new(angle.cos(), 0.0, angle.sin());
                let idx = fleet::corvette_index_for_direction(&corvettes, state.player.position, -approach_dir);
                let spawn = Some((corvettes[idx], idx as u8));
                state.tac_fighters.push(TacFighter::new_with_angle(state.player.position, angle, spawn));
            }
            state.tac_fighter_cooldown = 25.0 + rand::random::<f32>() * 20.0;
            state.orbital_strike_smoke = Some(SmokeCloud::new(state.player.position));
            state.game_messages.warning("ORBITAL STRIKE FLEET INBOUND — DANGER CLOSE!".to_string());
            state.game_messages.info("FLEET COM: Roger, four birds inbound. Good hunting.".to_string());
        }

        // Stratagem N = Supply Drop (ammo + health crate at position ahead of you)
        state.supply_drop_cooldown -= dt;
        if state.input.is_key_pressed(KeyCode::KeyN) && state.supply_drop_cooldown <= 0.0 {
            let fwd = Vec3::new(state.camera.forward().x, 0.0, state.camera.forward().z).normalize_or_zero();
            let drop_pos = state.player.position + fwd * 15.0;
            state.supply_crates.push(SupplyCrate {
                position: drop_pos,
                lifetime: 0.0,
                used: false,
            });
            state.supply_drop_smoke.push(SmokeCloud::new(drop_pos));
            state.supply_drop_cooldown = 60.0;
            state.game_messages.warning("SUPPLY DROP INBOUND!".to_string());
            state.game_messages.info("FLEET COM: Supply crate deploying to your position.".to_string());
        }

        // Update supply crates: lifetime, pickup (refill ammo + health)
        for supply_crate in &mut state.supply_crates {
            supply_crate.lifetime += dt;
            if !supply_crate.used {
                let dist = state.player.position.distance(supply_crate.position);
                if dist < 3.0 {
                    supply_crate.used = true;
                    state.player.health = (state.player.health + 50.0).min(state.player.max_health);
                    state.player.armor = (state.player.armor + 25.0).min(state.player.max_armor);
                    for w in &mut state.player.weapons {
                        w.current_ammo = w.magazine_size;
                        w.reserve_ammo = (w.reserve_ammo + 100).min(999);
                        w.is_reloading = false;
                    }
                    state.game_messages.success("Supply crate — ammo and health restored!".to_string());
                }
            }
        }
        state.supply_crates.retain(|sc| sc.lifetime < 30.0);

        // Stratagem R = Reinforce (full heal + armor + ammo from orbit — one life, no respawn, but reinforcements)
        state.reinforce_cooldown -= dt;
        if state.input.is_key_pressed(KeyCode::KeyR) && state.reinforce_cooldown <= 0.0 {
            state.player.health = state.player.max_health;
            state.player.armor = state.player.max_armor;
            for w in &mut state.player.weapons {
                w.current_ammo = w.magazine_size;
                w.reserve_ammo = (w.reserve_ammo + 150).min(999);
                w.is_reloading = false;
            }
            state.reinforce_cooldown = 90.0;
            state.reinforce_smoke = Some(SmokeCloud::new(state.player.position));
            state.game_messages.warning("REINFORCEMENTS INBOUND!".to_string());
            state.game_messages.success("Orbital supply run — health, armor, and ammo restored.".to_string());
        }

        // Update all tac fighters in the fleet
        let mut buzz_msg = false;
        let mut bombs_msg = false;
        let player_pos = state.player.position;
        let corvettes = surface_corvette_positions(
            state.camera.transform.position,
            state.orbital_time,
            state.time.elapsed_seconds(),
        );
        for fighter in &mut state.tac_fighters {
            let bomb_drops = fighter.update(dt, player_pos, Some(&corvettes));
            for drop_pos in bomb_drops {
                state.tac_bombs.push(TacBomb::new(drop_pos, fighter.velocity));
            }
            if fighter.phase == TacFighterPhase::BuzzPass && fighter.phase_timer < dt * 2.0 {
                buzz_msg = true;
            }
            if fighter.phase == TacFighterPhase::BombingRun && fighter.phase_timer < dt * 2.0 {
                bombs_msg = true;
            }
        }
        if buzz_msg {
            state.screen_shake.add_trauma(0.3);
            state.game_messages.warning("TAC FIGHTER PASSING OVERHEAD!".to_string());
        }
        if bombs_msg {
            state.game_messages.warning("BOMBS AWAY! TAKE COVER!".to_string());
        }

        // Remove completed fighters
        let before = state.tac_fighters.len();
        state.tac_fighters.retain(|f| !f.is_done());
        if state.tac_fighters.len() < before {
            state.game_messages.info("FLEET COM: Tac Fighter RTB. Good hunting, trooper.");
        }

        // Update falling bombs
        for bomb in &mut state.tac_bombs {
            bomb.age += dt;
            let is_in_water = state.chunk_manager.is_in_water(bomb.position.x, bomb.position.z);
            if is_in_water {
                bomb.velocity += Vec3::new(0.0, -12.0, 0.0) * dt; // sink slower in water
                bomb.velocity *= 1.0 - 2.0 * dt; // water drag
            } else {
                bomb.velocity += Vec3::new(0.0, -30.0, 0.0) * dt; // heavy gravity
            }
            bomb.position += bomb.velocity * dt;

            // Ground/lake bed collision
            let surface_y = state.chunk_manager.sample_height(bomb.position.x, bomb.position.z);
            if bomb.position.y <= surface_y + 0.5 {
                bomb.detonated = true;
            }
        }

        // Process bomb detonations (Helldivers-style massive destruction)
        let detonated_bombs: Vec<Vec3> = state.tac_bombs.iter()
            .filter(|b| b.detonated)
            .map(|b| b.position)
            .collect();
        state.tac_bombs.retain(|b| !b.detonated);

        for impact_pos in &detonated_bombs {
            // Explosion effect: fire/smoke billboards (flat look like red smoke)
            state.effects.spawn_tac_explosion(*impact_pos);
            // Destruction debris: flying terrain chunks from crater
            state.destruction.spawn_debris(
                &mut state.world,
                *impact_pos,
                28,
                0.35,
                &mut state.physics,
            );

            // MASSIVE crater - Helldivers 2 style
            // Primary crater (deep center)
            state.chunk_manager.deform_at(
                *impact_pos, 12.0, 5.0,
                state.renderer.device(), &mut state.physics,
            );
            // Outer blast ring
            state.chunk_manager.deform_at(
                *impact_pos, 20.0, 2.0,
                state.renderer.device(), &mut state.physics,
            );
            // Debris scars
            for i in 0..6 {
                let angle = i as f32 * std::f32::consts::TAU / 6.0 + rand::random::<f32>() * 0.5;
                let offset = Vec3::new(angle.cos() * 15.0, 0.0, angle.sin() * 15.0);
                state.chunk_manager.deform_at(
                    *impact_pos + offset, 4.0, 2.0,
                    state.renderer.device(), &mut state.physics,
                );
            }

            // MASSIVE screen shake
            let dist_to_player = (*impact_pos - state.player.position).length();
            let shake = (1.0 - (dist_to_player / 100.0).min(1.0)) * 0.8 + 0.2;
            state.screen_shake.add_trauma(shake);

            // Kill bugs in blast radius
            let kill_radius = 18.0;
            let kill_radius_sq = kill_radius * kill_radius;
            let mut kills = Vec::new();
            for (entity, (transform, _)) in state.world.query::<(&Transform, &Bug)>().iter() {
                let dist_sq = transform.position.distance_squared(*impact_pos);
                if dist_sq < kill_radius_sq {
                    kills.push(entity);
                }
            }
            for entity in &kills {
                if let Ok(mut health) = state.world.get::<&mut Health>(*entity) {
                    health.take_damage(9999.0);
                }
                if let Ok(mut pb) = state.world.get::<&mut PhysicsBug>(*entity) {
                    let dir = state.world.get::<&Transform>(*entity)
                        .map(|t| (t.position - *impact_pos).normalize_or_zero())
                        .unwrap_or(Vec3::Y);
                    pb.impact_velocity = dir * 30.0 + Vec3::Y * 20.0;
                }
            }

            // Spawn impact effects
            for i in 0..12 {
                let angle = i as f32 * std::f32::consts::TAU / 12.0;
                let offset = Vec3::new(angle.cos() * 8.0, 3.0 + (i as f32) * 0.5, angle.sin() * 8.0);
                state.effects.spawn_muzzle_flash(*impact_pos + offset, Vec3::Y);
            }

            // Destroy any destructibles in range
            state.destruction.apply_explosion(
                &mut state.world, &mut state.physics,
                *impact_pos, 15.0, 500.0,
            );

            // Destroy corpses in blast radius (Helldivers 2 style)
            let corpse_kill_radius_sq = kill_radius_sq;
            let mut corpses_to_destroy = Vec::new();
            for (entity, (transform, _)) in state.world.query::<(&Transform, &BugCorpse)>().iter() {
                if transform.position.distance_squared(*impact_pos) < corpse_kill_radius_sq {
                    corpses_to_destroy.push(entity);
                }
            }
            for e in corpses_to_destroy {
                state.world.despawn(e).ok();
            }

            state.game_messages.warning(format!(
                "IMPACT! Crater: {}m radius | {} bugs neutralized",
                20, kills.len()
            ));
        }
    }

    // ---- Orbital artillery (red smoke designator) ----
    if state.current_planet_idx.is_some() && state.phase == GamePhase::Playing {
        let was_rearming = state.artillery_cooldown > 0.0;
        state.artillery_cooldown -= dt;
        if was_rearming && state.artillery_cooldown <= 0.0 {
            state.game_messages.info("FLEET COM: Artillery batteries ready. Red smoke to designate.");
        }

        // Barrage: fire shells one after another with delay
        if let Some(ref mut barrage) = state.artillery_barrage {
            barrage.fire_timer -= dt;
            if barrage.fire_timer <= 0.0 && barrage.shells_remaining > 0 {
                let cam_pos = state.camera.transform.position;
                let corvettes = surface_corvette_positions(
                    cam_pos,
                    state.orbital_time,
                    state.time.elapsed_seconds(),
                );
                let destroyers = fleet::surface_destroyer_positions(
                    cam_pos,
                    state.orbital_time,
                    state.time.elapsed_seconds(),
                );
                let target = barrage.target + Vec3::new(
                    (rand::random::<f32>() - 0.5) * 25.0,
                    0.0,
                    (rand::random::<f32>() - 0.5) * 25.0,
                );
                let i = barrage.fire_index;
                // Fire from ventral guns — flash between ship and ground, visible when looking up
                let (from_pos, facing) = if i % 2 == 0 && !corvettes.is_empty() {
                    let idx = (i / 2) % corvettes.len();
                    let pos = corvettes[idx];
                    let to_tgt = (target - pos).normalize_or_zero();
                    (pos + to_tgt * 8.0 + Vec3::Y * -18.0, to_tgt)
                } else if !destroyers.is_empty() {
                    let idx = (i / 2) % destroyers.len();
                    let pos = destroyers[idx];
                    let to_tgt = (target - pos).normalize_or_zero();
                    (pos + to_tgt * 12.0 + Vec3::Y * -22.0, to_tgt)
                } else if !corvettes.is_empty() {
                    let idx = i % corvettes.len();
                    let pos = corvettes[idx];
                    let to_tgt = (target - pos).normalize_or_zero();
                    (pos + to_tgt * 8.0 + Vec3::Y * -18.0, to_tgt)
                } else {
                    (barrage.target + Vec3::Y * 250.0, Vec3::Y * -1.0) // fallback
                };
                state.artillery_shells.push(ArtilleryShell::new(from_pos, target));
                state.artillery_muzzle_flashes.push(ArtilleryMuzzleFlash::new(from_pos, facing));
                barrage.fire_timer = SHELL_FIRE_DELAY;
                barrage.shells_remaining -= 1;
                barrage.fire_index += 1;
            }
            if barrage.shells_remaining == 0 {
                state.artillery_barrage = None;
                state.artillery_cooldown = 40.0 + rand::random::<f32>() * 25.0; // rearm time
                state.game_messages.info("FLEET COM: Artillery batteries rearming. Stand by.");
            }
        }

        // Update muzzle flashes
        for flash in &mut state.artillery_muzzle_flashes {
            flash.age += dt;
        }
        state.artillery_muzzle_flashes.retain(|f| !f.is_done());

        // Update falling artillery shells + spawn trail particles
        const ARTILLERY_TRAIL_MAX: usize = 280;
        for shell in &mut state.artillery_shells {
            let prev_pos = shell.position;
            shell.age += dt;
            shell.velocity += Vec3::new(0.0, -90.0, 0.0) * dt; // orbital guns = high velocity
            shell.position += shell.velocity * dt;

            // Spawn trail particles (smoke/fire streak behind shell)
            if shell.velocity.length_squared() > 100.0 && state.artillery_trail_particles.len() < ARTILLERY_TRAIL_MAX {
                state.artillery_trail_particles.push(ArtilleryTrailParticle::new(
                    prev_pos, // trail appears where shell has passed
                    shell.velocity,
                ));
            }

            let surface_y = state.chunk_manager.sample_height(shell.position.x, shell.position.z);
            if shell.position.y <= surface_y + 0.5 {
                shell.detonated = true;
            }
        }

        // Update trail particles
        for p in &mut state.artillery_trail_particles {
            p.life -= dt;
            p.velocity *= 1.0 - 2.0 * dt; // drag
            p.velocity.y += 0.8 * dt; // buoyancy
            p.position += p.velocity * dt;
            let age_frac = 1.0 - (p.life / p.max_life);
            p.size = (0.6 + age_frac * 2.5).min(3.5);
        }
        state.artillery_trail_particles.retain(|p| p.life > 0.0);

        // Process artillery impacts (same destruction as tac bombs)
        let detonated_shells: Vec<Vec3> = state.artillery_shells.iter()
            .filter(|s| s.detonated)
            .map(|s| s.position)
            .collect();
        state.artillery_shells.retain(|s| !s.detonated);

        for impact_pos in &detonated_shells {
            // Spawn grounded shell casing (Helldivers 2 style — one big shell per impact)
            let surface_y = state.chunk_manager.sample_height(impact_pos.x, impact_pos.z);
            let angle = rand::random::<f32>() * std::f32::consts::TAU;
            let dist = 2.0 + rand::random::<f32>() * 5.0;
            let shell_pos = Vec3::new(
                impact_pos.x + angle.cos() * dist,
                surface_y + 0.15,
                impact_pos.z + angle.sin() * dist,
            );
            state.grounded_artillery_shells.push(GroundedArtilleryShell::new(shell_pos));
            state.effects.spawn_tac_explosion(*impact_pos);
            state.destruction.spawn_debris(
                &mut state.world,
                *impact_pos,
                42,
                0.45,
                &mut state.physics,
            );
            // MASSIVE craters — orbital artillery is more destructive than tac bombs
            state.chunk_manager.deform_at(
                *impact_pos, 18.0, 7.5,
                state.renderer.device(), &mut state.physics,
            );
            state.chunk_manager.deform_at(
                *impact_pos, 30.0, 3.0,
                state.renderer.device(), &mut state.physics,
            );
            for i in 0..8 {
                let angle = i as f32 * std::f32::consts::TAU / 8.0 + rand::random::<f32>() * 0.5;
                let offset = Vec3::new(angle.cos() * 22.0, 0.0, angle.sin() * 22.0);
                state.chunk_manager.deform_at(
                    *impact_pos + offset, 6.0, 2.5,
                    state.renderer.device(), &mut state.physics,
                );
            }
            let dist_to_player = (*impact_pos - state.player.position).length();
            let shake = (1.0 - (dist_to_player / 120.0).min(1.0)) * 0.8 + 0.2;
            state.screen_shake.add_trauma(shake);
            let kill_radius_sq = 28.0 * 28.0;
            for (entity, (transform, _)) in state.world.query::<(&Transform, &Bug)>().iter() {
                if transform.position.distance_squared(*impact_pos) < kill_radius_sq {
                    if let Ok(mut health) = state.world.get::<&mut Health>(entity) {
                        health.take_damage(9999.0);
                    }
                    if let Ok(mut pb) = state.world.get::<&mut PhysicsBug>(entity) {
                        let dir = state.world.get::<&Transform>(entity)
                            .map(|t| (t.position - *impact_pos).normalize_or_zero())
                            .unwrap_or(Vec3::Y);
                        pb.impact_velocity = dir * 30.0 + Vec3::Y * 20.0;
                    }
                }
            }
            for i in 0..14 {
                let angle = i as f32 * std::f32::consts::TAU / 14.0;
                let offset = Vec3::new(angle.cos() * 12.0, 3.0 + (i as f32) * 0.5, angle.sin() * 12.0);
                state.effects.spawn_muzzle_flash(*impact_pos + offset, Vec3::Y);
            }
            state.destruction.apply_explosion(
                &mut state.world, &mut state.physics,
                *impact_pos, 24.0, 600.0,
            );
        }
    }

    // ---- Extraction dropship ----
    if state.current_planet_idx.is_some() && state.phase == GamePhase::Playing {
        state.extraction_cooldown = (state.extraction_cooldown - dt).max(0.0);

        // Update LZ green smoke (keep it alive while on the surface)
        if let Some(ref mut smoke) = state.lz_smoke {
            smoke.update(dt);
            // Keep the smoke alive by resetting age while extraction is on the surface
            let on_surface = state.extraction.as_ref().map_or(false, |e| {
                matches!(e.phase,
                    ExtractionPhase::Called | ExtractionPhase::Inbound
                    | ExtractionPhase::Landing | ExtractionPhase::Waiting
                    | ExtractionPhase::Boarding
                )
            });
            if on_surface {
                // Reset age to keep spawning new particles indefinitely
                smoke.age = smoke.age.min(5.0);
            }
            // Clear smoke once in orbit or done
            if smoke.is_done() || state.extraction.as_ref().map_or(true, |e| {
                matches!(e.phase,
                    ExtractionPhase::Ascent
                )
            }) {
                state.lz_smoke = None;
            }
        }

        // V key calls for extraction
        if state.input.is_key_pressed(KeyCode::KeyV)
            && state.extraction.is_none()
            && state.extraction_cooldown <= 0.0
            && state.player.is_alive
        {
            let lz_forward = state.camera.forward();
            let lz_xz = Vec3::new(lz_forward.x, 0.0, lz_forward.z).normalize_or_zero();
            let lz_x = state.player.position.x + lz_xz.x * 30.0;
            let lz_z = state.player.position.z + lz_xz.z * 30.0;
            let lz_ground = state.chunk_manager.sample_height(lz_x, lz_z);

            // Retrieval boat launches from corvette above player (real-time descent)
            let cam_pos = state.camera.transform.position;
            let corvettes = surface_corvette_positions(
                cam_pos,
                state.orbital_time,
                state.time.elapsed_seconds(),
            );
            let approach_dir = Vec3::new(-lz_xz.x, 0.0, -lz_xz.z).normalize_or_zero();
            let lz_pos = Vec3::new(lz_x, lz_ground, lz_z);
            let corvette_idx = fleet::corvette_index_for_direction(&corvettes, lz_pos, approach_dir);
            let corvette_spawn = corvettes.get(corvette_idx).copied().unwrap_or_else(|| {
                lz_pos + approach_dir * 200.0 + Vec3::Y * 280.0
            });

            state.extraction = Some(ExtractionDropship::new(
                state.player.position,
                state.camera.forward(),
                lz_ground,
                corvette_spawn,
            ));
            // Spawn green smoke at the LZ
            state.lz_smoke = Some(SmokeCloud::new(Vec3::new(lz_x, lz_ground, lz_z)));
            state.game_messages.warning("FLEET COM: Copy that, retrieval boat launching from corvette. ETA 30 seconds.".to_string());
            state.game_messages.info("\"Come on you apes, get to the LZ!\"".to_string());
            state.game_messages.info("Get to the [LZ] marker and hold position!".to_string());
        }

        // Update extraction dropship
        if let Some(ref mut dropship) = state.extraction {
            // Extended far plane when retrieval boat is climbing/docking so Roger Young is visible from surface
            // (real-time: troopers on planet can watch the boat fly up and dock to the Roger Young)
            if dropship.roger_young_visible() {
                state.camera.far = 6000.0;
            } else {
                state.camera.far = 1000.0;
            }
            // Record boarding start position on phase transition
            if dropship.phase == ExtractionPhase::Boarding && dropship.boarding_start_pos.is_none() {
                dropship.boarding_start_pos = Some(state.player.position);
            }

            let msgs = dropship.update(dt, state.player.position);

            // NO TROOPER LEFT BEHIND: pick up squadmates when boat departs with player aboard
            if dropship.phase == ExtractionPhase::Departing
                && dropship.player_aboard
                && dropship.phase_timer < dt * 1.5
            {
                const SQUAD_BOARDING_RADIUS_SQ: f32 = 25.0 * 25.0;
                state.extraction_squadmates_aboard.clear();
                for (entity, (transform, _squad, health)) in
                    state.world.query::<(&Transform, &SquadMate, &Health)>().iter()
                {
                    if health.is_dead() {
                        continue;
                    }
                    let dist_sq = transform.position.distance_squared(dropship.lz_position);
                    if dist_sq < SQUAD_BOARDING_RADIUS_SQ {
                        state.extraction_squadmates_aboard.push(entity);
                    }
                }
                if !state.extraction_squadmates_aboard.is_empty() {
                    state.game_messages.success("Squad aboard! NO TROOPER LEFT BEHIND!".to_string());
                }
            }

            // Move squadmates with the boat during flight to Roger Young
            if dropship.player_camera_locked() && !state.extraction_squadmates_aboard.is_empty() {
                let boat_interior = dropship.position + dropship.ship_forward() * -2.0 + Vec3::Y * 0.5;
                for (i, &entity) in state.extraction_squadmates_aboard.iter().enumerate() {
                    if let Ok(mut transform) = state.world.get::<&mut Transform>(entity) {
                        let offset = Vec3::new(
                            ((i % 2) as f32 - 0.5) * 3.0,
                            0.0,
                            ((i / 2) as f32 - 0.5) * 2.0,
                        );
                        transform.position = boat_interior + offset;
                    }
                }
            }

            // Dispatch comms messages
            for msg in msgs {
                match msg {
                    ExtractionMessage::Info(text) => state.game_messages.info(text),
                    ExtractionMessage::Warning(text) => state.game_messages.warning(text),
                    ExtractionMessage::Success(text) => state.game_messages.success(text),
                }
            }

            // Screen shake when the boat is landing or departing nearby
            if dropship.phase == ExtractionPhase::Landing {
                let dist = dropship.distance_to_lz(state.player.position);
                if dist < 60.0 {
                    let intensity = 0.05 * (1.0 - dist / 60.0);
                    state.screen_shake.add_trauma(intensity);
                }
            }

            // ── Physics collider management ──
            let needs = dropship.needs_collider();
            if needs && dropship.hull_body.is_none() {
                // Create kinematic body + box collider for the hull
                let body_h = state.physics.add_kinematic_body(dropship.position);
                let half = dropship.hull_half_extents();
                let col_h = state.physics.add_box_collider(body_h, half);
                dropship.hull_body = Some(body_h);
                state.extraction_collider = Some(col_h);
            } else if !needs && dropship.hull_body.is_some() {
                // Remove collider when departing
                if let Some(body_h) = dropship.hull_body.take() {
                    state.physics.remove_body(body_h);
                }
                state.extraction_collider = None;
            }
            // Update kinematic body position each frame
            if let Some(body_h) = dropship.hull_body {
                state.physics.set_kinematic_position(body_h, dropship.position);
            }

            // ── Door gunner targeting ──
            if dropship.gunners_active() {
                let left_gun = dropship.gunner_left_pos();
                let right_gun = dropship.gunner_right_pos();
                let gun_range_sq = ExtractionDropship::GUNNER_RANGE * ExtractionDropship::GUNNER_RANGE;

                // Find nearest living bug to each gunner
                let mut best_left: Option<(Entity, Vec3, f32)> = None;
                let mut best_right: Option<(Entity, Vec3, f32)> = None;

                for (entity, (transform, health, _)) in state.world.query::<(&Transform, &Health, &Bug)>().iter() {
                    if health.is_dead() { continue; }
                    let pos = transform.position;
                    let dl = pos.distance_squared(left_gun);
                    let dr = pos.distance_squared(right_gun);
                    if dl < gun_range_sq {
                        if best_left.as_ref().map_or(true, |(_, _, d)| dl < *d) {
                            best_left = Some((entity, pos, dl));
                        }
                    }
                    if dr < gun_range_sq {
                        if best_right.as_ref().map_or(true, |(_, _, d)| dr < *d) {
                            best_right = Some((entity, pos, dr));
                        }
                    }
                }
                for (entity, (transform, health, _)) in state.world.query::<(&Transform, &Health, &Skinny)>().iter() {
                    if health.is_dead() { continue; }
                    let pos = transform.position;
                    let dl = pos.distance_squared(left_gun);
                    let dr = pos.distance_squared(right_gun);
                    if dl < gun_range_sq {
                        if best_left.as_ref().map_or(true, |(_, _, d)| dl < *d) {
                            best_left = Some((entity, pos, dl));
                        }
                    }
                    if dr < gun_range_sq {
                        if best_right.as_ref().map_or(true, |(_, _, d)| dr < *d) {
                            best_right = Some((entity, pos, dr));
                        }
                    }
                }

                dropship.gunner_left_target = best_left.map(|(_, p, _)| p);
                dropship.gunner_right_target = best_right.map(|(_, p, _)| p);

                // Fire gunners
                let (left_shots, right_shots) = dropship.update_gunners(dt);

                // Spawn tracers and apply damage for left gunner
                if let Some((target_entity, target_pos, _)) = best_left {
                    for _ in 0..left_shots {
                        let dir = (target_pos - left_gun).normalize_or_zero();
                        let spread = Vec3::new(
                            (rand::random::<f32>() - 0.5) * 0.04,
                            (rand::random::<f32>() - 0.5) * 0.04,
                            (rand::random::<f32>() - 0.5) * 0.04,
                        );
                        state.tracer_projectiles.push(TracerProjectile {
                            position: left_gun,
                            velocity: (dir + spread).normalize() * 160.0,
                            lifetime: 0.3,
                        });
                        // Apply damage to target bug
                        if let Ok(mut health) = state.world.get::<&mut Health>(target_entity) {
                            health.take_damage(ExtractionDropship::GUNNER_DAMAGE);
                        }
                    }
                }

                // Spawn tracers and apply damage for right gunner
                if let Some((target_entity, target_pos, _)) = best_right {
                    for _ in 0..right_shots {
                        let dir = (target_pos - right_gun).normalize_or_zero();
                        let spread = Vec3::new(
                            (rand::random::<f32>() - 0.5) * 0.04,
                            (rand::random::<f32>() - 0.5) * 0.04,
                            (rand::random::<f32>() - 0.5) * 0.04,
                        );
                        state.tracer_projectiles.push(TracerProjectile {
                            position: right_gun,
                            velocity: (dir + spread).normalize() * 160.0,
                            lifetime: 0.3,
                        });
                        if let Ok(mut health) = state.world.get::<&mut Health>(target_entity) {
                            health.take_damage(ExtractionDropship::GUNNER_DAMAGE);
                        }
                    }
                }

                // Muzzle flash screen shake for nearby gunner fire
                let total_shots = left_shots + right_shots;
                if total_shots > 0 {
                    let gun_dist = dropship.distance_to_lz(state.player.position);
                    if gun_dist < 30.0 {
                        state.screen_shake.add_trauma(0.01 * total_shots as f32);
                    }
                }
            }

            // ── Boarding walk: move player toward ramp and into the boat ──
            if dropship.phase == ExtractionPhase::Boarding {
                if let Some(start_pos) = dropship.boarding_start_pos {
                    let cam_pos = dropship.boarding_camera_pos(start_pos);
                    state.camera.transform.position = cam_pos;
                    state.player.position = cam_pos;

                    // Smoothly look toward the ship interior
                    let look = dropship.boarding_look_dir();
                    let target_yaw = look.z.atan2(look.x);
                    let target_pitch = look.y.asin();
                    let blend = (dropship.boarding_progress * 2.0).min(1.0);
                    let cur_yaw = state.camera.yaw();
                    let cur_pitch = state.camera.pitch();
                    let new_yaw = cur_yaw + (target_yaw - cur_yaw) * blend * dt * 3.0;
                    let new_pitch = cur_pitch + (target_pitch - cur_pitch) * blend * dt * 3.0;
                    state.camera.set_yaw_pitch(new_yaw, new_pitch);
                }
            }

            // ── Third-person extraction camera: watch the retrieval boat fly to the Roger Young (Helldivers 2 style) ──
            if dropship.player_camera_locked() {
                let cam_pos = dropship.extraction_chase_camera_pos();
                state.camera.transform.position = cam_pos;
                state.player.position = dropship.aboard_camera_pos(); // keep player logical position inside boat

                let target = dropship.extraction_chase_look_target();
                let dir = (target - cam_pos).normalize_or_zero();
                let target_yaw = dir.z.atan2(dir.x);
                let target_pitch = dir.y.asin();
                let cur_yaw = state.camera.yaw();
                let cur_pitch = state.camera.pitch();
                let blend_speed = 1.0 * dt;  // Slower chase cam — Roger Young in frame
                let new_yaw = cur_yaw + (target_yaw - cur_yaw) * blend_speed;
                let new_pitch = cur_pitch + (target_pitch - cur_pitch) * blend_speed;
                state.camera.set_yaw_pitch(new_yaw, new_pitch);

                state.renderer.update_camera(&state.camera, state.planet_radius_for_curvature());
            }
        } else {
            // Normal play: standard far plane
            state.camera.far = 1000.0;
        }

        // Handle extraction completion
        let extraction_done = state.extraction.as_ref().map_or(false, |e: &ExtractionDropship| e.is_done());
        if extraction_done {
            let player_aboard = state.extraction.as_ref().map_or(false, |e| e.player_aboard);

            // Clean up hull collider
            if let Some(ref mut dropship) = state.extraction {
                if let Some(body_h) = dropship.hull_body.take() {
                    state.physics.remove_body(body_h);
                }
            }
            state.extraction_collider = None;
            state.extraction = None;
            state.extraction_squadmates_aboard.clear();
            state.lz_smoke = None;

            if player_aboard {
                // Successful extraction — return to ship (squad stays on planet / despawn)
                despawn_squad(&mut state.world);
                state.complete_extraction();
            } else {
                // Failed — boat left without us, 90 second cooldown
                state.extraction_cooldown = 90.0;
                state.game_messages.warning("FLEET COM: Next retrieval window in 90 seconds. Stay alive!".to_string());
            }
        }
    }

    // ---- Viewmodel animation ----
    {
        let is_firing = state.viewmodel_anim.fire_flash_timer < 0.05;
        let is_sprinting = state.player.is_sprinting && state.player_grounded;
        let h_speed = Vec3::new(state.player_velocity.x, 0.0, state.player_velocity.z).length();
        let is_moving = h_speed > 1.0 && state.player_grounded;
        state.viewmodel_anim.update(dt, is_firing, is_sprinting, is_moving, h_speed);
    }

    // ---- Shell casing physics: fly, settle, then persist as grounded ----
    const MAX_FLYING_CASINGS: usize = 60;
    if state.shell_casings.len() > MAX_FLYING_CASINGS {
        state.shell_casings.drain(0..(state.shell_casings.len() - MAX_FLYING_CASINGS));
    }
    const MAX_GROUNDED_CASINGS: usize = 180;
    if state.grounded_shell_casings.len() > MAX_GROUNDED_CASINGS {
        state.grounded_shell_casings.drain(0..(state.grounded_shell_casings.len() - MAX_GROUNDED_CASINGS));
    }

    let mut to_grounded: Vec<usize> = Vec::new();
    for (i, casing) in state.shell_casings.iter_mut().enumerate() {
        casing.lifetime -= dt;
        let is_in_water = state.chunk_manager.is_in_water(casing.position.x, casing.position.z);
        let water_level = state.chunk_manager.water_level();
        if let Some(wl) = water_level.filter(|_| is_in_water) {
            let depth = wl - casing.position.y;
            if depth > 0.0 {
                casing.velocity.y += 8.0 * dt; // brass floats slightly
            }
            casing.velocity *= 1.0 - 4.0 * dt; // water drag
        } else {
            casing.velocity += Vec3::new(0.0, -15.0, 0.0) * dt; // gravity
            casing.velocity *= 1.0 - 2.0 * dt; // air drag
        }
        casing.position += casing.velocity * dt;

        // Tumble rotation
        let ang = casing.angular_velocity * dt;
        casing.rotation = casing.rotation * glam::Quat::from_euler(
            glam::EulerRot::XYZ, ang.x, ang.y, ang.z,
        );
        casing.angular_velocity *= 1.0 - 3.0 * dt; // angular drag

        // Bounce off terrain / float at water surface
        let surface_y = state.chunk_manager.walkable_height(casing.position.x, casing.position.z);
        if casing.position.y < surface_y + 0.01 {
            casing.position.y = surface_y + 0.01;
            casing.velocity.y = casing.velocity.y.abs() * 0.3; // bounce
            casing.velocity.x *= 0.5; // friction
            casing.velocity.z *= 0.5;
            casing.angular_velocity *= 0.5;
            // Settled on ground — convert to persistent grounded shell
            if casing.velocity.length_squared() < 0.15 && casing.lifetime > 0.5 {
                to_grounded.push(i);
            }
        }
    }
    // Move settled casings to grounded (reverse order to preserve indices)
    for &i in to_grounded.iter().rev() {
        let casing = state.shell_casings.remove(i);
        state.grounded_shell_casings.push(GroundedShellCasing::from_flying(&casing));
    }
    state.shell_casings.retain(|c| c.lifetime > 0.0);

    // Rain drops
    state.update_rain(dt);

    // Destructible debris physics (with water buoyancy)
    let surface_fn = |x: f32, z: f32| {
        let ground = state.chunk_manager.walkable_height(x, z);
        let water = state.chunk_manager.is_in_water(x, z).then(|| state.chunk_manager.water_level()).flatten();
        (ground, water)
    };
    state.destruction.update_debris(&mut state.world, dt, surface_fn);
    state.destruction.update_bug_gore(&mut state.world, dt, surface_fn);

    // Expire and despawn debris when lifetime runs out
    let mut debris_to_despawn: Vec<Entity> = Vec::new();
    for (entity, (_, mut lifetime)) in state.world.query_mut::<(&Debris, &mut Lifetime)>() {
        if lifetime.update(dt) {
            debris_to_despawn.push(entity);
        }
    }
    for e in debris_to_despawn {
        state.world.despawn(e).ok();
    }

    let mut gore_to_despawn: Vec<Entity> = Vec::new();
    for (entity, (_, mut lifetime)) in state.world.query_mut::<(&BugGoreChunk, &mut Lifetime)>() {
        if lifetime.update(dt) {
            gore_to_despawn.push(entity);
        }
    }
    for e in gore_to_despawn {
        state.world.despawn(e).ok();
    }

    // Update visible tracer projectiles
    for t in &mut state.tracer_projectiles {
        t.position += t.velocity * dt;
        t.lifetime -= dt;
    }
    state.tracer_projectiles.retain(|t| t.lifetime > 0.0);

    // Physics step (capped at 3 per frame to prevent death spiral on lag spikes)
    let mut physics_steps = 0;
    while state.time.should_fixed_update() && physics_steps < 3 {
        state.physics.step();
        physics_steps += 1;
    }

    // Clean up dead bugs (staggered: frame_count % 4 == 1)
    if state.time.frame_count() % 4 == 1 {
        state.cleanup_dead_bugs();
    }

    // Update horde state (difficulty escalation + mission tracking)
    state.spawner.update_difficulty(dt);
    let bugs_alive = state.count_living_bugs();
    state.mission.bugs_remaining = bugs_alive as u32;
    state.mission.bugs_killed = state.player.kills;
    state.mission.update(dt, state.player.is_alive);

    // Player respawn (on terrain at origin)
    if !state.player.is_alive && state.player.respawn_timer <= 0.0 {
        let respawn_y = state.chunk_manager.walkable_height(0.0, 0.0) + 1.8;
        state.player.respawn(Vec3::new(0.0, respawn_y, 0.0));
        state.game_messages.info("Player respawned!");
    }

    // Apply cinematic camera effects before uploading to GPU
    // Screen shake: offset the camera position
    if state.screen_shake.intensity > 0.001 {
        state.camera.transform.position += state.screen_shake.offset;
    }
    // Camera recoil: pitch the view up slightly
    if state.camera_recoil > 0.001 {
        state.camera.process_mouse(0.0, -state.camera_recoil * 60.0); // pitch up
    }

    // Update renderer camera
    state.renderer.update_camera(&state.camera, state.planet_radius_for_curvature());

    // Remove the shake offset so it doesn't accumulate on the real position
    if state.screen_shake.intensity > 0.001 {
        state.camera.transform.position -= state.screen_shake.offset;
    }

    // Update on-screen messages
    state.game_messages.update(dt);
}
