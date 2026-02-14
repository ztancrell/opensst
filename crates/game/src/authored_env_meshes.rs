//! Authored STE-style environment meshes.
//! Replaces generic spheres with organic crater, blob, and rock shapes.
//! Coordinate system: Y-up (meshes sit on ground, Y+ is up).

use glam::Vec3;
use renderer::Vertex;

const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// Beveled cube for UCF structures (bases, walls, colonies).
/// Chamfered edges read as manufactured military prefab, not low-poly.
pub fn build_beveled_cube() -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();
    let b = 0.08; // Bevel size (chamfer)
    let s = 0.5 - b; // Face extent after bevel

    // 6 faces, each with 4 vertices (inset quad). 8 corners become chamfer triangles.
    // Face +X: x=0.5, y,z in [-s,s]
    let px = 0.5f32;
    v.push(Vertex::with_color([px, -s, -s], [1.0, 0.0, 0.0], [0.0, 1.0], WHITE));
    v.push(Vertex::with_color([px, -s, s], [1.0, 0.0, 0.0], [1.0, 1.0], WHITE));
    v.push(Vertex::with_color([px, s, s], [1.0, 0.0, 0.0], [1.0, 0.0], WHITE));
    v.push(Vertex::with_color([px, s, -s], [1.0, 0.0, 0.0], [0.0, 0.0], WHITE));
    // Face -X
    let mx = -0.5f32;
    v.push(Vertex::with_color([mx, -s, s], [-1.0, 0.0, 0.0], [0.0, 1.0], WHITE));
    v.push(Vertex::with_color([mx, -s, -s], [-1.0, 0.0, 0.0], [1.0, 1.0], WHITE));
    v.push(Vertex::with_color([mx, s, -s], [-1.0, 0.0, 0.0], [1.0, 0.0], WHITE));
    v.push(Vertex::with_color([mx, s, s], [-1.0, 0.0, 0.0], [0.0, 0.0], WHITE));
    // Face +Y
    let py = 0.5f32;
    v.push(Vertex::with_color([-s, py, -s], [0.0, 1.0, 0.0], [0.0, 1.0], WHITE));
    v.push(Vertex::with_color([s, py, -s], [0.0, 1.0, 0.0], [1.0, 1.0], WHITE));
    v.push(Vertex::with_color([s, py, s], [0.0, 1.0, 0.0], [1.0, 0.0], WHITE));
    v.push(Vertex::with_color([-s, py, s], [0.0, 1.0, 0.0], [0.0, 0.0], WHITE));
    // Face -Y
    let my = -0.5f32;
    v.push(Vertex::with_color([-s, my, s], [0.0, -1.0, 0.0], [0.0, 1.0], WHITE));
    v.push(Vertex::with_color([s, my, s], [0.0, -1.0, 0.0], [1.0, 1.0], WHITE));
    v.push(Vertex::with_color([s, my, -s], [0.0, -1.0, 0.0], [1.0, 0.0], WHITE));
    v.push(Vertex::with_color([-s, my, -s], [0.0, -1.0, 0.0], [0.0, 0.0], WHITE));
    // Face +Z
    let pz = 0.5f32;
    v.push(Vertex::with_color([-s, -s, pz], [0.0, 0.0, 1.0], [0.0, 1.0], WHITE));
    v.push(Vertex::with_color([-s, s, pz], [0.0, 0.0, 1.0], [1.0, 1.0], WHITE));
    v.push(Vertex::with_color([s, s, pz], [0.0, 0.0, 1.0], [1.0, 0.0], WHITE));
    v.push(Vertex::with_color([s, -s, pz], [0.0, 0.0, 1.0], [0.0, 0.0], WHITE));
    // Face -Z
    let mz = -0.5f32;
    v.push(Vertex::with_color([s, -s, mz], [0.0, 0.0, -1.0], [0.0, 1.0], WHITE));
    v.push(Vertex::with_color([s, s, mz], [0.0, 0.0, -1.0], [1.0, 1.0], WHITE));
    v.push(Vertex::with_color([-s, s, mz], [0.0, 0.0, -1.0], [1.0, 0.0], WHITE));
    v.push(Vertex::with_color([-s, -s, mz], [0.0, 0.0, -1.0], [0.0, 0.0], WHITE));

    // Chamfer triangles at 8 corners (each corner: 3 vertices on the 3 edges meeting there)
    let c = 0.5 - b;
    let chamfer_norm = std::f32::consts::FRAC_1_SQRT_2;
    // +X+Y+Z corner
    v.push(Vertex::with_color([c, 0.5, 0.5], [chamfer_norm, chamfer_norm, chamfer_norm], [0.5, 0.5], WHITE));
    v.push(Vertex::with_color([0.5, c, 0.5], [chamfer_norm, chamfer_norm, chamfer_norm], [0.5, 0.5], WHITE));
    v.push(Vertex::with_color([0.5, 0.5, c], [chamfer_norm, chamfer_norm, chamfer_norm], [0.5, 0.5], WHITE));
    // -X+Y+Z, +X-Y+Z, +X+Y-Z, -X-Y+Z, -X+Y-Z, +X-Y-Z, -X-Y-Z (7 more corners)
    for (sx, sy, sz) in [
        (-1i32, 1, 1), (1, -1, 1), (1, 1, -1), (-1, -1, 1), (-1, 1, -1), (1, -1, -1), (-1, -1, -1),
    ] {
        let nx = chamfer_norm * sx as f32;
        let ny = chamfer_norm * sy as f32;
        let nz = chamfer_norm * sz as f32;
        let cx = 0.5 * sx as f32;
        let cy = 0.5 * sy as f32;
        let cz = 0.5 * sz as f32;
        v.push(Vertex::with_color([cx + b * sx as f32, cy, cz], [nx, ny, nz], [0.5, 0.5], WHITE));
        v.push(Vertex::with_color([cx, cy + b * sy as f32, cz], [nx, ny, nz], [0.5, 0.5], WHITE));
        v.push(Vertex::with_color([cx, cy, cz + b * sz as f32], [nx, ny, nz], [0.5, 0.5], WHITE));
    }

    // Indices: 6 faces * 2 tris = 12 tris, 8 corners * 1 tri = 8 tris
    let base = 0u32;
    for f in 0..6 {
        let o = base + f * 4;
        i.extend([o, o + 1, o + 2, o, o + 2, o + 3]);
    }
    for c in 0..8 {
        let o = base + 24 + c * 3;
        i.extend([o, o + 1, o + 2]);
    }

    (v, i)
}

/// Organic crater for bug holes, hazard pools, burn craters.
/// Irregular rim, sloping sides, dark interior. Rendered flat on ground (Y-up).
pub fn build_bug_hole() -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();

    let radial_segments = 16;
    let ring_count = 6;
    let start_idx = v.len() as u32;

    // Radial mesh: center (depressed) to rim (raised, irregular)
    // Center vertex (bottom of crater)
    v.push(Vertex::with_color(
        [0.0, -0.15, 0.0],
        [0.0, 1.0, 0.0],
        [0.5, 0.5],
        WHITE,
    ));

    for ring in 1..=ring_count {
        let t = ring as f32 / ring_count as f32;
        // Radius: inner small, outer larger with irregular rim
        let base_r = 0.3 + t * 0.65;
        // Height: center low, slope up to rim, rim slightly raised
        let y = -0.15 + t * 0.25 + (t * std::f32::consts::PI * 0.5).sin() * 0.08;

        for s in 0..radial_segments {
            let angle = (s as f32 / radial_segments as f32) * std::f32::consts::TAU;
            // Irregular rim: vary radius per angle
            let irregular = 1.0
                + (angle * 3.0).sin() * 0.08
                + (angle * 5.0).cos() * 0.05
                + (t * 4.0).sin() * 0.03;
            let r = base_r * irregular;
            let x = r * angle.cos();
            let z = r * angle.sin();

            // Normal: slope inward-down toward center
            let to_center = Vec3::new(-x, 0.15 - y, -z).normalize();
            let n = to_center.to_array();

            v.push(Vertex::with_color(
                [x, y, z],
                n,
                [s as f32 / radial_segments as f32, t],
                WHITE,
            ));
        }
    }

    // Indices: triangles from center, then ring-to-ring
    for s in 0..radial_segments as u32 {
        let next_s = (s + 1) % radial_segments;
        i.push(start_idx);
        i.push(start_idx + 1 + s);
        i.push(start_idx + 1 + next_s);
    }

    for ring in 0..ring_count - 1 {
        let curr_base = start_idx + 1 + (ring * radial_segments) as u32;
        let next_base = curr_base + radial_segments as u32;
        for s in 0..radial_segments as u32 {
            let next_s = (s + 1) % radial_segments;
            i.push(curr_base + s);
            i.push(next_base + s);
            i.push(curr_base + next_s);
            i.push(curr_base + next_s);
            i.push(next_base + s);
            i.push(next_base + next_s);
        }
    }

    (v, i)
}

/// Organic blob for hive mounds, spore towers.
/// Deformed sphere — bulbous, resin-like, not geometric.
pub fn build_hive_mound() -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();

    let seg = 12usize;
    let ring = 8usize;
    let start_idx = v.len() as u32;

    for r in 0..=ring {
        let phi = std::f32::consts::PI * r as f32 / ring as f32;
        let y = phi.cos();
        let ring_r = phi.sin();

        for s in 0..=seg {
            let theta = std::f32::consts::TAU * s as f32 / seg as f32;
            let x = ring_r * theta.cos();
            let z = ring_r * theta.sin();

            // Deform: bulge on sides, flatten bottom, irregular bumps
            let deform = 1.0
                + (theta * 2.0).sin() * 0.12
                + (phi * 3.0).cos() * 0.08
                + (theta * 4.0 + phi * 2.0).sin() * 0.05;
            let scale = if phi < std::f32::consts::FRAC_PI_2 {
                deform * (0.9 + phi * 0.2) // Slightly flatter bottom
            } else {
                deform
            };

            let px = x * scale;
            let py = y * scale;
            let pz = z * scale;
            let pos = Vec3::new(px, py, pz);
            let n = pos.normalize();

            v.push(Vertex::with_color(
                pos.to_array(),
                n.to_array(),
                [s as f32 / seg as f32, r as f32 / ring as f32],
                WHITE,
            ));
        }
    }

    for r in 0..ring {
        let curr = start_idx + (r * (seg + 1)) as u32;
        let next = curr + (seg + 1) as u32;
        for s in 0..=seg as u32 {
            let ns = if s < seg as u32 { s + 1 } else { 0 };
            i.push(curr + s);
            i.push(next + s);
            i.push(curr + ns);
            i.push(curr + ns);
            i.push(next + s);
            i.push(next + ns);
        }
    }

    (v, i)
}

/// Clustered ovoids for egg clusters, bone piles.
/// Several overlapping blobs — organic, not uniform spheres.
pub fn build_egg_cluster() -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();

    // 5 ovoids in a cluster
    let eggs: [(Vec3, Vec3); 5] = [
        (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.5, 0.6, 0.5)),
        (Vec3::new(0.35, 0.1, 0.0), Vec3::new(0.35, 0.45, 0.35)),
        (Vec3::new(-0.2, 0.05, 0.3), Vec3::new(0.3, 0.4, 0.35)),
        (Vec3::new(0.1, -0.05, -0.25), Vec3::new(0.25, 0.35, 0.3)),
        (Vec3::new(-0.15, 0.15, -0.15), Vec3::new(0.2, 0.3, 0.25)),
    ];

    for (center, scale) in eggs {
        add_deformed_sphere(&mut v, &mut i, center, scale, 8, 6);
    }

    (v, i)
}

/// Rock variant 1: Angular, fractured.
pub fn build_rock() -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();

    // Irregular rock: deformed box/cube with chopped corners
    let seg = 8usize;
    let ring = 6usize;
    let start_idx = v.len() as u32;

    for r in 0..=ring {
        let phi = std::f32::consts::PI * r as f32 / ring as f32;
        let y = phi.cos();
        let ring_r = phi.sin();

        for s in 0..=seg {
            let theta = std::f32::consts::TAU * s as f32 / seg as f32;
            let x = ring_r * theta.cos();
            let z = ring_r * theta.sin();

            // Angular deformation: flatten some areas, sharpen edges
            let u = theta / std::f32::consts::TAU;
            let v_ = r as f32 / ring as f32;
            let deform = 1.0
                + ((u * 4.0).fract() - 0.5).abs() * 0.3
                + ((v_ * 3.0).fract() - 0.5).abs() * 0.2
                + (theta * 2.0 + phi).sin() * 0.1;

            let px = x * deform;
            let py = y * deform * 0.9;
            let pz = z * deform;
            let pos = Vec3::new(px, py, pz);
            let n = pos.normalize();

            v.push(Vertex::with_color(
                pos.to_array(),
                n.to_array(),
                [u, v_],
                WHITE,
            ));
        }
    }

    for r in 0..ring {
        let curr = start_idx + (r * (seg + 1)) as u32;
        let next = curr + (seg + 1) as u32;
        for s in 0..=seg as u32 {
            let ns = if s < seg as u32 { s + 1 } else { 0 };
            i.push(curr + s);
            i.push(next + s);
            i.push(curr + ns);
            i.push(curr + ns);
            i.push(next + s);
            i.push(next + ns);
        }
    }

    (v, i)
}

/// Rock variant 2: Chunk — flat base, angular top.
pub fn build_rock_chunk() -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();

    let seg = 6usize;
    let ring = 5usize;
    let start_idx = v.len() as u32;

    for r in 0..=ring {
        let phi = std::f32::consts::PI * 0.5 * r as f32 / ring as f32; // Half sphere (top only)
        let y = phi.cos();
        let ring_r = phi.sin();

        for s in 0..=seg {
            let theta = std::f32::consts::TAU * s as f32 / seg as f32;
            let x = ring_r * theta.cos();
            let z = ring_r * theta.sin();

            let deform = 1.0 + (theta * 2.0).sin() * 0.2 + (phi * 4.0).cos() * 0.15;
            let px = x * deform;
            let py = y * deform * 0.6; // Flatter
            let pz = z * deform;

            let pos = Vec3::new(px, py, pz);
            let n = pos.normalize();
            v.push(Vertex::with_color(
                pos.to_array(),
                n.to_array(),
                [s as f32 / seg as f32, r as f32 / ring as f32],
                WHITE,
            ));
        }
    }

    // Flat bottom cap
    let cap_idx = v.len() as u32;
    v.push(Vertex::with_color([0.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.5, 0.5], WHITE));
    let last_ring_base = start_idx + ((ring - 1) * (seg + 1)) as u32;
    for s in 0..seg as u32 {
        let next_s = (s + 1) % seg as u32;
        i.push(cap_idx);
        i.push(last_ring_base + next_s);
        i.push(last_ring_base + s);
    }

    for r in 0..ring - 1 {
        let curr = start_idx + (r * (seg + 1)) as u32;
        let next = curr + (seg + 1) as u32;
        for s in 0..=seg as u32 {
            let ns = if s < seg as u32 { s + 1 } else { 0 };
            i.push(curr + s);
            i.push(next + s);
            i.push(curr + ns);
            i.push(curr + ns);
            i.push(next + s);
            i.push(next + ns);
        }
    }

    (v, i)
}

/// Rock variant 3: Boulder — rounder, weathered.
pub fn build_rock_boulder() -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();

    let seg = 10usize;
    let ring = 8usize;
    let start_idx = v.len() as u32;

    for r in 0..=ring {
        let phi = std::f32::consts::PI * r as f32 / ring as f32;
        let y = phi.cos();
        let ring_r = phi.sin();

        for s in 0..=seg {
            let theta = std::f32::consts::TAU * s as f32 / seg as f32;
            let x = ring_r * theta.cos();
            let z = ring_r * theta.sin();

            let deform = 1.0 + (theta * 3.0 + phi * 2.0).sin() * 0.06;
            let pos = Vec3::new(x * deform, y * deform, z * deform);
            let n = pos.normalize();
            v.push(Vertex::with_color(
                pos.to_array(),
                n.to_array(),
                [s as f32 / seg as f32, r as f32 / ring as f32],
                WHITE,
            ));
        }
    }

    for r in 0..ring {
        let curr = start_idx + (r * (seg + 1)) as u32;
        let next = curr + (seg + 1) as u32;
        for s in 0..=seg as u32 {
            let ns = if s < seg as u32 { s + 1 } else { 0 };
            i.push(curr + s);
            i.push(next + s);
            i.push(curr + ns);
            i.push(curr + ns);
            i.push(next + s);
            i.push(next + ns);
        }
    }

    (v, i)
}

fn add_deformed_sphere(
    v: &mut Vec<Vertex>,
    i: &mut Vec<u32>,
    center: Vec3,
    scale: Vec3,
    segments: u32,
    rings: u32,
) {
    let seg = segments as usize;
    let ring = rings as usize;
    let start_idx = v.len() as u32;

    for r in 0..=ring {
        let phi = std::f32::consts::PI * r as f32 / ring as f32;
        let sy = phi.cos();
        let ring_r = phi.sin();

        for s in 0..=seg {
            let theta = std::f32::consts::TAU * s as f32 / seg as f32;
            let sx = ring_r * theta.cos();
            let sz = ring_r * theta.sin();

            let pos = center + Vec3::new(sx * scale.x, sy * scale.y, sz * scale.z);
            let n = Vec3::new(sx, sy, sz).normalize();

            v.push(Vertex::with_color(
                pos.to_array(),
                n.to_array(),
                [s as f32 / seg as f32, r as f32 / ring as f32],
                WHITE,
            ));
        }
    }

    for r in 0..ring {
        let curr = start_idx + (r * (seg + 1)) as u32;
        let next = curr + (seg + 1) as u32;
        for s in 0..=seg as u32 {
            let ns = if s < seg as u32 { s + 1 } else { 0 };
            i.push(curr + s);
            i.push(next + s);
            i.push(curr + ns);
            i.push(curr + ns);
            i.push(next + s);
            i.push(next + ns);
        }
    }
}
