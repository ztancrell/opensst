//! Terrain generation using noise functions.
//! Includes procedural water: lakes, streams, rivers, and ocean.
//!
//! **Seed-based determinism:** All noise is derived from `config.seed` (planet seed) so that
//! the same seed always produces the same terrain at every (world_x, world_z), regardless of
//! chunk load order — Minecraft-style replayability.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use noise::{NoiseFn, Perlin, Simplex};

use crate::biome::PlanetBiomes;

/// Derive a deterministic u32 noise seed from a world seed and an offset.
/// Same (seed, offset) always gives the same result so terrain is reproducible.
#[inline]
fn deterministic_noise_seed(seed: u64, offset: u64) -> u32 {
    ((seed.wrapping_add(offset))
        .wrapping_mul(0x9e3779b97f4a7c15_u64)
        .wrapping_add(offset.wrapping_mul(0x6c078965_u64))
        >> 32) as u32
}

/// Vertex for terrain mesh (includes biome color).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct TerrainVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    /// Per-vertex biome color (blended at biome boundaries).
    pub color: [f32; 4],
}

/// Configuration for terrain generation.
#[derive(Debug, Clone)]
pub struct TerrainConfig {
    /// Size of terrain in world units.
    pub size: f32,
    /// Number of vertices per side.
    pub resolution: u32,
    /// Maximum height of terrain.
    pub height_scale: f32,
    /// Noise frequency (lower = smoother).
    pub frequency: f64,
    /// Number of octaves for fractal noise.
    pub octaves: u32,
    /// Lacunarity (frequency multiplier per octave).
    pub lacunarity: f64,
    /// Persistence (amplitude multiplier per octave).
    pub persistence: f64,
    /// Seed for random generation.
    pub seed: u64,
    /// World-space X offset (for chunked terrain; chunk center = offset).
    pub offset_x: f32,
    /// World-space Z offset (for chunked terrain; chunk center = offset).
    pub offset_z: f32,
    /// Water level (world Y). If Some, procedural water (lakes, streams, ocean) is generated.
    /// Vertices below this level in water basins become water surface.
    pub water_level: Option<f32>,
    /// Water coverage 0-1. Higher = more lakes, streams, ocean.
    pub water_coverage: f32,
    /// Voxel size for Castle Miner Z–style blocky terrain. Heights are quantized to this grid.
    /// e.g. 1.0 = 1m blocks. None = smooth terrain (no quantization).
    pub voxel_size: Option<f32>,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            size: 256.0,
            resolution: 128,
            height_scale: 30.0,
            frequency: 0.02,
            octaves: 4,
            lacunarity: 2.0,
            persistence: 0.5,
            seed: 0,
            offset_x: 0.0,
            offset_z: 0.0,
            water_level: Some(0.25), // Default: water at ~25% of normalized height
            water_coverage: 0.45,
            voxel_size: Some(1.0),   // Castle Miner Z style: 1m blocky terrain
        }
    }
}

/// Quantize height to voxel grid (floor to nearest voxel boundary).
#[inline]
pub fn quantize_height(y: f32, voxel_size: f32) -> f32 {
    (y / voxel_size).floor() * voxel_size
}

/// Water vertex for the water surface mesh (flat quads at water level).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct WaterVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    /// Water type: 0=ocean, 1=lake, 2=stream. Stored in color.a for shader.
    pub color: [f32; 4],
}

/// Generated terrain data.
#[derive(Debug)]
pub struct TerrainData {
    pub vertices: Vec<TerrainVertex>,
    pub indices: Vec<u32>,
    pub heightmap: Vec<f32>,
    pub config: TerrainConfig,
    /// Water surface mesh (empty if water_level is None).
    pub water_vertices: Vec<WaterVertex>,
    pub water_indices: Vec<u32>,
}

impl TerrainData {
    /// Generate terrain from configuration.
    /// If `planet_biomes` is provided, per-vertex biome colors and height variation are applied.
    ///
    /// Uses a padded grid (1 extra vertex on each edge) to compute normals that are
    /// consistent across chunk boundaries, eliminating visible seams between chunks.
    ///
    /// Noise seeds are derived only from `config.seed` so the same planet seed yields
    /// the same height at every world position, independent of chunk load order.
    pub fn generate(config: TerrainConfig, planet_biomes: Option<&PlanetBiomes>) -> Self {
        let perlin = Perlin::new(deterministic_noise_seed(config.seed, 0));
        let simplex = Simplex::new(deterministic_noise_seed(config.seed, 1));

        let res = config.resolution as usize;
        let step = config.size / (config.resolution - 1) as f32;

        // Generate a PADDED grid: (res+2) x (res+2) vertices extending 1 step beyond
        // each chunk edge. This ensures edge normals incorporate neighboring geometry.
        let padded_res = res + 2;
        let padded_count = padded_res * padded_res;
        let mut padded_vertices = Vec::with_capacity(padded_count);
        let mut raw_heights: Vec<f32> = Vec::with_capacity(padded_count);
        let water_perlin = Perlin::new(deterministic_noise_seed(config.seed, 2));
        let water_simplex = Simplex::new(deterministic_noise_seed(config.seed, 3));
        let mut water_level_world = config.water_level.map(|wl| wl * config.height_scale);
        if let (Some(wl), Some(vs)) = (water_level_world, config.voxel_size) {
            water_level_world = Some(quantize_height(wl, vs));
        }

        for pz in 0..padded_res {
            for px in 0..padded_res {
                // Map padded indices to world coords: px=1 corresponds to original x=0
                let orig_x = px as i32 - 1;
                let orig_z = pz as i32 - 1;
                let world_x = orig_x as f32 * step - config.size / 2.0 + config.offset_x;
                let world_z = orig_z as f32 * step - config.size / 2.0 + config.offset_z;

                let height = Self::fractal_noise(
                    &perlin,
                    &simplex,
                    world_x as f64,
                    world_z as f64,
                    &config,
                );

                let (vertex_color, biome_height_mult) = if let Some(pb) = planet_biomes {
                    let hs = pb.height_scale_at(world_x as f64, world_z as f64);
                    let (_, color) = pb.sample_at(world_x as f64, world_z as f64);
                    (color, hs)
                } else {
                    ([1.0, 1.0, 1.0, 1.0], 1.0)
                };

                let mut world_y = height as f32 * config.height_scale * biome_height_mult;
                if let Some(vs) = config.voxel_size {
                    world_y = quantize_height(world_y, vs);
                }
                raw_heights.push(world_y);

                padded_vertices.push(TerrainVertex {
                    position: [world_x, world_y, world_z],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0], // Will be set on inner vertices
                    color: vertex_color,
                });
            }
        }

        // Water pass: flatten water vertices to water level (visual only; heightmap stays raw)
        if let Some(wl) = water_level_world {
            let coverage = config.water_coverage.clamp(0.0, 1.0);
            for pz in 0..padded_res {
                for px in 0..padded_res {
                    let idx = pz * padded_res + px;
                    if raw_heights[idx] >= wl {
                        continue;
                    }
                    let world_x = padded_vertices[idx].position[0];
                    let world_z = padded_vertices[idx].position[2];
                    let (is_water, _) = Self::water_mask(
                        world_x as f64,
                        world_z as f64,
                        &water_perlin,
                        &water_simplex,
                        config.seed,
                        coverage,
                    );
                    if is_water {
                        padded_vertices[idx].position[1] = wl;
                        padded_vertices[idx].normal = [0.0, 1.0, 0.0];
                    }
                }
            }
        }

        // Calculate normals on the PADDED grid — edge normals now include
        // contributions from geometry that belongs to neighboring chunks.
        Self::calculate_normals(&mut padded_vertices, padded_res);

        // Extract the inner res x res vertices with correct normals
        let vertex_count = res * res;
        let mut vertices = Vec::with_capacity(vertex_count);
        let mut heightmap = Vec::with_capacity(vertex_count);

        for z in 0..res {
            for x in 0..res {
                let padded_idx = (z + 1) * padded_res + (x + 1);
                let mut v = padded_vertices[padded_idx];

                // Set proper UV for this chunk
                v.uv = [
                    x as f32 / (config.resolution - 1) as f32,
                    z as f32 / (config.resolution - 1) as f32,
                ];

                heightmap.push(raw_heights[padded_idx]);
                vertices.push(v);
            }
        }

        // Generate indices for the inner grid
        let mut indices = Vec::with_capacity((res - 1) * (res - 1) * 6);
        for z in 0..(res - 1) {
            for x in 0..(res - 1) {
                let top_left = (z * res + x) as u32;
                let top_right = top_left + 1;
                let bottom_left = ((z + 1) * res + x) as u32;
                let bottom_right = bottom_left + 1;

                indices.push(top_left);
                indices.push(bottom_left);
                indices.push(top_right);

                indices.push(top_right);
                indices.push(bottom_left);
                indices.push(bottom_right);
            }
        }

        let (water_vertices, water_indices) = if let Some(wl) = water_level_world {
            Self::generate_water_mesh(
                &vertices,
                &heightmap,
                &config,
                wl,
                &water_perlin,
                &water_simplex,
            )
        } else {
            (Vec::new(), Vec::new())
        };

        Self {
            vertices,
            indices,
            heightmap,
            config,
            water_vertices,
            water_indices,
        }
    }

    fn water_mask(
        x: f64,
        z: f64,
        perlin: &Perlin,
        simplex: &Simplex,
        seed: u64,
        coverage: f32,
    ) -> (bool, f32) {
        let _ = seed;
        let lake_freq = 0.008;
        let lake_noise = (perlin.get([x * lake_freq, z * lake_freq]) + 1.0) * 0.5;
        let is_lake = lake_noise < (0.5 - coverage as f64 * 0.25);

        let stream_freq = 0.025;
        let stream_raw = simplex.get([x * stream_freq + 100.0, z * stream_freq + 200.0]);
        let stream_valley = 1.0 - stream_raw.abs();
        let is_stream = stream_valley > (0.6 + coverage as f64 * 0.2);

        let ocean_freq = 0.002;
        let ocean_noise = (perlin.get([x * ocean_freq + 500.0, z * ocean_freq + 600.0]) + 1.0) * 0.5;
        let is_ocean = ocean_noise < (0.4 - coverage as f64 * 0.15);

        let is_water = is_lake || is_stream || is_ocean;
        let water_type = if is_ocean {
            0.0
        } else if is_lake {
            1.0
        } else {
            2.0
        };
        (is_water, water_type)
    }

    fn generate_water_mesh(
        _vertices: &[TerrainVertex],
        heightmap: &[f32],
        config: &TerrainConfig,
        water_level: f32,
        water_perlin: &Perlin,
        water_simplex: &Simplex,
    ) -> (Vec<WaterVertex>, Vec<u32>) {
        let res = config.resolution as usize;
        let step = config.size / (config.resolution - 1) as f32;
        let half_size = config.size / 2.0;
        let coverage = config.water_coverage.clamp(0.0, 1.0);

        let mut water_vertices = Vec::new();
        let mut water_indices = Vec::new();

        let ocean_color = [0.05, 0.15, 0.35, 0.0];
        let lake_color = [0.08, 0.22, 0.38, 1.0];
        let stream_color = [0.12, 0.28, 0.45, 2.0];

        for z in 0..(res - 1) {
            for x in 0..(res - 1) {
                let i00 = z * res + x;
                let i10 = z * res + x + 1;
                let i01 = (z + 1) * res + x;
                let i11 = (z + 1) * res + x + 1;

                let avg_height =
                    (heightmap[i00] + heightmap[i10] + heightmap[i01] + heightmap[i11]) * 0.25;
                if avg_height >= water_level {
                    continue;
                }

                // Generate water wherever terrain is below water level (conforms to terrain depressions)
                let world_x = x as f32 * step - half_size + config.offset_x;
                let world_z = z as f32 * step - half_size + config.offset_z;
                let cx = world_x + step * 0.5;
                let cz = world_z + step * 0.5;

                let (_is_water, water_type) =
                    Self::water_mask(cx as f64, cz as f64, water_perlin, water_simplex, config.seed, coverage);

                let color = if water_type < 0.5 {
                    ocean_color
                } else if water_type < 1.5 {
                    lake_color
                } else {
                    stream_color
                };

                let base = water_vertices.len() as u32;
                let uv_scale = 0.1;

                // Slight Y offset to avoid z-fighting with flattened terrain
                let water_y = water_level + 0.03;
                water_vertices.push(WaterVertex {
                    position: [world_x, water_y, world_z],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0],
                    color,
                });
                water_vertices.push(WaterVertex {
                    position: [world_x + step, water_y, world_z],
                    normal: [0.0, 1.0, 0.0],
                    uv: [uv_scale, 0.0],
                    color,
                });
                water_vertices.push(WaterVertex {
                    position: [world_x + step, water_y, world_z + step],
                    normal: [0.0, 1.0, 0.0],
                    uv: [uv_scale, uv_scale],
                    color,
                });
                water_vertices.push(WaterVertex {
                    position: [world_x, water_y, world_z + step],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, uv_scale],
                    color,
                });

                water_indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }

        (water_vertices, water_indices)
    }

    /// Regenerate water mesh from the current heightmap.
    /// Call after terrain deformation (crater, mound) so water flows into new depressions.
    pub fn regenerate_water_mesh(&mut self) {
        let water_level_world = match self.config.water_level {
            Some(wl) => wl * self.config.height_scale,
            None => return,
        };
        let water_perlin = Perlin::new(deterministic_noise_seed(self.config.seed, 2));
        let water_simplex = Simplex::new(deterministic_noise_seed(self.config.seed, 3));
        let (water_vertices, water_indices) = Self::generate_water_mesh(
            &self.vertices,
            &self.heightmap,
            &self.config,
            water_level_world,
            &water_perlin,
            &water_simplex,
        );
        self.water_vertices = water_vertices;
        self.water_indices = water_indices;
    }

    /// Sample height at a world position.
    pub fn sample_height(&self, x: f32, z: f32) -> f32 {
        let res = self.config.resolution as usize;
        let half_size = self.config.size / 2.0;
        let step = self.config.size / (self.config.resolution - 1) as f32;

        // Convert to grid coordinates (account for chunk offset)
        let gx = (x - self.config.offset_x + half_size) / step;
        let gz = (z - self.config.offset_z + half_size) / step;

        let x0 = (gx.floor() as usize).clamp(0, res - 2);
        let z0 = (gz.floor() as usize).clamp(0, res - 2);

        let fx = (gx - x0 as f32).clamp(0.0, 1.0);
        let fz = (gz - z0 as f32).clamp(0.0, 1.0);

        // Heights at four corners of the grid cell
        let h00 = self.heightmap[z0 * res + x0];       // top-left
        let h10 = self.heightmap[z0 * res + x0 + 1];   // top-right
        let h01 = self.heightmap[(z0 + 1) * res + x0];  // bottom-left
        let h11 = self.heightmap[(z0 + 1) * res + x0 + 1]; // bottom-right

        // Triangle-based interpolation matching the actual mesh triangulation.
        // The mesh diagonal goes from bottom-left (x0,z1) to top-right (x1,z0).
        // Triangle 1: top-left, bottom-left, top-right  (when fx + fz <= 1)
        // Triangle 2: top-right, bottom-left, bottom-right (when fx + fz > 1)
        if fx + fz <= 1.0 {
            h00 + fx * (h10 - h00) + fz * (h01 - h00)
        } else {
            h11 + (1.0 - fx) * (h01 - h11) + (1.0 - fz) * (h10 - h11)
        }
    }

    fn fractal_noise(
        perlin: &Perlin,
        simplex: &Simplex,
        x: f64,
        z: f64,
        config: &TerrainConfig,
    ) -> f64 {
        let mut value = 0.0;
        let mut amplitude = 1.0;
        let mut frequency = config.frequency;
        let mut max_value = 0.0;

        for _ in 0..config.octaves {
            // Mix Perlin and Simplex for variety
            let perlin_sample = perlin.get([x * frequency, z * frequency]);
            let simplex_sample = simplex.get([x * frequency + 1000.0, z * frequency + 1000.0]);
            
            value += (perlin_sample * 0.7 + simplex_sample * 0.3) * amplitude;
            max_value += amplitude;

            amplitude *= config.persistence;
            frequency *= config.lacunarity;
        }

        // Normalize to 0-1 range
        (value / max_value + 1.0) * 0.5
    }

    /// Check if a world position is within this chunk's bounds.
    pub fn contains(&self, x: f32, z: f32) -> bool {
        let half = self.config.size / 2.0;
        let min_x = self.config.offset_x - half;
        let max_x = self.config.offset_x + half;
        let min_z = self.config.offset_z - half;
        let max_z = self.config.offset_z + half;
        x >= min_x && x <= max_x && z >= min_z && z <= max_z
    }

    /// Deform terrain with a realistic impact crater: flat floor, steep walls, raised rim (ejecta).
    /// Returns `true` if any vertices were modified (mesh/collider need rebuild).
    /// Profile: flat floor from center to ~35% radius, steep wall to rim, then raised rim to ~115% radius.
    pub fn deform_crater(&mut self, center_x: f32, center_z: f32, radius: f32, depth: f32) -> bool {
        let res = self.config.resolution as usize;
        let step = self.config.size / (self.config.resolution - 1) as f32;
        let half_size = self.config.size / 2.0;
        let r2 = radius * radius;
        let rim_radius = radius * 1.15;
        let rim_radius2 = rim_radius * rim_radius;
        let floor_frac = 0.35; // flat floor out to 35% of radius
        let rim_height = depth * 0.25; // ejecta berm height
        let voxel_size = self.config.voxel_size;
        let mut modified = false;

        for z in 0..res {
            for x in 0..res {
                let idx = z * res + x;
                let vx = x as f32 * step - half_size + self.config.offset_x;
                let vz = z as f32 * step - half_size + self.config.offset_z;
                let dx = vx - center_x;
                let dz = vz - center_z;
                let dist2 = dx * dx + dz * dz;
                let norm_r = (dist2 / r2).sqrt();

                if dist2 < r2 {
                    // Inside crater: flat floor (center) then steep wall to rim
                    let lower = if norm_r <= floor_frac {
                        depth // flat floor
                    } else {
                        // steep wall: smoothstep from depth at floor_frac to 0 at 1.0
                        let t = (norm_r - floor_frac) / (1.0 - floor_frac);
                        let t = t.clamp(0.0, 1.0);
                        let s = t * t * (3.0 - 2.0 * t);
                        depth * (1.0 - s)
                    };
                    let new_height = self.heightmap[idx] - lower;
                    let final_height = match voxel_size {
                        Some(vs) => quantize_height(new_height, vs),
                        None => new_height,
                    };
                    self.heightmap[idx] = final_height;
                    self.vertices[idx].position[1] = final_height;
                    modified = true;
                } else if dist2 < rim_radius2 {
                    // Raised rim (ejecta): ramp up from crater edge to peak at mid-rim, then down
                    let t = (norm_r - 1.0) / 0.15; // 0 at edge, 1 at rim_radius
                    let t = t.clamp(0.0, 1.0);
                    let raise = rim_height * 4.0 * t * (1.0 - t); // peak at t=0.5
                    let new_height = self.heightmap[idx] + raise;
                    let final_height = match voxel_size {
                        Some(vs) => quantize_height(new_height, vs),
                        None => new_height,
                    };
                    self.heightmap[idx] = final_height;
                    self.vertices[idx].position[1] = final_height;
                    modified = true;
                }
            }
        }

        if modified {
            Self::calculate_normals(&mut self.vertices, res);
        }
        modified
    }

    /// Deform terrain by raising a mound/berm at `(center_x, center_z)` in world space.
    /// Used for entrenchment shovel: excavated earth forms a defensive wall.
    /// With voxel_size: blocky Castle Miner Z–style mound (quantized to voxel grid).
    pub fn deform_mound(&mut self, center_x: f32, center_z: f32, radius: f32, height: f32) -> bool {
        let res = self.config.resolution as usize;
        let step = self.config.size / (self.config.resolution - 1) as f32;
        let half_size = self.config.size / 2.0;
        let r2 = radius * radius;
        let voxel_size = self.config.voxel_size;
        let mut modified = false;

        for z in 0..res {
            for x in 0..res {
                let idx = z * res + x;
                let vx = x as f32 * step - half_size + self.config.offset_x;
                let vz = z as f32 * step - half_size + self.config.offset_z;
                let dx = vx - center_x;
                let dz = vz - center_z;
                let dist2 = dx * dx + dz * dz;
                if dist2 < r2 {
                    let t = 1.0 - (dist2 / r2).sqrt();
                    let falloff = t * t * (3.0 - 2.0 * t);
                    let raise = height * falloff;
                    let new_height = self.heightmap[idx] + raise;
                    let final_height = match voxel_size {
                        Some(vs) => quantize_height(new_height, vs),
                        None => new_height,
                    };
                    self.heightmap[idx] = final_height;
                    self.vertices[idx].position[1] = final_height;
                    modified = true;
                }
            }
        }

        if modified {
            Self::calculate_normals(&mut self.vertices, res);
        }
        modified
    }

    /// Recalculate vertex normals from positions (e.g. after collapse).
    pub fn recalculate_normals(&mut self) {
        let res = self.config.resolution as usize;
        Self::calculate_normals(&mut self.vertices, res);
    }

    fn calculate_normals(vertices: &mut [TerrainVertex], resolution: usize) {
        // Calculate face normals and accumulate
        let mut normals: Vec<Vec3> = vec![Vec3::ZERO; vertices.len()];

        for z in 0..(resolution - 1) {
            for x in 0..(resolution - 1) {
                let i0 = z * resolution + x;
                let i1 = i0 + 1;
                let i2 = (z + 1) * resolution + x;
                let i3 = i2 + 1;

                let v0: Vec3 = vertices[i0].position.into();
                let v1: Vec3 = vertices[i1].position.into();
                let v2: Vec3 = vertices[i2].position.into();
                let v3: Vec3 = vertices[i3].position.into();

                // First triangle
                let n1 = (v2 - v0).cross(v1 - v0).normalize();
                normals[i0] += n1;
                normals[i2] += n1;
                normals[i1] += n1;

                // Second triangle
                let n2 = (v3 - v1).cross(v2 - v1).normalize();
                normals[i1] += n2;
                normals[i2] += n2;
                normals[i3] += n2;
            }
        }

        // Normalize accumulated normals
        for (i, vertex) in vertices.iter_mut().enumerate() {
            let n = normals[i].normalize();
            vertex.normal = [n.x, n.y, n.z];
        }
    }
}

/// Terrain chunk for streaming large worlds.
#[derive(Debug)]
pub struct TerrainChunk {
    pub position: (i32, i32), // Chunk coordinates
    pub data: TerrainData,
    pub lod: u32,
}

impl TerrainChunk {
    /// Generate a terrain chunk at the given coordinates.
    pub fn generate(chunk_x: i32, chunk_z: i32, chunk_size: f32, resolution: u32, seed: u64) -> Self {
        let config = TerrainConfig {
            size: chunk_size,
            resolution,
            offset_x: chunk_x as f32 * chunk_size,
            offset_z: chunk_z as f32 * chunk_size,
            seed,
            ..Default::default()
        };

        Self {
            position: (chunk_x, chunk_z),
            data: TerrainData::generate(config, None),
            lod: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same planet seed and chunk config must produce identical heightmaps (replayability).
    #[test]
    fn terrain_deterministic_same_seed() {
        let seed = 98765_u64;
        let config = TerrainConfig {
            size: 64.0,
            resolution: 24,
            height_scale: 15.0,
            frequency: 0.02,
            offset_x: 0.0,
            offset_z: 0.0,
            seed,
            ..Default::default()
        };
        let a = TerrainData::generate(config.clone(), None);
        let b = TerrainData::generate(config, None);
        assert_eq!(a.heightmap.len(), b.heightmap.len());
        for (i, (&ha, &hb)) in a.heightmap.iter().zip(b.heightmap.iter()).enumerate() {
            assert_eq!(ha, hb, "heightmap[{}] should match for same seed", i);
        }
    }

    /// Different seeds must produce different terrain.
    #[test]
    fn terrain_different_seed_different_heights() {
        let config_a = TerrainConfig {
            size: 64.0,
            resolution: 24,
            seed: 11111,
            offset_x: 0.0,
            offset_z: 0.0,
            ..Default::default()
        };
        let config_b = TerrainConfig {
            seed: 22222,
            ..config_a.clone()
        };
        let a = TerrainData::generate(config_a, None);
        let b = TerrainData::generate(config_b, None);
        assert_ne!(a.heightmap, b.heightmap);
    }
}
