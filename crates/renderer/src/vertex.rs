//! Vertex types and layouts for rendering.

use bytemuck::{Pod, Zeroable};

/// Standard vertex with position, normal, UV coordinates, and color.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coords: [f32; 2],
    pub color: [f32; 4],
}

impl Vertex {
    pub fn new(position: [f32; 3], normal: [f32; 3], tex_coords: [f32; 2]) -> Self {
        Self { 
            position, 
            normal, 
            tex_coords,
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }

    pub fn with_color(position: [f32; 3], normal: [f32; 3], tex_coords: [f32; 2], color: [f32; 4]) -> Self {
        Self { position, normal, tex_coords, color }
    }

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Normal
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // UV/Tex coords
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }

    /// Layout that includes the vertex color (for terrain biome tinting).
    /// Uses location 3 for color -- only for non-instanced pipelines (terrain).
    pub fn layout_with_color() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Instance data for instanced rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct InstanceData {
    /// Model matrix (4x4)
    pub model: [[f32; 4]; 4],
    /// Color tint
    pub color: [f32; 4],
}

impl InstanceData {
    pub fn new(model: [[f32; 4]; 4], color: [f32; 4]) -> Self {
        Self { model, color }
    }

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceData>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // Model matrix row 0
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // Model matrix row 1
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // Model matrix row 2
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // Model matrix row 3
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // Color
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

impl Default for InstanceData {
    fn default() -> Self {
        Self {
            model: glam::Mat4::IDENTITY.to_cols_array_2d(),
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

/// Instance data for celestial body rendering (stars, planets, moons).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CelestialBodyInstance {
    /// Position relative to camera.
    pub position: [f32; 3],
    /// Visual radius in game units.
    pub radius: f32,
    /// RGBA color. w > 0.5 means emissive (star), w <= 0.5 means diffuse lit (planet/moon).
    pub color: [f32; 4],
    /// Direction to the star (for diffuse lighting of planets). w = has_atmosphere flag.
    pub star_direction: [f32; 4],
    /// Atmosphere color (rgb). w = ring_system flag.
    pub atmosphere_color: [f32; 4],
}

impl CelestialBodyInstance {
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CelestialBodyInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // position (vec3)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // radius (f32)
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32,
                },
                // color (vec4)
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // star_direction (vec4)
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // atmosphere_color (vec4)
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Vertex for screen-space text / UI overlay.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct OverlayVertex {
    /// NDC position (x, y) in -1..1
    pub position: [f32; 2],
    /// UV into font atlas (negative x = solid color quad)
    pub tex_coords: [f32; 2],
    /// RGBA color
    pub color: [f32; 4],
}

impl OverlayVertex {
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<OverlayVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Helper to build screen-space overlay text geometry.
/// Generates OverlayVertex quads for each character + optional background rects.
pub struct OverlayTextBuilder {
    pub vertices: Vec<OverlayVertex>,
    pub indices: Vec<u32>,
    screen_w: f32,
    screen_h: f32,
}

/// Font atlas layout: 16 columns x 6 rows of 6x8 pixel glyphs, covering ASCII 32..127.
const FONT_COLS: f32 = 16.0;
const FONT_ROWS: f32 = 6.0;
const GLYPH_PX_W: f32 = 6.0;
const GLYPH_PX_H: f32 = 8.0;

impl OverlayTextBuilder {
    pub fn new(screen_w: f32, screen_h: f32) -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            screen_w,
            screen_h,
        }
    }

    /// Convert pixel coords to NDC.
    fn px_to_ndc(&self, px: f32, py: f32) -> [f32; 2] {
        [
            (px / self.screen_w) * 2.0 - 1.0,
            1.0 - (py / self.screen_h) * 2.0,
        ]
    }

    /// Add a solid-color rectangle (for text background). Coordinates in pixels.
    pub fn add_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        let tl = self.px_to_ndc(x, y);
        let br = self.px_to_ndc(x + w, y + h);
        let base = self.vertices.len() as u32;
        let uv = [-1.0, -1.0]; // sentinel: solid color
        self.vertices.push(OverlayVertex { position: [tl[0], tl[1]], tex_coords: uv, color });
        self.vertices.push(OverlayVertex { position: [br[0], tl[1]], tex_coords: uv, color });
        self.vertices.push(OverlayVertex { position: [br[0], br[1]], tex_coords: uv, color });
        self.vertices.push(OverlayVertex { position: [tl[0], br[1]], tex_coords: uv, color });
        self.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Add a string of text at pixel position (x, y) with the given scale and color.
    /// `scale` = 1.0 means each glyph is 6x8 screen pixels; 2.0 doubles that.
    pub fn add_text(&mut self, x: f32, y: f32, text: &str, scale: f32, color: [f32; 4]) {
        let gw = GLYPH_PX_W * scale;
        let gh = GLYPH_PX_H * scale;
        let mut cx = x;
        for ch in text.chars() {
            let code = ch as u32;
            if code < 32 || code > 127 {
                cx += gw;
                continue;
            }
            let idx = code - 32;
            let col = (idx % 16) as f32;
            let row = (idx / 16) as f32;
            let u0 = col / FONT_COLS;
            let v0 = row / FONT_ROWS;
            let u1 = (col + 1.0) / FONT_COLS;
            let v1 = (row + 1.0) / FONT_ROWS;

            let tl = self.px_to_ndc(cx, y);
            let br = self.px_to_ndc(cx + gw, y + gh);
            let base = self.vertices.len() as u32;
            self.vertices.push(OverlayVertex { position: [tl[0], tl[1]], tex_coords: [u0, v0], color });
            self.vertices.push(OverlayVertex { position: [br[0], tl[1]], tex_coords: [u1, v0], color });
            self.vertices.push(OverlayVertex { position: [br[0], br[1]], tex_coords: [u1, v1], color });
            self.vertices.push(OverlayVertex { position: [tl[0], br[1]], tex_coords: [u0, v1], color });
            self.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            cx += gw;
        }
    }

    /// Add text with a dark background behind it. Returns the Y offset for the next line.
    pub fn add_text_with_bg(
        &mut self,
        x: f32,
        y: f32,
        text: &str,
        scale: f32,
        text_color: [f32; 4],
        bg_color: [f32; 4],
    ) -> f32 {
        let gw = GLYPH_PX_W * scale;
        let gh = GLYPH_PX_H * scale;
        let padding = 2.0 * scale;
        let text_w = text.len() as f32 * gw;
        self.add_rect(x, y, text_w + padding * 2.0, gh + padding * 2.0, bg_color);
        self.add_text(x + padding, y + padding, text, scale, text_color);
        gh + padding * 2.0
    }
}

// ---- Bitmap font atlas generation (6x8 pixel glyphs, ASCII 32..127) ----

/// Classic 6x8 bitmap font covering printable ASCII.
/// Returns an `R8Unorm`-compatible byte array (width=96, height=48) and (width, height).
pub fn generate_font_atlas() -> (Vec<u8>, u32, u32) {
    let atlas_w: u32 = (FONT_COLS as u32) * (GLYPH_PX_W as u32); // 96
    let atlas_h: u32 = (FONT_ROWS as u32) * (GLYPH_PX_H as u32); // 48
    let mut pixels = vec![0u8; (atlas_w * atlas_h) as usize];

    for code in 32u32..128 {
        let glyph = FONT_5X7[code as usize - 32];
        let idx = code - 32;
        let col = idx % 16;
        let row = idx / 16;
        let base_x = col * (GLYPH_PX_W as u32);
        let base_y = row * (GLYPH_PX_H as u32);

        for gy in 0..7u32 {
            let bits = glyph[gy as usize];
            for gx in 0..5u32 {
                if (bits >> (4 - gx)) & 1 != 0 {
                    let px = base_x + gx;
                    let py = base_y + gy;
                    if px < atlas_w && py < atlas_h {
                        pixels[(py * atlas_w + px) as usize] = 255;
                    }
                }
            }
        }
    }

    (pixels, atlas_w, atlas_h)
}

/// 5x7 bitmap font data for ASCII 32..127 (96 characters).
/// Each entry is 7 bytes; each byte encodes one row (5 MSBs used, bit4=leftmost).
#[rustfmt::skip]
const FONT_5X7: [[u8; 7]; 96] = [
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00], // 32 ' '
    [0x04,0x04,0x04,0x04,0x04,0x00,0x04], // 33 '!'
    [0x0A,0x0A,0x00,0x00,0x00,0x00,0x00], // 34 '"'
    [0x0A,0x1F,0x0A,0x0A,0x1F,0x0A,0x00], // 35 '#'
    [0x04,0x0F,0x14,0x0E,0x05,0x1E,0x04], // 36 '$'
    [0x18,0x19,0x02,0x04,0x08,0x13,0x03], // 37 '%'
    [0x08,0x14,0x14,0x08,0x15,0x12,0x0D], // 38 '&'
    [0x04,0x04,0x00,0x00,0x00,0x00,0x00], // 39 '''
    [0x02,0x04,0x08,0x08,0x08,0x04,0x02], // 40 '('
    [0x08,0x04,0x02,0x02,0x02,0x04,0x08], // 41 ')'
    [0x04,0x15,0x0E,0x1F,0x0E,0x15,0x04], // 42 '*'
    [0x00,0x04,0x04,0x1F,0x04,0x04,0x00], // 43 '+'
    [0x00,0x00,0x00,0x00,0x00,0x04,0x08], // 44 ','
    [0x00,0x00,0x00,0x1F,0x00,0x00,0x00], // 45 '-'
    [0x00,0x00,0x00,0x00,0x00,0x00,0x04], // 46 '.'
    [0x01,0x01,0x02,0x04,0x08,0x10,0x10], // 47 '/'
    [0x0E,0x11,0x13,0x15,0x19,0x11,0x0E], // 48 '0'
    [0x04,0x0C,0x04,0x04,0x04,0x04,0x0E], // 49 '1'
    [0x0E,0x11,0x01,0x06,0x08,0x10,0x1F], // 50 '2'
    [0x0E,0x11,0x01,0x06,0x01,0x11,0x0E], // 51 '3'
    [0x02,0x06,0x0A,0x12,0x1F,0x02,0x02], // 52 '4'
    [0x1F,0x10,0x1E,0x01,0x01,0x11,0x0E], // 53 '5'
    [0x06,0x08,0x10,0x1E,0x11,0x11,0x0E], // 54 '6'
    [0x1F,0x01,0x02,0x04,0x08,0x08,0x08], // 55 '7'
    [0x0E,0x11,0x11,0x0E,0x11,0x11,0x0E], // 56 '8'
    [0x0E,0x11,0x11,0x0F,0x01,0x02,0x0C], // 57 '9'
    [0x00,0x00,0x04,0x00,0x00,0x04,0x00], // 58 ':'
    [0x00,0x00,0x04,0x00,0x00,0x04,0x08], // 59 ';'
    [0x02,0x04,0x08,0x10,0x08,0x04,0x02], // 60 '<'
    [0x00,0x00,0x1F,0x00,0x1F,0x00,0x00], // 61 '='
    [0x08,0x04,0x02,0x01,0x02,0x04,0x08], // 62 '>'
    [0x0E,0x11,0x01,0x02,0x04,0x00,0x04], // 63 '?'
    [0x0E,0x11,0x17,0x15,0x17,0x10,0x0E], // 64 '@'
    [0x0E,0x11,0x11,0x1F,0x11,0x11,0x11], // 65 'A'
    [0x1E,0x11,0x11,0x1E,0x11,0x11,0x1E], // 66 'B'
    [0x0E,0x11,0x10,0x10,0x10,0x11,0x0E], // 67 'C'
    [0x1E,0x11,0x11,0x11,0x11,0x11,0x1E], // 68 'D'
    [0x1F,0x10,0x10,0x1E,0x10,0x10,0x1F], // 69 'E'
    [0x1F,0x10,0x10,0x1E,0x10,0x10,0x10], // 70 'F'
    [0x0E,0x11,0x10,0x17,0x11,0x11,0x0F], // 71 'G'
    [0x11,0x11,0x11,0x1F,0x11,0x11,0x11], // 72 'H'
    [0x0E,0x04,0x04,0x04,0x04,0x04,0x0E], // 73 'I'
    [0x07,0x02,0x02,0x02,0x02,0x12,0x0C], // 74 'J'
    [0x11,0x12,0x14,0x18,0x14,0x12,0x11], // 75 'K'
    [0x10,0x10,0x10,0x10,0x10,0x10,0x1F], // 76 'L'
    [0x11,0x1B,0x15,0x15,0x11,0x11,0x11], // 77 'M'
    [0x11,0x19,0x15,0x13,0x11,0x11,0x11], // 78 'N'
    [0x0E,0x11,0x11,0x11,0x11,0x11,0x0E], // 79 'O'
    [0x1E,0x11,0x11,0x1E,0x10,0x10,0x10], // 80 'P'
    [0x0E,0x11,0x11,0x11,0x15,0x12,0x0D], // 81 'Q'
    [0x1E,0x11,0x11,0x1E,0x14,0x12,0x11], // 82 'R'
    [0x0E,0x11,0x10,0x0E,0x01,0x11,0x0E], // 83 'S'
    [0x1F,0x04,0x04,0x04,0x04,0x04,0x04], // 84 'T'
    [0x11,0x11,0x11,0x11,0x11,0x11,0x0E], // 85 'U'
    [0x11,0x11,0x11,0x11,0x0A,0x0A,0x04], // 86 'V'
    [0x11,0x11,0x11,0x15,0x15,0x1B,0x11], // 87 'W'
    [0x11,0x11,0x0A,0x04,0x0A,0x11,0x11], // 88 'X'
    [0x11,0x11,0x0A,0x04,0x04,0x04,0x04], // 89 'Y'
    [0x1F,0x01,0x02,0x04,0x08,0x10,0x1F], // 90 'Z'
    [0x0E,0x08,0x08,0x08,0x08,0x08,0x0E], // 91 '['
    [0x10,0x10,0x08,0x04,0x02,0x01,0x01], // 92 '\'
    [0x0E,0x02,0x02,0x02,0x02,0x02,0x0E], // 93 ']'
    [0x04,0x0A,0x11,0x00,0x00,0x00,0x00], // 94 '^'
    [0x00,0x00,0x00,0x00,0x00,0x00,0x1F], // 95 '_'
    [0x08,0x04,0x00,0x00,0x00,0x00,0x00], // 96 '`'
    [0x00,0x00,0x0E,0x01,0x0F,0x11,0x0F], // 97 'a'
    [0x10,0x10,0x1E,0x11,0x11,0x11,0x1E], // 98 'b'
    [0x00,0x00,0x0E,0x11,0x10,0x11,0x0E], // 99 'c'
    [0x01,0x01,0x0F,0x11,0x11,0x11,0x0F], // 100 'd'
    [0x00,0x00,0x0E,0x11,0x1F,0x10,0x0E], // 101 'e'
    [0x06,0x08,0x1E,0x08,0x08,0x08,0x08], // 102 'f'
    [0x00,0x00,0x0F,0x11,0x0F,0x01,0x0E], // 103 'g'
    [0x10,0x10,0x1E,0x11,0x11,0x11,0x11], // 104 'h'
    [0x04,0x00,0x0C,0x04,0x04,0x04,0x0E], // 105 'i'
    [0x02,0x00,0x06,0x02,0x02,0x12,0x0C], // 106 'j'
    [0x10,0x10,0x12,0x14,0x18,0x14,0x12], // 107 'k'
    [0x0C,0x04,0x04,0x04,0x04,0x04,0x0E], // 108 'l'
    [0x00,0x00,0x1A,0x15,0x15,0x15,0x11], // 109 'm'
    [0x00,0x00,0x1E,0x11,0x11,0x11,0x11], // 110 'n'
    [0x00,0x00,0x0E,0x11,0x11,0x11,0x0E], // 111 'o'
    [0x00,0x00,0x1E,0x11,0x1E,0x10,0x10], // 112 'p'
    [0x00,0x00,0x0F,0x11,0x0F,0x01,0x01], // 113 'q'
    [0x00,0x00,0x16,0x19,0x10,0x10,0x10], // 114 'r'
    [0x00,0x00,0x0F,0x10,0x0E,0x01,0x1E], // 115 's'
    [0x08,0x08,0x1E,0x08,0x08,0x09,0x06], // 116 't'
    [0x00,0x00,0x11,0x11,0x11,0x13,0x0D], // 117 'u'
    [0x00,0x00,0x11,0x11,0x11,0x0A,0x04], // 118 'v'
    [0x00,0x00,0x11,0x15,0x15,0x15,0x0A], // 119 'w'
    [0x00,0x00,0x11,0x0A,0x04,0x0A,0x11], // 120 'x'
    [0x00,0x00,0x11,0x11,0x0F,0x01,0x0E], // 121 'y'
    [0x00,0x00,0x1F,0x02,0x04,0x08,0x1F], // 122 'z'
    [0x02,0x04,0x04,0x08,0x04,0x04,0x02], // 123 '{'
    [0x04,0x04,0x04,0x04,0x04,0x04,0x04], // 124 '|'
    [0x08,0x04,0x04,0x02,0x04,0x04,0x08], // 125 '}'
    [0x00,0x08,0x15,0x02,0x00,0x00,0x00], // 126 '~'
    [0x1F,0x1F,0x1F,0x1F,0x1F,0x1F,0x1F], // 127 DEL (solid block - useful for bg)
];

