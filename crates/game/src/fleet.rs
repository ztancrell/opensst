//! Federation fleet: corvettes, destroyers, tac fighter launch/RTB points.
//! Shared orbit params so tac fighters spawn from and return to corvettes.

use glam::Vec3;

/// Surface corvette orbit params (radius, phase, speed) — must match render/mod.rs Pass 0b0.
pub const SURFACE_CORVETTE_PARAMS: [(f32, f32, f32); 8] = [
    (95.0, 0.0, 0.14),
    (130.0, 2.2, 0.11),
    (65.0, 1.4, 0.18),
    (165.0, 4.2, 0.09),
    (80.0, 3.6, 0.16),
    (115.0, 5.4, 0.12),
    (75.0, 0.8, 0.19),
    (140.0, 2.8, 0.10),
];

/// Altitude of corvettes above camera (sky dome).
pub const CORVETTE_ALTITUDE: f32 = 280.0;

/// Compute surface corvette positions for tac fighter spawn/RTB. Matches render exactly.
pub fn surface_corvette_positions(cam_pos: Vec3, orbital_time: f64, elapsed: f32) -> Vec<Vec3> {
    let ot = orbital_time as f32;
    let t = elapsed;
    let sky_y = cam_pos.y + CORVETTE_ALTITUDE;
    SURFACE_CORVETTE_PARAMS
        .iter()
        .enumerate()
        .map(|(i, &(radius, phase, speed))| {
            let angle = phase + ot * speed + t * 0.015;
            let dx = angle.cos() * radius;
            let dz = angle.sin() * radius;
            Vec3::new(
                cam_pos.x + dx,
                sky_y + (i as f32 * 18.0),
                cam_pos.z + dz,
            )
        })
        .collect()
}

/// Destroyer orbit params for surface view — must match render/mod.rs Pass 0b0.
pub const SURFACE_DESTROYER_PARAMS: [(f32, f32); 3] = [
    (0.0, 80.0),
    (std::f32::consts::PI, 40.0),
    (std::f32::consts::PI * 0.6, 120.0),
];

/// Compute surface destroyer positions (for artillery barrages). Matches render exactly.
pub fn surface_destroyer_positions(cam_pos: Vec3, orbital_time: f64, elapsed: f32) -> Vec<Vec3> {
    let ot = orbital_time as f32;
    let t = elapsed;
    let sky_y = cam_pos.y + CORVETTE_ALTITUDE;
    let d_angle = ot * 0.07 + t * 0.008;
    let d_radius = 220.0;
    SURFACE_DESTROYER_PARAMS
        .iter()
        .enumerate()
        .map(|(i, &(phase_off, y_off))| {
            let a = d_angle + phase_off;
            Vec3::new(
                cam_pos.x + a.cos() * d_radius,
                sky_y + y_off,
                cam_pos.z + a.sin() * d_radius,
            )
        })
        .collect()
}

/// Index of corvette whose XZ direction from center best matches the given direction.
pub fn corvette_index_for_direction(positions: &[Vec3], center: Vec3, dir_xz: Vec3) -> usize {
    let dir_xz = Vec3::new(dir_xz.x, 0.0, dir_xz.z).normalize_or_zero();
    let mut best = 0;
    let mut best_dot = -2.0;
    for (i, &pos) in positions.iter().enumerate() {
        let to_corvette = Vec3::new(pos.x - center.x, 0.0, pos.z - center.z).normalize_or_zero();
        let dot = to_corvette.dot(dir_xz);
        if dot > best_dot {
            best_dot = dot;
            best = i;
        }
    }
    best
}
