//! Physics world management with Rapier3D.

use crate::collision::CollisionGroup;
use engine_core::{Transform, Vec3};
use rapier3d::na::{Isometry3, Vector3};
use rapier3d::prelude::*;

/// Environment collision groups so static geometry (terrain, roads, buildings) collides with player/enemies.
fn env_collision_groups() -> InteractionGroups {
    let (membership, filter) = CollisionGroup::environment();
    InteractionGroups::new(membership, filter)
}

/// Main physics world containing all simulation state.
pub struct PhysicsWorld {
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,
    pub gravity: Vector<Real>,
    pub integration_parameters: IntegrationParameters,
    pub physics_pipeline: PhysicsPipeline,
    pub island_manager: IslandManager,
    pub broad_phase: DefaultBroadPhase,
    pub narrow_phase: NarrowPhase,
    pub impulse_joint_set: ImpulseJointSet,
    pub multibody_joint_set: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
    pub query_pipeline: QueryPipeline,
}

impl Default for PhysicsWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl PhysicsWorld {
    /// Create a new physics world with default gravity.
    pub fn new() -> Self {
        Self {
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            gravity: vector![0.0, -9.81, 0.0],
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            query_pipeline: QueryPipeline::new(),
        }
    }

    /// Step the physics simulation.
    pub fn step(&mut self) {
        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            Some(&mut self.query_pipeline),
            &(),
            &(),
        );
    }

    /// Update query pipeline for raycasting.
    pub fn update_query_pipeline(&mut self) {
        self.query_pipeline.update(&self.collider_set);
    }

    /// Add a dynamic rigid body and return its handle.
    pub fn add_dynamic_body(&mut self, position: Vec3) -> RigidBodyHandle {
        let rigid_body = RigidBodyBuilder::dynamic()
            .translation(vector![position.x, position.y, position.z])
            .build();
        self.rigid_body_set.insert(rigid_body)
    }

    /// Add a kinematic rigid body (for player, enemies).
    pub fn add_kinematic_body(&mut self, position: Vec3) -> RigidBodyHandle {
        let rigid_body = RigidBodyBuilder::kinematic_position_based()
            .translation(vector![position.x, position.y, position.z])
            .build();
        self.rigid_body_set.insert(rigid_body)
    }

    /// Add a static rigid body (for terrain, walls).
    pub fn add_static_body(&mut self, position: Vec3) -> RigidBodyHandle {
        let rigid_body = RigidBodyBuilder::fixed()
            .translation(vector![position.x, position.y, position.z])
            .build();
        self.rigid_body_set.insert(rigid_body)
    }

    /// Add a box collider to a rigid body.
    pub fn add_box_collider(
        &mut self,
        body_handle: RigidBodyHandle,
        half_extents: Vec3,
    ) -> ColliderHandle {
        let collider = ColliderBuilder::cuboid(half_extents.x, half_extents.y, half_extents.z)
            .build();
        self.collider_set.insert_with_parent(collider, body_handle, &mut self.rigid_body_set)
    }

    /// Add a sphere collider to a rigid body.
    pub fn add_sphere_collider(
        &mut self,
        body_handle: RigidBodyHandle,
        radius: f32,
    ) -> ColliderHandle {
        let collider = ColliderBuilder::ball(radius).build();
        self.collider_set.insert_with_parent(collider, body_handle, &mut self.rigid_body_set)
    }

    /// Add a capsule collider (good for characters).
    pub fn add_capsule_collider(
        &mut self,
        body_handle: RigidBodyHandle,
        half_height: f32,
        radius: f32,
    ) -> ColliderHandle {
        let collider = ColliderBuilder::capsule_y(half_height, radius).build();
        self.collider_set.insert_with_parent(collider, body_handle, &mut self.rigid_body_set)
    }

    /// Add a ground plane collider (flat Y=0 half-space).
    pub fn add_ground_plane(&mut self) -> ColliderHandle {
        let collider = ColliderBuilder::halfspace(Vector::y_axis())
            .collision_groups(env_collision_groups())
            .build();
        self.collider_set.insert(collider)
    }

    /// Add a static cuboid collider (e.g. road segments). No parent body; collider is fixed in world.
    /// `translation`: world position of center. `rotation_y_rad`: rotation around Y axis in radians.
    /// `half_extents`: half sizes in local X, Y, Z (after rotation).
    pub fn add_static_cuboid(
        &mut self,
        translation: Vec3,
        rotation_y_rad: f32,
        half_extents: Vec3,
    ) -> ColliderHandle {
        let tra = vector![translation.x, translation.y, translation.z];
        let axisangle = Vector3::y_axis().into_inner() * (rotation_y_rad as Real);
        let position = Isometry3::new(tra, axisangle);
        let collider = ColliderBuilder::cuboid(
            half_extents.x as Real,
            half_extents.y as Real,
            half_extents.z as Real,
        )
        .position(position)
        .collision_groups(env_collision_groups())
        .build();
        self.collider_set.insert(collider)
    }

    /// Add a heightfield collider matching the triplanar terrain mesh.
    /// - `heights`: flat slice of height values in world Y, row-major order (index = z * ncols + x).
    /// - `nrows`, `ncols`: grid dimensions (must match terrain resolution).
    /// - `size_x`, `size_z`: total extent in world units (terrain spans -size/2 to +size/2 in X and Z).
    /// Heights are used as-is (scale_y = 1), so they must already be in world space.
    pub fn add_terrain_heightfield(
        &mut self,
        heights: &[f32],
        nrows: usize,
        ncols: usize,
        size_x: f32,
        size_z: f32,
    ) -> ColliderHandle {
        assert!(
            nrows >= 2 && ncols >= 2,
            "Terrain heightfield must have at least 2 rows and columns"
        );
        assert!(
            heights.len() >= nrows * ncols,
            "Heights slice too small for {}x{} grid",
            nrows,
            ncols
        );

        let heights_matrix = DMatrix::from_fn(nrows, ncols, |i, j| heights[i * ncols + j] as Real);
        let scale = vector![size_x, 1.0, size_z];

        let collider = ColliderBuilder::heightfield(heights_matrix, scale)
            .collision_groups(env_collision_groups())
            .build();
        self.collider_set.insert(collider)
    }

    /// Remove a collider by its handle.
    pub fn remove_collider(&mut self, handle: ColliderHandle) {
        self.collider_set.remove(
            handle,
            &mut self.island_manager,
            &mut self.rigid_body_set,
            true,
        );
    }

    /// Add a heightfield collider at a specific world offset (for chunked terrain).
    pub fn add_terrain_heightfield_at(
        &mut self,
        heights: &[f32],
        nrows: usize,
        ncols: usize,
        size_x: f32,
        size_z: f32,
        offset_x: f32,
        offset_z: f32,
    ) -> ColliderHandle {
        assert!(
            nrows >= 2 && ncols >= 2,
            "Terrain heightfield must have at least 2 rows and columns"
        );
        assert!(
            heights.len() >= nrows * ncols,
            "Heights slice too small for {}x{} grid",
            nrows,
            ncols
        );

        let heights_matrix = DMatrix::from_fn(nrows, ncols, |i, j| heights[i * ncols + j] as Real);
        let scale = vector![size_x, 1.0, size_z];

        let collider = ColliderBuilder::heightfield(heights_matrix, scale)
            .translation(vector![offset_x, 0.0, offset_z])
            .collision_groups(env_collision_groups())
            .build();
        self.collider_set.insert(collider)
    }

    /// Get the transform of a rigid body.
    pub fn get_body_transform(&self, handle: RigidBodyHandle) -> Option<Transform> {
        self.rigid_body_set.get(handle).map(|body| {
            let pos = body.translation();
            let rot = body.rotation();
            Transform {
                position: Vec3::new(pos.x, pos.y, pos.z),
                rotation: glam::Quat::from_xyzw(rot.i, rot.j, rot.k, rot.w),
                scale: Vec3::ONE,
            }
        })
    }

    /// Set the position of a kinematic body.
    pub fn set_kinematic_position(&mut self, handle: RigidBodyHandle, position: Vec3) {
        if let Some(body) = self.rigid_body_set.get_mut(handle) {
            body.set_next_kinematic_translation(vector![position.x, position.y, position.z]);
        }
    }

    /// Apply an impulse to a dynamic body.
    pub fn apply_impulse(&mut self, handle: RigidBodyHandle, impulse: Vec3) {
        if let Some(body) = self.rigid_body_set.get_mut(handle) {
            body.apply_impulse(vector![impulse.x, impulse.y, impulse.z], true);
        }
    }

    /// Remove a rigid body and its colliders.
    pub fn remove_body(&mut self, handle: RigidBodyHandle) {
        self.rigid_body_set.remove(
            handle,
            &mut self.island_manager,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            true,
        );
    }
}
