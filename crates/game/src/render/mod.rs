//! Rendering: all render passes (sky, terrain, bugs, ship interior, HUD, etc.).

mod overlay;
mod planet;
mod ship;

use anyhow::Result;
use engine_core::{Health, Lifetime, Transform};
use glam::{Quat, Vec3};
use procgen::BiomeType;
use renderer::{InstanceData, DEFORM_HALF_SIZE, DEFORM_TEXTURE_SIZE};
use std::collections::HashMap;
use wgpu;

use crate::biome_atmosphere::AtmoParticleKind;
use crate::bug::{Bug, BugType};
use crate::bug_entity::{GoreType, PhysicsBug, TrackKind};
use crate::skinny::Skinny;
use crate::destruction::{
    BugCorpse, BugGoreChunk, CachedRenderData, Debris, Destructible,
    MESH_GROUP_ROCK, MESH_GROUP_BUG_HOLE, MESH_GROUP_EGG_CLUSTER, MESH_GROUP_PROP_SPHERE,
    MESH_GROUP_CUBE, MESH_GROUP_LANDMARK, MESH_GROUP_HAZARD, MESH_GROUP_HIVE_MOUND,
    ENV_MESH_GROUP_COUNT,
};
use crate::extraction::{ExtractionDropship, ExtractionPhase, roger_young_parts};
use crate::fleet::{surface_corvette_positions, SURFACE_CORVETTE_PARAMS};
use crate::fps;
use crate::squad::{SquadMate, SquadMateKind};
use crate::weapons::WeaponType;
use crate::{
    interior_npc_parts, roger_young_interior_npcs, roger_young_interior_parts,
    DropPhase, GamePhase, GameState,
};

/// Run all render passes. Called from `GameState::render()`.
pub fn run(state: &mut GameState) -> Result<()> {
        let (output, mut encoder) = state.renderer.begin_frame()?;
        let scene_view = state.renderer.scene_view();
        let output_view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // ========== MINIMAL MAIN MENU (skip all 3D: no celestial, fleet, Roger Young) ==========
        if state.phase == GamePhase::MainMenu {
            state.renderer.update_camera(&state.camera, 0.0);
            state.renderer.update_sky(
                0.75,                      // night
                [0.0, -1.0, 0.0],         // sun down
                0.0, 0.0, 0.0, 0.0, 0.0,  // no clouds/dust/planet
                [0.02, 0.025, 0.04],
            );
            state.renderer.render_sky(
                &mut encoder,
                &scene_view,
                Some([0.02, 0.025, 0.04, 1.0]), // dark blue-black
            );
            let (sw, sh) = state.renderer.dimensions();
            let (sw, sh) = (sw as f32, sh as f32);
            let tb = overlay::build(state, sw, sh);
            let bloom_view = state.renderer.run_bloom_passes(&mut encoder, &scene_view);
            state.renderer.update_cinematic_uniform(state.time.elapsed_seconds());
            state.renderer.run_cinematic_pass(
                &mut encoder,
                &scene_view,
                &bloom_view,
                state.renderer.depth_texture_view(),
                &output_view,
            );
            state.renderer.render_overlay(&mut encoder, &output_view, &tb.vertices, &tb.indices);
            state.renderer.end_frame(output, encoder);
            return Ok(());
        }

        // ========== COLLECT RENDER DATA ==========

        // Bug instances by type (each bug type uses its own procedural mesh)
        let cam_pos = state.camera.position();
        const VIEWMODEL_CULL_RADIUS: f32 = 3.0; // Don't draw bugs/effects this close to camera (avoids geometry in face + tunnel)
        const VIEWMODEL_CULL_SQ: f32 = VIEWMODEL_CULL_RADIUS * VIEWMODEL_CULL_RADIUS;
        const BUG_RENDER_DIST_SQ: f32 = 250.0 * 250.0;    // Max bug render distance
        const ENTITY_RENDER_DIST_SQ: f32 = 350.0 * 350.0;  // Max rock/landmark/hazard distance (biome features visible at range)
        const GORE_RENDER_DIST_SQ: f32 = 80.0 * 80.0;      // Max gore splatter distance
        const TRACK_RENDER_DIST_SQ: f32 = 100.0 * 100.0;    // Max ground track (footprint) distance
        const EFFECT_RENDER_DIST_SQ: f32 = 120.0 * 120.0;  // Max impact/tracer/flash distance
        let mut bug_instances_by_type: HashMap<BugType, Vec<InstanceData>> = HashMap::new();
        for bug_type in [BugType::Warrior, BugType::Charger, BugType::Spitter, BugType::Tanker, BugType::Hopper] {
            bug_instances_by_type.insert(bug_type, Vec::new());
        }
        for (_, (transform, bug, health, physics_bug)) in
            state.world.query::<(&Transform, &Bug, &Health, &PhysicsBug)>().iter()
        {
            let dist_sq = transform.position.distance_squared(cam_pos);
            if dist_sq < VIEWMODEL_CULL_SQ || dist_sq > BUG_RENDER_DIST_SQ {
                continue; // Too close (viewmodel clip) or too far (not visible)
            }
            let health_factor = health.current / health.max;
            let mut color = bug.bug_type.color();
            if let Some(v) = bug.variant {
                let t = v.color_tint();
                color[0] *= t[0];
                color[1] *= t[1];
                color[2] *= t[2];
            }
            if physics_bug.is_ragdoll {
                color[0] *= 0.4;
                color[1] *= 0.4;
                color[2] *= 0.4;
            } else {
                color[0] *= 0.5 + health_factor * 0.5;
                color[1] *= 0.5 + health_factor * 0.5;
                color[2] *= 0.5 + health_factor * 0.5;
                if health_factor < 0.3 {
                    color[0] += 0.3;
                }
            }

            let (_death_offset, death_rotation, death_scale) = physics_bug.get_death_animation();
            let final_transform = if physics_bug.is_ragdoll {
                // Physics engine already drives position via rigid body —
                // only apply the procedural rotation/scale for visual flavor,
                // NOT the procedural offset (that would double-launch them).
                glam::Mat4::from_scale_rotation_translation(
                    transform.scale * death_scale,
                    transform.rotation * death_rotation,
                    transform.position,
                )
            } else {
                transform.to_matrix()
            };

            if let Some(instances) = bug_instances_by_type.get_mut(&bug.bug_type) {
                instances.push(InstanceData::new(final_transform.to_cols_array_2d(), color));
            }
        }

        // Gore instances (skip very close to camera to avoid tunnel/quads in face)
        let mut gore_instances: Vec<InstanceData> = Vec::new();
        for gore in &state.effects.gore_splatters {
            let dist_sq = gore.position.distance_squared(cam_pos);
            if dist_sq < VIEWMODEL_CULL_SQ || dist_sq > GORE_RENDER_DIST_SQ {
                continue;
            }
            let alpha = 1.0 - (gore.age / 30.0).min(1.0);
            let size = gore.size * (1.0 + gore.age * 0.15).min(2.5);
            
            // Vivid green ichor color (more saturated for cinematic impact)
            let color = match gore.splatter_type {
                GoreType::Splat => [0.15, 0.55, 0.08, alpha],
                GoreType::Spray => [0.25, 0.65, 0.12, alpha * 0.9],
                GoreType::Pool => [0.10, 0.45, 0.05, alpha],
                GoreType::Drip => [0.20, 0.60, 0.10, alpha * 0.9],
            };

            // Gore decals always lie flat on the ground (Y-up).
            // The plane mesh is an XZ plane with Y-up normal, so we only
            // rotate around Y for variety — never tilt.
            let rot_angle = (gore.position.x * 7.3 + gore.position.z * 13.1).fract() * std::f32::consts::TAU;
            let rotation = Quat::from_rotation_y(rot_angle);

            // Slight Y offset above the stored position to prevent z-fighting
            let decal_pos = Vec3::new(gore.position.x, gore.position.y + 0.03, gore.position.z);

            let matrix = glam::Mat4::from_scale_rotation_translation(
                Vec3::new(size, 1.0, size), // flat: scale X and Z, keep Y thin
                rotation,
                decal_pos,
            );

            gore_instances.push(InstanceData::new(matrix.to_cols_array_2d(), color));
        }

        // Ground track instances (footprints in snow/sand — Dune / Helldivers 2 style)
        let mut track_instances: Vec<InstanceData> = Vec::new();
        let primary_biome = state.planet.primary_biome;
        let track_visible = matches!(
            primary_biome,
            BiomeType::Desert | BiomeType::Frozen | BiomeType::Wasteland | BiomeType::Badlands
        );
        if track_visible {
            for track in &state.effects.ground_tracks {
                let dist_sq = track.position.distance_squared(cam_pos);
                if dist_sq < VIEWMODEL_CULL_SQ || dist_sq > TRACK_RENDER_DIST_SQ {
                    continue;
                }
                // Fade over ~120s; impression stays slightly visible
                let age_fade = 1.0 - (track.age / 120.0).min(1.0) * 0.75;
                let alpha = age_fade * 0.92;
                // Impression color: darker than surface (snow = shadow, sand = disturbed)
                let (r, g, b) = match primary_biome {
                    BiomeType::Frozen => (0.42, 0.48, 0.55),   // shadow in snow/ice
                    BiomeType::Desert => (0.45, 0.35, 0.24),   // disturbed sand
                    BiomeType::Wasteland => (0.32, 0.28, 0.26), // grey dust
                    BiomeType::Badlands => (0.38, 0.22, 0.14),  // red dirt
                    _ => (0.4, 0.38, 0.35),
                };
                let color = [r, g, b, alpha];
                let rotation = Quat::from_rotation_y(track.rotation_y);
                let size = track.size * (1.0 + track.age * 0.008).min(1.4); // slight spread over time
                let decal_pos = Vec3::new(
                    track.position.x,
                    track.position.y + 0.02,
                    track.position.z,
                );
                let aspect = match track.kind {
                    TrackKind::BugFoot => 1.4,
                    TrackKind::ShovelDig => 1.0, // circular dig mark
                    _ => 1.0,
                };
                let matrix = glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(size, 1.0, size * aspect),
                    rotation,
                    decal_pos,
                );
                track_instances.push(InstanceData::new(matrix.to_cols_array_2d(), color));
            }
        }

        // Bullet impact instances (skip very close to camera or very far)
        let mut impact_instances: Vec<InstanceData> = Vec::new();
        for impact in &state.effects.bullet_impacts {
            let dist_sq = impact.position.distance_squared(cam_pos);
            if dist_sq < VIEWMODEL_CULL_SQ || dist_sq > EFFECT_RENDER_DIST_SQ {
                continue;
            }
            let alpha = 1.0 - (impact.age / 2.0).min(1.0);
            let size = 0.1 + impact.age * 0.3;
            
            let color = if impact.is_blood {
                [0.3, 0.6, 0.15, alpha] // Green ichor
            } else {
                [0.8, 0.7, 0.5, alpha] // Dust/dirt
            };

            let rotation = Quat::from_rotation_arc(Vec3::Y, impact.normal);
            let matrix = glam::Mat4::from_scale_rotation_translation(
                Vec3::splat(size),
                rotation,
                impact.position + impact.normal * 0.01,
            );

            impact_instances.push(InstanceData::new(matrix.to_cols_array_2d(), color));
        }

        // Tracer projectile instances (proper bullet diamond mesh); skip if too close
        let mut tracer_instances: Vec<InstanceData> = Vec::new();
        for t in &state.tracer_projectiles {
            let dist_sq = t.position.distance_squared(cam_pos);
            if dist_sq < 2.0 * 2.0 || dist_sq > EFFECT_RENDER_DIST_SQ {
                continue;
            }
            let alpha = (t.lifetime / 0.25).min(1.0);
            // Hot bright tracer: yellow core, white-hot at start
            let heat = alpha; // brighter when fresh
            let color = [1.0, 0.7 + heat * 0.3, 0.2 + heat * 0.3, alpha];
            let dir = t.velocity.normalize_or_zero();
            let up = if dir.y.abs() < 0.99 {
                Vec3::Y
            } else {
                Vec3::Z
            };
            let right = dir.cross(up).normalize_or_zero();
            let actual_up = right.cross(dir).normalize_or_zero();
            let rot3 = glam::Mat3::from_cols(right, actual_up, dir);
            // Elongated bullet shape: thin in X/Y, long in Z (travel direction)
            let scale = Vec3::new(0.04, 0.04, 0.25);
            let matrix = glam::Mat4::from_scale_rotation_translation(
                scale,
                glam::Quat::from_mat3(&rot3),
                t.position,
            );
            tracer_instances.push(InstanceData::new(matrix.to_cols_array_2d(), color));
        }

        // Muzzle flash instances (star-shaped flash mesh)
        let mut flash_instances: Vec<InstanceData> = Vec::new();
        for flash in &state.effects.muzzle_flashes {
            if flash.intensity > 0.01 {
                // Hot white-yellow core with orange fringe
                let heat = flash.intensity;
                let color = [1.0, 0.8 + heat * 0.2, 0.3 + heat * 0.4, heat];
                let size = 0.25 * heat; // bigger flash
                
                // Rotate the flash star randomly so it doesn't look static
                let rot_angle = flash.position.x * 17.3 + flash.position.z * 31.7; // pseudo-random
                let rotation = Quat::from_rotation_z(rot_angle);
                let matrix = glam::Mat4::from_scale_rotation_translation(
                    Vec3::splat(size),
                    rotation,
                    flash.position,
                );

                flash_instances.push(InstanceData::new(matrix.to_cols_array_2d(), color));
            }
        }

        // ========== RENDER PASSES ==========

        // Sun direction from time of day (match sky shader)
        let (sun_dir, cloud_density, dust, fog_density) = state.sky_weather_params();
        let biome = state.planet.get_biome_config();
        // Biome atmosphere tints the tertiary/quaternary terrain colors for fog blending
        let fog_t = &state.biome_atmosphere.config.fog_tint;
        let amb_t = &state.biome_atmosphere.config.ambient_tint;
        let biome_colors = [
            [biome.base_color.x, biome.base_color.y, biome.base_color.z, 1.0],
            [biome.secondary_color.x, biome.secondary_color.y, biome.secondary_color.z, 1.0],
            [fog_t[0] * 0.6 + 0.2, fog_t[1] * 0.6 + 0.2, fog_t[2] * 0.6 + 0.2, 1.0],
            [amb_t[0] * 0.5 + 0.25, amb_t[1] * 0.5 + 0.25, amb_t[2] * 0.5 + 0.25, 1.0],
        ];

        // Planet physical parameters (vary by planet size)
        let planet_radius: f32 = match state.planet.size {
            procgen::PlanetSize::Small   => 2000.0,
            procgen::PlanetSize::Medium  => 3000.0,
            procgen::PlanetSize::Large   => 5000.0,
            procgen::PlanetSize::Massive => 8000.0,
        };
        let atmo_height = planet_radius * 0.15;

        // Altitude-scaled fog: push fog farther at high altitude so terrain stays visible
        let altitude = cam_pos.y.max(0.0);
        let alt_fog_mult = 1.0 + (altitude / 200.0).min(8.0);
        // Biome atmosphere modifies fog density for cinematic immersion
        let biome_fog_mult = state.biome_atmosphere.config.fog_density_mult;
        let fog_params = [
            fog_density * biome_fog_mult,
            0.05,
            (50.0 / biome_fog_mult.max(0.5)) * alt_fog_mult, // closer fog start for thick biomes
            (400.0 / (biome_fog_mult * 0.5 + 0.5)) * alt_fog_mult, // shorter visibility for thick biomes
        ];

        // Planet surface color for the orbital view
        // Weight the primary biome heavily so orbital color matches surface
        let planet_surface_color = {
            let primary_cfg = state.planet.get_biome_config();
            let primary_col = primary_cfg.base_color;
            let biomes = &state.chunk_manager.planet_biomes.biomes;
            let mut secondary_avg = Vec3::ZERO;
            let mut count = 0;
            for b in biomes {
                if *b != state.planet.primary_biome {
                    let cfg = procgen::BiomeConfig::from_type(*b);
                    secondary_avg += cfg.base_color;
                    count += 1;
                }
            }
            if count > 0 { secondary_avg /= count as f32; }
            // 70% primary biome, 30% secondary biomes for strong visual identity
            let final_col = primary_col * 0.70 + secondary_avg * 0.30;
            [final_col.x, final_col.y, final_col.z]
        };

        // Pass 0: Dynamic sky (clears and draws) -- includes planet sphere from orbit
        // Force space background: main menu, extraction orbit, or approach flight (piloting)
        let extraction_orbit = state.extraction.as_ref().map_or(false, |e: &ExtractionDropship| e.player_camera_locked());
        let approach_in_space = state.phase == GamePhase::ApproachPlanet && state.approach_flight_state.is_some();
        let in_space_view = state.phase == GamePhase::MainMenu || extraction_orbit || approach_in_space;
        if in_space_view {
            state.renderer.update_camera(&state.camera, 0.0);
        }
        // When in orbit: atmo_height=0 (no sky atmosphere), but pass cloud_density so the
        // planet sphere shows realtime weather (cloud coverage visible from orbit).
        // During drop sequence: use much larger effective atmo_height so atmosphere (clouds,
        // sun, weather) stays visible during descent — camera is at high altitude but we want
        // to show planet conditions in real time.
        let (sky_atmo_height, sky_cloud_density) = if in_space_view {
            (0.0, cloud_density)
        } else if state.phase == GamePhase::DropSequence {
            (atmo_height * 12.0, cloud_density) // ~5400 m effective atmo_end so descent shows sky
        } else {
            (atmo_height, cloud_density)
        };
        let biome_dust = dust + (biome_fog_mult - 1.0).max(0.0) * 0.08;
        state.renderer.update_sky(
            state.time_of_day,
            [sun_dir.x, sun_dir.y, sun_dir.z],
            sky_cloud_density,
            biome_dust,
            if state.planet.primary_biome == BiomeType::Toxic { 1.0 } else { 0.0 },
            planet_radius,
            sky_atmo_height,
            planet_surface_color,
        );
        state.renderer.render_sky(
            &mut encoder,
            &scene_view,
            if in_space_view {
                Some([0.002, 0.003, 0.006, 1.0]) // near-black space (Starfield/Star Citizen/HD2)
            } else {
                None
            },
        );

        // Pass 0b: Celestial bodies (stars, planets, moons in the solar system)
        let celestial_instances = state.build_celestial_instances();
        state.renderer.render_celestial(&mut encoder, &scene_view, &celestial_instances);

        // Pass 0b0: Federation fleet visible from planet surface (HellDivers 2 style)
        // Draw early (after sky/celestial) so ships are in the sky dome — large scale for visibility
        // Also show during extraction orbit (climb from surface) so Roger Young + fleet are visible
        if (!in_space_view || extraction_orbit) && state.current_planet_idx.is_some() && state.phase == GamePhase::Playing {
            let ot = state.orbital_time as f32;
            let t = state.time.elapsed_seconds();
            let sky_y = cam_pos.y + crate::fleet::CORVETTE_ALTITUDE;
            let corvette_positions = surface_corvette_positions(
                cam_pos,
                state.orbital_time,
                t,
            );
            let mut fleet_hull: Vec<InstanceData> = Vec::new();
            let mut fleet_glow: Vec<InstanceData> = Vec::new();
            let hull = [0.14, 0.16, 0.21, 1.0];
            let engine = [0.2, 0.42, 0.68, 0.6];
            // Corvettes: large silhouettes — tac fighters launch from and RTB to these
            for (i, &(radius, phase, speed)) in SURFACE_CORVETTE_PARAMS.iter().enumerate() {
                let angle = phase + ot * speed + t * 0.015;
                let pos = corvette_positions[i];
                let facing = Vec3::new(-angle.sin(), 0.0, angle.cos());
                let rot = Quat::from_rotation_arc(Vec3::Z, facing);
                let scale_len = 35.0 + (i as f32 * 0.08).sin() * 5.0; // ~35 units long
                let m = glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(3.0, 1.2, scale_len),
                    rot,
                    pos,
                );
                fleet_hull.push(InstanceData::new(m.to_cols_array_2d(), hull));
                let stern = pos - facing * scale_len * 0.55;
                let g = glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(1.5, 0.8, 1.2),
                    Quat::IDENTITY,
                    stern,
                );
                fleet_glow.push(InstanceData::new(g.to_cols_array_2d(), engine));
            }
            // Destroyers: massive silhouettes in the sky
            let d_angle = ot * 0.07 + t * 0.008;
            let d_radius = 220.0;
            for (i, &(phase_off, y_off)) in [
                (0.0f32, 80.0),
                (std::f32::consts::PI, 40.0),
                (std::f32::consts::PI * 0.6, 120.0),
            ].iter().enumerate() {
                let a = d_angle + phase_off;
                let pos = Vec3::new(
                    cam_pos.x + a.cos() * d_radius,
                    sky_y + y_off,
                    cam_pos.z + a.sin() * d_radius,
                );
                let facing = Vec3::new(-a.sin(), 0.0, a.cos());
                let rot = Quat::from_rotation_arc(Vec3::Z, facing);
                let scale_len = 70.0 + (i as f32 * 0.1).sin() * 5.0;
                let m = glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(4.5, 2.0, scale_len),
                    rot,
                    pos,
                );
                fleet_hull.push(InstanceData::new(m.to_cols_array_2d(), hull));
                let stern = pos - facing * scale_len * 0.5;
                let g = glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(2.5, 1.2, 2.0),
                    Quat::IDENTITY,
                    stern,
                );
                fleet_glow.push(InstanceData::new(g.to_cols_array_2d(), [0.25, 0.5, 0.78, 0.65]));
            }
            if !fleet_hull.is_empty() {
                state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.environment_meshes.cube, &fleet_hull);
            }
            if !fleet_glow.is_empty() {
                state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.flash_mesh, &fleet_glow);
            }
            // Pass 0b0a: Artillery muzzle flashes (ships firing — bright emissive, visible when looking up)
            if !state.artillery_muzzle_flashes.is_empty() {
                let mut muzzle_instances: Vec<InstanceData> = Vec::new();
                for flash in &state.artillery_muzzle_flashes {
                    let alpha = 1.0 - (flash.age / flash.duration).min(1.0);
                    let size = 18.0 - flash.age * 15.0; // start large (18), shrink as it fades
                    if size < 2.0 { continue; }
                    // Emissive (max > 1.5) so visible against bright sky when looking up
                    let color = [2.8, 0.7, 0.2, alpha];
                    let m = glam::Mat4::from_scale_rotation_translation(
                        Vec3::splat(size.max(2.0)),
                        Quat::from_rotation_arc(Vec3::Z, flash.facing),
                        flash.position,
                    );
                    muzzle_instances.push(InstanceData::new(m.to_cols_array_2d(), color));
                }
                if !muzzle_instances.is_empty() {
                    state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.flash_mesh, &muzzle_instances);
                }
            }
        }

        // Pass 0b1: Fleet corvettes in space (main menu, approach flight — NOT extraction orbit;
        // extraction climb uses Pass 0b0 for visible fleet + Pass 5h2 for Roger Young)
        if in_space_view && !extraction_orbit {
            let ot = state.orbital_time as f32;
            let t = state.time.elapsed_seconds();
            let mut fleet_rock: Vec<InstanceData> = Vec::new();
            let mut fleet_glow: Vec<InstanceData> = Vec::new();
            let hull = [0.14, 0.16, 0.22, 1.0];  // Slightly brighter for contrast on dark space
            let engine = [0.18, 0.35, 0.65, 0.6]; // Brighter engine glow
            // Corvettes: drift in space around the camera (distant background fleet)
            for (i, &(radius, phase, speed, y_off)) in [
                (550.0f32, 0.0, 0.12, 80.0),
                (720.0, 1.8, 0.09, -60.0),
                (480.0, 3.2, 0.15, 120.0),
                (650.0, 4.1, 0.10, -30.0),
                (820.0, 2.5, 0.07, 40.0),
                (580.0, 5.0, 0.13, -90.0),
                (750.0, 0.5, 0.08, 0.0),
                (620.0, 3.8, 0.11, 70.0),
            ].iter().enumerate() {
                let angle = phase + ot * speed + t * 0.02;
                let dx = angle.cos() * radius;
                let dz = angle.sin() * radius;
                let pos = Vec3::new(cam_pos.x + dx, cam_pos.y + y_off + (t * 0.3 + i as f32).sin() * 15.0, cam_pos.z + dz);
                let facing = Vec3::new(-angle.sin(), 0.0, angle.cos());
                let rot = Quat::from_rotation_arc(Vec3::Z, facing);
                let scale_len = 2.5 + (i as f32 * 0.1).sin() * 0.5;
                let m = glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(0.25, 0.12, scale_len),
                    rot,
                    pos,
                );
                fleet_rock.push(InstanceData::new(m.to_cols_array_2d(), hull));
                let stern = pos - facing * scale_len * 0.5;
                let g = glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(0.15, 0.08, 0.12),
                    Quat::IDENTITY,
                    stern,
                );
                fleet_glow.push(InstanceData::new(g.to_cols_array_2d(), engine));
            }
            // Two destroyers further out (larger silhouettes in the deep background)
            let d_angle = ot * 0.06 + t * 0.01;
            for (i, &(phase_off, dist, y_off)) in [(0.0f32, 1100.0, 150.0), (std::f32::consts::PI, 950.0, -100.0)].iter().enumerate() {
                let a = d_angle + phase_off;
                let pos = Vec3::new(
                    cam_pos.x + a.cos() * dist,
                    cam_pos.y + y_off,
                    cam_pos.z + a.sin() * dist,
                );
                let rot = Quat::from_rotation_arc(Vec3::Z, Vec3::new(-a.sin(), 0.0, a.cos()));
                let m = glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(0.4, 0.2, 4.0),
                    rot,
                    pos,
                );
                fleet_rock.push(InstanceData::new(m.to_cols_array_2d(), [0.15, 0.17, 0.23, 1.0]));
                let stern = pos - rot * Vec3::Z * 2.0;
                let g = glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(0.25, 0.15, 0.2),
                    Quat::IDENTITY,
                    stern,
                );
                fleet_glow.push(InstanceData::new(g.to_cols_array_2d(), [0.2, 0.4, 0.7, 0.55]));
            }
            if !fleet_rock.is_empty() {
                state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.environment_meshes.rock, &fleet_rock);
            }
            if !fleet_glow.is_empty() {
                state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.flash_mesh, &fleet_glow);
            }

            // Pass 0b1b: Roger Young in main menu (Starship Troopers 2005 orbit background)
            if state.phase == GamePhase::MainMenu {
                let ry_pos = Vec3::new(0.0, 0.0, 500.0); // Between camera (1200) and planet
                let ry_fwd = Vec3::new(0.0, 0.0, -1.0);  // Facing the planet
                let parts = roger_young_parts();
                let ship_rot = Quat::IDENTITY; // Ship faces -Z
                let scale_mult = 1.2; // Slightly larger for dramatic presence

                let mut rock_instances: Vec<InstanceData> = Vec::new();
                let mut sphere_instances: Vec<InstanceData> = Vec::new();
                let mut glow_instances: Vec<InstanceData> = Vec::new();

                for part in &parts {
                    let world_offset = ship_rot * part.offset;
                    let world_pos = ry_pos + world_offset;
                    let part_scale = part.scale * scale_mult;
                    let matrix = glam::Mat4::from_scale_rotation_translation(
                        part_scale, ship_rot, world_pos,
                    );
                    let inst = InstanceData::new(matrix.to_cols_array_2d(), part.color);
                    match part.mesh_type {
                        0 => rock_instances.push(inst),
                        1 => sphere_instances.push(inst),
                        _ => glow_instances.push(inst),
                    }
                }

                let engine_pulse = 0.85 + (state.time.elapsed_seconds() * 6.0).sin() * 0.15;
                for inst in &mut glow_instances {
                    inst.color[0] *= engine_pulse;
                    inst.color[1] *= engine_pulse;
                    inst.color[2] *= engine_pulse;
                }

                if !rock_instances.is_empty() {
                    state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.environment_meshes.rock, &rock_instances);
                }
                if !sphere_instances.is_empty() {
                    state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.environment_meshes.prop_sphere, &sphere_instances);
                }
                if !glow_instances.is_empty() {
                    state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.flash_mesh, &glow_instances);
                }
            }
        }

        // Pass 0b2: Distant destroyers (ship interior; also during warp for first-person quantum travel)
        let warp_active = state.warp_sequence.is_some();
        let ship_interior_visible = warp_active
            || ((state.phase == GamePhase::InShip || state.phase == GamePhase::ApproachPlanet)
                && !approach_in_space);
        if ship_interior_visible {
            let timer = state.ship_state.as_ref().map_or(0.0, |s| s.timer);
            let mut destroyer_rock: Vec<InstanceData> = Vec::new();
            let mut destroyer_glow: Vec<InstanceData> = Vec::new();
            let hull_color = [0.12, 0.14, 0.18, 1.0]; // dark hull silhouette
            let engine_glow = [0.18, 0.35, 0.55, 0.4]; // engine glow (emissive)
            // Forward view: destroyers ahead (elongated rock mesh = ship silhouette)
            for (i, &(z, x_off, scale_len)) in [(55.0, 0.0, 4.0), (95.0, 2.5, 3.0), (140.0, -1.5, 5.0), (75.0, -3.0, 2.5)].iter().enumerate() {
                let wobble = (timer * 0.2 + i as f32).sin() * 0.3;
                let pos = Vec3::new(x_off + wobble, 1.5 + (timer * 0.15 + i as f32 * 0.7).sin() * 0.5, z);
                let matrix = glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(0.25, 0.15, scale_len),
                    Quat::from_rotation_y(timer * 0.05 + i as f32 * 0.2),
                    pos,
                );
                destroyer_rock.push(InstanceData::new(matrix.to_cols_array_2d(), hull_color));
                let stern = pos - Vec3::new(0.0, 0.0, scale_len * 0.6);
                let glow_mat = glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(0.2, 0.1, 0.15),
                    Quat::IDENTITY,
                    stern,
                );
                destroyer_glow.push(InstanceData::new(glow_mat.to_cols_array_2d(), engine_glow));
            }
            // Port/starboard: destroyers visible through side windows
            let side_z = 15.0 + (timer * 0.1).sin() * 3.0;
            let port_pos = Vec3::new(-35.0, 2.0, side_z);
            let sb_pos = Vec3::new(38.0, 1.8, side_z - 5.0);
            for (pos, rot_y) in [(port_pos, 0.2), (sb_pos, -0.2)] {
                let matrix = glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(0.2, 0.2, 2.5),
                    Quat::from_rotation_y(rot_y),
                    pos,
                );
                destroyer_rock.push(InstanceData::new(matrix.to_cols_array_2d(), hull_color));
            }
            if !destroyer_rock.is_empty() {
                state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.environment_meshes.rock, &destroyer_rock);
            }
            if !destroyer_glow.is_empty() {
                state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.flash_mesh, &destroyer_glow);
            }

            // Pass 0b3: Federation corvettes in real-time planet orbit (Starship Troopers / Heinlein fleet)
            let ot = state.orbital_time as f32;
            let mut corvette_rock: Vec<InstanceData> = Vec::new();
            let mut corvette_glow: Vec<InstanceData> = Vec::new();
            let corvette_hull = [0.10, 0.12, 0.16, 1.0];
            let corvette_engine = [0.12, 0.28, 0.50, 0.5];
            // Corvettes orbit in the same plane as planets (XZ); orbital_time drives real-time motion — see them through the CIC windows
            for (i, &(radius, phase, speed)) in [
                (75.0f32, 0.0, 0.18),
                (110.0, 1.8, 0.14),
                (145.0, 3.5, 0.12),
                (65.0, 4.2, 0.22),
                (130.0, 5.1, 0.13),
                (90.0, 2.3, 0.16),
                (52.0, 0.7, 0.24),   // inner picket
                (165.0, 4.8, 0.10),  // outer patrol
            ].iter().enumerate() {
                let scale_len = 1.2 + (i as f32 * 0.17).sin() * 0.3;
                let angle = phase + ot * speed;
                let pos = Vec3::new(angle.cos() * radius, 0.8 + (ot * 0.5 + i as f32).sin() * 0.4, angle.sin() * radius);
                let facing = Vec3::new(-angle.sin(), 0.0, angle.cos()); // tangent = direction of motion
                let rot = Quat::from_rotation_arc(Vec3::Z, facing);
                let matrix = glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(0.12, 0.07, scale_len),
                    rot,
                    pos,
                );
                corvette_rock.push(InstanceData::new(matrix.to_cols_array_2d(), corvette_hull));
                let stern = pos - facing * scale_len * 0.5;
                let glow_mat = glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(0.08, 0.04, 0.06),
                    Quat::IDENTITY,
                    stern,
                );
                corvette_glow.push(InstanceData::new(glow_mat.to_cols_array_2d(), corvette_engine));
            }
            if !corvette_rock.is_empty() {
                state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.environment_meshes.rock, &corvette_rock);
            }
            if !corvette_glow.is_empty() {
                state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.flash_mesh, &corvette_glow);
            }
        }

        // Pass 0c: Ship interior (not when piloting in space)
        if ship_interior_visible {
            let parts = roger_young_interior_parts();
            let timer = state.ship_state.as_ref().map_or(0.0, |s| s.timer);
            let war_table_active = state.ship_state.as_ref().map_or(false, |s| s.war_table_active);

            let mut rock_instances: Vec<InstanceData> = Vec::new();
            let mut sphere_instances: Vec<InstanceData> = Vec::new();
            let mut glow_instances: Vec<InstanceData> = Vec::new();

            for part in &parts {
                // Pulsing for emissive elements
                let color = if part.mesh_type == 2 {
                    let pulse = (timer * 2.0).sin() * 0.15 + 0.85;
                    [part.color[0] * pulse, part.color[1] * pulse, part.color[2] * pulse, part.color[3]]
                } else {
                    part.color
                };

                let matrix = glam::Mat4::from_scale_rotation_translation(
                    part.scale, Quat::IDENTITY, part.pos,
                );
                let inst = InstanceData::new(matrix.to_cols_array_2d(), color);

                match part.mesh_type {
                    0 => rock_instances.push(inst),
                    1 => sphere_instances.push(inst),
                    2 => glow_instances.push(inst),
                    _ => {}
                }
            }

            // Red alert lights pulse
            let alert_pulse = ((timer * 3.0).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
            for x_sign in [-1.0f32, 1.0] {
                for z_pos in [-7.0f32, 7.0] {
                    let color = [0.8 * alert_pulse, 0.05, 0.02, alert_pulse * 0.8];
                    let matrix = glam::Mat4::from_scale_rotation_translation(
                        Vec3::splat(0.4 + alert_pulse * 0.2), Quat::IDENTITY,
                        Vec3::new(x_sign * 9.5, 3.8, z_pos),
                    );
                    glow_instances.push(InstanceData::new(matrix.to_cols_array_2d(), color));
                }
            }

            // War table holographic projection (floating planet spheres when active)
            if war_table_active {
                let num_planets = state.current_system.bodies.len();
                let selected = state.war_state.selected_planet;
                for (i, body) in state.current_system.bodies.iter().enumerate() {
                    let angle = (i as f32 / num_planets.max(1) as f32) * std::f32::consts::TAU + timer * 0.3;
                    let radius = 1.2;
                    let px = angle.cos() * radius;
                    let pz = angle.sin() * radius + 2.0; // offset to war table center
                    let py = 1.8 + (timer * 1.5 + i as f32).sin() * 0.05; // gentle bob
                    let is_sel = i == selected;
                    let size = if is_sel { 0.25 } else { 0.15 };
                    let color = if is_sel {
                        [0.4, 0.7, 1.0, 1.0]
                    } else {
                        let lib = state.war_state.planets.get(i).map_or(0.0, |p| p.liberation);
                        [0.2 + lib * 0.5, 0.3 + lib * 0.4, 0.6, 0.8]
                    };
                    let matrix = glam::Mat4::from_scale_rotation_translation(
                        Vec3::splat(size), Quat::IDENTITY, Vec3::new(px, py, pz),
                    );
                    sphere_instances.push(InstanceData::new(matrix.to_cols_array_2d(), color));

                    // Selection ring
                    if is_sel {
                        let ring_pulse = (timer * 4.0).sin() * 0.3 + 0.7;
                        let ring_matrix = glam::Mat4::from_scale_rotation_translation(
                            Vec3::new(size + 0.08, 0.02, size + 0.08),
                            Quat::from_rotation_y(timer * 2.0),
                            Vec3::new(px, py, pz),
                        );
                        glow_instances.push(InstanceData::new(ring_matrix.to_cols_array_2d(), [0.3, 0.6, 1.0, ring_pulse]));
                    }
                }
            }

            // ── Flag poles (static geometry) ──
            let pole_color = [0.35, 0.32, 0.18, 1.0]; // brass/bronze
            let pole_cap = [0.45, 0.40, 0.22, 1.0];
            // UCF flag pole (port wall)
            let ucf_pole_z = 8.0;
            rock_instances.push(InstanceData::new(
                glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(0.08, 0.08, 3.2),
                    Quat::IDENTITY,
                    Vec3::new(-9.4, 3.85, ucf_pole_z - 1.5),
                ).to_cols_array_2d(), pole_color,
            ));
            // Pole cap (ornamental sphere)
            sphere_instances.push(InstanceData::new(
                glam::Mat4::from_scale_rotation_translation(
                    Vec3::splat(0.12),
                    Quat::IDENTITY,
                    Vec3::new(-9.4, 3.85, ucf_pole_z + 0.1),
                ).to_cols_array_2d(), pole_cap,
            ));
            // MI flag pole (starboard wall)
            rock_instances.push(InstanceData::new(
                glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(0.08, 0.08, 3.2),
                    Quat::IDENTITY,
                    Vec3::new(9.4, 3.85, ucf_pole_z - 1.5),
                ).to_cols_array_2d(), pole_color,
            ));
            sphere_instances.push(InstanceData::new(
                glam::Mat4::from_scale_rotation_translation(
                    Vec3::splat(0.12),
                    Quat::IDENTITY,
                    Vec3::new(9.4, 3.85, ucf_pole_z + 0.1),
                ).to_cols_array_2d(), pole_cap,
            ));

            // ── Cloth flags (physics-simulated) ──
            if let Some(ref ship) = state.ship_state {
                for flag in [&ship.ucf_flag, &ship.mi_flag] {
                    for (matrix, color) in flag.render_instances() {
                        rock_instances.push(InstanceData::new(matrix.to_cols_array_2d(), color));
                    }
                }
            }

            // ── Interior NPCs: Fleet, Mobile Infantry, Marauder, Rico (ST universe, lived-in tints) ──
            let npcs = roger_young_interior_npcs();
            for npc in &npcs {
                let rot = Quat::from_rotation_y(npc.facing_yaw_rad);
                let [tr, tg, tb] = npc.color_tint;
                for part in interior_npc_parts(npc.kind) {
                    let world_pos = npc.position + rot * part.local_offset;
                    let matrix = glam::Mat4::from_scale_rotation_translation(
                        part.scale,
                        rot,
                        world_pos,
                    );
                    let color = [
                        (part.color[0] * tr).min(1.0),
                        (part.color[1] * tg).min(1.0),
                        (part.color[2] * tb).min(1.0),
                        part.color[3],
                    ];
                    let inst = InstanceData::new(matrix.to_cols_array_2d(), color);
                    match part.mesh_type {
                        0 => rock_instances.push(inst),
                        1 => sphere_instances.push(inst),
                        2 => glow_instances.push(inst),
                        _ => {}
                    }
                }
            }

            // Render ship interior
            if !rock_instances.is_empty() {
                state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.environment_meshes.rock, &rock_instances);
            }
            if !sphere_instances.is_empty() {
                state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.environment_meshes.prop_sphere, &sphere_instances);
            }
            if !glow_instances.is_empty() {
                state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.flash_mesh, &glow_instances);
            }
        }

        // Pass 1: Terrain (only when on/near a planet)
        if state.current_planet_idx.is_some() {
            let terrain_sun_intensity = sun_dir.y.max(0.0).powf(0.3) * (1.0 - cloud_density * 0.4);
            let primary_biome = state.planet.primary_biome;
            let deform_enabled = matches!(
                primary_biome,
                BiomeType::Desert | BiomeType::Frozen | BiomeType::Wasteland | BiomeType::Badlands
            );
            let (deform_origin_x, deform_origin_z) = if deform_enabled {
                let ox = cam_pos.x;
                let oz = cam_pos.z;
                // Stamp deformation heightfield from ground tracks (Helldivers 2 / Dune style)
                state.deformation_buffer.fill(0.0);
                let world_size = 2.0 * DEFORM_HALF_SIZE;
                let texels_per_unit = (DEFORM_TEXTURE_SIZE as f32) / world_size;
                for track in &state.effects.ground_tracks {
                    let (radius, depth) = match track.kind {
                        TrackKind::TrooperFoot => (0.24, 0.07),
                        TrackKind::BugFoot => (0.5, 0.12),
                        TrackKind::ShovelDig => (0.6, 0.12),
                    };
                    let depth_fade = 1.0 - (track.age / 120.0).min(1.0) * 0.6; // older tracks shallower
                    let depth = depth * depth_fade;
                    let tx = track.position.x;
                    let tz = track.position.z;
                    let min_i = ((tx - radius - (ox - DEFORM_HALF_SIZE)) * texels_per_unit).floor() as i32;
                    let max_i = ((tx + radius - (ox - DEFORM_HALF_SIZE)) * texels_per_unit).ceil() as i32;
                    let min_j = ((tz - radius - (oz - DEFORM_HALF_SIZE)) * texels_per_unit).floor() as i32;
                    let max_j = ((tz + radius - (oz - DEFORM_HALF_SIZE)) * texels_per_unit).ceil() as i32;
                    for j in min_j..=max_j {
                        for i in min_i..=max_i {
                            if i < 0 || i >= DEFORM_TEXTURE_SIZE as i32 || j < 0 || j >= DEFORM_TEXTURE_SIZE as i32 {
                                continue;
                            }
                            let wx = ox - DEFORM_HALF_SIZE + (i as f32 + 0.5) / texels_per_unit;
                            let wz = oz - DEFORM_HALF_SIZE + (j as f32 + 0.5) / texels_per_unit;
                            let dx = wx - tx;
                            let dz = wz - tz;
                            let dist = (dx * dx + dz * dz).sqrt();
                            if dist < radius {
                                let t = (dist / radius).min(1.0);
                                let falloff = 1.0 - t * t * (3.0 - 2.0 * t); // smoothstep
                                let idx = i as usize + j as usize * (DEFORM_TEXTURE_SIZE as usize);
                                let new_val = state.deformation_buffer[idx] + depth * falloff;
                                state.deformation_buffer[idx] = new_val.min(0.18); // cap max depression
                            }
                        }
                    }
                }
                state.renderer.upload_terrain_deformation(&state.deformation_buffer);
                (ox, oz)
            } else {
                (0.0, 0.0)
            };
            state.renderer.update_terrain(
                state.time.elapsed_seconds(),
                [sun_dir.x, sun_dir.y, sun_dir.z, terrain_sun_intensity],
                fog_params,
                biome_colors,
                planet_radius,
                state.chunk_manager.chunk_size,
                deform_origin_x,
                deform_origin_z,
                deform_enabled,
            );
            state.chunk_manager.render_visible(
                &state.renderer,
                &mut encoder,
                &scene_view,
                &state.camera,
            );
        }

        // Pass 1a0: Squad drop pods descending from orbit — look up to see them coming from the fleet
        if state.current_planet_idx.is_some() && state.phase == GamePhase::Playing {
            if let Some(ref squad_drop) = state.squad_drop_pods {
                let mut pod_rock: Vec<InstanceData> = Vec::new();
                let mut pod_glow: Vec<InstanceData> = Vec::new();
                for pod in squad_drop.pods_visible() {
                    let dist_sq = pod.position.distance_squared(cam_pos);
                    if dist_sq > 600.0 * 600.0 {
                        continue;
                    }
                    let pod_color = [0.14, 0.15, 0.18, 1.0];
                    let m = glam::Mat4::from_scale_rotation_translation(
                        Vec3::new(0.5, 1.4, 0.5),
                        Quat::IDENTITY,
                        pod.position,
                    );
                    pod_rock.push(InstanceData::new(m.to_cols_array_2d(), pod_color));
                    let glow_pos = pod.position - Vec3::Y * 1.8;
                    let gm = glam::Mat4::from_scale_rotation_translation(
                        Vec3::new(0.6, 0.4, 0.6),
                        Quat::IDENTITY,
                        glow_pos,
                    );
                    pod_glow.push(InstanceData::new(gm.to_cols_array_2d(), [0.9, 0.55, 0.2, 0.85]));
                }
                if !pod_rock.is_empty() {
                    state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.environment_meshes.rock, &pod_rock);
                }
                if !pod_glow.is_empty() {
                    state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.flash_mesh, &pod_glow);
                }
            }
        }

        // Pass 1a: Drop pod real-time visuals (Corvette hull + planet sphere)
        if let Some(ref pod) = state.drop_pod {
            let pod_cam = state.camera.position();

            // ── Corvette hull panels during Detach ──
            if pod.phase == DropPhase::Detach {
                let sep = pod.corvette_separation;
                let hull_alpha = (1.0 - sep / 40.0).clamp(0.0, 1.0);
                if hull_alpha > 0.01 {
                    let hull_color = [0.12, 0.13, 0.16, hull_alpha];
                    let detail_color = [0.08, 0.09, 0.11, hull_alpha];
                    let mut hull_instances: Vec<InstanceData> = Vec::new();

                    // Main hull ceiling (directly above the pod)
                    let ceiling_y = pod_cam.y + 5.0 + sep;
                    let m = glam::Mat4::from_scale_rotation_translation(
                        Vec3::new(25.0, 1.5, 40.0), Quat::IDENTITY,
                        Vec3::new(pod_cam.x, ceiling_y, pod_cam.z),
                    );
                    hull_instances.push(InstanceData::new(m.to_cols_array_2d(), hull_color));

                    // Side panels (port and starboard)
                    for side in [-1.0f32, 1.0] {
                        let wall_x = pod_cam.x + side * 18.0;
                        let m = glam::Mat4::from_scale_rotation_translation(
                            Vec3::new(1.5, 8.0, 35.0), Quat::IDENTITY,
                            Vec3::new(wall_x, ceiling_y - 4.0, pod_cam.z),
                        );
                        hull_instances.push(InstanceData::new(m.to_cols_array_2d(), hull_color));
                    }

                    // Structural ribs (cross-beams)
                    for i in 0..4 {
                        let beam_z = pod_cam.z - 15.0 + i as f32 * 10.0;
                        let m = glam::Mat4::from_scale_rotation_translation(
                            Vec3::new(20.0, 0.8, 0.8), Quat::IDENTITY,
                            Vec3::new(pod_cam.x, ceiling_y - 1.0, beam_z),
                        );
                        hull_instances.push(InstanceData::new(m.to_cols_array_2d(), detail_color));
                    }

                    // Engine nacelle shapes at the rear
                    for side in [-1.0f32, 1.0] {
                        let m = glam::Mat4::from_scale_rotation_translation(
                            Vec3::new(3.0, 3.0, 12.0), Quat::IDENTITY,
                            Vec3::new(pod_cam.x + side * 22.0, ceiling_y + 2.0, pod_cam.z - 20.0),
                        );
                        hull_instances.push(InstanceData::new(m.to_cols_array_2d(), detail_color));
                    }

                    // Bay door lights (amber, flashing)
                    let flash = (state.time.elapsed_seconds() * 8.0).sin() * 0.5 + 0.5;
                    for i in 0..3 {
                        let light_z = pod_cam.z - 12.0 + i as f32 * 12.0;
                        let m = glam::Mat4::from_scale_rotation_translation(
                            Vec3::splat(0.6 + flash * 0.3), Quat::IDENTITY,
                            Vec3::new(pod_cam.x, ceiling_y - 2.0, light_z),
                        );
                        let light_color = [2.0 * flash, 1.2 * flash, 0.2, hull_alpha];
                        hull_instances.push(InstanceData::new(m.to_cols_array_2d(), light_color));
                    }

                    if !hull_instances.is_empty() {
                        state.renderer.render_instanced_load(
                            &mut encoder, &scene_view,
                            &state.environment_meshes.rock,
                            &hull_instances,
                        );
                    }
                }
            }

            // ── Planet sphere (visible during high-altitude phases) ──
            if pod.planet_visual_radius > 5.0 && pod.altitude > 300.0 {
                // The planet appears below the pod as a massive sphere
                let sphere_y = pod_cam.y - pod.planet_visual_radius * 0.7 - 40.0;
                let sphere_pos = Vec3::new(pod_cam.x, sphere_y, pod_cam.z + 10.0);

                // Base planet color from current biome
                let planet_color = match state.planet.primary_biome {
                    BiomeType::Desert | BiomeType::Badlands => [0.7, 0.55, 0.3, 1.0],
                    BiomeType::Volcanic | BiomeType::Ashlands => [0.35, 0.15, 0.08, 1.0],
                    BiomeType::Frozen => [0.75, 0.82, 0.9, 1.0],
                    BiomeType::Swamp | BiomeType::Jungle => [0.25, 0.4, 0.2, 1.0],
                    BiomeType::Crystalline => [0.4, 0.25, 0.5, 1.0],
                    BiomeType::Toxic | BiomeType::Wasteland => [0.35, 0.4, 0.2, 1.0],
                    _ => [0.35, 0.45, 0.3, 1.0],
                };

                let radius = pod.planet_visual_radius;
                let m = glam::Mat4::from_scale_rotation_translation(
                    Vec3::splat(radius),
                    Quat::from_rotation_y(state.time.elapsed_seconds() * 0.01),
                    sphere_pos,
                );
                let inst = vec![InstanceData::new(m.to_cols_array_2d(), planet_color)];
                state.renderer.render_instanced_load(
                    &mut encoder, &scene_view,
                    &state.environment_meshes.prop_sphere,
                    &inst,
                );

                // Atmosphere halo ring around the planet
                if pod.atmosphere_glow > 0.01 {
                    let halo_radius = radius * 1.08;
                    let glow = pod.atmosphere_glow;
                    let halo_m = glam::Mat4::from_scale_rotation_translation(
                        Vec3::new(halo_radius, halo_radius * 0.3, halo_radius),
                        Quat::from_rotation_x(0.3),
                        Vec3::new(sphere_pos.x, sphere_pos.y + radius * 0.6, sphere_pos.z),
                    );
                    let halo_color = [0.4 * glow, 0.6 * glow, 1.5 * glow, glow * 0.5];
                    let halo_inst = vec![InstanceData::new(halo_m.to_cols_array_2d(), halo_color)];
                    state.renderer.render_instanced_load(
                        &mut encoder, &scene_view,
                        &state.flash_mesh,
                        &halo_inst,
                    );
                }
            }

            // ── Atmospheric entry plasma glow (hot shield below the pod) ──
            if pod.atmosphere_glow > 0.1 && pod.phase == DropPhase::AtmosphericEntry {
                let glow = pod.atmosphere_glow;
                let glow_pos = pod_cam - Vec3::Y * 4.0;
                let glow_size = 3.0 + glow * 5.0;
                let m = glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(glow_size, glow_size * 0.3, glow_size),
                    Quat::IDENTITY,
                    glow_pos,
                );
                let heat_color = [3.0 * glow, 1.5 * glow, 0.3 * glow, glow * 0.8];
                let inst = vec![InstanceData::new(m.to_cols_array_2d(), heat_color)];
                state.renderer.render_instanced_load(
                    &mut encoder, &scene_view,
                    &state.flash_mesh,
                    &inst,
                );

                // Plasma trail streaks behind the pod
                for i in 0..5 {
                    let fi = i as f32;
                    let trail_y = pod_cam.y + 3.0 + fi * 6.0;
                    let spread = fi * 0.5;
                    let trail_pos = Vec3::new(
                        pod_cam.x + (fi * 2.3 + state.time.elapsed_seconds() * 5.0).sin() * spread,
                        trail_y,
                        pod_cam.z + (fi * 3.1 + state.time.elapsed_seconds() * 5.0).cos() * spread,
                    );
                    let trail_size = 1.5 + fi * 0.8;
                    let trail_alpha = glow * (1.0 - fi / 6.0);
                    let tm = glam::Mat4::from_scale_rotation_translation(
                        Vec3::new(trail_size * 0.5, trail_size * 2.0, trail_size * 0.5),
                        Quat::IDENTITY, trail_pos,
                    );
                    let trail_color = [2.0 * trail_alpha, 0.8 * trail_alpha, 0.15, trail_alpha * 0.6];
                    let trail_inst = vec![InstanceData::new(tm.to_cols_array_2d(), trail_color)];
                    state.renderer.render_instanced_load(
                        &mut encoder, &scene_view,
                        &state.flash_mesh,
                        &trail_inst,
                    );
                }
            }

            // ── Retro-rocket exhaust glow (below the pod during braking) ──
            if pod.retro_active && pod.phase == DropPhase::RetroBoost {
                let retro_pos = pod_cam - Vec3::Y * 3.0;
                let pulse = 0.8 + (state.time.elapsed_seconds() * 15.0).sin() * 0.2;
                let retro_size = 2.0 + pulse * 2.0;
                let m = glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(retro_size, retro_size * 1.5, retro_size),
                    Quat::IDENTITY, retro_pos,
                );
                let retro_color = [0.5 * pulse, 0.8 * pulse, 3.0 * pulse, 0.7];
                let inst = vec![InstanceData::new(m.to_cols_array_2d(), retro_color)];
                state.renderer.render_instanced_load(
                    &mut encoder, &scene_view,
                    &state.flash_mesh,
                    &inst,
                );
            }
        }

        // Pass 1b-1l: All static environment entities via cached render data.
        // Query CachedRenderData only so destructibles, landmarks, and hazards (no Destructible) are all included.
        let mut env_instances: [Vec<InstanceData>; ENV_MESH_GROUP_COUNT] = Default::default();
        for (entity, (cached,)) in state.world.query::<(&CachedRenderData,)>().iter() {
            if let Ok(d) = state.world.get::<&Destructible>(entity) {
                if d.health <= 0.0 {
                    continue;
                }
            }
            let pos = Vec3::new(cached.matrix[3][0], cached.matrix[3][1], cached.matrix[3][2]);
            let dist_sq = pos.distance_squared(cam_pos);
            if dist_sq < VIEWMODEL_CULL_SQ || dist_sq > ENTITY_RENDER_DIST_SQ {
                continue;
            }
            let group = cached.mesh_group as usize;
            if group < ENV_MESH_GROUP_COUNT {
                env_instances[group].push(InstanceData::new(cached.matrix, cached.color));
            }
        }
        // Mesh group 0: rock (3 variants by position hash)
        {
            let rocks = [
                &state.environment_meshes.rock,
                &state.environment_meshes.rock_chunk,
                &state.environment_meshes.rock_boulder,
            ];
            let mut rock_by_variant: [Vec<InstanceData>; 3] = Default::default();
            for inst in &env_instances[MESH_GROUP_ROCK as usize] {
                let h = ((inst.model[3][0].to_bits().wrapping_add(inst.model[3][2].to_bits())) % 3) as usize;
                rock_by_variant[h].push(*inst);
            }
            for (variant, instances) in rock_by_variant.iter().enumerate() {
                if !instances.is_empty() {
                    state.renderer.render_instanced_load(&mut encoder, &scene_view, rocks[variant], instances);
                }
            }
        }
        // Mesh group 1: bug_hole (holes, hazard pools, burn craters)
        if !env_instances[MESH_GROUP_BUG_HOLE as usize].is_empty() {
            state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.environment_meshes.bug_hole, &env_instances[MESH_GROUP_BUG_HOLE as usize]);
        }
        // Mesh group 2: hive_mound (hive structures, spore towers)
        if !env_instances[MESH_GROUP_HIVE_MOUND as usize].is_empty() {
            state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.environment_meshes.hive_mound, &env_instances[MESH_GROUP_HIVE_MOUND as usize]);
        }
        // Mesh group 3: egg_cluster (egg clusters, bone piles)
        if !env_instances[MESH_GROUP_EGG_CLUSTER as usize].is_empty() {
            state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.environment_meshes.egg_cluster, &env_instances[MESH_GROUP_EGG_CLUSTER as usize]);
        }
        // Mesh group 4: prop_sphere (environment props)
        if !env_instances[MESH_GROUP_PROP_SPHERE as usize].is_empty() {
            state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.environment_meshes.prop_sphere, &env_instances[MESH_GROUP_PROP_SPHERE as usize]);
        }
        // Mesh group 5: cube (abandoned outposts)
        if !env_instances[MESH_GROUP_CUBE as usize].is_empty() {
            state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.environment_meshes.cube, &env_instances[MESH_GROUP_CUBE as usize]);
        }
        // Mesh group 6: landmark (biome-unique landmarks; rock variants)
        {
            let rocks = [
                &state.environment_meshes.rock,
                &state.environment_meshes.rock_chunk,
                &state.environment_meshes.rock_boulder,
            ];
            let mut landmark_by_variant: [Vec<InstanceData>; 3] = Default::default();
            for inst in &env_instances[MESH_GROUP_LANDMARK as usize] {
                let h = ((inst.model[3][0].to_bits().wrapping_add(inst.model[3][2].to_bits())) % 3) as usize;
                landmark_by_variant[h].push(*inst);
            }
            for (variant, instances) in landmark_by_variant.iter().enumerate() {
                if !instances.is_empty() {
                    state.renderer.render_instanced_load(&mut encoder, &scene_view, rocks[variant], instances);
                }
            }
        }
        // Mesh group 7: hazard (environmental hazards; use prop_sphere for disc/zone look)
        if !env_instances[MESH_GROUP_HAZARD as usize].is_empty() {
            state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.environment_meshes.prop_sphere, &env_instances[MESH_GROUP_HAZARD as usize]);
        }

        // Pass 1l2: Destruction debris (Tac Fighter / explosion flying chunks)
        {
            let mut debris_instances: Vec<InstanceData> = Vec::new();
            let debris_color = [0.22, 0.18, 0.14, 1.0];
            for (_, (transform, _, lifetime)) in state.world.query::<(&Transform, &Debris, &Lifetime)>().iter() {
                if lifetime.remaining <= 0.0 { continue; }
                let dist_sq = transform.position.distance_squared(cam_pos);
                if dist_sq < VIEWMODEL_CULL_SQ || dist_sq > ENTITY_RENDER_DIST_SQ { continue; }
                let t = transform.to_matrix();
                debris_instances.push(InstanceData::new(t.to_cols_array_2d(), debris_color));
            }
            if !debris_instances.is_empty() {
                state.renderer.render_instanced_load(
                    &mut encoder,
                    &scene_view,
                    &state.environment_meshes.rock,
                    &debris_instances,
                );
            }
        }

        // Pass 1l2b: Bug gore chunks (flying guts, dismemberment — Euphoria-style)
        {
            let mut gore_chunk_instances: Vec<InstanceData> = Vec::new();
            for (_, (transform, chunk, lifetime)) in state.world.query::<(&Transform, &BugGoreChunk, &Lifetime)>().iter() {
                if lifetime.remaining <= 0.0 { continue; }
                let dist_sq = transform.position.distance_squared(cam_pos);
                if dist_sq < VIEWMODEL_CULL_SQ || dist_sq > ENTITY_RENDER_DIST_SQ { continue; }
                let alpha = (lifetime.remaining / 1.5).min(1.0);
                let mut color = chunk.color;
                color[3] *= alpha;
                let t = transform.to_matrix();
                gore_chunk_instances.push(InstanceData::new(t.to_cols_array_2d(), color));
            }
            if !gore_chunk_instances.is_empty() {
                state.renderer.render_instanced_load(
                    &mut encoder,
                    &scene_view,
                    &state.environment_meshes.prop_sphere,
                    &gore_chunk_instances,
                );
            }
        }

        // Pass 1m: Bug corpses (decaying dead bugs)
        {
            let mut corpse_instances_by_type: HashMap<u8, Vec<InstanceData>> = HashMap::new();
            for (_, (transform, corpse)) in state.world.query::<(&Transform, &BugCorpse)>().iter() {
                let dist_sq = transform.position.distance_squared(cam_pos);
                if dist_sq > VIEWMODEL_CULL_SQ && dist_sq < ENTITY_RENDER_DIST_SQ {
                    let (color, scale_mult, sink, _) = corpse.decay_state();
                    let mut pos = transform.position;
                    pos.y -= sink; // sink into ground
                    let mat = glam::Mat4::from_scale_rotation_translation(
                        corpse.original_scale * scale_mult,
                        transform.rotation,
                        pos,
                    );
                    corpse_instances_by_type.entry(corpse.bug_type_idx)
                        .or_default()
                        .push(InstanceData::new(mat.to_cols_array_2d(), color));
                }
            }
            for (type_idx, instances) in &corpse_instances_by_type {
                if instances.is_empty() { continue; }
                let bug_type = match type_idx {
                    0 => BugType::Warrior,
                    1 => BugType::Charger,
                    2 => BugType::Spitter,
                    3 => BugType::Tanker,
                    4 => BugType::Hopper,
                    _ => BugType::Warrior,
                };
                let mesh = state.bug_meshes.get(bug_type);
                state.renderer.render_instanced_load(
                    &mut encoder,
                    &scene_view,
                    mesh,
                    instances,
                );
            }
        }

        // Pass 2: Bugs (each type with its correct mesh)
        for bug_type in [BugType::Warrior, BugType::Charger, BugType::Spitter, BugType::Tanker, BugType::Hopper] {
            let instances = &bug_instances_by_type[&bug_type];
            if instances.is_empty() {
                continue;
            }
            let mesh = state.bug_meshes.get(bug_type);
            state.renderer.render_instanced_load(
                &mut encoder,
                &scene_view,
                mesh,
                instances,
            );
        }

        // Pass 2a: Skinnies (Heinlein humanoid enemies — tall thin shape, grey-green)
        let mut skinny_instances: Vec<InstanceData> = Vec::new();
        for (_, (transform, skinny, health, physics_bug)) in
            state.world.query::<(&Transform, &Skinny, &Health, &PhysicsBug)>().iter()
        {
            let dist_sq = transform.position.distance_squared(cam_pos);
            if dist_sq < VIEWMODEL_CULL_SQ || dist_sq > BUG_RENDER_DIST_SQ {
                continue;
            }
            let health_factor = health.current / health.max;
            let mut color = skinny.skinny_type.color();
            if physics_bug.is_ragdoll {
                color[0] *= 0.4;
                color[1] *= 0.4;
                color[2] *= 0.4;
            } else {
                color[0] *= 0.5 + health_factor * 0.5;
                color[1] *= 0.5 + health_factor * 0.5;
                color[2] *= 0.5 + health_factor * 0.5;
            }
            let final_transform = if physics_bug.is_ragdoll {
                let (_death_offset, death_rotation, death_scale) = physics_bug.get_death_animation();
                glam::Mat4::from_scale_rotation_translation(
                    transform.scale * death_scale,
                    transform.rotation * death_rotation,
                    transform.position,
                )
            } else {
                transform.to_matrix()
            };
            skinny_instances.push(InstanceData::new(final_transform.to_cols_array_2d(), color));
        }
        if !skinny_instances.is_empty() {
            state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.environment_meshes.rock, &skinny_instances);
        }

        // Pass 2b: Squad mates (deployed with player — simple head + torso)
        let mut squad_rock: Vec<InstanceData> = Vec::new();
        let mut squad_sphere: Vec<InstanceData> = Vec::new();
        for (_, (transform, squad, health)) in state.world.query::<(&Transform, &SquadMate, &Health)>().iter() {
            if health.current <= 0.0 {
                continue;
            }
            let dist_sq = transform.position.distance_squared(cam_pos);
            if dist_sq < VIEWMODEL_CULL_SQ || dist_sq > BUG_RENDER_DIST_SQ {
                continue;
            }
            let (head_color, torso_color) = match squad.kind {
                SquadMateKind::Fleet => ([0.38, 0.40, 0.44, 1.0], [0.16, 0.18, 0.22, 1.0]),
                SquadMateKind::MobileInfantry => ([0.42, 0.36, 0.28, 1.0], [0.38, 0.32, 0.26, 1.0]),
                SquadMateKind::Marauder => ([0.18, 0.17, 0.16, 1.0], [0.15, 0.14, 0.13, 1.0]),
                SquadMateKind::Tech => ([0.35, 0.42, 0.48, 1.0], [0.12, 0.20, 0.28, 1.0]), // Tech blue-gray
            };
            let head_pos = transform.position + transform.rotation * Vec3::new(0.0, 1.5, 0.0);
            let torso_pos = transform.position + transform.rotation * Vec3::new(0.0, 0.9, 0.0);
            let head_m = glam::Mat4::from_scale_rotation_translation(
                Vec3::splat(0.22),
                transform.rotation,
                head_pos,
            );
            let torso_m = glam::Mat4::from_scale_rotation_translation(
                Vec3::new(0.28, 0.4, 0.14),
                transform.rotation,
                torso_pos,
            );
            squad_sphere.push(InstanceData::new(head_m.to_cols_array_2d(), head_color));
            squad_rock.push(InstanceData::new(torso_m.to_cols_array_2d(), torso_color));
        }
        if !squad_rock.is_empty() {
            state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.environment_meshes.rock, &squad_rock);
        }
        if !squad_sphere.is_empty() {
            state.renderer.render_instanced_load(&mut encoder, &scene_view, &state.environment_meshes.prop_sphere, &squad_sphere);
        }

        // Pass 3: Gore splatters
        if !gore_instances.is_empty() {
            state.renderer.render_instanced_load(
                &mut encoder,
                &scene_view,
                &state.gore_mesh,
                &gore_instances,
            );
        }

        // Pass 3b: Ground tracks (footprints in snow/sand)
        if !track_instances.is_empty() {
            state.renderer.render_instanced_load(
                &mut encoder,
                &scene_view,
                &state.gore_mesh,
                &track_instances,
            );
        }

        // Pass 4: Bullet impacts
        if !impact_instances.is_empty() {
            state.renderer.render_instanced_load(
                &mut encoder,
                &scene_view,
                &state.particle_mesh,
                &impact_instances,
            );
        }

        // Pass 4b: Tracer projectiles (proper bullet-shaped diamond mesh)
        if !tracer_instances.is_empty() {
            state.renderer.render_instanced_load(
                &mut encoder,
                &scene_view,
                &state.tracer_mesh,
                &tracer_instances,
            );
        }

        // Pass 5: Muzzle flashes (multi-pointed star mesh)
        if !flash_instances.is_empty() {
            state.renderer.render_instanced_load(
                &mut encoder,
                &scene_view,
                &state.flash_mesh,
                &flash_instances,
            );
        }

        // Pass 5b: Rain (tiny elongated spheres falling)
        if !state.rain_drops.is_empty() && state.player.is_alive {
            let mut rain_instances: Vec<InstanceData> = Vec::new();
            for r in &state.rain_drops {
                let alpha = (r.life / 1.0).min(1.0);
                if alpha < 0.2 { continue; }
                // Elongated vertically for a raindrop streak effect
                let matrix = glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(0.015, 0.12, 0.015), // thin and tall
                    Quat::IDENTITY,
                    r.position,
                );
                rain_instances.push(InstanceData::new(
                    matrix.to_cols_array_2d(),
                    [0.65, 0.75, 0.95, alpha],
                ));
            }
            if !rain_instances.is_empty() {
                state.renderer.render_instanced_load(
                    &mut encoder,
                    &scene_view,
                    &state.environment_meshes.prop_sphere,
                    &rain_instances,
                );
            }
        }

        // Pass 5c: Ambient dust particles (tiny sphere specks only)
        // NOTE: Billboard quads can't render semi-transparent on the opaque pipeline,
        // so all dust renders as tiny sphere specks instead.
        if !state.ambient_dust.particles.is_empty() && state.phase == GamePhase::Playing {
            let mut dust_instances: Vec<InstanceData> = Vec::new();

            for p in &state.ambient_dust.particles {
                let life_frac = (p.life / 4.0).min(1.0);
                let alpha = life_frac * 0.8; // Higher alpha for opaque pipeline
                if alpha < 0.2 { continue; }
                // Slight shimmer: modulate size with a slow sine based on life
                let shimmer = 1.0 + (p.life * 3.0).sin() * 0.15;
                let size = p.size * shimmer;
                // Warm dust color with slight variation per particle
                let tint = (p.position.x * 7.3 + p.position.z * 11.1).sin() * 0.05;
                let color = [0.82 + tint, 0.76 + tint, 0.65, alpha];

                // All dust renders as tiny spheres (opaque-friendly)
                let matrix = glam::Mat4::from_scale_rotation_translation(
                    Vec3::splat(size),
                    Quat::IDENTITY,
                    p.position,
                );
                dust_instances.push(InstanceData::new(matrix.to_cols_array_2d(), color));
            }

            if !dust_instances.is_empty() {
                state.renderer.render_instanced_load(
                    &mut encoder,
                    &scene_view,
                    &state.environment_meshes.prop_sphere,
                    &dust_instances,
                );
            }
        }

        // Pass 5d: Biome atmosphere particles (only small opaque-friendly types)
        // NOTE: Large translucent particles (fog banks, mist, toxic gas, god rays) are
        // NOT rendered here because the pipeline uses BlendState::REPLACE (no alpha blending).
        // They still influence the scene via fog_params and biome_colors passed to shaders.
        // Only small bright particles (embers, fireflies, sparkles, sand, spores, ash, ice)
        // are rendered as tiny opaque specs.
        if !state.biome_atmosphere.particles.is_empty() && state.phase == GamePhase::Playing {
            let cam_pos = state.camera.position();
            let mut sphere_insts: Vec<InstanceData> = Vec::new();
            let mut flash_insts: Vec<InstanceData> = Vec::new();

            let atmo_cull_dist_sq: f32 = 80.0 * 80.0;

            for p in &state.biome_atmosphere.particles {
                let dist_sq = (p.position - cam_pos).length_squared();
                if dist_sq > atmo_cull_dist_sq { continue; }
                if p.color[3] < 0.15 { continue; } // Skip low-alpha particles

                match p.kind {
                    // Skip all large translucent types - they can't render on opaque pipeline
                    AtmoParticleKind::FogBank
                    | AtmoParticleKind::ToxicGas
                    | AtmoParticleKind::MistTendril
                    | AtmoParticleKind::GodRay => { continue; }

                    // Small sphere particles: embers, fireflies, sand grains
                    AtmoParticleKind::Ember
                    | AtmoParticleKind::Firefly
                    | AtmoParticleKind::SandGrain => {
                        let matrix = glam::Mat4::from_scale_rotation_translation(
                            p.size,
                            Quat::IDENTITY,
                            p.position,
                        );
                        let glow_mult = match p.kind {
                            AtmoParticleKind::Ember => 3.0,
                            AtmoParticleKind::Firefly => 4.0,
                            _ => 1.0,
                        };
                        let color = [
                            p.color[0] * glow_mult,
                            p.color[1] * glow_mult,
                            p.color[2] * glow_mult,
                            p.color[3],
                        ];
                        sphere_insts.push(InstanceData::new(matrix.to_cols_array_2d(), color));
                    }

                    // Small opaque-ish particles rendered as tiny spheres
                    AtmoParticleKind::Spore
                    | AtmoParticleKind::Ash
                    | AtmoParticleKind::IceCrystal
                    | AtmoParticleKind::RadiationSpark => {
                        let matrix = glam::Mat4::from_scale_rotation_translation(
                            p.size,
                            Quat::IDENTITY,
                            p.position,
                        );
                        let color = [p.color[0], p.color[1], p.color[2], p.color[3]];
                        sphere_insts.push(InstanceData::new(matrix.to_cols_array_2d(), color));
                    }

                    // Crystal sparkles use the flash mesh
                    AtmoParticleKind::CrystalSparkle => {
                        let spin_rot = Quat::from_rotation_y(p.phase * 5.0)
                            * Quat::from_rotation_x(p.phase * 3.7);
                        let matrix = glam::Mat4::from_scale_rotation_translation(
                            p.size,
                            spin_rot,
                            p.position,
                        );
                        let color = [
                            p.color[0] * 5.0,
                            p.color[1] * 5.0,
                            p.color[2] * 5.0,
                            p.color[3],
                        ];
                        flash_insts.push(InstanceData::new(matrix.to_cols_array_2d(), color));
                    }
                }
            }

            // Draw sphere atmosphere particles (embers, fireflies, spores, ash, ice, sand)
            if !sphere_insts.is_empty() {
                state.renderer.render_instanced_load(
                    &mut encoder,
                    &scene_view,
                    &state.environment_meshes.prop_sphere,
                    &sphere_insts,
                );
            }
            // Draw flash/sparkle atmosphere particles
            if !flash_insts.is_empty() {
                state.renderer.render_instanced_load(
                    &mut encoder,
                    &scene_view,
                    &state.flash_mesh,
                    &flash_insts,
                );
            }
        }

        // Pass 5e: Smoke grenade clouds (dense red 2D billboard particles)
        {
            let mut smoke_instances: Vec<InstanceData> = Vec::new();
            let cam_fwd = state.camera.forward();
            let cam_right = cam_fwd.cross(Vec3::Y).normalize_or_zero();
            let cam_up = cam_right.cross(cam_fwd).normalize_or_zero();

            // In-flight grenades (small grey sphere)
            for grenade in &state.smoke_grenades {
                let dist_sq = grenade.position.distance_squared(cam_pos);
                if dist_sq > EFFECT_RENDER_DIST_SQ { continue; }
                let matrix = glam::Mat4::from_scale_rotation_translation(
                    Vec3::splat(0.08),
                    Quat::IDENTITY,
                    grenade.position,
                );
                let color = [0.3, 0.3, 0.3, 1.0]; // dark grey metal
                smoke_instances.push(InstanceData::new(matrix.to_cols_array_2d(), color));
            }

            // Smoke cloud particles (red billboard quads)
            for cloud in &state.smoke_clouds {
                for p in &cloud.particles {
                    let dist_sq = p.position.distance_squared(cam_pos);
                    if dist_sq > EFFECT_RENDER_DIST_SQ { continue; }

                    let life_frac = p.life / p.max_life;
                    // Skip very faded particles
                    if life_frac < 0.05 { continue; }

                    // Red smoke color with variation
                    let red_vary = (p.phase * 3.0).sin() * 0.1;
                    let alpha = life_frac.powf(0.6); // fade out gradually
                    // Only render if opaque enough for the discard threshold
                    if alpha < 0.2 { continue; }

                    let color = [
                        0.75 + red_vary,    // strong red
                        0.08 + red_vary * 0.3,
                        0.05,
                        alpha,
                    ];

                    // Billboard: face the camera
                    let right = cam_right * p.size;
                    let up = cam_up * p.size;
                    let billboard_rot = glam::Mat4::from_cols(
                        right.extend(0.0),
                        up.extend(0.0),
                        (cam_fwd * p.size).extend(0.0),
                        p.position.extend(1.0),
                    );
                    smoke_instances.push(InstanceData::new(billboard_rot.to_cols_array_2d(), color));
                }
            }

            if !smoke_instances.is_empty() {
                state.renderer.render_instanced_load(
                    &mut encoder,
                    &scene_view,
                    &state.billboard_mesh,
                    &smoke_instances,
                );
            }

            // Pass 5e2: Tac Fighter explosion particles (flat billboard fire/smoke, same look as red smoke)
            let mut explosion_instances: Vec<InstanceData> = Vec::new();
            for p in &state.effects.explosion_particles {
                let dist_sq = p.position.distance_squared(cam_pos);
                if dist_sq > EFFECT_RENDER_DIST_SQ { continue; }
                let life_frac = p.life / p.max_life;
                if life_frac < 0.05 { continue; }
                let alpha = life_frac.powf(0.5);
                if alpha < 0.15 { continue; }
                let vary = (p.phase + p.life * 2.0).sin() * 0.08;
                let (r, g, b) = match p.kind {
                    0 => (1.0, 0.85 + vary, 0.2 + vary),   // fire core
                    1 => (0.95, 0.5 + vary, 0.1),          // orange
                    _ => (0.2 + vary, 0.18, 0.15),         // dark smoke
                };
                let color = [r, g, b, alpha];
                let right = cam_right * p.size;
                let up = cam_up * p.size;
                let billboard_rot = glam::Mat4::from_cols(
                    right.extend(0.0),
                    up.extend(0.0),
                    (cam_fwd * p.size).extend(0.0),
                    p.position.extend(1.0),
                );
                explosion_instances.push(InstanceData::new(billboard_rot.to_cols_array_2d(), color));
            }
            if !explosion_instances.is_empty() {
                state.renderer.render_instanced_load(
                    &mut encoder,
                    &scene_view,
                    &state.billboard_mesh,
                    &explosion_instances,
                );
            }
        }

        // Pass 5f: Tac Fighter fleet (angular fuselage shape — cube reads as fighter, not lumpy rock)
        for fighter in &state.tac_fighters {
            let dist_sq = fighter.position.distance_squared(cam_pos);
            if dist_sq < 500.0 * 500.0 {
                let fwd = fighter.velocity.normalize_or_zero();
                let scale = Vec3::new(2.5, 0.6, 5.0); // Sleek fuselage: long, narrow, flat
                let rotation = if fwd.length_squared() > 0.01 {
                    Quat::from_rotation_arc(Vec3::Z, fwd)
                } else {
                    Quat::IDENTITY
                };
                let matrix = glam::Mat4::from_scale_rotation_translation(
                    scale,
                    rotation,
                    fighter.position,
                );
                let color = [0.18, 0.20, 0.24, 1.0]; // Dark military grey
                let instances = vec![InstanceData::new(matrix.to_cols_array_2d(), color)];
                state.renderer.render_instanced_load(
                    &mut encoder,
                    &scene_view,
                    &state.environment_meshes.cube,
                    &instances,
                );
                let exhaust_pos = fighter.position - fwd * 4.0;
                let exhaust_matrix = glam::Mat4::from_scale_rotation_translation(
                    Vec3::splat(1.2),
                    Quat::IDENTITY,
                    exhaust_pos,
                );
                let exhaust_color = [3.0, 1.5, 0.3, 1.0];
                let exhaust_instances = vec![InstanceData::new(exhaust_matrix.to_cols_array_2d(), exhaust_color)];
                state.renderer.render_instanced_load(
                    &mut encoder,
                    &scene_view,
                    &state.flash_mesh,
                    &exhaust_instances,
                );
            }
        }

        // Pass 5g: Tac bombs (falling ordnance)
        if !state.tac_bombs.is_empty() {
            let mut bomb_instances: Vec<InstanceData> = Vec::new();
            for bomb in &state.tac_bombs {
                let dist_sq = bomb.position.distance_squared(cam_pos);
                if dist_sq > EFFECT_RENDER_DIST_SQ { continue; }
                // Orient bomb along velocity
                let fwd = bomb.velocity.normalize_or_zero();
                let rotation = if fwd.length_squared() > 0.01 {
                    Quat::from_rotation_arc(Vec3::Y, -fwd) // nose down
                } else {
                    Quat::IDENTITY
                };
                let matrix = glam::Mat4::from_scale_rotation_translation(
                    Vec3::new(0.3, 1.2, 0.3), // long thin bomb shape
                    rotation,
                    bomb.position,
                );
                let color = [0.15, 0.15, 0.18, 1.0]; // dark ordnance
                bomb_instances.push(InstanceData::new(matrix.to_cols_array_2d(), color));
            }
            if !bomb_instances.is_empty() {
                state.renderer.render_instanced_load(
                    &mut encoder,
                    &scene_view,
                    &state.environment_meshes.prop_sphere, // cylindrical-ish shape
                    &bomb_instances,
                );
            }
        }

        // Pass 5g2: Artillery shells (orbital barrage — glowing red-hot, visible from orbit)
        if !state.artillery_shells.is_empty() {
            const ARTILLERY_RENDER_DIST_SQ: f32 = 450.0 * 450.0; // shells high in sky, need long range
            let mut shell_instances: Vec<InstanceData> = Vec::new();
            for shell in &state.artillery_shells {
                let dist_sq = shell.position.distance_squared(cam_pos);
                if dist_sq > ARTILLERY_RENDER_DIST_SQ { continue; }
                // Red-hot incandescent glow — emissive (max_channel > 1.5) so visible when looking up
                let color = [2.8, 0.35, 0.08, 1.0]; // glowing red-hot artillery shell
                let size = 5.0; // large so visible streaking from orbit to ground
                let matrix = glam::Mat4::from_scale_rotation_translation(
                    Vec3::splat(size),
                    Quat::IDENTITY,
                    shell.position,
                );
                shell_instances.push(InstanceData::new(matrix.to_cols_array_2d(), color));
            }
            if !shell_instances.is_empty() {
                state.renderer.render_instanced_load(
                    &mut encoder,
                    &scene_view,
                    &state.flash_mesh, // multi-pointed star = bright glowing point
                    &shell_instances,
                );
            }
        }

        // Pass 5g2b: Artillery shell trails (smoke/fire streak behind each shell)
        if !state.artillery_trail_particles.is_empty() {
            const ARTILLERY_TRAIL_RENDER_DIST_SQ: f32 = 450.0 * 450.0;
            let mut trail_instances: Vec<InstanceData> = Vec::new();
            let cam_fwd = state.camera.forward();
            let cam_right = cam_fwd.cross(Vec3::Y).normalize_or_zero();
            let cam_up = cam_right.cross(cam_fwd).normalize_or_zero();
            for p in &state.artillery_trail_particles {
                let dist_sq = p.position.distance_squared(cam_pos);
                if dist_sq > ARTILLERY_TRAIL_RENDER_DIST_SQ { continue; }
                let life_frac = p.life / p.max_life;
                if life_frac < 0.05 { continue; }
                let alpha = life_frac.powf(0.5);
                if alpha < 0.15 { continue; }
                // Mix of dark smoke and orange/red ember
                let vary = (p.phase + p.life * 2.0).sin() * 0.08;
                let (r, g, b) = if life_frac > 0.6 {
                    (0.9 + vary, 0.35 + vary, 0.1)  // hot orange near shell
                } else if life_frac > 0.3 {
                    (0.5 + vary, 0.2 + vary, 0.08)  // cooling ember
                } else {
                    (0.25 + vary, 0.2 + vary, 0.18)  // dark smoke
                };
                let color = [r, g, b, alpha];
                let right = cam_right * p.size;
                let up = cam_up * p.size;
                let billboard_rot = glam::Mat4::from_cols(
                    right.extend(0.0),
                    up.extend(0.0),
                    (cam_fwd * p.size).extend(0.0),
                    p.position.extend(1.0),
                );
                trail_instances.push(InstanceData::new(billboard_rot.to_cols_array_2d(), color));
            }
            if !trail_instances.is_empty() {
                state.renderer.render_instanced_load(
                    &mut encoder,
                    &scene_view,
                    &state.billboard_mesh,
                    &trail_instances,
                );
            }
        }

        // Pass 5g2: Grounded small-arms shell casings (persistent, weapon-specific)
        if !state.grounded_shell_casings.is_empty() {
            use crate::viewmodel::ShellCasingType;
            const SHELL_RENDER_DIST_SQ: f32 = 80.0 * 80.0;
            let mut shell_instances: Vec<InstanceData> = Vec::new();
            for s in &state.grounded_shell_casings {
                let dist_sq = s.position.distance_squared(cam_pos);
                if dist_sq > SHELL_RENDER_DIST_SQ { continue; }
                let color = match s.shell_type {
                    ShellCasingType::Rifle => [0.55, 0.48, 0.18, 1.0],
                    ShellCasingType::Shotgun => [0.58, 0.12, 0.10, 1.0],  // Faded red plastic
                    ShellCasingType::Sniper => [0.52, 0.42, 0.16, 1.0],
                    ShellCasingType::MachineGun => [0.54, 0.46, 0.20, 1.0],
                    ShellCasingType::Rocket => [0.28, 0.26, 0.24, 1.0],
                    ShellCasingType::Flamethrower => [0.20, 0.22, 0.18, 1.0],
                };
                let matrix = glam::Mat4::from_scale_rotation_translation(
                    s.scale,
                    s.rotation,
                    s.position,
                );
                shell_instances.push(InstanceData::new(matrix.to_cols_array_2d(), color));
            }
            if !shell_instances.is_empty() {
                state.renderer.render_instanced_load(
                    &mut encoder,
                    &scene_view,
                    &state.environment_meshes.cube,
                    &shell_instances,
                );
            }
        }

        // Pass 5g2a: Grounded artillery shells (Helldivers 2 style — shells on ground at impact sites)
        if !state.grounded_artillery_shells.is_empty() {
            const SHELL_RENDER_DIST_SQ: f32 = 120.0 * 120.0;
            let mut shell_instances: Vec<InstanceData> = Vec::new();
            for s in &state.grounded_artillery_shells {
                let dist_sq = s.position.distance_squared(cam_pos);
                if dist_sq > SHELL_RENDER_DIST_SQ { continue; }
                let matrix = glam::Mat4::from_scale_rotation_translation(
                    s.scale,
                    s.rotation,
                    s.position,
                );
                // Dark metallic artillery shell (burnt/oxidized)
                let color = [0.22, 0.20, 0.18, 1.0];
                shell_instances.push(InstanceData::new(matrix.to_cols_array_2d(), color));
            }
            if !shell_instances.is_empty() {
                state.renderer.render_instanced_load(
                    &mut encoder,
                    &scene_view,
                    &state.environment_meshes.cube,
                    &shell_instances,
                );
            }
        }

        // Pass 5g2: Supply drop crates (stratagem)
        if !state.supply_crates.is_empty() {
            let mut crate_instances: Vec<InstanceData> = Vec::new();
            for c in &state.supply_crates {
                if c.used { continue; }
                let dist_sq = c.position.distance_squared(cam_pos);
                if dist_sq > EFFECT_RENDER_DIST_SQ { continue; }
                let matrix = glam::Mat4::from_scale_rotation_translation(
                    Vec3::splat(0.8), // small box
                    Quat::IDENTITY,
                    c.position,
                );
                let color = [0.2, 0.5, 0.25, 1.0]; // green crate
                crate_instances.push(InstanceData::new(matrix.to_cols_array_2d(), color));
            }
            if !crate_instances.is_empty() {
                state.renderer.render_instanced_load(
                    &mut encoder,
                    &scene_view,
                    &state.environment_meshes.prop_sphere,
                    &crate_instances,
                );
            }
        }

        // Pass 5h: Extraction dropship (big retrieval boat)
        if let Some(ref dropship) = state.extraction {
            // Only render when visible (Inbound phase onward)
            if dropship.phase != ExtractionPhase::Called {
                let dist_sq = dropship.position.distance_squared(cam_pos);
                // When climbing/docking: visible from surface in real time (boat at 3000m+)
                let render_dist_sq = if dropship.roger_young_visible() {
                    4500.0 * 4500.0
                } else {
                    800.0 * 800.0
                };
                if dist_sq < render_dist_sq {
                    // ── Main hull: big, chunky transport ──
                    let vel = dropship.velocity;
                    let fwd = if vel.length_squared() > 1.0 {
                        vel.normalize()
                    } else {
                        -dropship.approach_dir // face away from approach when hovering
                    };
                    let right = fwd.cross(Vec3::Y).normalize_or_zero();
                    let up = right.cross(fwd).normalize_or_zero();
                    let rotation = if fwd.length_squared() > 0.01 {
                        Quat::from_rotation_arc(Vec3::Z, Vec3::new(fwd.x, 0.0, fwd.z).normalize_or_zero())
                    } else {
                        Quat::IDENTITY
                    };
                    // Big transport: wider and taller than the tac fighter
                    let hull_scale = Vec3::new(6.0, 3.0, 14.0);
                    let hull_matrix = glam::Mat4::from_scale_rotation_translation(
                        hull_scale, rotation, dropship.position,
                    );
                    // Military olive drab hull
                    let hull_color = [0.22, 0.24, 0.18, 1.0];
                    let hull_inst = vec![InstanceData::new(hull_matrix.to_cols_array_2d(), hull_color)];
                    state.renderer.render_instanced_load(
                        &mut encoder, &scene_view,
                        &state.environment_meshes.rock, // angular chunky shape
                        &hull_inst,
                    );

                    // ── Twin engine pods (left + right) ──
                    let engine_offset = 5.0;
                    let engine_scale = Vec3::new(1.5, 1.5, 4.0);
                    for side in [-1.0f32, 1.0] {
                        let eng_pos = dropship.position + right * (side * engine_offset) - Vec3::Y * 0.5;
                        let eng_matrix = glam::Mat4::from_scale_rotation_translation(
                            engine_scale, rotation, eng_pos,
                        );
                        let eng_color = [0.18, 0.18, 0.20, 1.0];
                        let eng_inst = vec![InstanceData::new(eng_matrix.to_cols_array_2d(), eng_color)];
                        state.renderer.render_instanced_load(
                            &mut encoder, &scene_view,
                            &state.environment_meshes.prop_sphere,
                            &eng_inst,
                        );

                        // Engine glow (intensity-driven)
                        let glow_pos = eng_pos - fwd * 2.5 - Vec3::Y * 0.3;
                        let glow_size = 1.0 + dropship.engine_intensity * 1.5;
                        let glow_matrix = glam::Mat4::from_scale_rotation_translation(
                            Vec3::splat(glow_size), Quat::IDENTITY, glow_pos,
                        );
                        let ei = dropship.engine_intensity;
                        let glow_color = [2.0 * ei, 1.2 * ei, 0.3 * ei, ei]; // hot orange-yellow
                        let glow_inst = vec![InstanceData::new(glow_matrix.to_cols_array_2d(), glow_color)];
                        state.renderer.render_instanced_load(
                            &mut encoder, &scene_view,
                            &state.flash_mesh,
                            &glow_inst,
                        );
                    }

                    // ── Landing lights (green when ramp open, white otherwise) ──
                    if dropship.ramp_open > 0.1 {
                        let ramp_pos = dropship.position + fwd * -8.0 - Vec3::Y * 1.0;
                        let ramp_matrix = glam::Mat4::from_scale_rotation_translation(
                            Vec3::splat(2.0 * dropship.ramp_open), Quat::IDENTITY, ramp_pos,
                        );
                        let ramp_color = [0.2, 2.5, 0.3, dropship.ramp_open]; // green "go" light
                        let ramp_inst = vec![InstanceData::new(ramp_matrix.to_cols_array_2d(), ramp_color)];
                        state.renderer.render_instanced_load(
                            &mut encoder, &scene_view,
                            &state.flash_mesh,
                            &ramp_inst,
                        );
                    }

                    // ── Door gunner muzzle flashes ──
                    if dropship.gunners_active() {
                        let mut gun_flash_instances: Vec<InstanceData> = Vec::new();

                        // Left gunner muzzle flash
                        if dropship.gunner_left_target.is_some() {
                            let flash = (state.time.elapsed_seconds() * 40.0).sin().abs();
                            if flash > 0.5 {
                                let gpos = dropship.gunner_left_pos();
                                let dir = dropship.gunner_left_target.map(|t| (t - gpos).normalize_or_zero()).unwrap_or(Vec3::Z);
                                let flash_pos = gpos + dir * 1.0;
                                let m = glam::Mat4::from_scale_rotation_translation(
                                    Vec3::splat(0.6), Quat::IDENTITY, flash_pos,
                                );
                                gun_flash_instances.push(InstanceData::new(m.to_cols_array_2d(), [4.0, 2.0, 0.3, 1.0]));
                            }
                        }

                        // Right gunner muzzle flash
                        if dropship.gunner_right_target.is_some() {
                            let flash = (state.time.elapsed_seconds() * 40.0 + 1.5).sin().abs();
                            if flash > 0.5 {
                                let gpos = dropship.gunner_right_pos();
                                let dir = dropship.gunner_right_target.map(|t| (t - gpos).normalize_or_zero()).unwrap_or(Vec3::Z);
                                let flash_pos = gpos + dir * 1.0;
                                let m = glam::Mat4::from_scale_rotation_translation(
                                    Vec3::splat(0.6), Quat::IDENTITY, flash_pos,
                                );
                                gun_flash_instances.push(InstanceData::new(m.to_cols_array_2d(), [4.0, 2.0, 0.3, 1.0]));
                            }
                        }

                        if !gun_flash_instances.is_empty() {
                            state.renderer.render_instanced_load(
                                &mut encoder, &scene_view,
                                &state.flash_mesh,
                                &gun_flash_instances,
                            );
                        }
                    }

                    // ── Ramp walkway (visible solid surface from ground to door) ──
                    if dropship.ramp_open > 0.1 {
                        let ramp_base = dropship.ramp_position();
                        let ramp_top = dropship.position + dropship.approach_dir * 4.0 - Vec3::Y * 1.5;
                        let ramp_center = (ramp_base + ramp_top) * 0.5;
                        let ramp_dir = (ramp_top - ramp_base).normalize_or_zero();
                        let ramp_len = (ramp_top - ramp_base).length();
                        let ramp_rot = if ramp_dir.length_squared() > 0.01 {
                            Quat::from_rotation_arc(Vec3::Y, ramp_dir)
                        } else {
                            Quat::IDENTITY
                        };
                        let ramp_scale = Vec3::new(2.5, ramp_len * 0.5, 0.2);
                        let ramp_m = glam::Mat4::from_scale_rotation_translation(
                            ramp_scale, ramp_rot, ramp_center,
                        );
                        let ramp_color = [0.25, 0.25, 0.22, dropship.ramp_open];
                        let ramp_inst = vec![InstanceData::new(ramp_m.to_cols_array_2d(), ramp_color)];
                        state.renderer.render_instanced_load(
                            &mut encoder, &scene_view,
                            &state.environment_meshes.rock,
                            &ramp_inst,
                        );
                    }
                }
            }

            // ── LZ green smoke cloud (same particle style as red tac smoke) ──
            if let Some(ref lz_cloud) = state.lz_smoke {
                let cam_fwd_lz = state.camera.forward();
                let cam_right_lz = cam_fwd_lz.cross(Vec3::Y).normalize_or_zero();
                let cam_up_lz = cam_right_lz.cross(cam_fwd_lz).normalize_or_zero();
                let mut lz_smoke_instances: Vec<InstanceData> = Vec::new();

                for p in &lz_cloud.particles {
                    let dist_sq = p.position.distance_squared(cam_pos);
                    if dist_sq > EFFECT_RENDER_DIST_SQ { continue; }

                    let life_frac = p.life / p.max_life;
                    if life_frac < 0.05 { continue; }

                    // Green smoke color with variation (same style as the red smoke)
                    let green_vary = (p.phase * 3.0).sin() * 0.1;
                    let alpha = life_frac.powf(0.6);
                    if alpha < 0.2 { continue; }

                    let color = [
                        0.05 + green_vary * 0.3,   // dark with slight variation
                        0.65 + green_vary,          // strong green
                        0.08,                       // minimal blue
                        alpha,
                    ];

                    // Billboard: face the camera (identical to red smoke)
                    let right = cam_right_lz * p.size;
                    let up = cam_up_lz * p.size;
                    let billboard_rot = glam::Mat4::from_cols(
                        right.extend(0.0),
                        up.extend(0.0),
                        (cam_fwd_lz * p.size).extend(0.0),
                        p.position.extend(1.0),
                    );
                    lz_smoke_instances.push(InstanceData::new(billboard_rot.to_cols_array_2d(), color));
                }

                if !lz_smoke_instances.is_empty() {
                    state.renderer.render_instanced_load(
                        &mut encoder, &scene_view,
                        &state.billboard_mesh,
                        &lz_smoke_instances,
                    );
                }
            }

            // ── Stratagem smoke (supply drop = green, reinforce = orange, orbital strike = red) ──
            let cam_fwd_s = state.camera.forward();
            let cam_right_s = cam_fwd_s.cross(Vec3::Y).normalize_or_zero();
            let cam_up_s = cam_right_s.cross(cam_fwd_s).normalize_or_zero();
            let mut stratagem_smoke_instances: Vec<InstanceData> = Vec::new();

            for cloud in &state.supply_drop_smoke {
                for p in &cloud.particles {
                    let dist_sq = p.position.distance_squared(cam_pos);
                    if dist_sq > EFFECT_RENDER_DIST_SQ { continue; }
                    let life_frac = p.life / p.max_life;
                    if life_frac < 0.05 { continue; }
                    let vary = (p.phase * 3.0).sin() * 0.1;
                    let alpha = life_frac.powf(0.6);
                    if alpha < 0.2 { continue; }
                    let color = [0.05 + vary * 0.3, 0.65 + vary, 0.08, alpha];
                    let right = cam_right_s * p.size;
                    let up = cam_up_s * p.size;
                    let billboard_rot = glam::Mat4::from_cols(
                        right.extend(0.0), up.extend(0.0), (cam_fwd_s * p.size).extend(0.0), p.position.extend(1.0),
                    );
                    stratagem_smoke_instances.push(InstanceData::new(billboard_rot.to_cols_array_2d(), color));
                }
            }
            if let Some(ref cloud) = state.reinforce_smoke {
                for p in &cloud.particles {
                    let dist_sq = p.position.distance_squared(cam_pos);
                    if dist_sq > EFFECT_RENDER_DIST_SQ { continue; }
                    let life_frac = p.life / p.max_life;
                    if life_frac < 0.05 { continue; }
                    let vary = (p.phase * 3.0).sin() * 0.1;
                    let alpha = life_frac.powf(0.6);
                    if alpha < 0.2 { continue; }
                    let color = [(0.85 + vary).min(1.0), (0.45 + vary * 0.3).min(1.0), 0.05, alpha];
                    let right = cam_right_s * p.size;
                    let up = cam_up_s * p.size;
                    let billboard_rot = glam::Mat4::from_cols(
                        right.extend(0.0), up.extend(0.0), (cam_fwd_s * p.size).extend(0.0), p.position.extend(1.0),
                    );
                    stratagem_smoke_instances.push(InstanceData::new(billboard_rot.to_cols_array_2d(), color));
                }
            }
            if let Some(ref cloud) = state.orbital_strike_smoke {
                for p in &cloud.particles {
                    let dist_sq = p.position.distance_squared(cam_pos);
                    if dist_sq > EFFECT_RENDER_DIST_SQ { continue; }
                    let life_frac = p.life / p.max_life;
                    if life_frac < 0.05 { continue; }
                    let vary = (p.phase * 3.0).sin() * 0.1;
                    let alpha = life_frac.powf(0.6);
                    if alpha < 0.2 { continue; }
                    let color = [0.75 + vary, 0.08 + vary * 0.3, 0.05, alpha];
                    let right = cam_right_s * p.size;
                    let up = cam_up_s * p.size;
                    let billboard_rot = glam::Mat4::from_cols(
                        right.extend(0.0), up.extend(0.0), (cam_fwd_s * p.size).extend(0.0), p.position.extend(1.0),
                    );
                    stratagem_smoke_instances.push(InstanceData::new(billboard_rot.to_cols_array_2d(), color));
                }
            }
            if !stratagem_smoke_instances.is_empty() {
                state.renderer.render_instanced_load(
                    &mut encoder, &scene_view,
                    &state.billboard_mesh,
                    &stratagem_smoke_instances,
                );
            }
        }

        // Pass 5h2: Roger Young Federation Destroyer (visible in real time during retrieval: orbit/atmosphere)
        if let Some(ref dropship) = state.extraction {
            if dropship.roger_young_visible() {
                let ry_pos = dropship.roger_young_pos;
                let ry_fwd = dropship.roger_young_fwd;
                let parts = roger_young_parts();

                // Build rotation matrix: ship faces along ry_fwd (toward planet)
                let fwd_xz = Vec3::new(ry_fwd.x, 0.0, ry_fwd.z).normalize_or_zero();
                let ship_rot = if fwd_xz.length_squared() > 0.01 {
                    Quat::from_rotation_arc(Vec3::Z, fwd_xz)
                } else {
                    Quat::IDENTITY
                };

                // Scale up ship when viewer is far so it reads clearly in orbit/atmosphere (not a dot)
                let dist_to_ship = cam_pos.distance(ry_pos);
                let scale_mult = if dist_to_ship > 600.0 {
                    (dist_to_ship / 500.0).min(2.2)
                } else {
                    1.0
                };

                let mut rock_instances: Vec<InstanceData> = Vec::new();
                let mut sphere_instances: Vec<InstanceData> = Vec::new();
                let mut glow_instances: Vec<InstanceData> = Vec::new();

                for part in &parts {
                    // Transform the part's local offset by the ship rotation
                    let world_offset = ship_rot * part.offset;
                    let world_pos = ry_pos + world_offset;
                    let part_scale = part.scale * scale_mult;

                    let matrix = glam::Mat4::from_scale_rotation_translation(
                        part_scale, ship_rot, world_pos,
                    );
                    let inst = InstanceData::new(matrix.to_cols_array_2d(), part.color);

                    match part.mesh_type {
                        0 => rock_instances.push(inst),
                        1 => sphere_instances.push(inst),
                        _ => glow_instances.push(inst),
                    }
                }

                // Pulsing engine glow animation
                let engine_pulse = 0.85 + (state.time.elapsed_seconds() * 6.0).sin() * 0.15;
                for inst in &mut glow_instances {
                    inst.color[0] *= engine_pulse;
                    inst.color[1] *= engine_pulse;
                    inst.color[2] *= engine_pulse;
                }

                if !rock_instances.is_empty() {
                    state.renderer.render_instanced_load(
                        &mut encoder, &scene_view,
                        &state.environment_meshes.rock,
                        &rock_instances,
                    );
                }
                if !sphere_instances.is_empty() {
                    state.renderer.render_instanced_load(
                        &mut encoder, &scene_view,
                        &state.environment_meshes.prop_sphere,
                        &sphere_instances,
                    );
                }
                if !glow_instances.is_empty() {
                    state.renderer.render_instanced_load(
                        &mut encoder, &scene_view,
                        &state.flash_mesh,
                        &glow_instances,
                    );
                }

                // ── Hangar bay approach lights (blinking during ascent to corvette) ──
                if dropship.phase == ExtractionPhase::Ascent {
                    let t = state.time.elapsed_seconds();
                    for i in 0..8 {
                        let blink = ((t * 3.0 + i as f32 * 0.5).sin() * 0.5 + 0.5).powi(2);
                        let light_z = -48.0 - i as f32 * 3.0;
                        let light_offset = ship_rot * Vec3::new(0.0, -4.0, light_z);
                        let light_pos = ry_pos + light_offset;
                        let m = glam::Mat4::from_scale_rotation_translation(
                            Vec3::splat(0.5 + blink * 0.5), Quat::IDENTITY, light_pos,
                        );
                        let c = [blink * 2.0, blink * 0.8, blink * 0.1, blink];
                        let inst = InstanceData::new(m.to_cols_array_2d(), c);
                        state.renderer.render_instanced_load(
                            &mut encoder, &scene_view,
                            &state.flash_mesh,
                            &vec![inst],
                        );
                    }
                }
            }
        }

        // Pass 5i: Shell casings (world-space, weapon-specific)
        if !state.shell_casings.is_empty() {
            use crate::viewmodel::ShellCasingType;
            let mut casing_instances: Vec<InstanceData> = Vec::new();
            for casing in &state.shell_casings {
                let dist_sq = casing.position.distance_squared(cam_pos);
                if dist_sq > 50.0 * 50.0 { continue; }
                let (scale_vec, color_base) = match casing.shell_type {
                    ShellCasingType::Rifle => (Vec3::new(0.4, 1.0, 0.4), [0.75, 0.60, 0.22]),
                    ShellCasingType::Shotgun => (Vec3::new(0.9, 1.0, 0.9), [0.72, 0.15, 0.12]),  // Red plastic hull
                    ShellCasingType::Sniper => (Vec3::new(0.35, 1.0, 0.35), [0.70, 0.55, 0.20]),
                    ShellCasingType::MachineGun => (Vec3::new(0.38, 1.0, 0.38), [0.72, 0.58, 0.24]),
                    ShellCasingType::Rocket => (Vec3::new(0.5, 1.0, 0.5), [0.35, 0.32, 0.30]),
                    ShellCasingType::Flamethrower => (Vec3::new(0.5, 1.0, 0.5), [0.25, 0.28, 0.22]),
                };
                let scale = scale_vec * casing.size;
                let matrix = glam::Mat4::from_scale_rotation_translation(
                    scale, casing.rotation, casing.position,
                );
                let alpha = (casing.lifetime / 4.0).min(1.0);
                let color = [color_base[0], color_base[1], color_base[2], alpha];
                casing_instances.push(InstanceData::new(matrix.to_cols_array_2d(), color));
            }
            if !casing_instances.is_empty() {
                state.renderer.render_instanced_load(
                    &mut encoder, &scene_view,
                    &state.environment_meshes.cube,
                    &casing_instances,
                );
            }
        }

        // Pass 6: Viewmodel (M1A4 Morita Rifle) - animated, multi-part composition
        // Each part is a unit cube scaled/positioned to form the Morita silhouette
        let player_in_boat = state.extraction.as_ref().map_or(false, |e: &ExtractionDropship| e.player_camera_locked());
        let show_viewmodel = !state.debug.noclip && state.current_planet_idx.is_some()
            && state.phase == GamePhase::Playing && state.player.is_alive && !player_in_boat
            && !state.player.is_shovel_equipped();
        if show_viewmodel {
            // Transform viewmodel from view space to world space using the
            // inverse of the ACTUAL view matrix. This guarantees a perfect
            // round-trip: view_matrix * (view_inverse * view_pos) == view_pos,
            // avoiding drift from quaternion/look-at decomposition mismatches.
            let view_to_world = state.camera.view_matrix().inverse();

            // Base viewmodel position in view space: right, below eye, forward (hip-fire)
            let base_pos = Vec3::new(0.18, -0.11, -0.38);

            // ADS target: gun pivot position when looking through sights.
            // Rear sight must align with screen center (0,0,-1). Computed from sight geometry.
            let ads_target = match state.player.current_weapon().weapon_type {
                WeaponType::Shotgun => {
                    // MI-22: bead on vent rib, rear near receiver. Sight line ~(0, 0.02, 0.02) to (0, 0.028, -0.35)
                    Vec3::new(0.0, -0.025, -0.22)
                }
                WeaponType::MachineGun => {
                    // Morita MG: sight rail along top. Rear ~(0, 0.035, 0.05), front ~(0, 0.035, -0.4)
                    Vec3::new(0.0, -0.038, -0.25)
                }
                _ => {
                    // M1A4 Morita Rifle: rear sight at (0, 0.042, 0.06), front at (0, 0.042, -0.32).
                    // For rear at (0, 0, -0.18) on view ray: pivot = (0, 0, -0.18) - (0, 0.042, 0.06)
                    Vec3::new(0.0, -0.042, -0.24)
                }
            };

            // Get animated transform from viewmodel state (sight-aligned ADS)
            let aim = state.player.aim_progress;
            let (anim_offset, anim_rot) = state.viewmodel_anim.compute_transform(aim, base_pos, ads_target);

            // Final animated base position and rotation
            let gun_pos = base_pos + anim_offset;
            let gun_rot = anim_rot;

            // Helper: viewmodel part (offset, scale, color). Barrel points along -Z.
            struct GunPart {
                offset: [f32; 3],
                scale: [f32; 3],
                color: [f32; 4],
            }
            let mut viewmodel_instances: Vec<InstanceData> = Vec::new();

            let (parts, muzzle_offset) = if state.player.current_weapon().weapon_type == WeaponType::Shotgun {
                // MI-22 Tactical Shotgun — pump-action, short barrel, stock
                let shotgun_parts: &[GunPart] = &[
                    GunPart { offset: [0.0, 0.0, 0.02], scale: [0.040, 0.038, 0.14], color: [0.20, 0.20, 0.22, 1.0] }, // receiver
                    GunPart { offset: [0.0, 0.006, -0.20], scale: [0.022, 0.022, 0.28], color: [0.28, 0.28, 0.30, 1.0] }, // barrel
                    GunPart { offset: [0.0, 0.0, -0.08], scale: [0.032, 0.026, 0.06], color: [0.18, 0.18, 0.20, 1.0] },   // pump / forend
                    GunPart { offset: [0.0, -0.006, 0.16], scale: [0.028, 0.034, 0.12], color: [0.14, 0.12, 0.10, 1.0] }, // stock
                    GunPart { offset: [0.0, -0.050, 0.0], scale: [0.020, 0.050, 0.026], color: [0.12, 0.12, 0.14, 1.0] },  // grip
                    GunPart { offset: [0.0, -0.028, -0.02], scale: [0.016, 0.008, 0.040], color: [0.22, 0.22, 0.24, 1.0] }, // trigger guard
                    GunPart { offset: [0.0, 0.028, -0.12], scale: [0.008, 0.012, 0.18], color: [0.24, 0.24, 0.26, 1.0] },  // vent rib / sight rail
                    GunPart { offset: [0.0, 0.006, -0.38], scale: [0.024, 0.024, 0.022], color: [0.26, 0.26, 0.28, 1.0] }, // muzzle
                ];
                (shotgun_parts, Vec3::new(0.0, 0.006, -0.40))
            } else if state.player.current_weapon().weapon_type == WeaponType::MachineGun {
                // Morita MG — heavy machine gun, longer barrel, ammo box
                let mg_parts: &[GunPart] = &[
                    GunPart { offset: [0.0, 0.0, 0.05], scale: [0.045, 0.050, 0.24], color: [0.20, 0.20, 0.22, 1.0] }, // receiver
                    GunPart { offset: [0.0, 0.010, -0.32], scale: [0.018, 0.018, 0.42], color: [0.28, 0.28, 0.30, 1.0] }, // barrel
                    GunPart { offset: [0.0, -0.012, 0.22], scale: [0.038, 0.042, 0.14], color: [0.16, 0.16, 0.18, 1.0] }, // ammo box
                    GunPart { offset: [0.0, -0.058, 0.0], scale: [0.022, 0.055, 0.030], color: [0.14, 0.14, 0.16, 1.0] }, // grip
                    GunPart { offset: [0.0, -0.038, -0.025], scale: [0.018, 0.010, 0.050], color: [0.22, 0.22, 0.24, 1.0] }, // trigger guard
                    GunPart { offset: [0.0, 0.035, -0.08], scale: [0.010, 0.014, 0.22], color: [0.24, 0.24, 0.26, 1.0] }, // sight rail
                    GunPart { offset: [0.0, 0.010, -0.52], scale: [0.028, 0.028, 0.028], color: [0.24, 0.24, 0.26, 1.0] }, // muzzle
                ];
                (mg_parts, Vec3::new(0.0, 0.010, -0.54))
            } else {
                // M1A4 Morita Rifle — Starship Troopers bullpup assault rifle
                let rifle_parts: &[GunPart] = &[
                    GunPart { offset: [0.0, 0.0, 0.04], scale: [0.038, 0.042, 0.20], color: [0.22, 0.22, 0.25, 1.0] },
                    GunPart { offset: [0.0, 0.008, -0.24], scale: [0.012, 0.012, 0.34], color: [0.30, 0.30, 0.33, 1.0] },
                    GunPart { offset: [0.0, 0.005, -0.08], scale: [0.026, 0.028, 0.12], color: [0.20, 0.20, 0.23, 1.0] },
                    GunPart { offset: [0.0, 0.008, -0.44], scale: [0.018, 0.018, 0.030], color: [0.14, 0.14, 0.16, 1.0] },
                    GunPart { offset: [0.0, -0.008, 0.17], scale: [0.034, 0.038, 0.10], color: [0.18, 0.18, 0.20, 1.0] },
                    GunPart { offset: [0.0, -0.008, 0.24], scale: [0.030, 0.036, 0.018], color: [0.10, 0.10, 0.12, 1.0] },
                    GunPart { offset: [0.0, -0.052, 0.0], scale: [0.018, 0.048, 0.024], color: [0.14, 0.14, 0.16, 1.0] },
                    GunPart { offset: [0.0, -0.032, -0.022], scale: [0.014, 0.008, 0.045], color: [0.22, 0.22, 0.25, 1.0] },
                    GunPart { offset: [0.0, 0.032, -0.01], scale: [0.014, 0.014, 0.16], color: [0.20, 0.20, 0.22, 1.0] },
                    GunPart { offset: [0.0, 0.042, -0.08], scale: [0.010, 0.010, 0.010], color: [0.25, 0.25, 0.28, 1.0] },
                    GunPart { offset: [0.0, 0.042, 0.06], scale: [0.010, 0.010, 0.010], color: [0.25, 0.25, 0.28, 1.0] },
                    GunPart { offset: [0.0, 0.030, -0.32], scale: [0.005, 0.020, 0.005], color: [0.25, 0.25, 0.28, 1.0] },
                    GunPart { offset: [0.0, -0.042, 0.08], scale: [0.020, 0.048, 0.028], color: [0.16, 0.16, 0.18, 1.0] },
                    GunPart { offset: [0.0, -0.022, -0.14], scale: [0.016, 0.016, 0.18], color: [0.26, 0.26, 0.28, 1.0] },
                    GunPart { offset: [0.0, -0.022, -0.04], scale: [0.024, 0.018, 0.055], color: [0.18, 0.18, 0.20, 1.0] },
                    GunPart { offset: [0.022, 0.024, 0.06], scale: [0.008, 0.008, 0.018], color: [0.30, 0.30, 0.32, 1.0] },
                    GunPart { offset: [0.022, 0.005, 0.02], scale: [0.004, 0.018, 0.035], color: [0.28, 0.28, 0.30, 1.0] },
                    GunPart { offset: [0.018, -0.005, -0.16], scale: [0.005, 0.005, 0.005], color: [0.30, 0.30, 0.32, 1.0] },
                ];
                (rifle_parts, Vec3::new(0.0, 0.008, -0.46))
            };

            for part in parts {
                let part_offset = Vec3::new(part.offset[0], part.offset[1], part.offset[2]);
                let part_scale = Vec3::new(part.scale[0], part.scale[1], part.scale[2]);

                // Build the part's transform in view space
                let rotated_offset = gun_rot * part_offset;
                let view_pos = gun_pos + rotated_offset;

                // Construct view-space matrix, then transform to world space
                // via the view matrix inverse for guaranteed-correct positioning.
                let view_mat = glam::Mat4::from_scale_rotation_translation(
                    part_scale, gun_rot, view_pos,
                );
                let world_mat = view_to_world * view_mat;

                viewmodel_instances.push(InstanceData::new(world_mat.to_cols_array_2d(), part.color));
            }

            // === MUZZLE FLASH (when firing) ===
            if state.viewmodel_anim.fire_flash_timer < 0.06 {
                let flash_t = state.viewmodel_anim.fire_flash_timer / 0.06;
                let flash_intensity = (1.0 - flash_t).max(0.0);
                let flash_size = 0.025 + flash_intensity * 0.02;

                // Muzzle position (rifle or shotgun, set above)
                let muzzle_view = gun_pos + gun_rot * muzzle_offset;

                let flash_color = [
                    2.0 + flash_intensity * 3.0,
                    1.5 + flash_intensity * 2.0,
                    0.5 + flash_intensity * 1.0,
                    1.0,
                ];

                let rot_angle = state.time.elapsed_seconds() * 137.0;
                let flash_rot = gun_rot * Quat::from_rotation_z(rot_angle);

                // Build in view space, then transform to world
                let flash_view = glam::Mat4::from_scale_rotation_translation(
                    Vec3::splat(flash_size), flash_rot, muzzle_view,
                );
                let flash_world = view_to_world * flash_view;
                viewmodel_instances.push(InstanceData::new(flash_world.to_cols_array_2d(), flash_color));

                // Secondary flash (slightly larger, dimmer, orange)
                let flash2_size = flash_size * 1.8;
                let flash2_color = [
                    1.5 * flash_intensity,
                    0.6 * flash_intensity,
                    0.1 * flash_intensity,
                    flash_intensity * 0.8,
                ];
                let flash2_rot = gun_rot * Quat::from_rotation_z(rot_angle + 1.0);
                let flash2_view = glam::Mat4::from_scale_rotation_translation(
                    Vec3::splat(flash2_size), flash2_rot,
                    muzzle_view + gun_rot * Vec3::new(0.0, 0.0, -0.01),
                );
                let flash2_world = view_to_world * flash2_view;
                viewmodel_instances.push(InstanceData::new(flash2_world.to_cols_array_2d(), flash2_color));
            }

            if !viewmodel_instances.is_empty() {
                state.renderer.render_viewmodel(&mut encoder, &scene_view, &viewmodel_instances);
            }
        }

        // Pass 7: Screen-space text overlay (debug info + game messages)
        {
            let (sw, sh) = state.renderer.dimensions();
            let (sw, sh) = (sw as f32, sh as f32);
            let tb = overlay::build(state, sw, sh);

            // Bloom: bright extract -> blur -> bloom texture
            let bloom_view = state.renderer.run_bloom_passes(&mut encoder, &scene_view);

            // Cinematic post-process: scene + bloom + SSAO -> swap chain (97 movie / Heinlein film look)
            state.renderer.update_cinematic_uniform(state.time.elapsed_seconds());
            state.renderer.run_cinematic_pass(
                &mut encoder,
                &scene_view,
                &bloom_view,
                state.renderer.depth_texture_view(),
                &output_view,
            );
            state.renderer.render_overlay(&mut encoder, &output_view, &tb.vertices, &tb.indices);
        }

        state.renderer.end_frame(output, encoder);

        Ok(())
}
