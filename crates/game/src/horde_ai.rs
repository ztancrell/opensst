//! Horde AI system using flow fields.

use engine_core::{AIComponent, AIState, Transform, Velocity, Vec3};
use hecs::World;
use procgen::FlowField;

use crate::bug::Bug;
use crate::skinny::Skinny;

/// Smoothing factor for velocity (higher = more responsive, lower = more natural/fluid)
const VELOCITY_SMOOTHING: f32 = 0.25;
/// Blend flow field with direct pursuit (0 = pure flow, 1 = pure direct)
const DIRECT_PURSUIT_BLEND: f32 = 0.35;

/// Manages AI behavior for the bug horde.
pub struct HordeAI {
    flow_field: FlowField,
    target_position: Vec3,
    update_interval: f32,
    time_since_update: f32,
}

impl HordeAI {
    pub fn new(flow_field: FlowField) -> Self {
        Self {
            flow_field,
            target_position: Vec3::ZERO,
            update_interval: 0.35, // Extermination: more responsive horde movement
            time_since_update: 0.0,
        }
    }

    /// Update the target position (usually player position).
    pub fn update_target(&mut self, target: Vec3) {
        self.target_position = target;
    }

    /// Update all bugs in the horde.
    pub fn update(&mut self, world: &mut World, dt: f32) {
        // Periodically update the flow field
        self.time_since_update += dt;
        if self.time_since_update >= self.update_interval {
            self.time_since_update = 0.0;
            self.flow_field.set_goal(self.target_position);
        }

        // Update each bug
        for (_, (transform, velocity, bug, ai)) in
            world.query_mut::<(&mut Transform, &mut Velocity, &Bug, &mut AIComponent)>()
        {
            // Update AI state based on distance to target
            let to_target = self.target_position - transform.position;
            let distance = to_target.length();

            // State transitions
            match ai.state {
                AIState::Idle => {
                    if distance < ai.aggro_range {
                        ai.state = AIState::Chasing;
                    }
                }
                AIState::Chasing => {
                    if distance < ai.attack_range {
                        ai.state = AIState::Attacking;
                    } else if distance > ai.aggro_range * 1.5 {
                        ai.state = AIState::Idle;
                    }
                }
                AIState::Attacking => {
                    if distance > ai.attack_range * 1.5 {
                        ai.state = AIState::Chasing;
                    }
                    ai.update_cooldown(dt);
                }
                AIState::Fleeing | AIState::Dead => {}
            }

            // Movement based on state
            match ai.state {
                AIState::Chasing => {
                    // Sample flow field and blend with direct pursuit for natural paths
                    let flow_dir = self.flow_field.sample_smooth(transform.position);
                    let direct_xz = Vec3::new(to_target.x, 0.0, to_target.z).normalize_or_zero();

                    let flow_3d = if flow_dir.length_squared() > 0.01 {
                        Vec3::new(flow_dir.x, 0.0, flow_dir.y)
                    } else {
                        direct_xz
                    };

                    // Blend flow field with direct pursuit — reduces zig-zag at cell boundaries
                    let move_dir = (flow_3d * (1.0 - DIRECT_PURSUIT_BLEND) + direct_xz * DIRECT_PURSUIT_BLEND)
                        .normalize_or_zero();

                    let target_vel = move_dir * bug.move_speed;

                    // Smooth velocity for natural, fluid movement (no instant direction snaps)
                    let current_speed = velocity.linear.length();
                    velocity.linear = if current_speed > 0.01 {
                        let smoothed = velocity.linear * (1.0 - VELOCITY_SMOOTHING)
                            + target_vel * VELOCITY_SMOOTHING;
                        smoothed.normalize_or_zero() * bug.move_speed
                    } else {
                        target_vel
                    };

                    // Update position
                    transform.position += velocity.linear * dt;

                    // Face movement direction
                    if velocity.linear.length_squared() > 0.01 {
                        let forward = velocity.linear.normalize();
                        transform.rotation = glam::Quat::from_rotation_arc(Vec3::Z, forward);
                    }
                }
                AIState::Attacking => {
                    // Stop moving, face target
                    velocity.linear = Vec3::ZERO;
                    let look_dir = to_target.normalize_or_zero();
                    if look_dir.length_squared() > 0.01 {
                        transform.rotation = glam::Quat::from_rotation_arc(
                            Vec3::Z,
                            Vec3::new(look_dir.x, 0.0, look_dir.z).normalize_or_zero(),
                        );
                    }
                }
                AIState::Idle => {
                    // Slow wander or idle animation
                    velocity.linear *= 0.9;
                }
                AIState::Fleeing => {
                    // Run away from target
                    let flee_dir = -to_target.normalize_or_zero();
                    velocity.linear = Vec3::new(flee_dir.x, 0.0, flee_dir.z) * bug.move_speed * 1.5;
                    transform.position += velocity.linear * dt;
                }
                AIState::Dead => {
                    velocity.linear = Vec3::ZERO;
                }
            }

            // Y position is managed by the terrain snap in update_gameplay
        }

        // Skinnies (Heinlein): same flow-field chase/attack
        for (_, (transform, velocity, skinny, ai)) in
            world.query_mut::<(&mut Transform, &mut Velocity, &Skinny, &mut AIComponent)>()
        {
            let to_target = self.target_position - transform.position;
            let distance = to_target.length();

            match ai.state {
                AIState::Idle => {
                    if distance < ai.aggro_range {
                        ai.state = AIState::Chasing;
                    }
                }
                AIState::Chasing => {
                    if distance < ai.attack_range {
                        ai.state = AIState::Attacking;
                    } else if distance > ai.aggro_range * 1.5 {
                        ai.state = AIState::Idle;
                    }
                }
                AIState::Attacking => {
                    if distance > ai.attack_range * 1.5 {
                        ai.state = AIState::Chasing;
                    }
                    ai.update_cooldown(dt);
                }
                AIState::Fleeing | AIState::Dead => {}
            }

            match ai.state {
                AIState::Chasing => {
                    let flow_dir = self.flow_field.sample_smooth(transform.position);
                    let direct_xz = Vec3::new(to_target.x, 0.0, to_target.z).normalize_or_zero();

                    let flow_3d = if flow_dir.length_squared() > 0.01 {
                        Vec3::new(flow_dir.x, 0.0, flow_dir.y)
                    } else {
                        direct_xz
                    };

                    let move_dir = (flow_3d * (1.0 - DIRECT_PURSUIT_BLEND) + direct_xz * DIRECT_PURSUIT_BLEND)
                        .normalize_or_zero();

                    let target_vel = move_dir * skinny.move_speed;

                    let current_speed = velocity.linear.length();
                    velocity.linear = if current_speed > 0.01 {
                        let smoothed = velocity.linear * (1.0 - VELOCITY_SMOOTHING)
                            + target_vel * VELOCITY_SMOOTHING;
                        smoothed.normalize_or_zero() * skinny.move_speed
                    } else {
                        target_vel
                    };

                    transform.position += velocity.linear * dt;
                    if velocity.linear.length_squared() > 0.01 {
                        let forward = velocity.linear.normalize();
                        transform.rotation = glam::Quat::from_rotation_arc(Vec3::Z, forward);
                    }
                }
                AIState::Attacking => {
                    velocity.linear = Vec3::ZERO;
                    let look_dir = to_target.normalize_or_zero();
                    if look_dir.length_squared() > 0.01 {
                        transform.rotation = glam::Quat::from_rotation_arc(
                            Vec3::Z,
                            Vec3::new(look_dir.x, 0.0, look_dir.z).normalize_or_zero(),
                        );
                    }
                }
                AIState::Idle => {
                    velocity.linear *= 0.9;
                }
                AIState::Fleeing => {
                    let flee_dir = -to_target.normalize_or_zero();
                    velocity.linear = Vec3::new(flee_dir.x, 0.0, flee_dir.z) * skinny.move_speed * 1.5;
                    transform.position += velocity.linear * dt;
                }
                AIState::Dead => {
                    velocity.linear = Vec3::ZERO;
                }
            }
        }
    }

    /// Add obstacle to flow field (for destruction/terrain).
    pub fn add_obstacle(&mut self, position: Vec3, radius: f32) {
        let grid_pos = self.flow_field.world_to_grid(position);
        let grid_radius = (radius / self.flow_field.cell_size).ceil() as i32;

        for dy in -grid_radius..=grid_radius {
            for dx in -grid_radius..=grid_radius {
                let x = grid_pos.x + dx;
                let y = grid_pos.y + dy;
                if x >= 0 && y >= 0 {
                    self.flow_field.set_blocked(x as usize, y as usize);
                }
            }
        }
    }

    /// Clear obstacles (for when destruction clears debris).
    pub fn clear_obstacles(&mut self) {
        self.flow_field.clear();
    }
}

/// Separation behavior to prevent bugs from overlapping.
/// Uses a spatial grid for O(n) average performance instead of O(n²).
pub fn apply_separation(world: &mut World, separation_radius: f32, separation_force: f32) {
    use std::collections::HashMap;
    use crate::bug_entity::PhysicsBug;

    // Collect living bug positions (skip ragdolling/dead bugs)
    let positions: Vec<(hecs::Entity, Vec3)> = world
        .query::<(&Transform, &PhysicsBug)>()
        .with::<&Bug>()
        .iter()
        .filter(|(_, (_, pb))| !pb.is_ragdoll)
        .map(|(e, (t, _))| (e, t.position))
        .collect();

    if positions.len() < 2 { return; }

    // Build spatial grid (cell size = separation_radius)
    let cell_size = separation_radius;
    let inv_cell = 1.0 / cell_size;
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();

    for (idx, (_, pos)) in positions.iter().enumerate() {
        let cx = (pos.x * inv_cell).floor() as i32;
        let cz = (pos.z * inv_cell).floor() as i32;
        grid.entry((cx, cz)).or_default().push(idx);
    }

    let sep_sq = separation_radius * separation_radius;

    // Compute separation forces using grid neighbors
    let mut forces: Vec<Vec3> = vec![Vec3::ZERO; positions.len()];

    for (idx, (_, pos)) in positions.iter().enumerate() {
        let cx = (pos.x * inv_cell).floor() as i32;
        let cz = (pos.z * inv_cell).floor() as i32;

        let mut separation = Vec3::ZERO;
        let mut count = 0u32;

        // Check 3x3 neighbor cells
        for dx in -1..=1 {
            for dz in -1..=1 {
                if let Some(cell) = grid.get(&(cx + dx, cz + dz)) {
                    for &other_idx in cell {
                        if other_idx == idx { continue; }
                        let diff = *pos - positions[other_idx].1;
                        let dist_sq = diff.x * diff.x + diff.z * diff.z; // XZ plane only
                        if dist_sq < sep_sq && dist_sq > 0.001 {
                            let dist = dist_sq.sqrt();
                            // Stronger push when closer
                            let push_strength = (1.0 - dist / separation_radius).max(0.0);
                            separation += Vec3::new(diff.x, 0.0, diff.z).normalize_or_zero() * push_strength;
                            count += 1;
                        }
                    }
                }
            }
        }

        if count > 0 {
            forces[idx] = separation.normalize_or_zero() * separation_force;
        }
    }

    // Apply forces to velocities
    for (idx, (entity, _)) in positions.iter().enumerate() {
        if forces[idx].length_squared() > 0.01 {
            if let Ok(mut velocity) = world.get::<&mut Velocity>(*entity) {
                velocity.linear += forces[idx];
            }
        }
    }
}
