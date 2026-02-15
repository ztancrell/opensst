//! Flow field pathfinding for horde AI.
//! 
//! Flow fields allow thousands of units to pathfind efficiently by computing
//! a single direction field that all units sample.

use glam::{IVec2, Vec2, Vec3};
use std::collections::VecDeque;

/// A flow field for pathfinding many units efficiently.
#[derive(Debug)]
pub struct FlowField {
    /// Width of the grid.
    pub width: usize,
    /// Height of the grid.
    pub height: usize,
    /// Cell size in world units.
    pub cell_size: f32,
    /// Origin of the grid in world space.
    pub origin: Vec2,
    /// Cost field (0 = walkable, 255 = blocked).
    costs: Vec<u8>,
    /// Integration field (distance to goal).
    integration: Vec<u16>,
    /// Flow directions (normalized).
    directions: Vec<Vec2>,
    /// Goal position in grid coordinates.
    goal: Option<IVec2>,
}

const BLOCKED: u8 = 255;
const MAX_INTEGRATION: u16 = u16::MAX;

impl FlowField {
    /// Create a new flow field with the given dimensions.
    pub fn new(width: usize, height: usize, cell_size: f32, origin: Vec2) -> Self {
        let size = width * height;
        Self {
            width,
            height,
            cell_size,
            origin,
            costs: vec![1; size],
            integration: vec![MAX_INTEGRATION; size],
            directions: vec![Vec2::ZERO; size],
            goal: None,
        }
    }

    /// Set a cell as blocked (obstacle).
    pub fn set_blocked(&mut self, x: usize, y: usize) {
        if x < self.width && y < self.height {
            self.costs[y * self.width + x] = BLOCKED;
        }
    }

    /// Set a cell's movement cost (1-254, higher = slower).
    pub fn set_cost(&mut self, x: usize, y: usize, cost: u8) {
        if x < self.width && y < self.height {
            self.costs[y * self.width + x] = cost.min(254).max(1);
        }
    }

    /// Clear all blocked cells.
    pub fn clear(&mut self) {
        self.costs.fill(1);
        self.integration.fill(MAX_INTEGRATION);
        self.directions.fill(Vec2::ZERO);
        self.goal = None;
    }

    /// Set the goal and recalculate the field.
    /// Centers the grid on the goal so the flow field always covers the area around the target.
    pub fn set_goal(&mut self, world_pos: Vec3) {
        // Center the grid on the goal so bugs pathfind correctly wherever the player is
        let half_w = (self.width as f32 * self.cell_size) * 0.5;
        let half_h = (self.height as f32 * self.cell_size) * 0.5;
        self.origin = Vec2::new(world_pos.x - half_w, world_pos.z - half_h);

        let grid_pos = self.world_to_grid(world_pos);
        self.set_goal_grid(grid_pos.x, grid_pos.y);
    }

    /// Set goal by grid coordinates.
    pub fn set_goal_grid(&mut self, x: i32, y: i32) {
        let x = x.clamp(0, self.width as i32 - 1) as usize;
        let y = y.clamp(0, self.height as i32 - 1) as usize;
        
        self.goal = Some(IVec2::new(x as i32, y as i32));
        self.calculate_integration(x, y);
        self.calculate_flow();
    }

    /// Calculate integration field using Dijkstra-style BFS.
    fn calculate_integration(&mut self, goal_x: usize, goal_y: usize) {
        self.integration.fill(MAX_INTEGRATION);

        let goal_idx = goal_y * self.width + goal_x;
        if self.costs[goal_idx] == BLOCKED {
            return;
        }

        self.integration[goal_idx] = 0;

        let mut open = VecDeque::new();
        open.push_back((goal_x, goal_y));

        // Cardinal and diagonal neighbors
        let neighbors: [(i32, i32, u16); 8] = [
            (-1, 0, 10),  // Cardinal cost 10
            (1, 0, 10),
            (0, -1, 10),
            (0, 1, 10),
            (-1, -1, 14), // Diagonal cost 14 (≈ √2 * 10)
            (1, -1, 14),
            (-1, 1, 14),
            (1, 1, 14),
        ];

        while let Some((x, y)) = open.pop_front() {
            let current_idx = y * self.width + x;
            let current_cost = self.integration[current_idx];

            for (dx, dy, base_cost) in neighbors {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;

                if nx < 0 || ny < 0 || nx >= self.width as i32 || ny >= self.height as i32 {
                    continue;
                }

                let nx = nx as usize;
                let ny = ny as usize;
                let neighbor_idx = ny * self.width + nx;

                if self.costs[neighbor_idx] == BLOCKED {
                    continue;
                }

                let new_cost = current_cost
                    .saturating_add(base_cost)
                    .saturating_add(self.costs[neighbor_idx] as u16 * 10);

                if new_cost < self.integration[neighbor_idx] {
                    self.integration[neighbor_idx] = new_cost;
                    open.push_back((nx, ny));
                }
            }
        }
    }

    /// Calculate flow directions from integration field.
    fn calculate_flow(&mut self) {
        let neighbors: [(i32, i32); 8] = [
            (-1, 0),
            (1, 0),
            (0, -1),
            (0, 1),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
        ];

        for y in 0..self.height {
            for x in 0..self.width {
                let idx = y * self.width + x;

                if self.costs[idx] == BLOCKED || self.integration[idx] == MAX_INTEGRATION {
                    self.directions[idx] = Vec2::ZERO;
                    continue;
                }

                // Find neighbor with lowest integration value
                let mut best_dir = Vec2::ZERO;
                let mut best_cost = self.integration[idx];

                for (dx, dy) in neighbors {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;

                    if nx < 0 || ny < 0 || nx >= self.width as i32 || ny >= self.height as i32 {
                        continue;
                    }

                    let neighbor_idx = ny as usize * self.width + nx as usize;
                    let neighbor_cost = self.integration[neighbor_idx];

                    if neighbor_cost < best_cost {
                        best_cost = neighbor_cost;
                        best_dir = Vec2::new(dx as f32, dy as f32);
                    }
                }

                self.directions[idx] = best_dir.normalize_or_zero();
            }
        }
    }

    /// Convert world position to grid coordinates.
    pub fn world_to_grid(&self, world_pos: Vec3) -> IVec2 {
        let local = Vec2::new(world_pos.x, world_pos.z) - self.origin;
        IVec2::new(
            (local.x / self.cell_size) as i32,
            (local.y / self.cell_size) as i32,
        )
    }

    /// Convert grid coordinates to world position (center of cell).
    pub fn grid_to_world(&self, grid_pos: IVec2) -> Vec3 {
        let x = self.origin.x + (grid_pos.x as f32 + 0.5) * self.cell_size;
        let z = self.origin.y + (grid_pos.y as f32 + 0.5) * self.cell_size;
        Vec3::new(x, 0.0, z)
    }

    /// Sample the flow direction at a world position.
    pub fn sample(&self, world_pos: Vec3) -> Vec2 {
        let grid_pos = self.world_to_grid(world_pos);

        if grid_pos.x < 0
            || grid_pos.y < 0
            || grid_pos.x >= self.width as i32
            || grid_pos.y >= self.height as i32
        {
            // Return direction towards center of field
            let center = self.origin + Vec2::new(
                self.width as f32 * self.cell_size * 0.5,
                self.height as f32 * self.cell_size * 0.5,
            );
            return (center - Vec2::new(world_pos.x, world_pos.z)).normalize_or_zero();
        }

        let idx = grid_pos.y as usize * self.width + grid_pos.x as usize;
        self.directions[idx]
    }

    /// Sample with bilinear interpolation for smoother movement.
    /// Uses a 3x3 weighted average for even smoother transitions at cell boundaries.
    pub fn sample_smooth(&self, world_pos: Vec3) -> Vec2 {
        let local = Vec2::new(world_pos.x, world_pos.z) - self.origin;
        let gx = local.x / self.cell_size - 0.5;
        let gy = local.y / self.cell_size - 0.5;

        let x0 = gx.floor() as i32;
        let y0 = gy.floor() as i32;
        let fx = gx.fract();
        let fy = gy.fract();

        // 3x3 weighted sample — center cells have more weight for smoother paths
        let w_center = 2.0;
        let w_edge = 1.0;
        let mut acc = Vec2::ZERO;
        let mut total_w = 0.0;

        for dy in -1..=1 {
            for dx in -1..=1 {
                let d = self.sample_grid(x0 + dx, y0 + dy);
                if d.length_squared() > 0.001 {
                    let w = if dx == 0 && dy == 0 {
                        w_center
                    } else if dx == 0 || dy == 0 {
                        w_edge
                    } else {
                        w_edge * 0.7 // corners slightly less
                    };
                    acc += d * w;
                    total_w += w;
                }
            }
        }

        if total_w > 0.01 {
            (acc / total_w).normalize_or_zero()
        } else {
            // Fallback: bilinear over center 2x2
            let d00 = self.sample_grid(x0, y0);
            let d10 = self.sample_grid(x0 + 1, y0);
            let d01 = self.sample_grid(x0, y0 + 1);
            let d11 = self.sample_grid(x0 + 1, y0 + 1);
            let d0 = d00 * (1.0 - fx) + d10 * fx;
            let d1 = d01 * (1.0 - fx) + d11 * fx;
            (d0 * (1.0 - fy) + d1 * fy).normalize_or_zero()
        }
    }

    fn sample_grid(&self, x: i32, y: i32) -> Vec2 {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return Vec2::ZERO;
        }
        let idx = y as usize * self.width + x as usize;
        self.directions[idx]
    }

    /// Check if a grid cell is walkable.
    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return false;
        }
        let idx = y as usize * self.width + x as usize;
        self.costs[idx] != BLOCKED
    }

    /// Get the goal position if set.
    pub fn goal(&self) -> Option<IVec2> {
        self.goal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_field_new_dimensions() {
        let f = FlowField::new(20, 30, 2.0, Vec2::new(-10.0, -15.0));
        assert_eq!(f.width, 20);
        assert_eq!(f.height, 30);
        assert_eq!(f.cell_size, 2.0);
        assert_eq!(f.origin, Vec2::new(-10.0, -15.0));
    }

    #[test]
    fn flow_field_set_goal_updates_origin() {
        let mut f = FlowField::new(10, 10, 2.0, Vec2::ZERO);
        f.set_goal(Vec3::new(100.0, 0.0, 50.0));
        assert!(f.goal().is_some());
        // Origin should be centered on goal (half_w = 10, half_h = 10)
        assert!((f.origin.x - 90.0).abs() < 1.0);
        assert!((f.origin.y - 40.0).abs() < 1.0);
    }

    #[test]
    fn flow_field_world_to_grid_roundtrip() {
        let f = FlowField::new(10, 10, 2.0, Vec2::new(0.0, 0.0));
        let world = Vec3::new(5.0, 0.0, 7.0);
        let grid = f.world_to_grid(world);
        let back = f.grid_to_world(grid);
        assert!((back.x - 5.0).abs() < 2.0);
        assert!((back.z - 7.0).abs() < 2.0);
    }

    #[test]
    fn flow_field_set_blocked_is_walkable() {
        let mut f = FlowField::new(8, 8, 1.0, Vec2::ZERO);
        assert!(f.is_walkable(4, 4));
        f.set_blocked(4, 4);
        assert!(!f.is_walkable(4, 4));
    }
}
