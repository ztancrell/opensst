//! Procedural texture generation for bugs, terrain, and effects
//! Creates PBR-ready textures at runtime

use glam::{Vec2, Vec3};
use noise::{NoiseFn, Perlin, Simplex};
use rand::prelude::*;

/// RGBA pixel
#[derive(Debug, Clone, Copy)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Pixel {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_rgb(r: f32, g: f32, b: f32) -> Self {
        Self {
            r: (r.clamp(0.0, 1.0) * 255.0) as u8,
            g: (g.clamp(0.0, 1.0) * 255.0) as u8,
            b: (b.clamp(0.0, 1.0) * 255.0) as u8,
            a: 255,
        }
    }

    pub fn from_rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            r: (r.clamp(0.0, 1.0) * 255.0) as u8,
            g: (g.clamp(0.0, 1.0) * 255.0) as u8,
            b: (b.clamp(0.0, 1.0) * 255.0) as u8,
            a: (a.clamp(0.0, 1.0) * 255.0) as u8,
        }
    }

    pub fn to_bytes(&self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

/// Generated texture data
#[derive(Debug, Clone)]
pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Pixel>,
}

impl TextureData {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![Pixel::new(0, 0, 0, 255); (width * height) as usize],
        }
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, pixel: Pixel) {
        if x < self.width && y < self.height {
            self.pixels[(y * self.width + x) as usize] = pixel;
        }
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> Pixel {
        if x < self.width && y < self.height {
            self.pixels[(y * self.width + x) as usize]
        } else {
            Pixel::new(0, 0, 0, 255)
        }
    }

    pub fn sample(&self, u: f32, v: f32) -> Pixel {
        let x = ((u.fract() + 1.0).fract() * self.width as f32) as u32 % self.width;
        let y = ((v.fract() + 1.0).fract() * self.height as f32) as u32 % self.height;
        self.get_pixel(x, y)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.pixels.len() * 4);
        for pixel in &self.pixels {
            bytes.extend_from_slice(&pixel.to_bytes());
        }
        bytes
    }
}

/// PBR texture set (albedo, normal, roughness-metallic-AO)
#[derive(Debug, Clone)]
pub struct PBRTextureSet {
    pub albedo: TextureData,
    pub normal: TextureData,
    pub roughness_metallic_ao: TextureData,
}

/// Procedural texture generator
pub struct TextureGenerator {
    perlin: Perlin,
    _simplex: Simplex,
    _rng: StdRng,
}

impl TextureGenerator {
    pub fn new(seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        Self {
            perlin: Perlin::new(rng.gen()),
            _simplex: Simplex::new(rng.gen()),
            _rng: rng,
        }
    }

    /// Generate bug carapace texture
    pub fn generate_carapace(&mut self, width: u32, height: u32, config: &CarapaceConfig) -> PBRTextureSet {
        let mut albedo = TextureData::new(width, height);
        let mut normal = TextureData::new(width, height);
        let mut rma = TextureData::new(width, height);

        for y in 0..height {
            for x in 0..width {
                let u = x as f64 / width as f64;
                let v = y as f64 / height as f64;

                // Voronoi for carapace plates
                let (cell_dist, edge_dist) = self.voronoi(u * config.plate_scale, v * config.plate_scale);

                // Plate pattern
                let plate_edge = self.smooth_step(0.02, 0.08, edge_dist - cell_dist);

                // Base color with variation per plate
                let plate_id = self.hash2d(
                    (u * config.plate_scale).floor(),
                    (v * config.plate_scale).floor(),
                );
                let color_var = 0.85 + plate_id * 0.3;

                let mut color = Vec3::new(
                    config.base_color.x * color_var as f32,
                    config.base_color.y * color_var as f32,
                    config.base_color.z * color_var as f32,
                );

                // Darken edges between plates
                color *= plate_edge as f32 * 0.5 + 0.5;

                // Add micro detail
                let detail = self.fbm(u * 50.0, v * 50.0, 4);
                color *= (0.9 + detail * 0.2) as f32;

                // Chitin iridescence
                let iridescence = (u * 10.0 + v * 5.0 + plate_id * 3.0).sin().abs() as f32 * config.iridescence;
                color.x += iridescence * 0.1;
                color.z += iridescence * 0.05;

                albedo.set_pixel(x, y, Pixel::from_rgb(color.x, color.y, color.z));

                // Normal map
                let normal_strength = 0.5;
                let dx = self.sample_height(u + 0.001, v, config) - self.sample_height(u - 0.001, v, config);
                let dy = self.sample_height(u, v + 0.001, config) - self.sample_height(u, v - 0.001, config);

                // Edge grooves
                let groove_normal = if plate_edge < 0.3 {
                    Vec3::new(0.0, (1.0 - plate_edge as f32 / 0.3) * 0.3, 0.0)
                } else {
                    Vec3::ZERO
                };

                let normal_vec = Vec3::new(
                    (-dx as f32 * normal_strength + groove_normal.x) * 0.5 + 0.5,
                    (-dy as f32 * normal_strength + groove_normal.y) * 0.5 + 0.5,
                    1.0,
                ).normalize() * 0.5 + 0.5;

                normal.set_pixel(x, y, Pixel::from_rgb(normal_vec.x, normal_vec.y, normal_vec.z));

                // Roughness-Metallic-AO
                let roughness = (0.3 + (1.0 - plate_edge) as f32 * 0.2 + detail as f32 * 0.1).clamp(0.1, 0.9);
                let metallic = config.metallic * plate_edge as f32;
                let ao = (0.5 + plate_edge as f32 * 0.5).clamp(0.3, 1.0);

                rma.set_pixel(x, y, Pixel::from_rgb(ao, roughness, metallic));
            }
        }

        PBRTextureSet {
            albedo,
            normal,
            roughness_metallic_ao: rma,
        }
    }

    /// Generate terrain texture
    pub fn generate_terrain(&mut self, width: u32, height: u32, config: &TerrainTextureConfig) -> PBRTextureSet {
        let mut albedo = TextureData::new(width, height);
        let mut normal = TextureData::new(width, height);
        let mut rma = TextureData::new(width, height);

        for y in 0..height {
            for x in 0..width {
                let u = x as f64 / width as f64;
                let v = y as f64 / height as f64;

                // Multi-scale noise
                let large_scale = self.fbm(u * 4.0, v * 4.0, 3);
                let medium_scale = self.fbm(u * 16.0, v * 16.0, 4);
                let small_scale = self.fbm(u * 64.0, v * 64.0, 3);

                // Blend between rock and sand based on noise
                let rock_blend = self.smooth_step(0.4, 0.6, large_scale);

                let sand_color = Vec3::new(
                    config.sand_color.x,
                    config.sand_color.y,
                    config.sand_color.z,
                );
                let rock_color = Vec3::new(
                    config.rock_color.x,
                    config.rock_color.y,
                    config.rock_color.z,
                );

                let mut color = sand_color.lerp(rock_color, rock_blend as f32);

                // Add detail variation
                color *= (0.8 + medium_scale * 0.4) as f32;

                // Cracks in rock areas
                if rock_blend > 0.5 {
                    let crack = self.crack_pattern(u * 20.0, v * 20.0);
                    color *= 1.0 - crack as f32 * 0.3 * rock_blend as f32;
                }

                albedo.set_pixel(x, y, Pixel::from_rgb(color.x, color.y, color.z));

                // Normal map
                let dx = self.terrain_height(u + 0.001, v, config) - self.terrain_height(u - 0.001, v, config);
                let dy = self.terrain_height(u, v + 0.001, config) - self.terrain_height(u, v - 0.001, config);

                let normal_vec = Vec3::new(
                    -dx as f32 * 2.0 * 0.5 + 0.5,
                    -dy as f32 * 2.0 * 0.5 + 0.5,
                    1.0,
                ).normalize() * 0.5 + 0.5;

                normal.set_pixel(x, y, Pixel::from_rgb(normal_vec.x, normal_vec.y, normal_vec.z));

                // RMA
                let roughness = (0.6 + rock_blend as f32 * 0.2 + small_scale as f32 * 0.1).clamp(0.3, 1.0);
                let ao = (0.7 + medium_scale as f32 * 0.3).clamp(0.5, 1.0);

                rma.set_pixel(x, y, Pixel::from_rgb(ao, roughness, 0.0));
            }
        }

        PBRTextureSet {
            albedo,
            normal,
            roughness_metallic_ao: rma,
        }
    }

    /// Generate ichor/gore texture
    pub fn generate_gore(&mut self, width: u32, height: u32) -> TextureData {
        let mut texture = TextureData::new(width, height);

        for y in 0..height {
            for x in 0..width {
                let u = x as f64 / width as f64;
                let v = y as f64 / height as f64;

                // Organic blobs
                let blobs = self.fbm(u * 8.0, v * 8.0, 4);
                let drips = self.fbm(u * 4.0, v * 20.0 + blobs * 2.0, 3);

                let intensity = (blobs * 0.5 + drips * 0.5).clamp(0.0, 1.0);

                // Bug blood colors (yellow-green)
                let color = if intensity > 0.5 {
                    let t = (intensity - 0.5) * 2.0;
                    Vec3::new(
                        (0.6 + t * 0.3) as f32, // More yellow when intense
                        (0.5 + t * 0.2) as f32,
                        (0.1 - t * 0.05) as f32,
                    )
                } else {
                    Vec3::new(0.4, 0.3, 0.1)
                };

                let alpha = self.smooth_step(0.2, 0.4, intensity) as f32;

                texture.set_pixel(x, y, Pixel::from_rgba(color.x, color.y, color.z, alpha));
            }
        }

        texture
    }

    /// Generate explosion/muzzle flash texture
    pub fn generate_explosion(&mut self, width: u32, height: u32) -> TextureData {
        let mut texture = TextureData::new(width, height);

        let cx = width as f32 / 2.0;
        let cy = height as f32 / 2.0;
        let max_dist = (cx * cx + cy * cy).sqrt();

        for y in 0..height {
            for x in 0..width {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt() / max_dist;
                let angle = dy.atan2(dx);

                // Radial gradient
                let radial = 1.0 - dist;

                // Add spiky rays
                let rays = ((angle * 12.0).sin() * 0.5 + 0.5).powf(3.0);
                let intensity = (radial * (0.5 + rays * 0.5)).powf(2.0);

                // Add noise for organic feel
                let u = x as f64 / width as f64;
                let v = y as f64 / height as f64;
                let noise = self.fbm(u * 10.0, v * 10.0, 3) as f32;
                let intensity = (intensity * (0.7 + noise * 0.6)).clamp(0.0, 1.0);

                // Fire colors
                let color = if intensity > 0.8 {
                    Vec3::new(1.0, 1.0, 0.9) // White core
                } else if intensity > 0.5 {
                    Vec3::new(1.0, 0.8, 0.3) // Yellow
                } else if intensity > 0.2 {
                    Vec3::new(1.0, 0.4, 0.1) // Orange
                } else {
                    Vec3::new(0.8, 0.2, 0.05) // Red edge
                };

                texture.set_pixel(x, y, Pixel::from_rgba(color.x, color.y, color.z, intensity));
            }
        }

        texture
    }

    // Noise helper functions

    fn fbm(&self, x: f64, y: f64, octaves: u32) -> f64 {
        let mut value = 0.0;
        let mut amplitude = 0.5;
        let mut frequency = 1.0;

        for _ in 0..octaves {
            value += amplitude * (self.perlin.get([x * frequency, y * frequency]) * 0.5 + 0.5);
            amplitude *= 0.5;
            frequency *= 2.0;
        }

        value
    }

    fn voronoi(&self, x: f64, y: f64) -> (f64, f64) {
        let n = (x.floor(), y.floor());
        let f = (x.fract(), y.fract());

        let mut min_dist = 8.0;
        let mut second_dist = 8.0;

        for j in -1..=1 {
            for i in -1..=1 {
                let g = (i as f64, j as f64);
                let o = (
                    self.hash2d(n.0 + g.0, n.1 + g.1),
                    self.hash2d(n.0 + g.0 + 17.0, n.1 + g.1 + 31.0),
                );
                let r = (g.0 + o.0 - f.0, g.1 + o.1 - f.1);
                let d = r.0 * r.0 + r.1 * r.1;

                if d < min_dist {
                    second_dist = min_dist;
                    min_dist = d;
                } else if d < second_dist {
                    second_dist = d;
                }
            }
        }

        (min_dist.sqrt(), second_dist.sqrt())
    }

    fn hash2d(&self, x: f64, y: f64) -> f64 {
        let p = Vec2::new(x as f32, y as f32);
        let p3 = (Vec3::new(p.x, p.y, p.x) * 0.1031).fract();
        let p3 = p3 + Vec3::splat(p3.dot(Vec3::new(p3.y + 33.33, p3.z + 33.33, p3.x + 33.33)));
        ((p3.x + p3.y) * p3.z).fract() as f64
    }

    fn smooth_step(&self, edge0: f64, edge1: f64, x: f64) -> f64 {
        let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    fn sample_height(&self, u: f64, v: f64, _config: &CarapaceConfig) -> f64 {
        self.fbm(u * 20.0, v * 20.0, 3)
    }

    fn terrain_height(&self, u: f64, v: f64, _config: &TerrainTextureConfig) -> f64 {
        self.fbm(u * 10.0, v * 10.0, 4)
    }

    fn crack_pattern(&self, u: f64, v: f64) -> f64 {
        let (min_dist, second_dist) = self.voronoi(u, v);
        let edge = second_dist - min_dist;
        self.smooth_step(0.02, 0.06, edge)
    }
}

/// Configuration for carapace texture generation
#[derive(Debug, Clone)]
pub struct CarapaceConfig {
    pub base_color: Vec3,
    pub plate_scale: f64,
    pub iridescence: f32,
    pub metallic: f32,
}

impl Default for CarapaceConfig {
    fn default() -> Self {
        Self {
            base_color: Vec3::new(0.35, 0.28, 0.22), // Dark brown
            plate_scale: 8.0,
            iridescence: 0.3,
            metallic: 0.2,
        }
    }
}

impl CarapaceConfig {
    pub fn warrior() -> Self {
        Self {
            base_color: Vec3::new(0.3, 0.25, 0.2),
            plate_scale: 6.0,
            iridescence: 0.2,
            metallic: 0.15,
        }
    }

    pub fn tanker() -> Self {
        Self {
            base_color: Vec3::new(0.25, 0.22, 0.2),
            plate_scale: 4.0,
            iridescence: 0.1,
            metallic: 0.3,
        }
    }

    pub fn plasma() -> Self {
        Self {
            base_color: Vec3::new(0.2, 0.15, 0.3),
            plate_scale: 8.0,
            iridescence: 0.6,
            metallic: 0.4,
        }
    }
}

/// Configuration for terrain texture generation
#[derive(Debug, Clone)]
pub struct TerrainTextureConfig {
    pub sand_color: Vec3,
    pub rock_color: Vec3,
}

impl Default for TerrainTextureConfig {
    fn default() -> Self {
        Self {
            sand_color: Vec3::new(0.76, 0.60, 0.42),
            rock_color: Vec3::new(0.45, 0.40, 0.35),
        }
    }
}
