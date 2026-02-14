//! Mesh data structures and primitive generation.

use crate::vertex::Vertex;
use glam::Vec3;
use wgpu::util::DeviceExt;

/// A GPU mesh with vertex and index buffers.
pub struct Mesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

impl Mesh {
    /// Create a mesh from vertex and index data.
    pub fn new(device: &wgpu::Device, vertices: &[Vertex], indices: &[u32]) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
            num_indices: indices.len() as u32,
        }
    }

    /// Alias for `new` - create a mesh from vertex and index data.
    pub fn from_data(device: &wgpu::Device, vertices: &[Vertex], indices: &[u32]) -> Self {
        Self::new(device, vertices, indices)
    }

    /// Create a unit cube centered at origin.
    pub fn cube(device: &wgpu::Device) -> Self {
        let vertices = [
            // Front face
            Vertex::new([-0.5, -0.5, 0.5], [0.0, 0.0, 1.0], [0.0, 1.0]),
            Vertex::new([0.5, -0.5, 0.5], [0.0, 0.0, 1.0], [1.0, 1.0]),
            Vertex::new([0.5, 0.5, 0.5], [0.0, 0.0, 1.0], [1.0, 0.0]),
            Vertex::new([-0.5, 0.5, 0.5], [0.0, 0.0, 1.0], [0.0, 0.0]),
            // Back face
            Vertex::new([0.5, -0.5, -0.5], [0.0, 0.0, -1.0], [0.0, 1.0]),
            Vertex::new([-0.5, -0.5, -0.5], [0.0, 0.0, -1.0], [1.0, 1.0]),
            Vertex::new([-0.5, 0.5, -0.5], [0.0, 0.0, -1.0], [1.0, 0.0]),
            Vertex::new([0.5, 0.5, -0.5], [0.0, 0.0, -1.0], [0.0, 0.0]),
            // Top face
            Vertex::new([-0.5, 0.5, 0.5], [0.0, 1.0, 0.0], [0.0, 1.0]),
            Vertex::new([0.5, 0.5, 0.5], [0.0, 1.0, 0.0], [1.0, 1.0]),
            Vertex::new([0.5, 0.5, -0.5], [0.0, 1.0, 0.0], [1.0, 0.0]),
            Vertex::new([-0.5, 0.5, -0.5], [0.0, 1.0, 0.0], [0.0, 0.0]),
            // Bottom face
            Vertex::new([-0.5, -0.5, -0.5], [0.0, -1.0, 0.0], [0.0, 1.0]),
            Vertex::new([0.5, -0.5, -0.5], [0.0, -1.0, 0.0], [1.0, 1.0]),
            Vertex::new([0.5, -0.5, 0.5], [0.0, -1.0, 0.0], [1.0, 0.0]),
            Vertex::new([-0.5, -0.5, 0.5], [0.0, -1.0, 0.0], [0.0, 0.0]),
            // Right face
            Vertex::new([0.5, -0.5, 0.5], [1.0, 0.0, 0.0], [0.0, 1.0]),
            Vertex::new([0.5, -0.5, -0.5], [1.0, 0.0, 0.0], [1.0, 1.0]),
            Vertex::new([0.5, 0.5, -0.5], [1.0, 0.0, 0.0], [1.0, 0.0]),
            Vertex::new([0.5, 0.5, 0.5], [1.0, 0.0, 0.0], [0.0, 0.0]),
            // Left face
            Vertex::new([-0.5, -0.5, -0.5], [-1.0, 0.0, 0.0], [0.0, 1.0]),
            Vertex::new([-0.5, -0.5, 0.5], [-1.0, 0.0, 0.0], [1.0, 1.0]),
            Vertex::new([-0.5, 0.5, 0.5], [-1.0, 0.0, 0.0], [1.0, 0.0]),
            Vertex::new([-0.5, 0.5, -0.5], [-1.0, 0.0, 0.0], [0.0, 0.0]),
        ];

        #[rustfmt::skip]
        let indices: [u32; 36] = [
            0, 1, 2, 2, 3, 0,       // Front
            4, 5, 6, 6, 7, 4,       // Back
            8, 9, 10, 10, 11, 8,   // Top
            12, 13, 14, 14, 15, 12, // Bottom
            16, 17, 18, 18, 19, 16, // Right
            20, 21, 22, 22, 23, 20, // Left
        ];

        Self::new(device, &vertices, &indices)
    }

    /// Create a billboard quad (XY plane, facing +Z). Use with a camera-facing rotation
    /// to create particles that always face the viewer.
    pub fn billboard_quad(device: &wgpu::Device, size: f32) -> Self {
        let half = size / 2.0;
        let vertices = [
            Vertex::new([-half, -half, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0]),
            Vertex::new([ half, -half, 0.0], [0.0, 0.0, 1.0], [1.0, 1.0]),
            Vertex::new([ half,  half, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0]),
            Vertex::new([-half,  half, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0]),
        ];
        let indices: [u32; 6] = [0, 1, 2, 2, 3, 0];
        Self::new(device, &vertices, &indices)
    }

    /// Create a ground plane.
    pub fn plane(device: &wgpu::Device, size: f32) -> Self {
        let half = size / 2.0;
        let vertices = [
            Vertex::new([-half, 0.0, half], [0.0, 1.0, 0.0], [0.0, 0.0]),
            Vertex::new([half, 0.0, half], [0.0, 1.0, 0.0], [1.0, 0.0]),
            Vertex::new([half, 0.0, -half], [0.0, 1.0, 0.0], [1.0, 1.0]),
            Vertex::new([-half, 0.0, -half], [0.0, 1.0, 0.0], [0.0, 1.0]),
        ];

        let indices: [u32; 6] = [0, 1, 2, 2, 3, 0];

        Self::new(device, &vertices, &indices)
    }

    /// Unit cube for composable viewmodel rendering (each instance = one rifle part).
    /// The viewmodel pass composes the full M1A4 Morita rifle from many scaled/positioned instances.
    pub fn rifle_viewmodel(device: &wgpu::Device) -> Self {
        Self::cube(device)
    }

    /// Create a bullet tracer mesh: an elongated octahedron (diamond) pointing along +Z.
    /// Much more visible and bullet-like than a flat quad.
    pub fn bullet_tracer(device: &wgpu::Device) -> Self {
        // 6-vertex diamond: front tip, back tip, and 4 side verts
        // Oriented along Z axis so scaling Z makes it longer
        let front = [0.0, 0.0, 0.5_f32];
        let back  = [0.0, 0.0, -0.5_f32];
        let top   = [0.0, 0.3, 0.0_f32];
        let bot   = [0.0, -0.3, 0.0_f32];
        let right = [0.3, 0.0, 0.0_f32];
        let left  = [-0.3, 0.0, 0.0_f32];

        let vertices = [
            // Front-top
            Vertex::new(front, [0.0, 0.5, 0.86], [0.5, 0.0]),
            Vertex::new(right, [0.5, 0.5, 0.0], [1.0, 0.5]),
            Vertex::new(top,   [0.0, 1.0, 0.0], [0.5, 0.5]),
            // Front-right
            Vertex::new(front, [0.5, 0.0, 0.86], [0.5, 0.0]),
            Vertex::new(bot,   [0.0, -1.0, 0.0], [1.0, 0.5]),
            Vertex::new(right, [0.5, -0.5, 0.0], [0.5, 0.5]),
            // Front-bottom
            Vertex::new(front, [0.0, -0.5, 0.86], [0.5, 0.0]),
            Vertex::new(left,  [-0.5, -0.5, 0.0], [0.0, 0.5]),
            Vertex::new(bot,   [0.0, -1.0, 0.0], [0.5, 0.5]),
            // Front-left
            Vertex::new(front, [-0.5, 0.0, 0.86], [0.5, 0.0]),
            Vertex::new(top,   [0.0, 1.0, 0.0], [0.0, 0.5]),
            Vertex::new(left,  [-0.5, 0.5, 0.0], [0.5, 0.5]),
            // Back-top
            Vertex::new(back, [0.0, 0.5, -0.86], [0.5, 1.0]),
            Vertex::new(top,  [0.0, 1.0, 0.0], [0.5, 0.5]),
            Vertex::new(right,[0.5, 0.5, 0.0], [1.0, 0.5]),
            // Back-right
            Vertex::new(back, [0.5, 0.0, -0.86], [0.5, 1.0]),
            Vertex::new(right,[0.5, -0.5, 0.0], [1.0, 0.5]),
            Vertex::new(bot,  [0.0, -1.0, 0.0], [0.5, 0.5]),
            // Back-bottom
            Vertex::new(back, [0.0, -0.5, -0.86], [0.5, 1.0]),
            Vertex::new(bot,  [0.0, -1.0, 0.0], [0.5, 0.5]),
            Vertex::new(left, [-0.5, -0.5, 0.0], [0.0, 0.5]),
            // Back-left
            Vertex::new(back, [-0.5, 0.0, -0.86], [0.5, 1.0]),
            Vertex::new(left, [-0.5, 0.5, 0.0], [0.0, 0.5]),
            Vertex::new(top,  [0.0, 1.0, 0.0], [0.5, 0.5]),
        ];

        let indices: Vec<u32> = (0..24).collect();
        Self::new(device, &vertices, &indices)
    }

    /// Create a muzzle flash mesh: a multi-pointed star (3 intersecting quads).
    /// Bright flash effect visible from any angle.
    pub fn muzzle_flash(device: &wgpu::Device) -> Self {
        let s = 0.5_f32;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // 3 quads at different rotations (0, 60, 120 degrees around Z)
        for i in 0..3 {
            let angle = (i as f32) * std::f32::consts::PI / 3.0;
            let (sin_a, cos_a) = angle.sin_cos();
            let base = vertices.len() as u32;

            // Quad corners rotated around Z axis
            let dx = cos_a * s;
            let dy = sin_a * s;

            vertices.push(Vertex::new([-dx, -dy, -s * 0.3], [0.0, 0.0, 1.0], [0.0, 0.0]));
            vertices.push(Vertex::new([ dx,  dy, -s * 0.3], [0.0, 0.0, 1.0], [1.0, 0.0]));
            vertices.push(Vertex::new([ dx,  dy,  s * 0.3], [0.0, 0.0, 1.0], [1.0, 1.0]));
            vertices.push(Vertex::new([-dx, -dy,  s * 0.3], [0.0, 0.0, 1.0], [0.0, 1.0]));

            indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
        }

        Self::new(device, &vertices, &indices)
    }

    /// Create a UV sphere.
    pub fn sphere(device: &wgpu::Device, radius: f32, segments: u32, rings: u32) -> Self {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for ring in 0..=rings {
            let phi = std::f32::consts::PI * ring as f32 / rings as f32;
            let y = radius * phi.cos();
            let ring_radius = radius * phi.sin();

            for segment in 0..=segments {
                let theta = 2.0 * std::f32::consts::PI * segment as f32 / segments as f32;
                let x = ring_radius * theta.cos();
                let z = ring_radius * theta.sin();

                let position = [x, y, z];
                let normal = Vec3::new(x, y, z).normalize();
                let uv = [
                    segment as f32 / segments as f32,
                    ring as f32 / rings as f32,
                ];

                vertices.push(Vertex::new(position, normal.into(), uv));
            }
        }

        for ring in 0..rings {
            for segment in 0..segments {
                let current = ring * (segments + 1) + segment;
                let next = current + segments + 1;

                indices.push(current);
                indices.push(next);
                indices.push(current + 1);

                indices.push(current + 1);
                indices.push(next);
                indices.push(next + 1);
            }
        }

        Self::new(device, &vertices, &indices)
    }
}

/// Mesh data before GPU upload (for procedural generation).
#[derive(Debug, Clone, Default)]
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upload(&self, device: &wgpu::Device) -> Mesh {
        Mesh::new(device, &self.vertices, &self.indices)
    }
}
