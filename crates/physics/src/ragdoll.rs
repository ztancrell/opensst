//! Euphoria-style active ragdoll system
//! Provides procedural animation and physics-driven death animations for bugs

use engine_core::{Transform, Vec3};
use glam::Quat;
use rapier3d::prelude::*;
use std::collections::HashMap;

/// Active ragdoll controller - combines physics simulation with muscle forces
/// Inspired by Euphoria/NaturalMotion's approach to procedural animation
#[derive(Debug)]
pub struct ActiveRagdoll {
    /// Rigid bodies for each bone
    pub bodies: Vec<RagdollBody>,
    /// Joints connecting bodies
    pub joints: Vec<RagdollJoint>,
    /// Muscle controllers
    pub muscles: Vec<Muscle>,
    /// Current state of the ragdoll
    pub state: RagdollState,
    /// Balance controller
    pub balance: BalanceController,
    /// Pain/damage response
    pub damage_response: DamageResponse,
}

/// State of the ragdoll
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RagdollState {
    /// Fully animated, no physics
    Animated,
    /// Partially physics-driven (stumbling, hit reactions)
    Active,
    /// Fully physics-driven (dying, dead)
    Ragdoll,
    /// Dead and settled
    Dead,
}

/// A single rigid body in the ragdoll
#[derive(Debug, Clone)]
pub struct RagdollBody {
    pub name: String,
    pub bone_index: usize,
    pub body_handle: RigidBodyHandle,
    pub collider_handle: ColliderHandle,
    /// Target pose from animation
    pub target_rotation: Quat,
    /// Strength of muscles attached to this body (0 = ragdoll, 1 = full control)
    pub muscle_strength: f32,
    /// Mass of this body part
    pub mass: f32,
    /// Is this a critical body part (damage here causes more reaction)
    pub is_critical: bool,
}

/// A joint connecting two bodies
#[derive(Debug)]
pub struct RagdollJoint {
    pub name: String,
    pub body_a: usize,
    pub body_b: usize,
    pub joint_handle: ImpulseJointHandle,
    /// Angular limits (min, max) for each axis
    pub limits: [(f32, f32); 3],
    /// Joint stiffness
    pub stiffness: f32,
    /// Joint damping
    pub damping: f32,
}

/// Muscle that applies forces between bodies
#[derive(Debug, Clone)]
pub struct Muscle {
    pub name: String,
    pub body_a: usize,
    pub body_b: usize,
    /// Attachment point on body A (local space)
    pub attach_a: Vec3,
    /// Attachment point on body B (local space)
    pub attach_b: Vec3,
    /// Maximum force this muscle can apply
    pub max_force: f32,
    /// Current activation (0-1)
    pub activation: f32,
    /// Target length (rest length)
    pub rest_length: f32,
    /// Current length
    pub current_length: f32,
    /// Muscle type
    pub muscle_type: MuscleType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MuscleType {
    /// Flexor - bends joints
    Flexor,
    /// Extensor - straightens joints  
    Extensor,
    /// Stabilizer - maintains position
    Stabilizer,
    /// Locomotion - used for walking/running
    Locomotion,
}

/// Balance controller for maintaining/recovering balance
#[derive(Debug, Clone)]
pub struct BalanceController {
    /// Center of mass tracking
    pub com_position: Vec3,
    pub com_velocity: Vec3,
    /// Support polygon (feet positions)
    pub support_points: Vec<Vec3>,
    /// Balance state
    pub is_balanced: bool,
    /// Recovery urgency (0 = stable, 1 = falling)
    pub recovery_urgency: f32,
    /// Direction to lean for recovery
    pub recovery_direction: Vec3,
    /// Ground contact points
    pub ground_contacts: Vec<GroundContact>,
}

#[derive(Debug, Clone, Copy)]
pub struct GroundContact {
    pub body_index: usize,
    pub world_position: Vec3,
    pub normal: Vec3,
    pub is_stable: bool,
}

/// Response to damage/impacts
#[derive(Debug, Clone)]
pub struct DamageResponse {
    /// Recent impacts
    pub impacts: Vec<Impact>,
    /// Overall pain level (0-1)
    pub pain_level: f32,
    /// Accumulated damage per body part
    pub body_damage: HashMap<usize, f32>,
    /// Whether the ragdoll should enter death state
    pub is_dying: bool,
    /// Time until death animation completes
    pub death_timer: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Impact {
    pub body_index: usize,
    pub position: Vec3,
    pub direction: Vec3,
    pub force: f32,
    pub time: f32,
}

impl Default for BalanceController {
    fn default() -> Self {
        Self {
            com_position: Vec3::ZERO,
            com_velocity: Vec3::ZERO,
            support_points: Vec::new(),
            is_balanced: true,
            recovery_urgency: 0.0,
            recovery_direction: Vec3::ZERO,
            ground_contacts: Vec::new(),
        }
    }
}

impl Default for DamageResponse {
    fn default() -> Self {
        Self {
            impacts: Vec::new(),
            pain_level: 0.0,
            body_damage: HashMap::new(),
            is_dying: false,
            death_timer: 0.0,
        }
    }
}

impl ActiveRagdoll {
    /// Create a new active ragdoll from a skeleton definition
    pub fn new() -> Self {
        Self {
            bodies: Vec::new(),
            joints: Vec::new(),
            muscles: Vec::new(),
            state: RagdollState::Animated,
            balance: BalanceController::default(),
            damage_response: DamageResponse::default(),
        }
    }

    /// Build ragdoll physics bodies for a bug
    pub fn build_for_bug(
        &mut self,
        physics: &mut crate::PhysicsWorld,
        root_position: Vec3,
        collision_shapes: &[crate::ragdoll::CollisionCapsule],
    ) {
        // Create rigid bodies for each collision shape
        for (i, shape) in collision_shapes.iter().enumerate() {
            let center = (shape.start + shape.end) * 0.5 + root_position;
            let half_height = (shape.end - shape.start).length() * 0.5;

            // Create dynamic body
            let body_handle = physics.add_dynamic_body(center);

            // Add capsule collider
            let collider = ColliderBuilder::capsule_z(half_height, shape.radius)
                .friction(0.8)
                .restitution(0.2)
                .density(1.2)
                .build();

            let collider_handle = physics.collider_set.insert_with_parent(
                collider,
                body_handle,
                &mut physics.rigid_body_set,
            );

            // Determine if critical (head, thorax)
            let is_critical = i == 0 || shape.bone_index < 3;

            self.bodies.push(RagdollBody {
                name: format!("body_{}", i),
                bone_index: shape.bone_index as usize,
                body_handle,
                collider_handle,
                target_rotation: Quat::IDENTITY,
                muscle_strength: 1.0,
                mass: shape.radius * shape.radius * half_height * 2.0 * 1.2,
                is_critical,
            });
        }

        // Create joints between adjacent bodies
        self.create_joints(physics);

        // Create muscles
        self.create_muscles();
    }

    fn create_joints(&mut self, physics: &mut crate::PhysicsWorld) {
        // Connect bodies based on bone hierarchy
        for i in 1..self.bodies.len() {
            // Find parent body (simplified - connect to previous)
            let parent_idx = if i > 0 { i - 1 } else { 0 };

            let body_a = self.bodies[parent_idx].body_handle;
            let body_b = self.bodies[i].body_handle;

            // Create spherical joint with limits
            let joint = SphericalJointBuilder::new()
                .local_anchor1(point![0.0, 0.0, 0.2])
                .local_anchor2(point![0.0, 0.0, -0.2])
                .motor_position(
                    JointAxis::AngX,
                    0.0,
                    50.0,  // Stiffness
                    10.0,  // Damping
                )
                .motor_position(
                    JointAxis::AngY,
                    0.0,
                    50.0,
                    10.0,
                )
                .motor_position(
                    JointAxis::AngZ,
                    0.0,
                    30.0,
                    8.0,
                )
                .limits(JointAxis::AngX, [-0.5, 0.5])
                .limits(JointAxis::AngY, [-0.3, 0.3])
                .limits(JointAxis::AngZ, [-0.4, 0.4])
                .build();

            let joint_handle = physics.impulse_joint_set.insert(body_a, body_b, joint, true);

            self.joints.push(RagdollJoint {
                name: format!("joint_{}_{}", parent_idx, i),
                body_a: parent_idx,
                body_b: i,
                joint_handle,
                limits: [(-0.5, 0.5), (-0.3, 0.3), (-0.4, 0.4)],
                stiffness: 50.0,
                damping: 10.0,
            });
        }
    }

    fn create_muscles(&mut self) {
        // Create muscles between body parts
        for joint in &self.joints {
            // Flexor muscle
            self.muscles.push(Muscle {
                name: format!("{}_flexor", joint.name),
                body_a: joint.body_a,
                body_b: joint.body_b,
                attach_a: Vec3::new(0.0, 0.05, 0.1),
                attach_b: Vec3::new(0.0, 0.05, -0.1),
                max_force: 100.0,
                activation: 0.0,
                rest_length: 0.2,
                current_length: 0.2,
                muscle_type: MuscleType::Flexor,
            });

            // Extensor muscle
            self.muscles.push(Muscle {
                name: format!("{}_extensor", joint.name),
                body_a: joint.body_a,
                body_b: joint.body_b,
                attach_a: Vec3::new(0.0, -0.05, 0.1),
                attach_b: Vec3::new(0.0, -0.05, -0.1),
                max_force: 100.0,
                activation: 0.0,
                rest_length: 0.2,
                current_length: 0.2,
                muscle_type: MuscleType::Extensor,
            });
        }
    }

    /// Update the ragdoll simulation
    pub fn update(&mut self, physics: &mut crate::PhysicsWorld, dt: f32) {
        // Update damage response
        self.update_damage_response(dt);

        // Update balance
        self.update_balance(physics);

        match self.state {
            RagdollState::Animated => {
                // Kinematic control - set positions from animation
                self.apply_kinematic_control(physics);
            }
            RagdollState::Active => {
                // Active ragdoll - muscles try to maintain pose
                self.apply_muscle_forces(physics, dt);
                self.apply_balance_forces(physics);
            }
            RagdollState::Ragdoll => {
                // Pure physics - reduce muscle strength over time
                for body in &mut self.bodies {
                    body.muscle_strength *= 0.95;
                }
                self.apply_muscle_forces(physics, dt);
                self.apply_dying_behavior(physics, dt);
            }
            RagdollState::Dead => {
                // No active control, just physics
            }
        }

        // Clean up old impacts
        self.damage_response.impacts.retain(|i| i.time < 2.0);
        for impact in &mut self.damage_response.impacts {
            impact.time += dt;
        }
    }

    fn update_damage_response(&mut self, dt: f32) {
        // Decay pain over time
        self.damage_response.pain_level *= 0.98;

        // Update death timer
        if self.damage_response.is_dying {
            self.damage_response.death_timer += dt;

            if self.damage_response.death_timer > 3.0 {
                self.state = RagdollState::Dead;
            } else if self.state != RagdollState::Ragdoll {
                self.state = RagdollState::Ragdoll;
            }
        }
    }

    fn update_balance(&mut self, physics: &crate::PhysicsWorld) {
        // Calculate center of mass
        let mut total_mass = 0.0f32;
        let mut weighted_pos = Vec3::ZERO;
        let mut weighted_vel = Vec3::ZERO;

        for body in &self.bodies {
            if let Some(rb) = physics.rigid_body_set.get(body.body_handle) {
                let pos = rb.translation();
                let vel = rb.linvel();
                weighted_pos += Vec3::new(pos.x, pos.y, pos.z) * body.mass;
                weighted_vel += Vec3::new(vel.x, vel.y, vel.z) * body.mass;
                total_mass += body.mass;
            }
        }

        if total_mass > 0.0 {
            self.balance.com_position = weighted_pos / total_mass;
            self.balance.com_velocity = weighted_vel / total_mass;
        }

        // Check if COM is over support polygon
        self.balance.is_balanced = self.check_balance_stability();

        if !self.balance.is_balanced {
            self.balance.recovery_urgency = (self.balance.recovery_urgency + 0.1).min(1.0);
            self.calculate_recovery_direction();
        } else {
            self.balance.recovery_urgency *= 0.9;
        }
    }

    fn check_balance_stability(&self) -> bool {
        // Simplified balance check - is COM above ground contacts?
        if self.balance.ground_contacts.is_empty() {
            return false;
        }

        // Check if COM projection is within support polygon
        let com_2d = Vec3::new(self.balance.com_position.x, 0.0, self.balance.com_position.z);

        for contact in &self.balance.ground_contacts {
            let dist = (com_2d - Vec3::new(contact.world_position.x, 0.0, contact.world_position.z)).length();
            if dist < 0.5 {
                return true;
            }
        }

        false
    }

    fn calculate_recovery_direction(&mut self) {
        // Direction to lean to recover balance
        if let Some(contact) = self.balance.ground_contacts.first() {
            let to_support = contact.world_position - self.balance.com_position;
            self.balance.recovery_direction = Vec3::new(to_support.x, 0.0, to_support.z).normalize_or_zero();
        }
    }

    fn apply_kinematic_control(&self, physics: &mut crate::PhysicsWorld) {
        for body in &self.bodies {
            if let Some(rb) = physics.rigid_body_set.get_mut(body.body_handle) {
                rb.set_body_type(RigidBodyType::KinematicPositionBased, true);
            }
        }
    }

    fn apply_muscle_forces(&self, physics: &mut crate::PhysicsWorld, _dt: f32) {
        for muscle in &self.muscles {
            if muscle.activation < 0.01 {
                continue;
            }

            let body_a = &self.bodies[muscle.body_a];
            let body_b = &self.bodies[muscle.body_b];

            // Get world positions of attachment points
            let (pos_a, pos_b) = {
                let rb_a = physics.rigid_body_set.get(body_a.body_handle);
                let rb_b = physics.rigid_body_set.get(body_b.body_handle);

                if let (Some(ra), Some(rb)) = (rb_a, rb_b) {
                    let world_a = ra.position() * point![muscle.attach_a.x, muscle.attach_a.y, muscle.attach_a.z];
                    let world_b = rb.position() * point![muscle.attach_b.x, muscle.attach_b.y, muscle.attach_b.z];
                    (
                        Vec3::new(world_a.x, world_a.y, world_a.z),
                        Vec3::new(world_b.x, world_b.y, world_b.z),
                    )
                } else {
                    continue;
                }
            };

            // Calculate muscle force
            let direction = (pos_b - pos_a).normalize_or_zero();
            let current_length = (pos_b - pos_a).length();
            let length_diff = current_length - muscle.rest_length;

            // Spring-damper muscle model
            let force_magnitude = muscle.activation * muscle.max_force * length_diff.signum() * length_diff.abs().min(1.0);
            let force = direction * force_magnitude * body_a.muscle_strength.min(body_b.muscle_strength);

            // Apply forces to bodies
            if let Some(rb_a) = physics.rigid_body_set.get_mut(body_a.body_handle) {
                rb_a.apply_impulse(vector![force.x, force.y, force.z] * 0.01, true);
            }
            if let Some(rb_b) = physics.rigid_body_set.get_mut(body_b.body_handle) {
                rb_b.apply_impulse(vector![-force.x, -force.y, -force.z] * 0.01, true);
            }
        }
    }

    fn apply_balance_forces(&self, physics: &mut crate::PhysicsWorld) {
        if self.balance.is_balanced || self.balance.recovery_urgency < 0.1 {
            return;
        }

        // Apply corrective forces to maintain balance
        let recovery_force = self.balance.recovery_direction * self.balance.recovery_urgency * 50.0;

        // Apply to root body (thorax)
        if let Some(body) = self.bodies.first() {
            if let Some(rb) = physics.rigid_body_set.get_mut(body.body_handle) {
                rb.apply_impulse(
                    vector![recovery_force.x, recovery_force.y + 10.0, recovery_force.z] * 0.01,
                    true,
                );
            }
        }
    }

    fn apply_dying_behavior(&self, physics: &mut crate::PhysicsWorld, dt: f32) {
        // Add procedural dying movements - twitching, curling up
        let time = self.damage_response.death_timer;

        // Spasms in early death
        if time < 1.5 {
            let spasm_intensity = (1.5 - time) * 0.5;
            let spasm = ((time * 20.0).sin() * spasm_intensity) as f32;

            for (i, body) in self.bodies.iter().enumerate() {
                if let Some(rb) = physics.rigid_body_set.get_mut(body.body_handle) {
                    // Random twitching
                    let twitch = Vec3::new(
                        ((time * 15.0 + i as f32 * 3.0).sin() * spasm),
                        ((time * 12.0 + i as f32 * 2.0).cos() * spasm * 0.5),
                        ((time * 18.0 + i as f32 * 4.0).sin() * spasm),
                    );

                    rb.apply_torque_impulse(vector![twitch.x, twitch.y, twitch.z], true);
                }
            }
        }

        // Legs curl up
        for (i, body) in self.bodies.iter().enumerate() {
            if body.name.contains("leg") {
                if let Some(rb) = physics.rigid_body_set.get_mut(body.body_handle) {
                    // Curl legs inward
                    let curl_force = Vec3::new(0.0, 0.5, 0.0) * (1.0 - (-time).exp()) * 20.0;
                    rb.apply_impulse(vector![curl_force.x, curl_force.y, curl_force.z] * dt, true);
                }
            }
        }
    }

    /// Apply an impact to the ragdoll
    pub fn apply_impact(
        &mut self,
        physics: &mut crate::PhysicsWorld,
        body_index: usize,
        position: Vec3,
        direction: Vec3,
        force: f32,
    ) {
        // Record impact
        self.damage_response.impacts.push(Impact {
            body_index,
            position,
            direction,
            force,
            time: 0.0,
        });

        // Increase pain
        let pain_increase = force * 0.01;
        self.damage_response.pain_level = (self.damage_response.pain_level + pain_increase).min(1.0);

        // Track damage to body part
        *self.damage_response.body_damage.entry(body_index).or_insert(0.0) += force * 0.01;

        // Apply physics impulse
        if let Some(body) = self.bodies.get(body_index) {
            if let Some(rb) = physics.rigid_body_set.get_mut(body.body_handle) {
                rb.apply_impulse_at_point(
                    vector![direction.x * force, direction.y * force, direction.z * force],
                    point![position.x, position.y, position.z],
                    true,
                );
            }

            // Reduce muscle strength in affected area
            if let Some(b) = self.bodies.get_mut(body_index) {
                b.muscle_strength *= 0.8;
            }
        }

        // Transition to active ragdoll on hit
        if self.state == RagdollState::Animated && force > 10.0 {
            self.state = RagdollState::Active;
        }

        // Check for death
        if body_index < self.bodies.len() {
            let body = &self.bodies[body_index];
            let damage = self.damage_response.body_damage.get(&body_index).copied().unwrap_or(0.0);

            if body.is_critical && damage > 0.8 {
                self.damage_response.is_dying = true;
            }
        }

        if self.damage_response.pain_level > 0.9 {
            self.damage_response.is_dying = true;
        }
    }

    /// Transition to full ragdoll mode (death)
    pub fn kill(&mut self) {
        self.damage_response.is_dying = true;
        self.state = RagdollState::Ragdoll;

        // Immediately reduce all muscle strength
        for body in &mut self.bodies {
            body.muscle_strength *= 0.3;
        }
    }

    /// Check if the ragdoll has settled (not moving)
    pub fn is_settled(&self, physics: &crate::PhysicsWorld) -> bool {
        if self.state != RagdollState::Dead {
            return false;
        }

        // Check if all bodies are nearly stationary
        for body in &self.bodies {
            if let Some(rb) = physics.rigid_body_set.get(body.body_handle) {
                let vel = rb.linvel();
                let ang_vel = rb.angvel();

                if vel.magnitude() > 0.1 || ang_vel.magnitude() > 0.1 {
                    return false;
                }
            }
        }

        true
    }

    /// Get the current transform of a bone
    pub fn get_bone_transform(&self, physics: &crate::PhysicsWorld, body_index: usize) -> Option<Transform> {
        self.bodies.get(body_index).and_then(|body| {
            physics.rigid_body_set.get(body.body_handle).map(|rb| {
                let pos = rb.translation();
                let rot = rb.rotation();
                Transform {
                    position: Vec3::new(pos.x, pos.y, pos.z),
                    rotation: Quat::from_xyzw(rot.i, rot.j, rot.k, rot.w),
                    scale: Vec3::ONE,
                }
            })
        })
    }
}

impl Default for ActiveRagdoll {
    fn default() -> Self {
        Self::new()
    }
}

/// Collision capsule for ragdoll building
#[derive(Debug, Clone, Copy)]
pub struct CollisionCapsule {
    pub start: Vec3,
    pub end: Vec3,
    pub radius: f32,
    pub bone_index: u32,
}
