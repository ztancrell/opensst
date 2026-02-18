//! Federation Earth territory: cities, towns, farms — your own little Starship Troopers world.
//!
//! Defines named places (cities, towns, farms) with waypoints, prefab buildings, roads, and citizen spawns.

use glam::{Quat, Vec3};
use hecs::World;
use rand::Rng;
use renderer::Vertex;

use crate::citizen::{Citizen, despawn_citizens};
use crate::destruction::{CachedRenderData, MESH_GROUP_BEVELED_CUBE};
use engine_core::{Transform, Velocity};

/// Type of settlement — affects density and flavor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceType {
    City,
    Town,
    Farm,
}

/// One named place in the Federation territory (city, town, or farm).
#[derive(Debug, Clone)]
pub struct Place {
    pub name: &'static str,
    /// World X offset from origin (origin = dropship landing / capital).
    pub center_x: f32,
    /// World Z offset from origin.
    pub center_z: f32,
    /// Approximate radius for "you are here" detection.
    pub radius: f32,
    pub place_type: PlaceType,
    /// Waypoints relative to (center_x, center_z). Citizens walk between these.
    pub waypoints_local: &'static [(f32, f32)],
    /// How many citizens to spawn here.
    pub citizen_count: usize,
}

/// Federation territory: one country's worth of cities, towns, and farms.
/// Starship Troopers flavor — Buenos Aires, Riverside, Valley Farms, etc.
pub const FEDERATION_TERRITORY: &[Place] = &[
    // —— CITIES ——
    Place {
        name: "Buenos Aires Metro",
        center_x: 0.0,
        center_z: 0.0,
        radius: 85.0,
        place_type: PlaceType::City,
        waypoints_local: &[
            (-35.0, -25.0),
            (30.0, -28.0),
            (-30.0, 20.0),
            (28.0, 22.0),
            (0.0, -40.0),
            (5.0, 0.0),
            (-12.0, 28.0),
            (20.0, 30.0),
            (-8.0, -12.0),
            (14.0, -10.0),
            (-42.0, -18.0),
            (40.0, -20.0),
            (-22.0, -48.0),
            (26.0, -50.0),
            (-38.0, 32.0),
            (35.0, 38.0),
            (0.0, 48.0),
            (-14.0, -32.0),
            (16.0, -28.0),
            (-24.0, 2.0),
            (30.0, 8.0),
            (-18.0, -42.0),
            (22.0, -38.0),
            (-32.0, 8.0),
            (38.0, -8.0),
        ],
        citizen_count: 22,
    },
    Place {
        name: "Port District",
        center_x: -65.0,
        center_z: 95.0,
        radius: 45.0,
        place_type: PlaceType::City,
        waypoints_local: &[
            (-20.0, -15.0),
            (18.0, -18.0),
            (-15.0, 20.0),
            (20.0, 18.0),
            (0.0, 0.0),
            (-25.0, 5.0),
            (22.0, -8.0),
            (-10.0, -22.0),
            (12.0, 25.0),
            (-28.0, 12.0),
            (25.0, -15.0),
        ],
        citizen_count: 12,
    },
    // —— TOWNS ——
    Place {
        name: "Riverside",
        center_x: 125.0,
        center_z: 38.0,
        radius: 42.0,
        place_type: PlaceType::Town,
        waypoints_local: &[
            (-18.0, -12.0),
            (20.0, -14.0),
            (-12.0, 18.0),
            (16.0, 20.0),
            (0.0, 0.0),
            (-22.0, 5.0),
            (18.0, -10.0),
            (-8.0, -20.0),
            (10.0, 22.0),
            (-15.0, 10.0),
            (14.0, -18.0),
        ],
        citizen_count: 10,
    },
    Place {
        name: "Northgate",
        center_x: -95.0,
        center_z: -82.0,
        radius: 38.0,
        place_type: PlaceType::Town,
        waypoints_local: &[
            (-16.0, -10.0),
            (18.0, -14.0),
            (-14.0, 16.0),
            (16.0, 18.0),
            (0.0, 0.0),
            (-20.0, 4.0),
            (14.0, -12.0),
            (-10.0, -18.0),
            (12.0, 20.0),
        ],
        citizen_count: 8,
    },
    Place {
        name: "Hillside",
        center_x: 52.0,
        center_z: 105.0,
        radius: 35.0,
        place_type: PlaceType::Town,
        waypoints_local: &[
            (-14.0, -8.0),
            (16.0, -10.0),
            (-10.0, 14.0),
            (14.0, 16.0),
            (0.0, 0.0),
            (-16.0, 6.0),
            (12.0, -12.0),
            (-8.0, -14.0),
            (10.0, 18.0),
        ],
        citizen_count: 8,
    },
    Place {
        name: "Outpost Seven",
        center_x: -50.0,
        center_z: -120.0,
        radius: 32.0,
        place_type: PlaceType::Town,
        waypoints_local: &[
            (-12.0, -8.0),
            (14.0, -10.0),
            (-10.0, 12.0),
            (12.0, 14.0),
            (0.0, 0.0),
            (-14.0, 4.0),
            (10.0, -10.0),
        ],
        citizen_count: 6,
    },
    // —— FARMS / RURAL ——
    Place {
        name: "Valley Farms",
        center_x: 88.0,
        center_z: -98.0,
        radius: 55.0,
        place_type: PlaceType::Farm,
        waypoints_local: &[
            (-28.0, -18.0),
            (30.0, -22.0),
            (-22.0, 25.0),
            (26.0, 28.0),
            (0.0, 0.0),
            (-35.0, 8.0),
            (32.0, -12.0),
            (-18.0, -30.0),
            (20.0, 32.0),
            (-25.0, 15.0),
            (28.0, -25.0),
            (-12.0, 35.0),
            (15.0, -28.0),
        ],
        citizen_count: 7,
    },
    Place {
        name: "Prairie Belt",
        center_x: -105.0,
        center_z: 72.0,
        radius: 48.0,
        place_type: PlaceType::Farm,
        waypoints_local: &[
            (-22.0, -14.0),
            (24.0, -18.0),
            (-18.0, 20.0),
            (22.0, 24.0),
            (0.0, 0.0),
            (-28.0, 6.0),
            (26.0, -10.0),
            (-14.0, -24.0),
            (16.0, 26.0),
            (-20.0, 12.0),
            (20.0, -20.0),
        ],
        citizen_count: 6,
    },
    Place {
        name: "Greenfield",
        center_x: 140.0,
        center_z: -45.0,
        radius: 40.0,
        place_type: PlaceType::Farm,
        waypoints_local: &[
            (-16.0, -12.0),
            (18.0, -14.0),
            (-12.0, 16.0),
            (16.0, 18.0),
            (0.0, 0.0),
            (-20.0, 5.0),
            (18.0, -12.0),
            (-10.0, -18.0),
            (12.0, 20.0),
        ],
        citizen_count: 5,
    },
];

/// Axis-aligned bounds of the full territory (all places + margin). Used to ensure chunks are loaded before building.
pub fn territory_bounds() -> (f32, f32, f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for place in FEDERATION_TERRITORY {
        min_x = min_x.min(place.center_x - place.radius);
        max_x = max_x.max(place.center_x + place.radius);
        min_z = min_z.min(place.center_z - place.radius);
        max_z = max_z.max(place.center_z + place.radius);
    }
    // Include road endpoints (polylines) so roads don't sample unloaded chunks
    for poly in road_polylines() {
        for &(x, z) in &poly {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_z = min_z.min(z);
            max_z = max_z.max(z);
        }
    }
    for poly in path_polylines() {
        for &(x, z) in &poly {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_z = min_z.min(z);
            max_z = max_z.max(z);
        }
    }
    (min_x, max_x, min_z, max_z)
}

/// Build flat list of all waypoints in world space (for citizen pathfinding).
/// Order: all waypoints from place 0, then place 1, etc. Indices match citizen logic.
pub fn all_waypoints_global() -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    for place in FEDERATION_TERRITORY {
        for &(lx, lz) in place.waypoints_local {
            out.push((place.center_x + lx, place.center_z + lz));
        }
    }
    out
}

// ─── Roads and walking paths (city layout) ───
// Polylines in world (x, z). Each is a list of points; consecutive points form segments.
// Roads connect place centers and main streets; paths are narrower and link plazas/waypoints.

const ROAD_WIDTH: f32 = 8.0;
const PATH_WIDTH: f32 = 4.0;

/// Main roads: capital to each settlement, plus a few cross-links.
fn road_polylines() -> Vec<Vec<(f32, f32)>> {
    let cap = (0.0_f32, 0.0_f32);
    let port = (-65.0, 95.0);
    let river = (125.0, 38.0);
    let north = (-95.0, -82.0);
    let hill = (52.0, 105.0);
    let outpost = (-50.0, -120.0);
    let valley = (88.0, -98.0);
    let prairie = (-105.0, 72.0);
    let green = (140.0, -45.0);

    vec![
        vec![cap, port],
        vec![cap, river],
        vec![cap, north],
        vec![cap, hill],
        vec![cap, outpost],
        vec![cap, valley],
        vec![cap, prairie],
        vec![cap, green],
        vec![port, prairie],
        vec![river, hill],
        vec![north, outpost],
        vec![valley, green],
    ]
}

/// Walking paths: main streets and plazas within the capital and key waypoints.
fn path_polylines() -> Vec<Vec<(f32, f32)>> {
    let cap = (0.0_f32, 0.0_f32);
    vec![
        vec![cap, (0.0, -40.0)],
        vec![cap, (25.0, 0.0)],
        vec![cap, (-25.0, 0.0)],
        vec![cap, (0.0, 35.0)],
        vec![(5.0, 0.0), (5.0, -25.0)],
        vec![(-8.0, -12.0), (14.0, -10.0)],
        vec![(-22.0, -18.0), (18.0, -25.0)],
        vec![(0.0, -8.0), (8.0, 40.0)],
        vec![(-35.0, 5.0), (30.0, -35.0)],
    ]
}

/// Segment descriptor for road/path collision: (center_x, center_z, half_length, half_width, rotation_y_rad).
/// Used to create box colliders that match the visual road strips.
pub fn road_collider_segments() -> Vec<(f32, f32, f32, f32, f32)> {
    let mut out = Vec::new();
    for poly in road_polylines() {
        add_segment_descriptors(&poly, ROAD_WIDTH, &mut out);
    }
    for poly in path_polylines() {
        add_segment_descriptors(&poly, PATH_WIDTH, &mut out);
    }
    out
}

/// If (x, z) is inside any road/path segment, returns the ground Y at that position (for solid road feel).
pub fn road_ground_y_at<F: Fn(f32, f32) -> f32>(x: f32, z: f32, sample_terrain_y: F) -> Option<f32> {
    for (cx, cz, half_len, half_w, rot) in road_collider_segments() {
        let c = rot.cos();
        let s = rot.sin();
        let rel_x = x - cx;
        let rel_z = z - cz;
        let local_along = rel_x * s + rel_z * c;
        let local_across = -rel_x * c + rel_z * s;
        if local_along.abs() <= half_len && local_across.abs() <= half_w {
            return Some(sample_terrain_y(x, z));
        }
    }
    None
}

fn add_segment_descriptors(points: &[(f32, f32)], width: f32, out: &mut Vec<(f32, f32, f32, f32, f32)>) {
    if points.len() < 2 {
        return;
    }
    let half_w = width * 0.5;
    for i in 0..points.len() - 1 {
        let (ax, az) = points[i];
        let (bx, bz) = points[i + 1];
        let dx = bx - ax;
        let dz = bz - az;
        let len = (dx * dx + dz * dz).sqrt().max(0.001);
        let half_len = len * 0.5;
        let cx = (ax + bx) * 0.5;
        let cz = (az + bz) * 0.5;
        let rotation_y_rad = f32::atan2(dx, dz);
        out.push((cx, cz, half_len, half_w, rotation_y_rad));
    }
}

/// Build road+path mesh geometry (vertices and indices). Y is sampled from terrain.
/// Draw with one instance (identity matrix, asphalt/concrete color).
pub fn build_earth_roads_mesh(
    sample_terrain_y: impl Fn(f32, f32) -> f32,
) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let up = [0.0_f32, 1.0, 0.0];
    let white = [1.0_f32, 1.0, 1.0, 1.0];

    for poly in road_polylines() {
        add_strip(&mut vertices, &mut indices, &poly, ROAD_WIDTH, &sample_terrain_y, &up, &white);
    }
    for poly in path_polylines() {
        add_strip(&mut vertices, &mut indices, &poly, PATH_WIDTH, &sample_terrain_y, &up, &white);
    }

    (vertices, indices)
}

fn add_strip<F: Fn(f32, f32) -> f32>(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    points: &[(f32, f32)],
    width: f32,
    sample_terrain_y: &F,
    normal: &[f32; 3],
    color: &[f32; 4],
) {
    if points.len() < 2 {
        return;
    }
    let half = width * 0.5;
    for i in 0..points.len() - 1 {
        let (ax, az) = points[i];
        let (bx, bz) = points[i + 1];
        let dx = bx - ax;
        let dz = bz - az;
        let len = (dx * dx + dz * dz).sqrt().max(0.001);
        let perp_x = -dz / len;
        let perp_z = dx / len;

        let ax_l = ax - perp_x * half;
        let az_l = az - perp_z * half;
        let ax_r = ax + perp_x * half;
        let az_r = az + perp_z * half;
        let bx_r = bx + perp_x * half;
        let bz_r = bz + perp_z * half;
        let bx_l = bx - perp_x * half;
        let bz_l = bz - perp_z * half;

        let y0 = sample_terrain_y(ax_l, az_l) + 0.02;
        let y1 = sample_terrain_y(ax_r, az_r) + 0.02;
        let y2 = sample_terrain_y(bx_r, bz_r) + 0.02;
        let y3 = sample_terrain_y(bx_l, bz_l) + 0.02;

        // Use actual quad normal so lighting is correct on slopes (avoids black/dark patches from (0,1,0)).
        let v0 = [ax_l, y0, az_l];
        let v1 = [ax_r, y1, az_r];
        let v3 = [bx_l, y3, bz_l];
        let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
        let e2 = [v3[0] - v0[0], v3[1] - v0[1], v3[2] - v0[2]];
        let nx = e1[1] * e2[2] - e1[2] * e2[1];
        let ny = e1[2] * e2[0] - e1[0] * e2[2];
        let nz = e1[0] * e2[1] - e1[1] * e2[0];
        let len = (nx * nx + ny * ny + nz * nz).sqrt().max(1e-6);
        let quad_normal = [nx / len, ny / len, nz / len];

        let base = vertices.len() as u32;
        vertices.push(Vertex::with_color(v0, quad_normal, [0.0, 0.0], *color));
        vertices.push(Vertex::with_color(v1, quad_normal, [1.0, 0.0], *color));
        vertices.push(Vertex::with_color([bx_r, y2, bz_r], quad_normal, [1.0, 1.0], *color));
        vertices.push(Vertex::with_color(v3, quad_normal, [0.0, 1.0], *color));
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

/// Get the name of the place at world position (x, z), if any.
/// Used for HUD "You are in: Buenos Aires Metro".
pub fn place_name_at(x: f32, z: f32) -> Option<&'static str> {
    let mut best: Option<(&'static str, f32)> = None;
    for place in FEDERATION_TERRITORY {
        let dx = x - place.center_x;
        let dz = z - place.center_z;
        let dist_sq = dx * dx + dz * dz;
        let r_sq = place.radius * place.radius;
        if dist_sq <= r_sq {
            // Prefer smaller radius when overlapping (more specific place)
            match best {
                None => best = Some((place.name, place.radius)),
                Some((_, r)) if place.radius < r => best = Some((place.name, place.radius)),
                _ => {}
            }
        }
    }
    best.map(|(name, _)| name)
}

/// Spawn citizens across the whole territory (cities, towns, farms).
/// Replaces any existing citizens. Call when landing on Earth.
pub fn spawn_territory_citizens(
    world: &mut World,
    sample_terrain_y: impl Fn(f32, f32) -> f32,
) {
    despawn_citizens(world);
    let mut rng = rand::thread_rng();
    // Civilian names only — no overlap with Roger Young crew (Rico, Zim, Levy, Chen, Brice, Martinez, etc.)
    let names = [
        "Carlos", "Maria", "Jake", "Yuki", "Hans", "Elena", "Dizzy",
        "Sanders", "Deladrier", "Shujumi", "Hendrick", "Nadia", "Viktor",
        "O'Brien", "Vargas", "Kim", "Petra", "Miguel", "Sasha", "Anya",
        "Omar", "Lena", "Felix", "Irina", "Marcus", "Sofia", "Tomas",
    ];
    let mut name_idx = 0;
    let mut waypoint_start = 0;
    for place in FEDERATION_TERRITORY {
        let waypoints = place.waypoints_local;
        let place_waypoint_count = waypoints.len();
        for _ in 0..place.citizen_count {
            let (lx, lz) = waypoints[rng.gen_range(0..waypoints.len())];
            let x = place.center_x + lx + rng.gen::<f32>() * 4.0 - 2.0;
            let z = place.center_z + lz + rng.gen::<f32>() * 4.0 - 2.0;
            let y = sample_terrain_y(x, z) + 0.5;
            let pos = Vec3::new(x, y, z);
            let name = names[name_idx % names.len()].to_string();
            name_idx += 1;
            let dialogue_id = rng.gen_range(0..5);
            world.spawn((
                Transform {
                    position: pos,
                    rotation: glam::Quat::IDENTITY,
                    scale: Vec3::splat(1.0),
                },
                Velocity::default(),
                Citizen::new(name, dialogue_id, &mut rng, waypoint_start, place_waypoint_count),
            ));
        }
        waypoint_start += place_waypoint_count;
    }
}

// ─── Prefab buildings: (world_x, world_z, size_x, size_y, size_z, color_rgba) ───
// UCF / Starship Troopers Federation: military-industrial, brutalist, solid prefab boxes.
// Solid cube mesh so corners connect; palette: concrete, olive drab, steel, bunker grey.
const CONCRETE: [f32; 4] = [0.48, 0.48, 0.50, 1.0];
const OLIVE: [f32; 4] = [0.35, 0.40, 0.32, 1.0];
const DARK_GREY: [f32; 4] = [0.26, 0.28, 0.30, 1.0];
const LIGHT_GREY: [f32; 4] = [0.68, 0.70, 0.72, 1.0];
const FED_BEIGE: [f32; 4] = [0.52, 0.50, 0.46, 1.0];
const BUNKER: [f32; 4] = [0.30, 0.32, 0.34, 1.0];
const STEEL: [f32; 4] = [0.40, 0.42, 0.45, 1.0];
const RUST_ACCENT: [f32; 4] = [0.36, 0.24, 0.18, 1.0]; // UCF industrial rust

const EARTH_BUILDINGS: &[(f32, f32, f32, f32, f32, [f32; 4])] = &[
    // Buenos Aires Metro — HQ, barracks, comm towers, pillboxes
    (0.0, -8.0, 16.0, 20.0, 12.0, LIGHT_GREY),        // Federation HQ tower
    (-22.0, -18.0, 12.0, 6.0, 10.0, CONCRETE),        // barracks block
    (18.0, -25.0, 8.0, 14.0, 8.0, DARK_GREY),         // comm tower
    (-5.0, 15.0, 18.0, 5.0, 16.0, BUNKER),           // pillbox bunker
    (25.0, 12.0, 10.0, 8.0, 10.0, OLIVE),
    (-35.0, 5.0, 8.0, 16.0, 6.0, DARK_GREY),         // narrow comm tower
    (30.0, -35.0, 14.0, 6.0, 12.0, RUST_ACCENT),     // depot with rust
    (-15.0, 32.0, 7.0, 12.0, 7.0, STEEL),            // relay tower
    (8.0, 40.0, 20.0, 10.0, 16.0, LIGHT_GREY),       // civic / admin block
    (-40.0, -25.0, 10.0, 6.0, 10.0, OLIVE),
    (38.0, 8.0, 12.0, 7.0, 9.0, FED_BEIGE),
    // Port District — dock control, storage, bunkers
    (-65.0, 95.0, 16.0, 8.0, 14.0, CONCRETE),
    (-80.0, 88.0, 10.0, 5.0, 12.0, BUNKER),
    (-52.0, 102.0, 12.0, 14.0, 8.0, DARK_GREY),      // control tower
    (-72.0, 78.0, 14.0, 5.0, 12.0, OLIVE),
    // Riverside
    (118.0, 35.0, 12.0, 6.0, 10.0, CONCRETE),
    (128.0, 42.0, 7.0, 10.0, 7.0, DARK_GREY),
    (122.0, 28.0, 10.0, 5.0, 8.0, OLIVE),
    // Northgate — gatehouse, pillbox, depot
    (-92.0, -88.0, 10.0, 6.0, 9.0, CONCRETE),
    (-98.0, -78.0, 16.0, 4.0, 14.0, BUNKER),         // low pillbox / wall
    (-88.0, -92.0, 8.0, 8.0, 8.0, FED_BEIGE),
    // Hillside
    (50.0, 108.0, 10.0, 6.0, 8.0, LIGHT_GREY),
    (56.0, 98.0, 12.0, 5.0, 10.0, OLIVE),
    // Outpost Seven — bunker, watchtower
    (-48.0, -122.0, 14.0, 6.0, 12.0, BUNKER),
    (-54.0, -116.0, 7.0, 12.0, 7.0, DARK_GREY),      // watchtower
    // Valley Farms — depot / storage
    (88.0, -98.0, 16.0, 5.0, 12.0, CONCRETE),
    (82.0, -108.0, 14.0, 6.0, 16.0, FED_BEIGE),
    (94.0, -92.0, 10.0, 4.0, 10.0, OLIVE),
    // Prairie Belt
    (-108.0, 70.0, 12.0, 5.0, 10.0, CONCRETE),
    (-100.0, 78.0, 10.0, 8.0, 8.0, STEEL),
    // Greenfield
    (138.0, -48.0, 10.0, 5.0, 8.0, OLIVE),
    (142.0, -42.0, 12.0, 4.0, 10.0, BUNKER),
];

/// Terrain height for building placement: use max of four footprint corners so the building never floats on slopes.
pub fn building_footprint_base_y<F: Fn(f32, f32) -> f32>(bx: f32, bz: f32, sx: f32, sz: f32, sample_terrain_y: F) -> f32 {
    let hx = sx * 0.5;
    let hz = sz * 0.5;
    let y0 = sample_terrain_y(bx - hx, bz - hz);
    let y1 = sample_terrain_y(bx + hx, bz - hz);
    let y2 = sample_terrain_y(bx + hx, bz + hz);
    let y3 = sample_terrain_y(bx - hx, bz + hz);
    y0.max(y1).max(y2).max(y3)
}

/// Building box data for physics: (world_x, world_z, size_x, size_y, size_z).
pub fn earth_building_boxes() -> &'static [(f32, f32, f32, f32, f32)] {
    static BOXES: std::sync::OnceLock<Vec<(f32, f32, f32, f32, f32)>> = std::sync::OnceLock::new();
    BOXES
        .get_or_init(|| {
            EARTH_BUILDINGS
                .iter()
                .map(|&(bx, bz, sx, sy, sz, _)| (bx, bz, sx, sy, sz))
                .collect()
        })
        .as_slice()
}

/// Margin so player center stays clear of building walls (avoids jitter on boundary).
const BUILDING_PUSH_MARGIN: f32 = 0.45;

/// Push (x, z) out of all building footprints so the player cannot walk through buildings.
/// Uses minimum displacement to nearest footprint edge; iterates until outside all (max 4).
pub fn push_out_of_building_footprints(mut x: f32, mut z: f32) -> (f32, f32) {
    const MAX_ITER: usize = 4;
    for _ in 0..MAX_ITER {
        let mut best_dx = 0.0f32;
        let mut best_dz = 0.0f32;
        let mut best_len_sq = f32::INFINITY;
        for &(bx, bz, sx, _sy, sz) in earth_building_boxes() {
            let half_x = sx * 0.5 + BUILDING_PUSH_MARGIN;
            let half_z = sz * 0.5 + BUILDING_PUSH_MARGIN;
            let left = bx - half_x;
            let right = bx + half_x;
            let front = bz - half_z;
            let back = bz + half_z;
            if x >= left && x <= right && z >= front && z <= back {
                let to_left = left - x;
                let to_right = right - x;
                let to_front = front - z;
                let to_back = back - z;
                let candidates = [(to_left, 0.0), (to_right, 0.0), (0.0, to_front), (0.0, to_back)];
                for (dx, dz) in candidates {
                    let len_sq = dx * dx + dz * dz;
                    if len_sq > 0.0 && len_sq < best_len_sq {
                        best_len_sq = len_sq;
                        best_dx = dx;
                        best_dz = dz;
                    }
                }
            }
        }
        if best_len_sq == f32::INFINITY {
            break;
        }
        x += best_dx;
        z += best_dz;
    }
    (x, z)
}

/// Spawn prefab buildings across the territory. Call when landing on Earth.
pub fn spawn_earth_buildings(
    world: &mut World,
    sample_terrain_y: impl Fn(f32, f32) -> f32,
) {
    for &(bx, bz, sx, sy, sz, color) in EARTH_BUILDINGS {
        let base_y = building_footprint_base_y(bx, bz, sx, sz, &sample_terrain_y);
        let y = base_y + sy * 0.5;
        let pos = Vec3::new(bx, y, bz);
        let scale = Vec3::new(sx, sy, sz);
        let t = Transform {
            position: pos,
            rotation: Quat::IDENTITY,
            scale,
        };
        let cached = CachedRenderData {
            matrix: t.to_matrix().to_cols_array_2d(),
            color,
            mesh_group: MESH_GROUP_BEVELED_CUBE,
        };
        world.spawn((t, cached));
    }
}
