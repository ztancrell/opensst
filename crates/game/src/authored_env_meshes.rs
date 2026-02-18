//! Authored STE-style environment meshes.
//! Replaces generic spheres with organic crater, blob, and rock shapes.
//! Coordinate system: Y-up (meshes sit on ground, Y+ is up).

use glam::Vec3;
use renderer::Vertex;

const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// Beveled cube for UCF structures (bases, walls, colonies).
/// Chamfered edges: each face is a pentagon (one corner cut by the bevel), corners are triangles.
/// Vertices are shared so no gaps; winding matches renderer cube (CCW from outside).
pub fn build_beveled_cube() -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();
    let b = 0.08f32; // Bevel (chamfer) size
    let s = 0.5 - b; // Face extent: full corners at ±s, cut corners use 0.5 on one axis

    // Chamfer normal (45° bevel)
    let cn = std::f32::consts::FRAC_1_SQRT_2;

    // Helper: add vertex with color
    let mut add = |pos: [f32; 3], n: [f32; 3], uv: [f32; 2]| {
        v.push(Vertex::with_color(pos, n, uv, WHITE));
    };

    // ─── 6 faces: each is a pentagon (one corner replaced by two chamfer-edge vertices) ───
    // Order per face: 4 "full" corners then the 2 chamfer vertices at the cut corner (CCW from outside).
    // Face +X (right): corners (-s,-s), (-s,+s), (+s,+s) cut, (+s,-s), then chamfer (0.5,c), (c,0.5) in +Y+Z
    add([0.5, -s, s], [1.0, 0.0, 0.0], [0.0, 1.0]);
    add([0.5, -s, -s], [1.0, 0.0, 0.0], [1.0, 1.0]);
    add([0.5, s, -s], [1.0, 0.0, 0.0], [1.0, 0.0]);
    add([0.5, 0.5, s], [1.0, 0.0, 0.0], [0.5, 0.0]); // chamfer edge
    add([0.5, s, 0.5], [1.0, 0.0, 0.0], [0.0, 0.5]);
    // Face -X (left): cut corner at -Y+Z; CCW order (-s,-s), (-s,0.5), (0.5,s), (s,s), (s,-s)
    add([-0.5, -s, -s], [-1.0, 0.0, 0.0], [0.0, 1.0]);
    add([-0.5, -s, 0.5], [-1.0, 0.0, 0.0], [0.0, 0.5]);
    add([-0.5, 0.5, s], [-1.0, 0.0, 0.0], [0.5, 0.0]);
    add([-0.5, s, s], [-1.0, 0.0, 0.0], [1.0, 0.0]);
    add([-0.5, s, -s], [-1.0, 0.0, 0.0], [1.0, 1.0]);
    // Face +Y (top): cut at +X+Z
    add([-s, 0.5, s], [0.0, 1.0, 0.0], [0.0, 1.0]);
    add([s, 0.5, s], [0.0, 1.0, 0.0], [1.0, 1.0]);
    add([s, 0.5, -s], [0.0, 1.0, 0.0], [1.0, 0.0]);
    add([s, 0.5, 0.5], [0.0, 1.0, 0.0], [0.5, 0.0]);
    add([0.5, 0.5, s], [0.0, 1.0, 0.0], [0.0, 0.5]);
    // Face -Y (bottom): cut at -X-Z; chamfer verts (-0.5,-0.5,-s), (-s,-0.5,-0.5)
    add([-0.5, -0.5, -s], [0.0, -1.0, 0.0], [0.0, 0.5]);
    add([-s, -0.5, -0.5], [0.0, -1.0, 0.0], [0.5, 0.0]);
    add([s, -0.5, -s], [0.0, -1.0, 0.0], [1.0, 1.0]);
    add([s, -0.5, s], [0.0, -1.0, 0.0], [1.0, 0.0]);
    add([-s, -0.5, s], [0.0, -1.0, 0.0], [0.0, 0.0]);
    // Face +Z (front): cut at +X+Y
    add([-s, -s, 0.5], [0.0, 0.0, 1.0], [0.0, 1.0]);
    add([s, -s, 0.5], [0.0, 0.0, 1.0], [1.0, 1.0]);
    add([s, s, 0.5], [0.0, 0.0, 1.0], [1.0, 0.0]);
    add([s, 0.5, 0.5], [0.0, 0.0, 1.0], [0.5, 0.0]);
    add([0.5, s, 0.5], [0.0, 0.0, 1.0], [0.0, 0.5]);
    // Face -Z (back): cut at -X-Y; chamfer verts (-s,-0.5,-0.5), (-0.5,-s,-0.5)
    add([-s, -0.5, -0.5], [0.0, 0.0, -1.0], [0.5, 0.0]);
    add([-0.5, -s, -0.5], [0.0, 0.0, -1.0], [0.0, 0.5]);
    add([s, -s, -0.5], [0.0, 0.0, -1.0], [1.0, 1.0]);
    add([s, s, -0.5], [0.0, 0.0, -1.0], [1.0, 0.0]);
    add([-s, s, -0.5], [0.0, 0.0, -1.0], [0.0, 0.0]);

    // Pentagon indices: 5 vertices → 3 triangles (0,1,2), (0,2,3), (0,3,4)
    let mut pentagon = |base: u32| {
        i.push(base);
        i.push(base + 1);
        i.push(base + 2);
        i.push(base);
        i.push(base + 2);
        i.push(base + 3);
        i.push(base);
        i.push(base + 3);
        i.push(base + 4);
    };
    for f in 0..6 {
        pentagon(f * 5);
    }

    // ─── 8 chamfer corner triangles (shared conceptually with face edges; we add unique verts for clean normals) ───
    let base_c = 6 * 5;
    // +X+Y+Z
    add([s, 0.5, 0.5], [cn, cn, cn], [0.5, 0.5]);
    add([0.5, s, 0.5], [cn, cn, cn], [0.5, 0.5]);
    add([0.5, 0.5, s], [cn, cn, cn], [0.5, 0.5]);
    // -X+Y+Z
    add([-s, 0.5, 0.5], [-cn, cn, cn], [0.5, 0.5]);
    add([-0.5, s, 0.5], [-cn, cn, cn], [0.5, 0.5]);
    add([-0.5, 0.5, s], [-cn, cn, cn], [0.5, 0.5]);
    // +X-Y+Z
    add([s, -0.5, 0.5], [cn, -cn, cn], [0.5, 0.5]);
    add([0.5, -s, 0.5], [cn, -cn, cn], [0.5, 0.5]);
    add([0.5, -0.5, s], [cn, -cn, cn], [0.5, 0.5]);
    // +X+Y-Z
    add([s, 0.5, -0.5], [cn, cn, -cn], [0.5, 0.5]);
    add([0.5, s, -0.5], [cn, cn, -cn], [0.5, 0.5]);
    add([0.5, 0.5, -s], [cn, cn, -cn], [0.5, 0.5]);
    // -X-Y+Z
    add([-s, -0.5, 0.5], [-cn, -cn, cn], [0.5, 0.5]);
    add([-0.5, -s, 0.5], [-cn, -cn, cn], [0.5, 0.5]);
    add([-0.5, -0.5, s], [-cn, -cn, cn], [0.5, 0.5]);
    // -X+Y-Z
    add([-s, 0.5, -0.5], [-cn, cn, -cn], [0.5, 0.5]);
    add([-0.5, s, -0.5], [-cn, cn, -cn], [0.5, 0.5]);
    add([-0.5, 0.5, -s], [-cn, cn, -cn], [0.5, 0.5]);
    // +X-Y-Z
    add([s, -0.5, -0.5], [cn, -cn, -cn], [0.5, 0.5]);
    add([0.5, -s, -0.5], [cn, -cn, -cn], [0.5, 0.5]);
    add([0.5, -0.5, -s], [cn, -cn, -cn], [0.5, 0.5]);
    // -X-Y-Z
    add([-s, -0.5, -0.5], [-cn, -cn, -cn], [0.5, 0.5]);
    add([-0.5, -s, -0.5], [-cn, -cn, -cn], [0.5, 0.5]);
    add([-0.5, -0.5, -s], [-cn, -cn, -cn], [0.5, 0.5]);

    // Chamfer triangle winding: outward from corner. Order so normal points out.
    let mut chamfer_tri = |base: u32, swap: bool| {
        if swap {
            i.extend([base, base + 2, base + 1]);
        } else {
            i.extend([base, base + 1, base + 2]);
        }
    };
    chamfer_tri(base_c + 0, false);   // +X+Y+Z
    chamfer_tri(base_c + 3, true);    // -X+Y+Z
    chamfer_tri(base_c + 6, true);    // +X-Y+Z
    chamfer_tri(base_c + 9, true);    // +X+Y-Z
    chamfer_tri(base_c + 12, false);  // -X-Y+Z
    chamfer_tri(base_c + 15, true);   // -X+Y-Z
    chamfer_tri(base_c + 18, true);   // +X-Y-Z
    chamfer_tri(base_c + 21, true);   // -X-Y-Z

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
        let base_r = 0.3 + t * 0.65;
        let y = -0.15 + t * 0.25 + (t * std::f32::consts::PI * 0.5).sin() * 0.08;

        for s in 0..radial_segments {
            let angle = (s as f32 / radial_segments as f32) * std::f32::consts::TAU;
            let irregular = 1.0
                + (angle * 3.0).sin() * 0.08
                + (angle * 5.0).cos() * 0.05
                + (t * 4.0).sin() * 0.03;
            let r = base_r * irregular;
            let x = r * angle.cos();
            let z = r * angle.sin();

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

/// Hive cave / tunnel entrance: arched mouth like Minecraft surface caves.
/// Wider than tall at the opening; deep recess; irregular organic rim. Y-up, sits on ground.
pub fn build_hive_cave_entrance() -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();
    let radial_segments = 20;
    let ring_count = 8;
    let start_idx = v.len() as u32;

    // Center vertex (deep inside the tunnel)
    v.push(Vertex::with_color(
        [0.0, -0.35, 0.0],
        [0.0, 1.0, 0.0],
        [0.5, 0.5],
        WHITE,
    ));

    for ring in 1..=ring_count {
        let t = ring as f32 / ring_count as f32;
        // Elliptical radius: wider than deep (cave mouth shape)
        let base_rx = 0.25 + t * 0.7;
        let base_rz = 0.2 + t * 0.55;
        // Arched profile: top of arch higher (like a tunnel)
        let arch = (t * std::f32::consts::PI * 0.5).sin();
        let y = -0.35 + t * 0.4 + arch * 0.25;

        for s in 0..radial_segments {
            let angle = (s as f32 / radial_segments as f32) * std::f32::consts::TAU;
            let irregular = 1.0
                + (angle * 2.0).sin() * 0.12
                + (angle * 4.0).cos() * 0.06
                + (t * 3.0).sin() * 0.04;
            let rx = base_rx * irregular;
            let rz = base_rz * irregular;
            let x = rx * angle.cos();
            let z = rz * angle.sin();

            let to_center = Vec3::new(-x, 0.35 - y, -z).normalize();
            v.push(Vertex::with_color(
                [x, y, z],
                to_center.to_array(),
                [s as f32 / radial_segments as f32, t],
                WHITE,
            ));
        }
    }

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

            let deform = 1.0
                + (theta * 2.0).sin() * 0.12
                + (phi * 3.0).cos() * 0.08
                + (theta * 4.0 + phi * 2.0).sin() * 0.05;
            let scale = if phi < std::f32::consts::FRAC_PI_2 {
                deform * (0.9 + phi * 0.2)
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
pub fn build_egg_cluster() -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();

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

/// Rock variant 1: Angular, fractured — more irregular and rock-like (facets, flat-ish base).
pub fn build_rock() -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();

    let seg = 10usize;
    let ring = 7usize;
    let start_idx = v.len() as u32;

    for r in 0..=ring {
        let phi = std::f32::consts::PI * r as f32 / ring as f32;
        let y = phi.cos();
        let ring_r = phi.sin();

        for s in 0..=seg {
            let theta = std::f32::consts::TAU * s as f32 / seg as f32;
            let x = ring_r * theta.cos();
            let z = ring_r * theta.sin();

            let u = theta / std::f32::consts::TAU;
            let v_ = r as f32 / ring as f32;
            // Stronger asymmetric deformation: facets and bumps for a fractured look
            let facet = ((u * 5.0).floor() * 0.12 + (v_ * 4.0).floor() * 0.08).abs();
            let bump = (theta * 3.0 + phi * 2.0).sin() * 0.15 + (theta * 1.5).cos() * 0.1;
            let deform = 1.0 + facet + bump;
            // Flatten bottom so rock sits on ground
            let bottom_flat = (1.0 - (v_ * 1.4).min(1.0)) * 0.25;
            let py_scale = 0.85 - bottom_flat;

            let px = x * deform;
            let py = y * deform * py_scale;
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

/// Rock variant 2: Chunk — flat base, angular fractured top (broken rock slab).
pub fn build_rock_chunk() -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();

    let seg = 8usize;
    let ring = 6usize;
    let start_idx = v.len() as u32;

    for r in 0..=ring {
        let phi = std::f32::consts::PI * 0.5 * r as f32 / ring as f32;
        let y = phi.cos();
        let ring_r = phi.sin();

        for s in 0..=seg {
            let theta = std::f32::consts::TAU * s as f32 / seg as f32;
            let x = ring_r * theta.cos();
            let z = ring_r * theta.sin();

            // Angular facets on top, flatter toward base
            let facet = ((theta * 2.5).floor() * 0.08 + (phi * 3.0).floor() * 0.05).abs();
            let deform = 1.0 + (theta * 2.0).sin() * 0.22 + (phi * 4.0).cos() * 0.18 + facet;
            let py_scale = 0.55 - (1.0 - r as f32 / ring as f32) * 0.1; // slightly flatter base
            let px = x * deform;
            let py = y * deform * py_scale;
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

    let cap_idx = v.len() as u32;
    v.push(Vertex::with_color([0.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.5, 0.5], WHITE));
    let last_ring_base = start_idx + ((ring - 1) * (seg + 1)) as u32;
    let seg_u = seg as u32;
    for s in 0..=seg_u {
        let next_s = (s + 1) % (seg_u + 1);
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

/// Rock variant 3: Boulder — rounder but irregular (weathered, lumpy).
pub fn build_rock_boulder() -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();

    let seg = 12usize;
    let ring = 9usize;
    let start_idx = v.len() as u32;

    for r in 0..=ring {
        let phi = std::f32::consts::PI * r as f32 / ring as f32;
        let y = phi.cos();
        let ring_r = phi.sin();

        for s in 0..=seg {
            let theta = std::f32::consts::TAU * s as f32 / seg as f32;
            let x = ring_r * theta.cos();
            let z = ring_r * theta.sin();

            // Multiple noise scales for lumpy boulder; slight flat bottom
            let lump = (theta * 3.0 + phi * 2.0).sin() * 0.08
                + (theta * 5.0 + phi * 3.0).cos() * 0.05
                + (theta * 1.0 + phi * 4.0).sin() * 0.04;
            let deform = 1.0 + lump;
            let bottom = (1.0 - (r as f32 / ring as f32).min(0.5) * 2.0).max(0.0) * 0.12;
            let py_scale = 1.0 - bottom;
            let pos = Vec3::new(x * deform, y * deform * py_scale, z * deform);
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

/// Add an axis-aligned box (center, half-extents) for building compound meshes.
fn add_box(
    v: &mut Vec<Vertex>,
    i: &mut Vec<u32>,
    center: [f32; 3],
    half: [f32; 3],
) {
    let [cx, cy, cz] = center;
    let [hx, hy, hz] = half;
    let base = v.len() as u32;
    // 24 vertices (4 per face, CCW from outside) — same order as renderer cube
    let mut add = |pos: [f32; 3], n: [f32; 3], uv: [f32; 2]| {
        v.push(Vertex::with_color(pos, n, uv, WHITE));
    };
    add([cx + hx, cy - hy, cz + hz], [1.0, 0.0, 0.0], [0.0, 1.0]);
    add([cx + hx, cy - hy, cz - hz], [1.0, 0.0, 0.0], [1.0, 1.0]);
    add([cx + hx, cy + hy, cz - hz], [1.0, 0.0, 0.0], [1.0, 0.0]);
    add([cx + hx, cy + hy, cz + hz], [1.0, 0.0, 0.0], [0.0, 0.0]);
    add([cx - hx, cy - hy, cz - hz], [-1.0, 0.0, 0.0], [0.0, 1.0]);
    add([cx - hx, cy - hy, cz + hz], [-1.0, 0.0, 0.0], [1.0, 1.0]);
    add([cx - hx, cy + hy, cz + hz], [-1.0, 0.0, 0.0], [1.0, 0.0]);
    add([cx - hx, cy + hy, cz - hz], [-1.0, 0.0, 0.0], [0.0, 0.0]);
    add([cx - hx, cy + hy, cz + hz], [0.0, 1.0, 0.0], [0.0, 1.0]);
    add([cx + hx, cy + hy, cz + hz], [0.0, 1.0, 0.0], [1.0, 1.0]);
    add([cx + hx, cy + hy, cz - hz], [0.0, 1.0, 0.0], [1.0, 0.0]);
    add([cx - hx, cy + hy, cz - hz], [0.0, 1.0, 0.0], [0.0, 0.0]);
    add([cx - hx, cy - hy, cz - hz], [0.0, -1.0, 0.0], [0.0, 1.0]);
    add([cx + hx, cy - hy, cz - hz], [0.0, -1.0, 0.0], [1.0, 1.0]);
    add([cx + hx, cy - hy, cz + hz], [0.0, -1.0, 0.0], [1.0, 0.0]);
    add([cx - hx, cy - hy, cz + hz], [0.0, -1.0, 0.0], [0.0, 0.0]);
    add([cx - hx, cy - hy, cz + hz], [0.0, 0.0, 1.0], [0.0, 1.0]);
    add([cx + hx, cy - hy, cz + hz], [0.0, 0.0, 1.0], [1.0, 1.0]);
    add([cx + hx, cy + hy, cz + hz], [0.0, 0.0, 1.0], [1.0, 0.0]);
    add([cx - hx, cy + hy, cz + hz], [0.0, 0.0, 1.0], [0.0, 0.0]);
    add([cx + hx, cy - hy, cz - hz], [0.0, 0.0, -1.0], [0.0, 1.0]);
    add([cx - hx, cy - hy, cz - hz], [0.0, 0.0, -1.0], [1.0, 1.0]);
    add([cx - hx, cy + hy, cz - hz], [0.0, 0.0, -1.0], [1.0, 0.0]);
    add([cx + hx, cy + hy, cz - hz], [0.0, 0.0, -1.0], [0.0, 0.0]);
    for face in 0..6u32 {
        let o = base + face * 4;
        i.extend([o, o + 1, o + 2, o, o + 2, o + 3]);
    }
}

/// Heinlein Skinnies: tall, gaunt humanoids — subjugated race, elongated and thin.
/// Proportions from the book: "skinny" build, long limbs, small head, narrow torso.
/// Unit height ~1.0; game scales by SkinnyType. Y-up, standing.
pub fn build_skinny() -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();
    // Torso: narrow and tall (gaunt, ribby silhouette — very thin in X and Z)
    add_box(&mut v, &mut i, [0.0, 0.28, 0.0], [0.04, 0.36, 0.025]);
    // Neck: thin stalk connecting to head
    add_box(&mut v, &mut i, [0.0, 0.68, 0.0], [0.02, 0.06, 0.018]);
    // Head: small, elongated (oval — taller than wide), alien
    add_box(&mut v, &mut i, [0.0, 0.82, 0.0], [0.035, 0.06, 0.03]);
    // Legs: long thin stalks (Heinlein "spindly")
    add_box(&mut v, &mut i, [-0.028, -0.22, 0.0], [0.018, 0.24, 0.016]);
    add_box(&mut v, &mut i, [0.028, -0.22, 0.0], [0.018, 0.24, 0.016]);
    // Arms: very long and thin (distinctly non-human proportion)
    add_box(&mut v, &mut i, [-0.07, 0.26, 0.0], [0.012, 0.26, 0.012]);
    add_box(&mut v, &mut i, [0.07, 0.26, 0.0], [0.012, 0.26, 0.012]);
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
