//! Authored STE-style bug meshes.
//! Replaces procedural BugMeshGenerator with hand-crafted silhouettes.
//! Coordinate system: Y-up, Z-forward (bug faces +Z).

use glam::Vec3;
use renderer::Vertex;

const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// Build mesh data (vertices, indices) for an authored bug.
/// Meshes are in unit space; BugType::scale() is applied at render time.
pub fn build_warrior() -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();

    // Body: 3 segments, oval cross-section, armor plates
    add_capsule(&mut v, &mut i, Vec3::new(0.0, 0.15, -0.3), Vec3::new(0.0, 0.2, 0.0), 0.22, 0.28, 10, 4);
    add_capsule(&mut v, &mut i, Vec3::new(0.0, 0.18, 0.0), Vec3::new(0.0, 0.2, 0.35), 0.28, 0.32, 10, 4);
    add_capsule(&mut v, &mut i, Vec3::new(0.0, 0.2, 0.35), Vec3::new(0.0, 0.18, 0.55), 0.25, 0.22, 10, 4);

    // Head: wedge, forward
    add_capsule(&mut v, &mut i, Vec3::new(0.0, 0.22, 0.55), Vec3::new(0.0, 0.2, 0.85), 0.18, 0.12, 8, 4);

    // Eyes (compound bumps)
    add_sphere(&mut v, &mut i, Vec3::new(-0.12, 0.28, 0.72), 0.06, 6, 4);
    add_sphere(&mut v, &mut i, Vec3::new(0.12, 0.28, 0.72), 0.06, 6, 4);

    // Mandibles
    add_leg_capsule(&mut v, &mut i, Vec3::new(-0.08, 0.18, 0.88), Vec3::new(-0.05, 0.08, 1.05), 0.03, 8);
    add_leg_capsule(&mut v, &mut i, Vec3::new(0.08, 0.18, 0.88), Vec3::new(0.05, 0.08, 1.05), 0.03, 8);

    // 6 legs
    let leg_positions = [
        (Vec3::new(-0.25, 0.05, -0.15), Vec3::new(-0.35, -0.35, -0.2)),
        (Vec3::new(0.25, 0.05, -0.15), Vec3::new(0.35, -0.35, -0.2)),
        (Vec3::new(-0.28, 0.08, 0.1), Vec3::new(-0.38, -0.38, 0.0)),
        (Vec3::new(0.28, 0.08, 0.1), Vec3::new(0.38, -0.38, 0.0)),
        (Vec3::new(-0.25, 0.1, 0.35), Vec3::new(-0.32, -0.35, 0.25)),
        (Vec3::new(0.25, 0.1, 0.35), Vec3::new(0.32, -0.35, 0.25)),
    ];
    for (base_pos, tip) in leg_positions {
        add_leg_capsule(&mut v, &mut i, base_pos, tip, 0.04, 6);
    }

    (v, i)
}

pub fn build_charger() -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();

    // Sleek body: 2 segments, thinner
    add_capsule(&mut v, &mut i, Vec3::new(0.0, 0.12, -0.2), Vec3::new(0.0, 0.15, 0.2), 0.18, 0.22, 8, 4);
    add_capsule(&mut v, &mut i, Vec3::new(0.0, 0.15, 0.2), Vec3::new(0.0, 0.14, 0.6), 0.22, 0.18, 8, 4);

    // Head: streamlined
    add_capsule(&mut v, &mut i, Vec3::new(0.0, 0.16, 0.6), Vec3::new(0.0, 0.14, 0.9), 0.14, 0.08, 8, 4);

    // Sharp mandibles
    add_leg_capsule(&mut v, &mut i, Vec3::new(-0.06, 0.12, 0.92), Vec3::new(-0.04, 0.04, 1.08), 0.02, 6);
    add_leg_capsule(&mut v, &mut i, Vec3::new(0.06, 0.12, 0.92), Vec3::new(0.04, 0.04, 1.08), 0.02, 6);

    // 6 legs - longer, more angled for speed
    let leg_positions = [
        (Vec3::new(-0.22, 0.02, -0.1), Vec3::new(-0.35, -0.4, -0.15)),
        (Vec3::new(0.22, 0.02, -0.1), Vec3::new(0.35, -0.4, -0.15)),
        (Vec3::new(-0.24, 0.04, 0.15), Vec3::new(-0.38, -0.42, 0.05)),
        (Vec3::new(0.24, 0.04, 0.15), Vec3::new(0.38, -0.42, 0.05)),
        (Vec3::new(-0.22, 0.06, 0.4), Vec3::new(-0.32, -0.38, 0.35)),
        (Vec3::new(0.22, 0.06, 0.4), Vec3::new(0.32, -0.38, 0.35)),
    ];
    for (base_pos, tip) in leg_positions {
        add_leg_capsule(&mut v, &mut i, base_pos, tip, 0.03, 6);
    }

    (v, i)
}

pub fn build_spitter() -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();

    // Bloated abdomen (acid sac) - large rear
    add_capsule(&mut v, &mut i, Vec3::new(0.0, 0.18, -0.4), Vec3::new(0.0, 0.22, 0.0), 0.25, 0.32, 10, 4);
    add_sphere(&mut v, &mut i, Vec3::new(0.0, 0.2, -0.55), 0.35, 10, 6); // Acid sac bulge

    // Thorax
    add_capsule(&mut v, &mut i, Vec3::new(0.0, 0.22, 0.0), Vec3::new(0.0, 0.2, 0.35), 0.28, 0.25, 10, 4);

    // Small head, no mandibles
    add_capsule(&mut v, &mut i, Vec3::new(0.0, 0.2, 0.35), Vec3::new(0.0, 0.18, 0.6), 0.16, 0.12, 8, 4);

    // Spitter mouth (rounded, no pincers)
    add_sphere(&mut v, &mut i, Vec3::new(0.0, 0.18, 0.68), 0.06, 6, 4);

    // 6 legs
    let leg_positions = [
        (Vec3::new(-0.28, 0.05, -0.25), Vec3::new(-0.38, -0.32, -0.3)),
        (Vec3::new(0.28, 0.05, -0.25), Vec3::new(0.38, -0.32, -0.3)),
        (Vec3::new(-0.3, 0.08, -0.05), Vec3::new(-0.4, -0.35, -0.1)),
        (Vec3::new(0.3, 0.08, -0.05), Vec3::new(0.4, -0.35, -0.1)),
        (Vec3::new(-0.26, 0.1, 0.2), Vec3::new(-0.34, -0.33, 0.15)),
        (Vec3::new(0.26, 0.1, 0.2), Vec3::new(0.34, -0.33, 0.15)),
    ];
    for (base_pos, tip) in leg_positions {
        add_leg_capsule(&mut v, &mut i, base_pos, tip, 0.045, 6);
    }

    (v, i)
}

pub fn build_tanker() -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();

    // Heavy body: 4 segments, thick armor
    add_capsule(&mut v, &mut i, Vec3::new(0.0, 0.35, -0.5), Vec3::new(0.0, 0.4, -0.2), 0.4, 0.45, 12, 4);
    add_capsule(&mut v, &mut i, Vec3::new(0.0, 0.4, -0.2), Vec3::new(0.0, 0.42, 0.15), 0.45, 0.5, 12, 4);
    add_capsule(&mut v, &mut i, Vec3::new(0.0, 0.42, 0.15), Vec3::new(0.0, 0.4, 0.5), 0.5, 0.48, 12, 4);
    add_capsule(&mut v, &mut i, Vec3::new(0.0, 0.4, 0.5), Vec3::new(0.0, 0.35, 0.75), 0.42, 0.35, 12, 4);

    // Head: armored
    add_capsule(&mut v, &mut i, Vec3::new(0.0, 0.38, 0.75), Vec3::new(0.0, 0.32, 1.0), 0.28, 0.2, 10, 4);

    // Mandibles - thick
    add_leg_capsule(&mut v, &mut i, Vec3::new(-0.15, 0.28, 1.0), Vec3::new(-0.1, 0.15, 1.2), 0.05, 8);
    add_leg_capsule(&mut v, &mut i, Vec3::new(0.15, 0.28, 1.0), Vec3::new(0.1, 0.15, 1.2), 0.05, 8);

    // 8 legs - thick
    let leg_positions = [
        (Vec3::new(-0.45, 0.15, -0.4), Vec3::new(-0.6, -0.4, -0.45)),
        (Vec3::new(0.45, 0.15, -0.4), Vec3::new(0.6, -0.4, -0.45)),
        (Vec3::new(-0.5, 0.2, -0.15), Vec3::new(-0.65, -0.42, -0.25)),
        (Vec3::new(0.5, 0.2, -0.15), Vec3::new(0.65, -0.42, -0.25)),
        (Vec3::new(-0.52, 0.22, 0.15), Vec3::new(-0.65, -0.4, 0.05)),
        (Vec3::new(0.52, 0.22, 0.15), Vec3::new(0.65, -0.4, 0.05)),
        (Vec3::new(-0.48, 0.2, 0.5), Vec3::new(-0.58, -0.38, 0.45)),
        (Vec3::new(0.48, 0.2, 0.5), Vec3::new(0.58, -0.38, 0.45)),
    ];
    for (base_pos, tip) in leg_positions {
        add_leg_capsule(&mut v, &mut i, base_pos, tip, 0.06, 6);
    }

    (v, i)
}

pub fn build_hopper() -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();

    // Compact body: 2 segments
    add_capsule(&mut v, &mut i, Vec3::new(0.0, 0.12, -0.15), Vec3::new(0.0, 0.15, 0.1), 0.2, 0.24, 8, 4);
    add_capsule(&mut v, &mut i, Vec3::new(0.0, 0.15, 0.1), Vec3::new(0.0, 0.16, 0.45), 0.24, 0.2, 8, 4);

    // Head
    add_capsule(&mut v, &mut i, Vec3::new(0.0, 0.18, 0.45), Vec3::new(0.0, 0.16, 0.7), 0.16, 0.1, 8, 4);

    // Mandibles
    add_leg_capsule(&mut v, &mut i, Vec3::new(-0.07, 0.14, 0.72), Vec3::new(-0.05, 0.06, 0.88), 0.025, 6);
    add_leg_capsule(&mut v, &mut i, Vec3::new(0.07, 0.14, 0.72), Vec3::new(0.05, 0.06, 0.88), 0.025, 6);

    // Wings (triangular, swept back)
    add_wing_quad(&mut v, &mut i, Vec3::new(-0.2, 0.25, 0.1), Vec3::new(-0.55, 0.35, -0.2), Vec3::new(-0.25, 0.2, -0.15), -1.0);
    add_wing_quad(&mut v, &mut i, Vec3::new(0.2, 0.25, 0.1), Vec3::new(0.25, 0.2, -0.15), Vec3::new(0.55, 0.35, -0.2), 1.0);

    // 6 legs - powerful, jumping
    let leg_positions = [
        (Vec3::new(-0.24, 0.02, -0.08), Vec3::new(-0.38, -0.42, -0.12)),
        (Vec3::new(0.24, 0.02, -0.08), Vec3::new(0.38, -0.42, -0.12)),
        (Vec3::new(-0.26, 0.05, 0.12), Vec3::new(-0.4, -0.44, 0.08)),
        (Vec3::new(0.26, 0.05, 0.12), Vec3::new(0.4, -0.44, 0.08)),
        (Vec3::new(-0.24, 0.08, 0.32), Vec3::new(-0.35, -0.4, 0.3)),
        (Vec3::new(0.24, 0.08, 0.32), Vec3::new(0.35, -0.4, 0.3)),
    ];
    for (base_pos, tip) in leg_positions {
        add_leg_capsule(&mut v, &mut i, base_pos, tip, 0.04, 6);
    }

    (v, i)
}

// ---- Primitives ----

/// Add a capsule (cylinder with hemispherical caps). start/end are centers; r0 at start, r1 at end.
fn add_capsule(
    v: &mut Vec<Vertex>,
    i: &mut Vec<u32>,
    start: Vec3,
    end: Vec3,
    r0: f32,
    r1: f32,
    segments: u32,
    rings: u32,
) {
    let dir = (end - start).normalize();
    let len = (end - start).length();
    let up = if dir.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let right = dir.cross(up).normalize();
    let actual_up = right.cross(dir).normalize();

    let seg = segments as usize;
    let ring = rings as usize;
    let start_idx = v.len() as u32;

    for r in 0..=ring {
        let t = r as f32 / ring as f32;
        let pos = start + dir * (t * len);
        let radius = r0 + (r1 - r0) * t;

        for s in 0..seg {
            let angle = (s as f32 / seg as f32) * std::f32::consts::TAU;
            let offset = (right * angle.cos() + actual_up * angle.sin()) * radius;
            let n = offset.normalize();
            v.push(Vertex::with_color(
                (pos + offset).to_array(),
                n.to_array(),
                [s as f32 / seg as f32, t],
                WHITE,
            ));
        }
    }

    for r in 0..ring {
        let curr = start_idx + (r * seg) as u32;
        let next = curr + seg as u32;
        for s in 0..seg as u32 {
            let ns = (s + 1) % seg as u32;
            i.push(curr + s);
            i.push(next + s);
            i.push(curr + ns);
            i.push(curr + ns);
            i.push(next + s);
            i.push(next + ns);
        }
    }
}

/// Add a sphere at center.
fn add_sphere(
    v: &mut Vec<Vertex>,
    i: &mut Vec<u32>,
    center: Vec3,
    radius: f32,
    segments: u32,
    rings: u32,
) {
    let seg = segments as usize;
    let ring = rings as usize;
    let start_idx = v.len() as u32;

    for r in 0..=ring {
        let phi = std::f32::consts::PI * r as f32 / ring as f32;
        let y = radius * phi.cos();
        let ring_r = radius * phi.sin();

        for s in 0..=seg {
            let theta = std::f32::consts::TAU * s as f32 / seg as f32;
            let x = ring_r * theta.cos();
            let z = ring_r * theta.sin();
            let pos = center + Vec3::new(x, y, z);
            let n = Vec3::new(x, y, z).normalize();
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

/// Add a thin leg (capsule from base to tip).
fn add_leg_capsule(
    v: &mut Vec<Vertex>,
    i: &mut Vec<u32>,
    base_pos: Vec3,
    tip: Vec3,
    radius: f32,
    segments: u32,
) {
    add_capsule(v, i, base_pos, tip, radius, radius * 0.7, segments, 3);
}

/// Add a wing quad (triangle). side: -1 left, +1 right (for winding).
fn add_wing_quad(
    v: &mut Vec<Vertex>,
    i: &mut Vec<u32>,
    root: Vec3,
    tip: Vec3,
    back: Vec3,
    side: f32,
) {
    let n = (tip - root).cross(back - root).normalize() * side;
    let start = v.len() as u32;
    v.push(Vertex::with_color(root.to_array(), n.to_array(), [0.0, 0.0], WHITE));
    v.push(Vertex::with_color(tip.to_array(), n.to_array(), [1.0, 0.0], WHITE));
    v.push(Vertex::with_color(back.to_array(), n.to_array(), [0.5, 1.0], WHITE));
    if side > 0.0 {
        i.extend([start, start + 1, start + 2]);
    } else {
        i.extend([start, start + 2, start + 1]);
    }
}
