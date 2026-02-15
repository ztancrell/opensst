//! Procedural arachnid bug mesh generation
//! Creates Starship Troopers-style warrior bugs, tankers, hoppers, etc.

use glam::{Mat4, Vec2, Vec3};
use rand::prelude::*;

/// Vertex with position, normal, UV, and tangent
#[derive(Debug, Clone, Copy)]
pub struct BugVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub tangent: [f32; 4],
    pub bone_indices: [u32; 4],
    pub bone_weights: [f32; 4],
}

impl BugVertex {
    pub fn new(position: Vec3, normal: Vec3, uv: Vec2) -> Self {
        Self {
            position: position.into(),
            normal: normal.into(),
            uv: uv.into(),
            tangent: [1.0, 0.0, 0.0, 1.0],
            bone_indices: [0; 4],
            bone_weights: [1.0, 0.0, 0.0, 0.0],
        }
    }

    pub fn with_bone(mut self, bone_index: u32, weight: f32) -> Self {
        self.bone_indices[0] = bone_index;
        self.bone_weights[0] = weight;
        self
    }
}

/// Configuration for procedural bug generation
#[derive(Debug, Clone)]
pub struct BugConfig {
    /// Random seed for variation
    pub seed: u64,
    /// Overall scale
    pub scale: f32,
    /// Number of body segments
    pub body_segments: u32,
    /// Number of legs (typically 6 or 8)
    pub leg_count: u32,
    /// Leg segment count (2 or 3)
    pub leg_segments: u32,
    /// Has mandibles/pincers
    pub has_mandibles: bool,
    /// Has tail/stinger
    pub has_tail: bool,
    /// Armor plate thickness
    pub armor_thickness: f32,
    /// Body type
    pub body_type: BugBodyType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BugBodyType {
    Warrior,    // Standard soldier bug
    Charger,    // Sleek, fast
    Tanker,     // Heavy, armored
    Spitter,    // Bloated acid sac
    Hopper,     // Wings/jumping legs
    Plasma,     // Glowing plasma bug
}

impl Default for BugConfig {
    fn default() -> Self {
        Self {
            seed: 0,
            scale: 1.0,
            body_segments: 3,
            leg_count: 6,
            leg_segments: 3,
            has_mandibles: true,
            has_tail: false,
            armor_thickness: 0.1,
            body_type: BugBodyType::Warrior,
        }
    }
}

impl BugConfig {
    pub fn warrior() -> Self {
        Self {
            body_segments: 3,
            leg_count: 6,
            leg_segments: 3,
            has_mandibles: true,
            has_tail: true,
            armor_thickness: 0.12,
            body_type: BugBodyType::Warrior,
            ..Default::default()
        }
    }

    pub fn charger() -> Self {
        Self {
            body_segments: 2,
            leg_count: 6,
            leg_segments: 3,
            has_mandibles: true,
            has_tail: false,
            armor_thickness: 0.08,
            scale: 0.8,
            body_type: BugBodyType::Charger,
            ..Default::default()
        }
    }

    pub fn tanker() -> Self {
        Self {
            body_segments: 4,
            leg_count: 8,
            leg_segments: 2,
            has_mandibles: true,
            has_tail: true,
            armor_thickness: 0.25,
            scale: 2.5,
            body_type: BugBodyType::Tanker,
            ..Default::default()
        }
    }

    pub fn spitter() -> Self {
        Self {
            body_segments: 3,
            leg_count: 6,
            leg_segments: 3,
            has_mandibles: false,
            has_tail: false,
            armor_thickness: 0.1,
            scale: 1.2,
            body_type: BugBodyType::Spitter,
            ..Default::default()
        }
    }

    pub fn hopper() -> Self {
        Self {
            body_segments: 2,
            leg_count: 6,
            leg_segments: 3,
            has_mandibles: true,
            has_tail: false,
            armor_thickness: 0.06,
            scale: 0.7,
            body_type: BugBodyType::Hopper,
            ..Default::default()
        }
    }
}

/// Generated bug mesh data
#[derive(Debug, Clone)]
pub struct BugMeshData {
    pub vertices: Vec<BugVertex>,
    pub indices: Vec<u32>,
    pub bones: Vec<BugBone>,
    pub collision_shapes: Vec<CollisionCapsule>,
}

/// Bone for skeletal animation
#[derive(Debug, Clone)]
pub struct BugBone {
    pub name: String,
    pub parent: Option<usize>,
    pub local_transform: Mat4,
    pub inverse_bind_pose: Mat4,
}

/// Collision capsule for physics
#[derive(Debug, Clone, Copy)]
pub struct CollisionCapsule {
    pub start: Vec3,
    pub end: Vec3,
    pub radius: f32,
    pub bone_index: u32,
}

/// Procedural bug mesh generator
pub struct BugMeshGenerator {
    _rng: StdRng,
    config: BugConfig,
    vertices: Vec<BugVertex>,
    indices: Vec<u32>,
    bones: Vec<BugBone>,
    collision_shapes: Vec<CollisionCapsule>,
}

impl BugMeshGenerator {
    pub fn new(config: BugConfig) -> Self {
        Self {
            _rng: StdRng::seed_from_u64(config.seed),
            config,
            vertices: Vec::new(),
            indices: Vec::new(),
            bones: Vec::new(),
            collision_shapes: Vec::new(),
        }
    }

    /// Generate the complete bug mesh
    pub fn generate(mut self) -> BugMeshData {
        // Create skeleton first
        self.create_skeleton();

        // Generate body parts
        self.generate_body();
        self.generate_head();
        self.generate_legs();

        if self.config.has_mandibles {
            self.generate_mandibles();
        }

        if self.config.has_tail {
            self.generate_tail();
        }

        // Special features based on type
        match self.config.body_type {
            BugBodyType::Spitter => self.generate_acid_sac(),
            BugBodyType::Hopper => self.generate_wings(),
            BugBodyType::Plasma => self.generate_plasma_organ(),
            _ => {}
        }

        // Calculate tangents
        self.calculate_tangents();

        // Scale everything
        let scale = self.config.scale;
        for vertex in &mut self.vertices {
            vertex.position[0] *= scale;
            vertex.position[1] *= scale;
            vertex.position[2] *= scale;
        }
        for shape in &mut self.collision_shapes {
            shape.start *= scale;
            shape.end *= scale;
            shape.radius *= scale;
        }

        BugMeshData {
            vertices: self.vertices,
            indices: self.indices,
            bones: self.bones,
            collision_shapes: self.collision_shapes,
        }
    }

    fn create_skeleton(&mut self) {
        // Root bone
        self.bones.push(BugBone {
            name: "root".to_string(),
            parent: None,
            local_transform: Mat4::IDENTITY,
            inverse_bind_pose: Mat4::IDENTITY,
        });

        // Spine bones
        for i in 0..self.config.body_segments {
            let z_offset = (i as f32 - self.config.body_segments as f32 * 0.5) * 0.5;
            self.bones.push(BugBone {
                name: format!("spine_{}", i),
                parent: Some(if i == 0 { 0 } else { i as usize }),
                local_transform: Mat4::from_translation(Vec3::new(0.0, 0.0, z_offset)),
                inverse_bind_pose: Mat4::from_translation(Vec3::new(0.0, 0.0, -z_offset)),
            });
        }

        // Head bone
        let head_idx = self.bones.len();
        self.bones.push(BugBone {
            name: "head".to_string(),
            parent: Some(1), // First spine
            local_transform: Mat4::from_translation(Vec3::new(0.0, 0.1, 0.8)),
            inverse_bind_pose: Mat4::from_translation(Vec3::new(0.0, -0.1, -0.8)),
        });

        // Mandible bones
        if self.config.has_mandibles {
            self.bones.push(BugBone {
                name: "mandible_l".to_string(),
                parent: Some(head_idx),
                local_transform: Mat4::from_translation(Vec3::new(-0.15, -0.05, 0.3)),
                inverse_bind_pose: Mat4::from_translation(Vec3::new(0.15, 0.05, -0.3)),
            });
            self.bones.push(BugBone {
                name: "mandible_r".to_string(),
                parent: Some(head_idx),
                local_transform: Mat4::from_translation(Vec3::new(0.15, -0.05, 0.3)),
                inverse_bind_pose: Mat4::from_translation(Vec3::new(-0.15, 0.05, -0.3)),
            });
        }

        // Leg bones (3 segments each)
        let leg_attach_bone = 1; // Attach to first spine segment
        for leg in 0..self.config.leg_count {
            let side = if leg % 2 == 0 { -1.0 } else { 1.0 };
            let pair = (leg / 2) as f32;
            let z_offset = (pair - (self.config.leg_count as f32 / 4.0 - 0.5)) * 0.4;

            let leg_base_idx = self.bones.len();

            // Coxa (hip)
            self.bones.push(BugBone {
                name: format!("leg_{}_coxa", leg),
                parent: Some(leg_attach_bone),
                local_transform: Mat4::from_translation(Vec3::new(side * 0.3, -0.1, z_offset)),
                inverse_bind_pose: Mat4::from_translation(Vec3::new(-side * 0.3, 0.1, -z_offset)),
            });

            // Femur
            self.bones.push(BugBone {
                name: format!("leg_{}_femur", leg),
                parent: Some(leg_base_idx),
                local_transform: Mat4::from_translation(Vec3::new(side * 0.4, -0.1, 0.0)),
                inverse_bind_pose: Mat4::from_translation(Vec3::new(-side * 0.4, 0.1, 0.0)),
            });

            // Tibia
            self.bones.push(BugBone {
                name: format!("leg_{}_tibia", leg),
                parent: Some(leg_base_idx + 1),
                local_transform: Mat4::from_translation(Vec3::new(side * 0.3, -0.4, 0.0)),
                inverse_bind_pose: Mat4::from_translation(Vec3::new(-side * 0.3, 0.4, 0.0)),
            });

            if self.config.leg_segments > 2 {
                // Tarsus (foot)
                self.bones.push(BugBone {
                    name: format!("leg_{}_tarsus", leg),
                    parent: Some(leg_base_idx + 2),
                    local_transform: Mat4::from_translation(Vec3::new(0.0, -0.3, 0.1)),
                    inverse_bind_pose: Mat4::from_translation(Vec3::new(0.0, 0.3, -0.1)),
                });
            }
        }

        // Tail bones if applicable
        if self.config.has_tail {
            let tail_attach = self.config.body_segments as usize;
            for i in 0..3 {
                self.bones.push(BugBone {
                    name: format!("tail_{}", i),
                    parent: Some(if i == 0 { tail_attach } else { self.bones.len() - 1 }),
                    local_transform: Mat4::from_translation(Vec3::new(0.0, 0.05 * i as f32, -0.3)),
                    inverse_bind_pose: Mat4::from_translation(Vec3::new(0.0, -0.05 * i as f32, 0.3)),
                });
            }
        }
    }

    fn generate_body(&mut self) {
        let segments = self.config.body_segments;
        let armor = self.config.armor_thickness;

        // Generate each body segment
        for seg in 0..segments {
            let t = seg as f32 / (segments - 1).max(1) as f32;
            let z = (t - 0.5) * 2.0; // -1 to 1

            // Body profile varies along length
            let (width, height) = match self.config.body_type {
                BugBodyType::Warrior => (
                    0.35 * (1.0 - (t - 0.5).abs() * 0.5),
                    0.25 * (1.0 - (t - 0.3).abs() * 0.3),
                ),
                BugBodyType::Charger => (
                    0.25 * (1.0 - t * 0.3),
                    0.2 * (1.0 - t * 0.2),
                ),
                BugBodyType::Tanker => (
                    0.6 * (1.0 - (t - 0.5).abs() * 0.3),
                    0.5 * (1.0 - (t - 0.5).abs() * 0.2),
                ),
                BugBodyType::Spitter => (
                    0.4 * (1.0 + t * 0.5), // Bloated rear
                    0.35 * (1.0 + t * 0.6),
                ),
                _ => (0.3, 0.25),
            };

            // Generate ring of vertices for this segment
            let ring_verts = 12;
            let bone_idx = (seg + 1) as u32; // Spine bone

            for i in 0..ring_verts {
                let angle = (i as f32 / ring_verts as f32) * std::f32::consts::TAU;
                let x = angle.cos() * width;
                let y = angle.sin() * height;

                // Add armor plate detail
                let plate_bump = ((angle * 3.0).sin().abs() * 0.5 + 0.5) * armor;

                let pos = Vec3::new(x * (1.0 + plate_bump), y * (1.0 + plate_bump), z * 0.8);
                let normal = Vec3::new(x, y, 0.0).normalize();
                let uv = Vec2::new(i as f32 / ring_verts as f32, t);

                self.vertices.push(
                    BugVertex::new(pos, normal, uv).with_bone(bone_idx, 1.0)
                );
            }

            // Generate faces between rings
            if seg > 0 {
                let curr_start = self.vertices.len() as u32 - ring_verts as u32;
                let prev_start = curr_start - ring_verts as u32;

                for i in 0..ring_verts as u32 {
                    let next = (i + 1) % ring_verts as u32;

                    // Two triangles per quad
                    self.indices.push(prev_start + i);
                    self.indices.push(curr_start + i);
                    self.indices.push(prev_start + next);

                    self.indices.push(prev_start + next);
                    self.indices.push(curr_start + i);
                    self.indices.push(curr_start + next);
                }
            }

            // Add collision capsule
            if seg < segments - 1 {
                let next_z = ((seg + 1) as f32 / (segments - 1).max(1) as f32 - 0.5) * 2.0 * 0.8;
                self.collision_shapes.push(CollisionCapsule {
                    start: Vec3::new(0.0, 0.0, z * 0.8),
                    end: Vec3::new(0.0, 0.0, next_z),
                    radius: width.max(height) * 0.8,
                    bone_index: bone_idx,
                });
            }
        }
    }

    fn generate_head(&mut self) {
        let head_bone = self.find_bone("head").unwrap_or(0) as u32;

        // Head is roughly triangular/wedge shaped
        let head_length = 0.4;
        let head_width = 0.25;
        let head_height = 0.2;

        // Generate head vertices
        let segments = 8;
        let rings = 6;
        let start_idx = self.vertices.len() as u32;

        for ring in 0..rings {
            let t = ring as f32 / (rings - 1) as f32;
            let z = t * head_length;

            // Taper towards front
            let taper = 1.0 - t * 0.6;
            let w = head_width * taper;
            let h = head_height * taper;

            for seg in 0..segments {
                let angle = (seg as f32 / segments as f32) * std::f32::consts::TAU;
                let x = angle.cos() * w;
                let y = angle.sin() * h + 0.1; // Offset up

                let pos = Vec3::new(x, y, z + 0.5); // Offset forward from body
                let normal = Vec3::new(angle.cos(), angle.sin(), 0.3).normalize();
                let uv = Vec2::new(seg as f32 / segments as f32, t);

                self.vertices.push(
                    BugVertex::new(pos, normal, uv).with_bone(head_bone, 1.0)
                );
            }
        }

        // Index the head rings
        for ring in 0..(rings - 1) {
            let curr_start = start_idx + (ring * segments) as u32;
            let next_start = curr_start + segments as u32;

            for seg in 0..segments as u32 {
                let next_seg = (seg + 1) % segments as u32;

                self.indices.push(curr_start + seg);
                self.indices.push(next_start + seg);
                self.indices.push(curr_start + next_seg);

                self.indices.push(curr_start + next_seg);
                self.indices.push(next_start + seg);
                self.indices.push(next_start + next_seg);
            }
        }

        // Eyes (compound eye bumps)
        self.add_eye(Vec3::new(-0.12, 0.15, 0.7), 0.08, head_bone);
        self.add_eye(Vec3::new(0.12, 0.15, 0.7), 0.08, head_bone);

        // Head collision
        self.collision_shapes.push(CollisionCapsule {
            start: Vec3::new(0.0, 0.1, 0.5),
            end: Vec3::new(0.0, 0.1, 0.9),
            radius: 0.15,
            bone_index: head_bone,
        });
    }

    fn add_eye(&mut self, center: Vec3, radius: f32, bone: u32) {
        let start_idx = self.vertices.len() as u32;
        let segments = 8;
        let rings = 4;

        for ring in 0..rings {
            let phi = (ring as f32 / (rings - 1) as f32) * std::f32::consts::FRAC_PI_2;

            for seg in 0..segments {
                let theta = (seg as f32 / segments as f32) * std::f32::consts::TAU;

                let x = phi.sin() * theta.cos() * radius;
                let y = phi.sin() * theta.sin() * radius;
                let z = phi.cos() * radius;

                let pos = center + Vec3::new(x, y, z);
                let normal = Vec3::new(x, y, z).normalize();
                let uv = Vec2::new(
                    seg as f32 / segments as f32,
                    ring as f32 / (rings - 1) as f32,
                );

                self.vertices.push(
                    BugVertex::new(pos, normal, uv).with_bone(bone, 1.0)
                );
            }
        }

        // Index eye
        for ring in 0..(rings - 1) {
            let curr = start_idx + (ring * segments) as u32;
            let next = curr + segments as u32;

            for seg in 0..segments as u32 {
                let next_seg = (seg + 1) % segments as u32;

                self.indices.push(curr + seg);
                self.indices.push(next + seg);
                self.indices.push(curr + next_seg);

                self.indices.push(curr + next_seg);
                self.indices.push(next + seg);
                self.indices.push(next + next_seg);
            }
        }
    }

    fn generate_legs(&mut self) {
        for leg in 0..self.config.leg_count {
            self.generate_single_leg(leg);
        }
    }

    fn generate_single_leg(&mut self, leg_index: u32) {
        let side = if leg_index % 2 == 0 { -1.0 } else { 1.0 };
        let pair = (leg_index / 2) as f32;
        let z_offset = (pair - (self.config.leg_count as f32 / 4.0 - 0.5)) * 0.4;

        // Find leg bones
        let coxa_bone = self.find_bone(&format!("leg_{}_coxa", leg_index)).unwrap_or(0) as u32;
        let femur_bone = self.find_bone(&format!("leg_{}_femur", leg_index)).unwrap_or(0) as u32;
        let tibia_bone = self.find_bone(&format!("leg_{}_tibia", leg_index)).unwrap_or(0) as u32;

        // Leg segment dimensions
        let coxa_length = 0.15;
        let coxa_radius = 0.06;
        let _femur_length = 0.35;
        let femur_radius = 0.04;
        let _tibia_length = 0.4;
        let tibia_radius = 0.03;

        // Generate coxa (hip joint)
        let coxa_start = Vec3::new(side * 0.25, 0.0, z_offset);
        let coxa_end = coxa_start + Vec3::new(side * coxa_length, -0.05, 0.0);
        self.add_limb_segment(coxa_start, coxa_end, coxa_radius, coxa_bone);
        self.collision_shapes.push(CollisionCapsule {
            start: coxa_start,
            end: coxa_end,
            radius: coxa_radius,
            bone_index: coxa_bone,
        });

        // Generate femur (upper leg)
        let femur_start = coxa_end;
        let femur_end = femur_start + Vec3::new(side * 0.2, -0.3, 0.05);
        self.add_limb_segment(femur_start, femur_end, femur_radius, femur_bone);
        self.collision_shapes.push(CollisionCapsule {
            start: femur_start,
            end: femur_end,
            radius: femur_radius,
            bone_index: femur_bone,
        });

        // Generate tibia (lower leg)
        let tibia_start = femur_end;
        let tibia_end = tibia_start + Vec3::new(side * 0.1, -0.35, 0.1);
        self.add_limb_segment(tibia_start, tibia_end, tibia_radius, tibia_bone);
        self.collision_shapes.push(CollisionCapsule {
            start: tibia_start,
            end: tibia_end,
            radius: tibia_radius,
            bone_index: tibia_bone,
        });

        // Add claw/foot
        self.add_claw(tibia_end, side, tibia_bone);
    }

    fn add_limb_segment(&mut self, start: Vec3, end: Vec3, radius: f32, bone: u32) {
        let start_idx = self.vertices.len() as u32;
        let direction = (end - start).normalize();
        let length = (end - start).length();

        // Create orthonormal basis
        let up = if direction.y.abs() < 0.9 {
            Vec3::Y
        } else {
            Vec3::X
        };
        let right = direction.cross(up).normalize();
        let actual_up = right.cross(direction).normalize();

        let segments = 6;
        let rings = 4;

        for ring in 0..rings {
            let t = ring as f32 / (rings - 1) as f32;
            let pos_along = start + direction * (t * length);

            // Slight taper
            let taper = 1.0 - t * 0.2;

            for seg in 0..segments {
                let angle = (seg as f32 / segments as f32) * std::f32::consts::TAU;
                let offset = (right * angle.cos() + actual_up * angle.sin()) * radius * taper;

                let pos = pos_along + offset;
                let normal = offset.normalize();
                let uv = Vec2::new(seg as f32 / segments as f32, t);

                self.vertices.push(
                    BugVertex::new(pos, normal, uv).with_bone(bone, 1.0)
                );
            }
        }

        // Index
        for ring in 0..(rings - 1) {
            let curr = start_idx + (ring * segments) as u32;
            let next = curr + segments as u32;

            for seg in 0..segments as u32 {
                let next_seg = (seg + 1) % segments as u32;

                self.indices.push(curr + seg);
                self.indices.push(next + seg);
                self.indices.push(curr + next_seg);

                self.indices.push(curr + next_seg);
                self.indices.push(next + seg);
                self.indices.push(next + next_seg);
            }
        }
    }

    fn add_claw(&mut self, base: Vec3, side: f32, bone: u32) {
        // Simple pointed claw
        let _claw_length = 0.1;
        let claw_tip = base + Vec3::new(side * 0.02, -0.08, 0.05);

        let start_idx = self.vertices.len() as u32;

        // Base ring
        let radius = 0.02;
        for i in 0..4 {
            let angle = (i as f32 / 4.0) * std::f32::consts::TAU;
            let offset = Vec3::new(angle.cos() * radius, angle.sin() * radius, 0.0);
            let pos = base + offset;
            let normal = offset.normalize();

            self.vertices.push(
                BugVertex::new(pos, normal, Vec2::new(i as f32 / 4.0, 0.0))
                    .with_bone(bone, 1.0)
            );
        }

        // Tip
        let tip_idx = self.vertices.len() as u32;
        self.vertices.push(
            BugVertex::new(claw_tip, Vec3::new(0.0, -0.5, 0.5).normalize(), Vec2::new(0.5, 1.0))
                .with_bone(bone, 1.0)
        );

        // Index claw cone
        for i in 0..4u32 {
            let next = (i + 1) % 4;
            self.indices.push(start_idx + i);
            self.indices.push(start_idx + next);
            self.indices.push(tip_idx);
        }
    }

    fn generate_mandibles(&mut self) {
        let left_bone = self.find_bone("mandible_l").unwrap_or(0) as u32;
        let right_bone = self.find_bone("mandible_r").unwrap_or(0) as u32;

        // Left mandible
        self.generate_mandible(-1.0, left_bone);
        // Right mandible
        self.generate_mandible(1.0, right_bone);
    }

    fn generate_mandible(&mut self, side: f32, bone: u32) {
        let base = Vec3::new(side * 0.1, 0.0, 0.85);
        let tip = Vec3::new(side * 0.05, -0.1, 1.1);

        // Curved mandible shape
        let start_idx = self.vertices.len() as u32;

        let segments = 5;
        let points = 6;

        for p in 0..points {
            let t = p as f32 / (points - 1) as f32;
            let pos_along = base.lerp(tip, t);

            // Curved profile
            let curve = (t * std::f32::consts::PI).sin() * 0.03 * side;
            let pos_along = pos_along + Vec3::new(curve, 0.0, 0.0);

            let radius = 0.025 * (1.0 - t * 0.6); // Taper

            for seg in 0..segments {
                let angle = (seg as f32 / segments as f32) * std::f32::consts::TAU;
                let offset = Vec3::new(0.0, angle.cos() * radius, angle.sin() * radius);

                let pos = pos_along + offset;
                let normal = offset.normalize();
                let uv = Vec2::new(seg as f32 / segments as f32, t);

                self.vertices.push(
                    BugVertex::new(pos, normal, uv).with_bone(bone, 1.0)
                );
            }
        }

        // Index
        for p in 0..(points - 1) {
            let curr = start_idx + (p * segments) as u32;
            let next = curr + segments as u32;

            for seg in 0..segments as u32 {
                let next_seg = (seg + 1) % segments as u32;

                self.indices.push(curr + seg);
                self.indices.push(next + seg);
                self.indices.push(curr + next_seg);

                self.indices.push(curr + next_seg);
                self.indices.push(next + seg);
                self.indices.push(next + next_seg);
            }
        }

        self.collision_shapes.push(CollisionCapsule {
            start: base,
            end: tip,
            radius: 0.02,
            bone_index: bone,
        });
    }

    fn generate_tail(&mut self) {
        let tail_segments = 3;

        for i in 0..tail_segments {
            let bone_name = format!("tail_{}", i);
            let bone = self.find_bone(&bone_name).unwrap_or(0) as u32;

            let t = i as f32 / (tail_segments - 1).max(1) as f32;
            let z = -0.7 - t * 0.6;
            let y = t * 0.2;
            let radius = 0.08 * (1.0 - t * 0.5);

            let center = Vec3::new(0.0, y, z);
            self.add_sphere_segment(center, radius, bone);

            if i < tail_segments - 1 {
                let next_z = -0.7 - (i + 1) as f32 / (tail_segments - 1).max(1) as f32 * 0.6;
                let next_y = (i + 1) as f32 / (tail_segments - 1).max(1) as f32 * 0.2;
                self.collision_shapes.push(CollisionCapsule {
                    start: center,
                    end: Vec3::new(0.0, next_y, next_z),
                    radius,
                    bone_index: bone,
                });
            }
        }

        // Stinger tip
        let stinger_bone = self.find_bone("tail_2").unwrap_or(0) as u32;
        let stinger_base = Vec3::new(0.0, 0.2, -1.3);
        let stinger_tip = Vec3::new(0.0, 0.15, -1.5);
        self.add_limb_segment(stinger_base, stinger_tip, 0.02, stinger_bone);
    }

    fn add_sphere_segment(&mut self, center: Vec3, radius: f32, bone: u32) {
        let start_idx = self.vertices.len() as u32;
        let segments = 8;
        let rings = 6;

        for ring in 0..rings {
            let phi = (ring as f32 / (rings - 1) as f32) * std::f32::consts::PI;

            for seg in 0..segments {
                let theta = (seg as f32 / segments as f32) * std::f32::consts::TAU;

                let x = phi.sin() * theta.cos() * radius;
                let y = phi.cos() * radius;
                let z = phi.sin() * theta.sin() * radius;

                let pos = center + Vec3::new(x, y, z);
                let normal = Vec3::new(x, y, z).normalize();
                let uv = Vec2::new(
                    seg as f32 / segments as f32,
                    ring as f32 / (rings - 1) as f32,
                );

                self.vertices.push(
                    BugVertex::new(pos, normal, uv).with_bone(bone, 1.0)
                );
            }
        }

        for ring in 0..(rings - 1) {
            let curr = start_idx + (ring * segments) as u32;
            let next = curr + segments as u32;

            for seg in 0..segments as u32 {
                let next_seg = (seg + 1) % segments as u32;

                self.indices.push(curr + seg);
                self.indices.push(next + seg);
                self.indices.push(curr + next_seg);

                self.indices.push(curr + next_seg);
                self.indices.push(next + seg);
                self.indices.push(next + next_seg);
            }
        }
    }

    fn generate_acid_sac(&mut self) {
        // Bloated abdomen for spitter bugs
        let bone = self.find_bone(&format!("spine_{}", self.config.body_segments - 1)).unwrap_or(0) as u32;
        let center = Vec3::new(0.0, 0.1, -0.5);
        let radius = 0.4;

        self.add_sphere_segment(center, radius, bone);
        self.collision_shapes.push(CollisionCapsule {
            start: center - Vec3::new(0.0, 0.0, radius * 0.5),
            end: center + Vec3::new(0.0, 0.0, radius * 0.5),
            radius: radius * 0.8,
            bone_index: bone,
        });
    }

    fn generate_wings(&mut self) {
        // Vestigial or functional wings for hoppers
        let spine_bone = self.find_bone("spine_1").unwrap_or(0) as u32;

        for side in [-1.0f32, 1.0] {
            let start_idx = self.vertices.len() as u32;

            let wing_base = Vec3::new(side * 0.2, 0.2, 0.0);
            let wing_tip = Vec3::new(side * 0.8, 0.4, -0.3);
            let wing_back = Vec3::new(side * 0.5, 0.15, -0.5);

            // Simple triangular wing
            let normal = Vec3::new(0.0, 1.0, 0.0);

            self.vertices.push(BugVertex::new(wing_base, normal, Vec2::new(0.0, 0.0)).with_bone(spine_bone, 1.0));
            self.vertices.push(BugVertex::new(wing_tip, normal, Vec2::new(1.0, 0.0)).with_bone(spine_bone, 1.0));
            self.vertices.push(BugVertex::new(wing_back, normal, Vec2::new(0.5, 1.0)).with_bone(spine_bone, 1.0));

            // Two-sided
            if side > 0.0 {
                self.indices.push(start_idx);
                self.indices.push(start_idx + 1);
                self.indices.push(start_idx + 2);
            } else {
                self.indices.push(start_idx);
                self.indices.push(start_idx + 2);
                self.indices.push(start_idx + 1);
            }
        }
    }

    fn generate_plasma_organ(&mut self) {
        // Glowing plasma sac for plasma bugs
        let bone = self.find_bone("spine_1").unwrap_or(0) as u32;
        let center = Vec3::new(0.0, 0.3, 0.0);

        self.add_sphere_segment(center, 0.25, bone);
    }

    fn calculate_tangents(&mut self) {
        // Calculate tangents for normal mapping
        // Simplified - just use X axis as tangent
        for vertex in &mut self.vertices {
            let normal = Vec3::from(vertex.normal);
            let tangent = if normal.y.abs() < 0.9 {
                Vec3::Y.cross(normal).normalize()
            } else {
                Vec3::X.cross(normal).normalize()
            };
            vertex.tangent = [tangent.x, tangent.y, tangent.z, 1.0];
        }
    }

    fn find_bone(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|b| b.name == name)
    }
}
