//! Raycasting for weapon hit detection and queries.

use crate::PhysicsWorld;
use engine_core::Vec3;
use rapier3d::prelude::*;

/// Result of a raycast query.
#[derive(Debug, Clone, Copy)]
pub struct RaycastHit {
    /// The collider that was hit.
    pub collider: ColliderHandle,
    /// Distance along the ray to the hit point.
    pub distance: f32,
    /// World position of the hit.
    pub point: Vec3,
    /// Surface normal at the hit point.
    pub normal: Vec3,
}

impl PhysicsWorld {
    /// Cast a ray and return the first hit.
    pub fn raycast(&self, origin: Vec3, direction: Vec3, max_distance: f32) -> Option<RaycastHit> {
        let ray = Ray::new(
            point![origin.x, origin.y, origin.z],
            vector![direction.x, direction.y, direction.z],
        );

        let filter = QueryFilter::default();

        self.query_pipeline
            .cast_ray_and_get_normal(
                &self.rigid_body_set,
                &self.collider_set,
                &ray,
                max_distance,
                true,
                filter,
            )
            .map(|(collider, intersection)| {
                let point = ray.point_at(intersection.time_of_impact);
                RaycastHit {
                    collider,
                    distance: intersection.time_of_impact,
                    point: Vec3::new(point.x, point.y, point.z),
                    normal: Vec3::new(
                        intersection.normal.x,
                        intersection.normal.y,
                        intersection.normal.z,
                    ),
                }
            })
    }

    /// Cast a ray and return all hits up to max_distance.
    pub fn raycast_all(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
    ) -> Vec<RaycastHit> {
        let ray = Ray::new(
            point![origin.x, origin.y, origin.z],
            vector![direction.x, direction.y, direction.z],
        );

        let filter = QueryFilter::default();
        let mut hits = Vec::new();

        self.query_pipeline.intersections_with_ray(
            &self.rigid_body_set,
            &self.collider_set,
            &ray,
            max_distance,
            true,
            filter,
            |collider, intersection: RayIntersection| {
                let point = ray.point_at(intersection.time_of_impact);
                hits.push(RaycastHit {
                    collider,
                    distance: intersection.time_of_impact,
                    point: Vec3::new(point.x, point.y, point.z),
                    normal: Vec3::new(
                        intersection.normal.x,
                        intersection.normal.y,
                        intersection.normal.z,
                    ),
                });
                true // Continue searching
            },
        );

        // Sort by distance (use unwrap_or to avoid panic on NaN)
        hits.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
        hits
    }

    /// Check if there's a clear line of sight between two points.
    pub fn line_of_sight(&self, from: Vec3, to: Vec3) -> bool {
        let direction = to - from;
        let distance = direction.length();
        if distance < 0.001 {
            return true;
        }

        let direction = direction / distance;
        self.raycast(from, direction, distance).is_none()
    }

    /// Find all colliders within a sphere.
    pub fn overlap_sphere(&self, center: Vec3, radius: f32) -> Vec<ColliderHandle> {
        let shape = Ball::new(radius);
        let shape_pos = Isometry::translation(center.x, center.y, center.z);
        let filter = QueryFilter::default();

        let mut results = Vec::new();
        self.query_pipeline.intersections_with_shape(
            &self.rigid_body_set,
            &self.collider_set,
            &shape_pos,
            &shape,
            filter,
            |collider| {
                results.push(collider);
                true // Continue searching
            },
        );

        results
    }
}
