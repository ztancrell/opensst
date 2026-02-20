//! Minecraft / Ace of Spades style voxel terrain.
//!
//! Chunks are 3D grids of blocks. Generation uses the same noise as heightfield terrain
//! to get height at (x,z), then fills columns with Stone, Dirt, Grass/Sand/Snow.
//! Minecraft-style caves are carved underground on every planet (denser on HiveWorld/Fungal).
//! Mesh is built from culled cube faces; physics uses a heightfield derived from voxel tops.

use crate::biome::{BiomeType, PlanetBiomes};
use crate::terrain::{TerrainConfig, TerrainData, TerrainVertex};
use noise::{NoiseFn, Perlin};

/// Block type for voxel terrain (Minecraft/Ace of Spades style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BlockId {
    Air = 0,
    Stone = 1,
    Dirt = 2,
    Grass = 3,
    Sand = 4,
    Water = 5,
    Snow = 6,
    /// Bottom layer of planet (Minecraft-style bedrock).
    Bedrock = 7,
}

impl BlockId {
    pub fn is_solid(self) -> bool {
        !matches!(self, BlockId::Air | BlockId::Water)
    }

    /// Drawn in terrain mesh (solid blocks + water).
    pub fn is_renderable(self) -> bool {
        !matches!(self, BlockId::Air)
    }

    /// Vertex color for terrain shader (RGBA).
    pub fn color(self) -> [f32; 4] {
        match self {
            BlockId::Air => [0.0, 0.0, 0.0, 0.0],
            BlockId::Stone => [0.45, 0.42, 0.40, 1.0],
            BlockId::Dirt => [0.42, 0.32, 0.22, 1.0],
            BlockId::Grass => [0.28, 0.48, 0.22, 1.0],
            BlockId::Sand => [0.82, 0.72, 0.52, 1.0],
            BlockId::Water => [0.2, 0.35, 0.6, 0.7],
            BlockId::Snow => [0.92, 0.94, 0.98, 1.0],
            BlockId::Bedrock => [0.22, 0.20, 0.22, 1.0],
        }
    }
}

/// Minecraft-style layer counts (in blocks). Tune per planet if desired.
const BEDROCK_LAYERS: usize = 2;
const DIRT_LAYERS: usize = 3;
/// Minimum terrain depth in world units (baseline so valleys still have blocks to dig). Added to height, not clamped.
const MIN_TERRAIN_WORLD_Y: f32 = 24.0; // 24 blocks at 1m (Minecraft Steve scale)
/// When filling water after deform: only fill air if solid ground is within this many blocks below (avoids water-over-cave deadfall pits).
const WATER_FILL_BUFFER: usize = 6;

/// Deterministic noise seed from world seed (same formula as terrain/biome for reproducibility).
#[inline]
fn cave_noise_seed(seed: u64, offset: u64) -> u32 {
    ((seed.wrapping_add(offset))
        .wrapping_mul(0x9e3779b97f4a7c15_u64)
        .wrapping_add(offset.wrapping_mul(0x6c078965_u64))
        >> 32) as u32
}

/// One chunk of voxel terrain. Block-aligned grid.
/// Chunk spans [offset_x - size_x/2, offset_x + size_x/2] in X, same for Z, Y from 0 to size_y.
#[derive(Debug, Clone)]
pub struct VoxelChunk {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub block_size: f32,
    pub offset_x: f32,
    pub offset_z: f32,
    pub data: Vec<BlockId>,
}

impl VoxelChunk {
    pub fn index(&self, ix: usize, iy: usize, iz: usize) -> usize {
        ix + self.nx * (iy + self.ny * iz)
    }

    pub fn get(&self, ix: usize, iy: usize, iz: usize) -> BlockId {
        if ix < self.nx && iy < self.ny && iz < self.nz {
            self.data[self.index(ix, iy, iz)]
        } else {
            BlockId::Air
        }
    }

    pub fn set(&mut self, ix: usize, iy: usize, iz: usize, block: BlockId) {
        if ix < self.nx && iy < self.ny && iz < self.nz {
            let i = self.index(ix, iy, iz);
            self.data[i] = block;
        }
    }

    /// World X at block center (ix, iz).
    pub fn world_x(&self, ix: usize) -> f32 {
        let half = (self.nx as f32 * self.block_size) * 0.5;
        self.offset_x - half + (ix as f32 + 0.5) * self.block_size
    }

    pub fn world_z(&self, iz: usize) -> f32 {
        let half = (self.nz as f32 * self.block_size) * 0.5;
        self.offset_z - half + (iz as f32 + 0.5) * self.block_size
    }

    /// World Y at block bottom (iy).
    pub fn world_y(&self, iy: usize) -> f32 {
        (iy as f32) * self.block_size
    }

    /// Generate voxel chunk from terrain config (same noise as heightfield terrain).
    pub fn generate(
        config: &TerrainConfig,
        planet_biomes: Option<&PlanetBiomes>,
    ) -> Self {
        let block_size = 1.0; // 1m blocks, Minecraft Steve scale
        let nx = (config.size / block_size) as usize;
        let nz = (config.size / block_size) as usize;
        // Minecraft-like depth: many vertical layers so caves have room (surface sits in upper third).
        let ny = ((config.height_scale * 2.0) / block_size).ceil().max(64.0) as usize;
        let len = nx * ny * nz;
        let mut data = vec![BlockId::Air; len];
        let mut top_block_y_col: Vec<usize> = vec![0; nx * nz];

        let has_water = config.water_level.is_some();
        let _water_level_norm = 0.35;
        // Baseline + variation: hills, mountains, plains (Minecraft-style). Sea level sits above baseline.
        let sea_level_world = config.water_level.map(|w| MIN_TERRAIN_WORLD_Y + w * config.height_scale);

        for iz in 0..nz {
            for ix in 0..nx {
                let wx = config.offset_x - config.size * 0.5 + (ix as f32 + 0.5) * block_size;
                let wz = config.offset_z - config.size * 0.5 + (iz as f32 + 0.5) * block_size;
                let norm = TerrainData::sample_height_for_voxel(config, wx as f64, wz as f64);
                let height_mult = planet_biomes
                    .map(|pb| pb.height_scale_at(wx as f64, wz as f64))
                    .unwrap_or(1.0);
                // Additive baseline + amplified variation: plains, hills, mountains (Minecraft-style).
                let variation = (norm as f32 * config.height_scale * height_mult).max(0.0) * 1.25;
                let world_y = variation + MIN_TERRAIN_WORLD_Y;
                let top_block_y = (world_y / block_size).floor() as usize;
                let top_block_y = top_block_y.min(ny.saturating_sub(1));
                top_block_y_col[ix + nx * iz] = top_block_y;

                // Minecraft-style surface block from biome
                let surface_block = if let Some(pb) = planet_biomes {
                    let (biome_cfg, _) = pb.sample_at(wx as f64, wz as f64);
                    match biome_cfg.biome_type {
                        BiomeType::Frozen | BiomeType::Tundra => BlockId::Snow,
                        BiomeType::Desert | BiomeType::Wasteland | BiomeType::SaltFlat => BlockId::Sand,
                        _ => BlockId::Grass,
                    }
                } else {
                    BlockId::Grass
                };

                // Layers: bedrock (bottom) -> stone -> dirt -> surface (top)
                let stone_start = top_block_y.saturating_sub(DIRT_LAYERS);

                for iy in 0..ny {
                    let idx = ix + nx * (iy + ny * iz);
                    if iy > top_block_y {
                        if has_water {
                            if let Some(sw) = sea_level_world {
                                let water_top = (sw / block_size).floor() as usize;
                                if iy <= water_top && iy <= ny.saturating_sub(1) {
                                    data[idx] = BlockId::Water;
                                }
                            }
                        }
                        continue;
                    }
                    if iy < BEDROCK_LAYERS {
                        data[idx] = BlockId::Bedrock;
                    } else if iy == top_block_y {
                        data[idx] = surface_block;
                    } else if iy >= stone_start {
                        data[idx] = BlockId::Dirt;
                    } else {
                        data[idx] = BlockId::Stone;
                    }
                }
            }
        }

        // Minecraft-style caves: smaller tunnels, entrances only (never carve near surface), varied sizes.
        let (cave_scale, base_threshold) = if let Some(pb) = planet_biomes {
            let (biome_cfg, _) = pb.sample_at(config.offset_x as f64, config.offset_z as f64);
            match biome_cfg.biome_type {
                BiomeType::HiveWorld | BiomeType::Fungal => (0.038, 0.04), // slightly more caves, still small
                _ => (0.032, 0.06), // small tunnels, rare carve = walkable surface
            }
        } else {
            (0.032, 0.06)
        };
        const CAVE_SURFACE_BUFFER: usize = 12; // solid crust below surface/water so no deadfall pits; caves start deeper
        let cave_noise = Perlin::new(cave_noise_seed(config.seed, 10));
        let size_noise = Perlin::new(cave_noise_seed(config.seed, 11)); // varies tunnel size by area
        for iz in 0..nz {
            for iy in BEDROCK_LAYERS..ny {
                for ix in 0..nx {
                    let top_y = top_block_y_col[ix + nx * iz];
                    if iy >= top_y {
                        continue;
                    }
                    // Keep surface solid: only carve well below so we get cave entrances, not holes everywhere.
                    if iy + CAVE_SURFACE_BUFFER > top_y {
                        continue;
                    }
                    let idx = ix + nx * (iy + ny * iz);
                    if !data[idx].is_solid() {
                        continue;
                    }
                    let wx = config.offset_x - config.size * 0.5 + (ix as f32 + 0.5) * block_size;
                    let wy = (iy as f32 + 0.5) * block_size;
                    let wz = config.offset_z - config.size * 0.5 + (iz as f32 + 0.5) * block_size;
                    let n = cave_noise.get([
                        wx as f64 * cave_scale,
                        wy as f64 * cave_scale,
                        wz as f64 * cave_scale,
                    ]);
                    // Per-region size variation: some areas slightly bigger passages, some tighter.
                    let size_var = size_noise.get([
                        wx as f64 * 0.015,
                        wy as f64 * 0.015,
                        wz as f64 * 0.015,
                    ]);
                    let threshold = base_threshold + size_var * 0.04; // ±0.04 variation
                    if n < threshold {
                        data[idx] = BlockId::Air;
                    }
                }
            }
        }

        VoxelChunk {
            nx,
            ny,
            nz,
            block_size,
            offset_x: config.offset_x,
            offset_z: config.offset_z,
            data,
        }
    }

    /// Heightmap for physics: (nx+1) x (nz+1) grid of top solid block Y in world space.
    pub fn to_heightmap(&self) -> Vec<f32> {
        let rows = self.nz + 1;
        let cols = self.nx + 1;
        let mut out = Vec::with_capacity(rows * cols);
        for iz in 0..=self.nz {
            for ix in 0..=self.nx {
                let (ix0, iz0) = (ix.min(self.nx.saturating_sub(1)), iz.min(self.nz.saturating_sub(1)));
                let mut top_y = 0f32;
                for iy in (0..self.ny).rev() {
                    let b = self.get(ix0, iy, iz0);
                    if b.is_solid() {
                        top_y = self.world_y(iy) + self.block_size;
                        break;
                    }
                }
                out.push(top_y);
            }
        }
        out
    }

    /// Build terrain mesh (vertices + indices) from voxel data. Only exposed faces.
    /// Excludes water so it can be drawn separately with transparency.
    pub fn to_mesh(&self) -> (Vec<TerrainVertex>, Vec<u32>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let half = self.block_size * 0.5;

        for iz in 0..self.nz {
            for iy in 0..self.ny {
                for ix in 0..self.nx {
                    let b = self.get(ix, iy, iz);
                    if !b.is_renderable() || b == BlockId::Water {
                        continue;
                    }
                    let cx = self.world_x(ix);
                    let cy = self.world_y(iy) + half;
                    let cz = self.world_z(iz);
                    let color = b.color();

                    let px = cx - half;
                    let py = cy - half;
                    let pz = cz - half;
                    let px1 = cx + half;
                    let py1 = cy + half;
                    let pz1 = cz + half;

                    // CCW winding when viewed from outside (terrain pipeline culls back face).
                    let add_quad = |v: &mut Vec<TerrainVertex>, i: &mut Vec<u32>, pos: [[f32; 3]; 4], normal: [f32; 3]| {
                        let base = v.len() as u32;
                        for p in pos {
                            v.push(TerrainVertex {
                                position: p,
                                normal,
                                uv: [0.0, 0.0],
                                color,
                            });
                        }
                        i.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
                    };
                    // Reverse vertex order so quad is CCW from outside (was CW, so back-face culled).
                    let add_quad_ccw = |v: &mut Vec<TerrainVertex>, i: &mut Vec<u32>, pos: [[f32; 3]; 4], normal: [f32; 3]| {
                        let rev: [[f32; 3]; 4] = [pos[3], pos[2], pos[1], pos[0]];
                        add_quad(v, i, rev, normal);
                    };

                    let neighbor_solid_or_water = |bx: usize, by: usize, bz: usize| {
                        self.get(bx, by, bz).is_renderable()
                    };
                    if !neighbor_solid_or_water(ix, iy + 1, iz) || iy + 1 >= self.ny {
                        add_quad_ccw(&mut vertices, &mut indices,
                            [[px, py1, pz], [px1, py1, pz], [px1, py1, pz1], [px, py1, pz1]],
                            [0.0, 1.0, 0.0]);
                    }
                    if iy == 0 || !neighbor_solid_or_water(ix, iy - 1, iz) {
                        add_quad_ccw(&mut vertices, &mut indices,
                            [[px, py, pz1], [px1, py, pz1], [px1, py, pz], [px, py, pz]],
                            [0.0, -1.0, 0.0]);
                    }
                    if ix + 1 >= self.nx || !neighbor_solid_or_water(ix + 1, iy, iz) {
                        add_quad_ccw(&mut vertices, &mut indices,
                            [[px1, py, pz], [px1, py1, pz], [px1, py1, pz1], [px1, py, pz1]],
                            [1.0, 0.0, 0.0]);
                    }
                    if ix == 0 || !neighbor_solid_or_water(ix - 1, iy, iz) {
                        add_quad_ccw(&mut vertices, &mut indices,
                            [[px, py, pz], [px, py1, pz], [px, py1, pz1], [px, py, pz1]],
                            [-1.0, 0.0, 0.0]);
                    }
                    if iz + 1 >= self.nz || !neighbor_solid_or_water(ix, iy, iz + 1) {
                        add_quad_ccw(&mut vertices, &mut indices,
                            [[px, py, pz1], [px1, py, pz1], [px1, py1, pz1], [px, py1, pz1]],
                            [0.0, 0.0, 1.0]);
                    }
                    if iz == 0 || !neighbor_solid_or_water(ix, iy, iz - 1) {
                        add_quad_ccw(&mut vertices, &mut indices,
                            [[px, py, pz], [px, py1, pz], [px1, py1, pz], [px1, py, pz]],
                            [0.0, 0.0, -1.0]);
                    }
                }
            }
        }

        (vertices, indices)
    }

    /// Build water-only mesh for transparent rendering (Minecraft-style). Only Water block faces.
    pub fn to_water_mesh(&self) -> (Vec<TerrainVertex>, Vec<u32>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let half = self.block_size * 0.5;
        // Minecraft-like transparency
        let water_color = [0.15, 0.28, 0.52, 0.58];

        for iz in 0..self.nz {
            for iy in 0..self.ny {
                for ix in 0..self.nx {
                    if self.get(ix, iy, iz) != BlockId::Water {
                        continue;
                    }
                    let cx = self.world_x(ix);
                    let cy = self.world_y(iy) + half;
                    let cz = self.world_z(iz);
                    let px = cx - half;
                    let py = cy - half;
                    let pz = cz - half;
                    let px1 = cx + half;
                    let py1 = cy + half;
                    let pz1 = cz + half;

                    let add_quad_ccw = |v: &mut Vec<TerrainVertex>, i: &mut Vec<u32>, pos: [[f32; 3]; 4], normal: [f32; 3]| {
                        let base = v.len() as u32;
                        let rev: [[f32; 3]; 4] = [pos[3], pos[2], pos[1], pos[0]];
                        for p in rev {
                            v.push(TerrainVertex { position: p, normal, uv: [0.0, 0.0], color: water_color });
                        }
                        i.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
                    };

                    let neighbor_water_or_solid = |bx: usize, by: usize, bz: usize| {
                        let n = self.get(bx, by, bz);
                        n == BlockId::Water || n.is_solid()
                    };
                    if iy + 1 >= self.ny || !neighbor_water_or_solid(ix, iy + 1, iz) {
                        add_quad_ccw(&mut vertices, &mut indices,
                            [[px, py1, pz], [px1, py1, pz], [px1, py1, pz1], [px, py1, pz1]], [0.0, 1.0, 0.0]);
                    }
                    if iy == 0 || !neighbor_water_or_solid(ix, iy - 1, iz) {
                        add_quad_ccw(&mut vertices, &mut indices,
                            [[px, py, pz1], [px1, py, pz1], [px1, py, pz], [px, py, pz]], [0.0, -1.0, 0.0]);
                    }
                    if ix + 1 >= self.nx || !neighbor_water_or_solid(ix + 1, iy, iz) {
                        add_quad_ccw(&mut vertices, &mut indices,
                            [[px1, py, pz], [px1, py1, pz], [px1, py1, pz1], [px1, py, pz1]], [1.0, 0.0, 0.0]);
                    }
                    if ix == 0 || !neighbor_water_or_solid(ix - 1, iy, iz) {
                        add_quad_ccw(&mut vertices, &mut indices,
                            [[px, py, pz], [px, py1, pz], [px, py1, pz1], [px, py, pz1]], [-1.0, 0.0, 0.0]);
                    }
                    if iz + 1 >= self.nz || !neighbor_water_or_solid(ix, iy, iz + 1) {
                        add_quad_ccw(&mut vertices, &mut indices,
                            [[px, py, pz1], [px1, py, pz1], [px1, py1, pz1], [px, py1, pz1]], [0.0, 0.0, 1.0]);
                    }
                    if iz == 0 || !neighbor_water_or_solid(ix, iy, iz - 1) {
                        add_quad_ccw(&mut vertices, &mut indices,
                            [[px, py, pz], [px, py1, pz], [px1, py1, pz], [px1, py, pz]], [0.0, 0.0, -1.0]);
                    }
                }
            }
        }
        (vertices, indices)
    }

    /// Sample height at world (x, z). Returns top solid block Y (world space).
    pub fn sample_height(&self, x: f32, z: f32) -> f32 {
        let half = (self.nx as f32 * self.block_size) * 0.5;
        let ix = ((x - self.offset_x + half) / self.block_size).floor() as i32;
        let iz = ((z - self.offset_z + half) / self.block_size).floor() as i32;
        if ix < 0 || ix >= self.nx as i32 || iz < 0 || iz >= self.nz as i32 {
            return 0.0;
        }
        let (ix, iz) = (ix as usize, iz as usize);
        for iy in (0..self.ny).rev() {
            if self.get(ix, iy, iz).is_solid() {
                return self.world_y(iy) + self.block_size;
            }
        }
        0.0
    }

    /// Top surface block at (x, z): solid or water. None if column is empty or out of bounds.
    /// Use to detect "in water" so craters (dry) are not treated as water.
    pub fn surface_block_at(&self, x: f32, z: f32) -> Option<BlockId> {
        let half = (self.nx as f32 * self.block_size) * 0.5;
        let ix = ((x - self.offset_x + half) / self.block_size).floor() as i32;
        let iz = ((z - self.offset_z + half) / self.block_size).floor() as i32;
        if ix < 0 || ix >= self.nx as i32 || iz < 0 || iz >= self.nz as i32 {
            return None;
        }
        let (ix, iz) = (ix as usize, iz as usize);
        for iy in (0..self.ny).rev() {
            let b = self.get(ix, iy, iz);
            if b.is_renderable() {
                return Some(b);
            }
        }
        None
    }

    /// Fill air blocks within a sphere that are below water level (water flows into craters).
    /// Only fills when solid ground is within WATER_FILL_BUFFER blocks below, so we don't create
    /// water-over-cave deadfall pits.
    pub fn fill_water_in_sphere_below(
        &mut self,
        center_x: f32,
        center_y: f32,
        center_z: f32,
        radius: f32,
        water_level_world: f32,
    ) -> bool {
        let r2 = radius * radius;
        let mut modified = false;
        for iz in 0..self.nz {
            for iy in 0..self.ny {
                for ix in 0..self.nx {
                    if self.get(ix, iy, iz) != BlockId::Air {
                        continue;
                    }
                    let wx = self.world_x(ix);
                    let wy = self.world_y(iy) + self.block_size * 0.5;
                    let wz = self.world_z(iz);
                    let dx = wx - center_x;
                    let dy = wy - center_y;
                    let dz = wz - center_z;
                    if dx * dx + dy * dy + dz * dz <= r2 {
                        let block_top_y = self.world_y(iy) + self.block_size;
                        if block_top_y <= water_level_world {
                            // Only fill if solid ground is within buffer below (shallow depression, not pit into cave).
                            let depth_limit = iy.saturating_sub(WATER_FILL_BUFFER);
                            let has_ground_below = (depth_limit..iy).any(|iy_below| self.get(ix, iy_below, iz).is_solid());
                            if has_ground_below {
                                self.set(ix, iy, iz, BlockId::Water);
                                modified = true;
                            }
                        }
                    }
                }
            }
        }
        modified
    }

    /// Remove blocks in sphere (for craters, deformation). Sets to Air.
    pub fn deform_sphere(&mut self, center_x: f32, center_y: f32, center_z: f32, radius: f32) -> bool {
        let r2 = radius * radius;
        let mut modified = false;
        for iz in 0..self.nz {
            for iy in 0..self.ny {
                for ix in 0..self.nx {
                    let wx = self.world_x(ix);
                    let wy = self.world_y(iy) + self.block_size * 0.5;
                    let wz = self.world_z(iz);
                    let dx = wx - center_x;
                    let dy = wy - center_y;
                    let dz = wz - center_z;
                    if dx * dx + dy * dy + dz * dz <= r2 {
                        self.set(ix, iy, iz, BlockId::Air);
                        modified = true;
                    }
                }
            }
        }
        modified
    }

    /// Set a column's surface to the given world Y (top of top block). Fills below with Stone/Dirt, clears above.
    pub fn set_column_height(&mut self, ix: usize, iz: usize, world_y_top: f32) -> bool {
        if ix >= self.nx || iz >= self.nz {
            return false;
        }
        // Top block has top face at world_y_top -> iy_top = (world_y_top / block_size).floor() - 1.
        let iy_top = (world_y_top / self.block_size).floor() as i32 - 1;
        let mut modified = false;
        for iy in 0..self.ny {
            let iy_i = iy as i32;
            if iy_top < 0 || iy_i > iy_top {
                if self.get(ix, iy, iz) != BlockId::Air {
                    self.set(ix, iy, iz, BlockId::Air);
                    modified = true;
                }
            } else {
                let block = if iy_i == iy_top {
                    BlockId::Dirt
                } else {
                    BlockId::Stone
                };
                self.set(ix, iy, iz, block);
                modified = true;
            }
        }
        modified
    }

    /// Fill sphere with a block (for mound / raise terrain). Only sets cells inside the sphere.
    pub fn fill_sphere(
        &mut self,
        center_x: f32,
        center_y: f32,
        center_z: f32,
        radius: f32,
        block: BlockId,
    ) -> bool {
        let r2 = radius * radius;
        let mut modified = false;
        for iz in 0..self.nz {
            for iy in 0..self.ny {
                for ix in 0..self.nx {
                    let wx = self.world_x(ix);
                    let wy = self.world_y(iy) + self.block_size * 0.5;
                    let wz = self.world_z(iz);
                    let dx = wx - center_x;
                    let dy = wy - center_y;
                    let dz = wz - center_z;
                    if dx * dx + dy * dy + dz * dz <= r2 {
                        self.set(ix, iy, iz, block);
                        modified = true;
                    }
                }
            }
        }
        modified
    }
}
