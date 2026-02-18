//! Main renderer managing wgpu state and rendering.

use crate::{
    camera::{Camera, CameraUniform},
    mesh::Mesh,
    pipeline::{
        create_blur_bind_group_layout,
        create_blur_pipeline,
        create_bright_bind_group_layout,
        create_bright_pipeline,
        create_camera_bind_group_layout,
        create_celestial_pipeline,
        create_cinematic_bind_group_layout,
        create_cinematic_pipeline,
        create_main_shadow_pipeline,
        create_overlay_bind_group_layout,
        create_overlay_pipeline,
        create_render_pipeline,
        create_shadow_bind_group_layout,
        create_shadow_pass_bind_group_layout,
        create_sky_bind_group_layout,
        create_sky_pipeline,
        create_terrain_bind_group_layout,
        create_terrain_pipeline,
        create_terrain_shadow_pipeline,
        create_water_pipeline,
        create_texture_bind_group_layout,
        create_viewmodel_pipeline,
    },
    texture::Texture,
    vertex::{CelestialBodyInstance, InstanceData, OverlayVertex},
};
use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

/// Size of the terrain deformation heightfield (world space follows player).
pub const DEFORM_TEXTURE_SIZE: u32 = 256;
/// Half-extent of deformation region in world units (total 128m x 128m).
pub const DEFORM_HALF_SIZE: f32 = 64.0;

/// Terrain shader uniform (must match terrain.wgsl TerrainUniform).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct TerrainUniform {
    pub biome_colors: [[f32; 4]; 4],
    pub biome_params: [f32; 4],
    pub sun_direction: [f32; 4],
    pub fog_params: [f32; 4],
    /// x = deform_origin_x, y = deform_origin_z, z = deform_half_size, w = deform_enabled (0 or 1)
    pub deform_params: [f32; 4],
    /// x = snow_enabled (0 or 1), yzw unused
    pub snow_params: [f32; 4],
}

impl Default for TerrainUniform {
    fn default() -> Self {
        Self {
            biome_colors: [
                [0.6, 0.5, 0.4, 1.0],
                [0.4, 0.5, 0.35, 1.0],
                [0.5, 0.45, 0.4, 1.0],
                [0.55, 0.48, 0.42, 1.0],
            ],
            biome_params: [4.0, 2.0, 0.0, 0.0], // blend_sharpness, detail_scale, time, unused
            sun_direction: [0.5, 1.0, 0.3, 0.0],
            fog_params: [0.0003, 0.05, 50.0, 400.0], // density, height_falloff, start, end
            deform_params: [0.0, 0.0, DEFORM_HALF_SIZE, 0.0], // origin_x, origin_z, half_size, enabled
            snow_params: [0.0, 0.0, 0.0, 0.0], // x = snow_enabled
        }
    }
}

/// Sky shader uniform (must match sky.wgsl SkyUniform).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SkyUniform {
    pub sun_direction: [f32; 4],     // xyz = direction, w = intensity
    pub sun_color: [f32; 4],         // rgb = color, w = sun disk size
    pub sky_color_zenith: [f32; 4],
    pub sky_color_horizon: [f32; 4],
    pub ground_color: [f32; 4],      // rgb = ground, w = haze amount
    pub params: [f32; 4],            // x = time, y = cloud_density, z = dust, w = planet_type
}

impl Default for SkyUniform {
    fn default() -> Self {
        Self {
            sun_direction: [0.3, 0.8, 0.2, 1.0],
            sun_color: [1.0, 0.98, 0.9, 0.02],
            sky_color_zenith: [0.2, 0.4, 0.7, 1.0],
            sky_color_horizon: [0.6, 0.65, 0.75, 1.0],
            ground_color: [0.4, 0.35, 0.3, 0.3],
            params: [0.0, 0.3, 0.1, 0.0],
        }
    }
}

/// Shadow map uniform (must match shadow.wgsl ShadowUniform). Used for depth pass and for sampling.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ShadowUniform {
    pub light_view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub planet_radius: f32,
    pub _pad: [f32; 4],
}

/// Main renderer state.
pub struct Renderer {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub window: Arc<Window>,

    // Pipelines
    render_pipeline: wgpu::RenderPipeline,
    terrain_pipeline: wgpu::RenderPipeline,
    water_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    viewmodel_pipeline: wgpu::RenderPipeline,

    // Bind groups and layouts
    camera_bind_group_layout: wgpu::BindGroupLayout,
    terrain_bind_group: wgpu::BindGroup,
    terrain_buffer: wgpu::Buffer,
    /// Heightfield for terrain deformation (footprints in snow/sand). R32Float, 256x256.
    deform_texture: wgpu::Texture,
    deform_sampler: wgpu::Sampler,
    /// Snow accumulation heightfield (weather-driven). R32Float, 256x256.
    snow_texture: wgpu::Texture,
    sky_bind_group: wgpu::BindGroup,
    sky_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    camera_buffer: wgpu::Buffer,
    camera_uniform: CameraUniform,

    texture_bind_group_layout: wgpu::BindGroupLayout,
    default_texture_bind_group: wgpu::BindGroup,

    // Shadow mapping (directional sun shadow)
    shadow_map_texture: wgpu::Texture,
    shadow_map_view: wgpu::TextureView,
    shadow_sampler: wgpu::Sampler,
    shadow_buffer: wgpu::Buffer,
    shadow_pass_bind_group: wgpu::BindGroup,
    shadow_bind_group: wgpu::BindGroup,
    terrain_shadow_pipeline: wgpu::RenderPipeline,
    main_shadow_pipeline: wgpu::RenderPipeline,

    // Depth buffer
    depth_texture: Texture,

    // Instance buffer for batched rendering
    instance_buffer: wgpu::Buffer,
    max_instances: u32,
    /// Tracks current write offset into instance_buffer per frame.
    /// Each render pass writes to a unique region so `queue.write_buffer` calls
    /// don't overwrite each other (all writes execute before command buffer).
    frame_instance_offset: u32,

    /// Viewmodel mesh (rifle) owned by renderer so this pass can never draw a bug mesh by mistake.
    viewmodel_mesh: Mesh,

    // Celestial body rendering
    celestial_pipeline: wgpu::RenderPipeline,
    celestial_sphere_mesh: Mesh,
    celestial_instance_buffer: wgpu::Buffer,
    celestial_max_instances: u32,

    // Text overlay
    overlay_pipeline: wgpu::RenderPipeline,
    overlay_bind_group: wgpu::BindGroup,

    // Cinematic post-process (97 movie / Heinlein look)
    scene_color_texture: wgpu::Texture,
    cinematic_pipeline: wgpu::RenderPipeline,
    cinematic_bind_group_layout: wgpu::BindGroupLayout,
    cinematic_uniform_buffer: wgpu::Buffer,
    cinematic_sampler: wgpu::Sampler,

    // Bloom: bright pass + blur
    bloom_texture_a: wgpu::Texture,
    bloom_texture_b: wgpu::Texture,
    bright_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    bright_bind_group_layout: wgpu::BindGroupLayout,
    blur_bind_group_layout: wgpu::BindGroupLayout,
    bright_uniform_buffer: wgpu::Buffer,
    blur_uniform_h: wgpu::Buffer,
    blur_uniform_v: wgpu::Buffer,

    // Depth sampler for SSAO (non-compare, for sampling depth values)
    depth_sampler_linear: wgpu::Sampler,
}

impl Renderer {
    /// Create a new renderer for the given window.
    pub async fn new(window: Arc<Window>) -> Result<Self> {
        let size = window.inner_size();

        // Create wgpu instance: Vulkan/DX12 on Windows/Linux, Metal on macOS (native; no MoltenVK needed)
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // Create surface
        let surface = instance.create_surface(window.clone())?;

        // Request adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("Failed to find suitable GPU adapter"))?;

        log::info!("Using GPU: {:?}", adapter.get_info().name);

        // Request device
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Main Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await?;

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        // Prefer Mailbox (low-latency vsync) if available; otherwise AutoVsync.
        // Mailbox presents the most recent frame at vblank, reducing input lag vs Fifo.
        let present_mode = surface_caps
            .present_modes
            .iter()
            .find(|m| matches!(m, wgpu::PresentMode::Mailbox))
            .copied()
            .unwrap_or(wgpu::PresentMode::AutoVsync);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            // 1 = minimum latency (CPU/GPU less parallel but snappier input)
            desired_maximum_frame_latency: 1,
        };
        surface.configure(&device, &config);

        // Create camera uniform buffer
        let camera_uniform = CameraUniform::new();
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create bind group layouts
        let camera_bind_group_layout = create_camera_bind_group_layout(&device);
        let texture_bind_group_layout = create_texture_bind_group_layout(&device);

        // Create camera bind group
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // Create default white texture
        let default_texture = Texture::white_pixel(&device, &queue);
        let default_texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Default Texture Bind Group"),
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&default_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&default_texture.sampler),
                },
            ],
        });

        // Shadow mapping: directional sun shadow map (2048x2048 depth)
        const SHADOW_MAP_SIZE: u32 = 2048;
        let shadow_pass_layout = create_shadow_pass_bind_group_layout(&device);
        let shadow_sample_layout = create_shadow_bind_group_layout(&device);
        let shadow_uniform = ShadowUniform {
            light_view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0, 0.0, 0.0],
            planet_radius: 0.0,
            _pad: [0.0; 4],
        };
        let shadow_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shadow Uniform"),
            contents: bytemuck::cast_slice(&[shadow_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let shadow_map_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Shadow Map"),
            size: wgpu::Extent3d { width: SHADOW_MAP_SIZE, height: SHADOW_MAP_SIZE, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Texture::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_map_view = shadow_map_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let shadow_pass_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Pass Bind Group"),
            layout: &shadow_pass_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: shadow_buffer.as_entire_binding() }],
        });
        let shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Bind Group"),
            layout: &shadow_sample_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: shadow_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shadow_map_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&shadow_sampler) },
            ],
        });
        let terrain_shadow_pipeline = create_terrain_shadow_pipeline(&device, &shadow_pass_layout);
        let main_shadow_pipeline = create_main_shadow_pipeline(&device, &shadow_pass_layout);

        // Create render pipeline
        let render_pipeline = create_render_pipeline(
            &device,
            &config,
            &camera_bind_group_layout,
            &texture_bind_group_layout,
            &shadow_sample_layout,
        );

        // Terrain pipeline (camera + terrain uniform in one bind group)
        let terrain_bind_group_layout = create_terrain_bind_group_layout(&device);
        let terrain_uniform = TerrainUniform::default();
        let terrain_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Terrain Uniform Buffer"),
            contents: bytemuck::cast_slice(&[terrain_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        // Deformation heightfield: R32Float 256x256, world-space footprint depressions
        let deform_size = DEFORM_TEXTURE_SIZE;
        let deform_pixels: Vec<f32> = vec![0.0; (deform_size * deform_size) as usize];
        let deform_texture = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("Terrain Deformation"),
                size: wgpu::Extent3d {
                    width: deform_size,
                    height: deform_size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            bytemuck::cast_slice(&deform_pixels),
        );
        let deform_view = deform_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let deform_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Deform Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        // Snow accumulation heightfield (same size/format as deform; weather-driven knee-deep snow)
        let snow_texture = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("Terrain Snow"),
                size: wgpu::Extent3d {
                    width: deform_size,
                    height: deform_size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            bytemuck::cast_slice(&deform_pixels),
        );
        let snow_view = snow_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let terrain_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Terrain Bind Group"),
            layout: &terrain_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: terrain_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&deform_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&deform_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&snow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&deform_sampler),
                },
            ],
        });
        let terrain_pipeline = create_terrain_pipeline(&device, &config, &terrain_bind_group_layout, &shadow_sample_layout);
        let water_pipeline = create_water_pipeline(&device, &config, &terrain_bind_group_layout);

        let sky_bind_group_layout = create_sky_bind_group_layout(&device);
        let sky_uniform = SkyUniform::default();
        let sky_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Sky Uniform Buffer"),
            contents: bytemuck::cast_slice(&[sky_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let sky_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sky Bind Group"),
            layout: &sky_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: sky_buffer.as_entire_binding(),
                },
            ],
        });
        let sky_pipeline = create_sky_pipeline(&device, &config, &sky_bind_group_layout);

        let viewmodel_pipeline =
            create_viewmodel_pipeline(&device, &config, &camera_bind_group_layout, &texture_bind_group_layout, &shadow_sample_layout);

        // Create depth texture
        let depth_texture = Texture::create_depth_texture(&device, config.width, config.height, "Depth Texture");

        // Create instance buffer (support up to 65536 instances — bugs + env + debris + corpses)
        let max_instances = 65536u32;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instance Buffer"),
            size: (std::mem::size_of::<InstanceData>() * max_instances as usize) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let viewmodel_mesh = Mesh::rifle_viewmodel(&device);

        // --- Celestial body rendering ---
        let celestial_pipeline = create_celestial_pipeline(&device, &config, &camera_bind_group_layout);
        let celestial_sphere_mesh = Mesh::sphere(&device, 1.0, 24, 16);
        let celestial_max_instances = 256u32;
        let celestial_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Celestial Instance Buffer"),
            size: (std::mem::size_of::<CelestialBodyInstance>() * celestial_max_instances as usize) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // --- Overlay (text) pipeline ---
        let overlay_bind_group_layout = create_overlay_bind_group_layout(&device);
        let overlay_pipeline = create_overlay_pipeline(&device, &config, &overlay_bind_group_layout);

        // Generate bitmap font atlas and upload as a texture
        let (font_pixels, font_w, font_h) = crate::vertex::generate_font_atlas();
        let font_texture = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("Font Atlas"),
                size: wgpu::Extent3d {
                    width: font_w,
                    height: font_h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &font_pixels,
        );
        let font_view = font_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let font_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let overlay_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Overlay Bind Group"),
            layout: &overlay_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&font_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&font_sampler),
                },
            ],
        });

        // Scene texture for cinematic pass (render 3D to this, then post-process to swap chain)
        let scene_color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Scene Color"),
            size: wgpu::Extent3d {
                width: config.width.max(1),
                height: config.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        // Cinematic post-process (MIRO-style stylized + Starship Troopers military palette)
        let cinematic_bind_group_layout = create_cinematic_bind_group_layout(&device);
        let cinematic_pipeline = create_cinematic_pipeline(&device, &config, &cinematic_bind_group_layout);
        // Uniform: time, dither, vignette, bloom_strength, lift+ssao_scale, inv_gamma+ssao_radius, gain+ssao_bias
        let cinematic_uniform: [f32; 16] = [
            0.0,   // time
            0.03,  // dither_strength (lighter for cleaner stylized look)
            0.38,  // vignette_strength (softer for atmospheric MIRO feel)
            0.42,  // bloom_strength (slightly stronger glow)
            0.06, 0.03, 0.01, 0.4,    // lift (warmer orange/amber SST shadows), ssao_scale
            0.92, 0.92, 0.92, 0.018,  // inv_gamma (slightly flatter for stylized), ssao_radius
            1.12, 1.08, 1.05, 0.002,  // gain (punchier highlights), ssao_bias
        ];
        let cinematic_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cinematic Uniform"),
            contents: bytemuck::cast_slice(&cinematic_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let cinematic_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Scene Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Depth sampler for SSAO (no compare - we sample raw depth values)
        let depth_sampler_linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Depth Sampler Linear"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: None, // Non-comparison for textureSample
            ..Default::default()
        });

        // Bloom textures (1/4 resolution for performance)
        let bloom_w = (config.width / 4).max(1);
        let bloom_h = (config.height / 4).max(1);
        let bloom_texture_a = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Bloom A"),
            size: wgpu::Extent3d { width: bloom_w, height: bloom_h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let bloom_texture_b = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Bloom B"),
            size: wgpu::Extent3d { width: bloom_w, height: bloom_h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        // Bright pass: threshold 0.7
        let bright_bind_group_layout = create_bright_bind_group_layout(&device);
        let bright_pipeline = create_bright_pipeline(&device, &config, &bright_bind_group_layout);
        let bright_uniform: [f32; 4] = [0.7, 0.0, 0.0, 0.0]; // threshold
        let bright_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Bright Uniform"),
            contents: bytemuck::cast_slice(&bright_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Blur: separate direction uniforms for H and V (avoid overwrite between passes)
        let blur_bind_group_layout = create_blur_bind_group_layout(&device);
        let blur_pipeline = create_blur_pipeline(&device, &config, &blur_bind_group_layout);
        let blur_h: [f32; 4] = [1.0, 0.0, 0.0, 0.0];
        let blur_v: [f32; 4] = [0.0, 1.0, 0.0, 0.0];
        let blur_uniform_h = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Blur Uniform H"),
            contents: bytemuck::cast_slice(&blur_h),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let blur_uniform_v = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Blur Uniform V"),
            contents: bytemuck::cast_slice(&blur_v),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size,
            window,
            render_pipeline,
            terrain_pipeline,
            water_pipeline,
            sky_pipeline,
            viewmodel_pipeline,
            camera_bind_group_layout,
            camera_bind_group,
            terrain_bind_group,
            terrain_buffer,
            deform_texture,
            deform_sampler,
            snow_texture,
            sky_bind_group,
            sky_buffer,
            camera_buffer,
            camera_uniform,
            texture_bind_group_layout,
            default_texture_bind_group,
            shadow_map_texture,
            shadow_map_view,
            shadow_sampler,
            shadow_buffer,
            shadow_pass_bind_group,
            shadow_bind_group,
            terrain_shadow_pipeline,
            main_shadow_pipeline,
            depth_texture,
            instance_buffer,
            max_instances,
            frame_instance_offset: 0,
            viewmodel_mesh,
            celestial_pipeline,
            celestial_sphere_mesh,
            celestial_instance_buffer,
            celestial_max_instances,
            overlay_pipeline,
            overlay_bind_group,
            scene_color_texture,
            cinematic_pipeline,
            cinematic_bind_group_layout,
            cinematic_uniform_buffer,
            cinematic_sampler,
            bloom_texture_a,
            bloom_texture_b,
            bright_pipeline,
            blur_pipeline,
            bright_bind_group_layout,
            blur_bind_group_layout,
            bright_uniform_buffer,
            blur_uniform_h,
            blur_uniform_v,
            depth_sampler_linear,
        })
    }

    /// Update shadow light view-proj and camera/planet for curvature. Call before shadow pass and before main scene.
    pub fn update_shadow_light(
        &mut self,
        sun_dir: [f32; 3],
        camera_pos: [f32; 3],
        planet_radius: f32,
    ) {
        let sun = glam::Vec3::from_array(sun_dir);
        let cam = glam::Vec3::from_array(camera_pos);
        let dist = 120.0;
        let light_eye = cam + sun * dist;
        let light_target = cam;
        let up = if sun.y.abs() > 0.99 { glam::Vec3::Z } else { glam::Vec3::Y };
        let view = glam::Mat4::look_at_rh(light_eye, light_target, up);
        let half = 70.0f32;
        let proj = glam::Mat4::orthographic_rh(-half, half, -half, half, 10.0, 280.0);
        let light_view_proj = proj * view;
        let u = ShadowUniform {
            light_view_proj: light_view_proj.to_cols_array_2d(),
            camera_pos,
            planet_radius,
            _pad: [0.0; 4],
        };
        self.queue.write_buffer(&self.shadow_buffer, 0, bytemuck::cast_slice(&[u]));
    }

    /// Run shadow pass: clear shadow map, set bind group, then run the closure to draw terrain and instanced geometry.
    pub fn with_shadow_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        f: impl FnOnce(&Self, &mut wgpu::RenderPass),
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Shadow Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.shadow_map_view,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_bind_group(0, &self.shadow_pass_bind_group, &[]);
        f(self, &mut pass);
    }

    /// Draw one terrain chunk into the shadow map. Call after begin_shadow_pass; use terrain_shadow_pipeline.
    pub fn render_terrain_shadow(
        &self,
        pass: &mut wgpu::RenderPass,
        mesh: &Mesh,
    ) {
        pass.set_pipeline(&self.terrain_shadow_pipeline);
        pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
    }

    /// Draw instanced geometry into the shadow map. Call after begin_shadow_pass; use main_shadow_pipeline.
    pub fn render_shadow_instanced(
        &self,
        pass: &mut wgpu::RenderPass,
        mesh: &Mesh,
        instances: &[InstanceData],
        base_offset: u32,
    ) {
        if instances.is_empty() {
            return;
        }
        let offset = base_offset as usize * std::mem::size_of::<InstanceData>();
        self.queue.write_buffer(&self.instance_buffer, offset as u64, bytemuck::cast_slice(instances));
        pass.set_pipeline(&self.main_shadow_pipeline);
        pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(offset as u64..));
        pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..mesh.num_indices, 0, base_offset..(base_offset + instances.len() as u32));
    }

    /// Handle window resize.
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture = Texture::create_depth_texture(
                &self.device,
                self.config.width,
                self.config.height,
                "Depth Texture",
            );
            self.scene_color_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Scene Color"),
                size: wgpu::Extent3d {
                    width: self.config.width.max(1),
                    height: self.config.height.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let bloom_w = (self.config.width / 4).max(1);
            let bloom_h = (self.config.height / 4).max(1);
            self.bloom_texture_a = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Bloom A"),
                size: wgpu::Extent3d { width: bloom_w, height: bloom_h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            self.bloom_texture_b = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Bloom B"),
                size: wgpu::Extent3d { width: bloom_w, height: bloom_h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
        }
    }

    /// View of the offscreen scene texture. Render all 3D content to this; then run cinematic pass to swap chain.
    pub fn scene_view(&self) -> wgpu::TextureView {
        self.scene_color_texture
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// Update cinematic uniform (call once per frame before run_cinematic_pass).
    pub fn update_cinematic_uniform(&mut self, time: f32) {
        let cinematic_uniform: [f32; 16] = [
            time,
            0.03,  // dither_strength
            0.38,  // vignette_strength
            0.42,  // bloom_strength
            0.0, 0.0, 0.0, 0.4,       // lift (neutral — no orange/amber piss filter), ssao_scale
            0.92, 0.92, 0.92, 0.018,  // inv_gamma, ssao_radius
            1.12, 1.08, 1.05, 0.002,  // gain, ssao_bias
        ];
        self.queue.write_buffer(
            &self.cinematic_uniform_buffer,
            0,
            bytemuck::cast_slice(&cinematic_uniform),
        );
    }

    /// Run bloom passes: bright extract -> blur H -> blur V. Returns bloom texture view.
    pub fn run_bloom_passes(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        scene_view: &wgpu::TextureView,
    ) -> wgpu::TextureView {
        let bloom_a_view = self.bloom_texture_a.create_view(&wgpu::TextureViewDescriptor::default());
        let bloom_b_view = self.bloom_texture_b.create_view(&wgpu::TextureViewDescriptor::default());

        // Bright pass: scene -> bloom_a
        let bright_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bright Bind Group"),
            layout: &self.bright_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(scene_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.cinematic_sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: self.bright_uniform_buffer.as_entire_binding() },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Bright Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &bloom_a_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.bright_pipeline);
        pass.set_bind_group(0, &bright_bind, &[]);
        pass.draw(0..3, 0..1);
        drop(pass);

        // Blur horizontal: bloom_a -> bloom_b
        let blur_bind_h = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blur Bind H"),
            layout: &self.blur_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&bloom_a_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.cinematic_sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: self.blur_uniform_h.as_entire_binding() },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Blur H Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &bloom_b_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.blur_pipeline);
        pass.set_bind_group(0, &blur_bind_h, &[]);
        pass.draw(0..3, 0..1);
        drop(pass);

        // Blur vertical: bloom_b -> bloom_a
        let blur_bind_v = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blur Bind V"),
            layout: &self.blur_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&bloom_b_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.cinematic_sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: self.blur_uniform_v.as_entire_binding() },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Blur V Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &bloom_a_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.blur_pipeline);
        pass.set_bind_group(0, &blur_bind_v, &[]);
        pass.draw(0..3, 0..1);

        bloom_a_view
    }

    /// Run cinematic post-process: scene + bloom + SSAO + color grading -> output.
    pub fn run_cinematic_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        scene_view: &wgpu::TextureView,
        bloom_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
    ) {
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Cinematic Bind Group"),
            layout: &self.cinematic_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(scene_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.cinematic_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.cinematic_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(bloom_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&self.depth_sampler_linear),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Cinematic Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.cinematic_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Update camera uniform. `planet_radius` > 0 enables curvature for instanced objects to match terrain.
    pub fn update_camera(&mut self, camera: &Camera, planet_radius: f32) {
        self.camera_uniform.update(camera, planet_radius);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
    }

    /// Set camera uniform for viewmodel pass (view space, camera at origin). Call before render_viewmodel; next frame update_camera restores normal view.
    pub fn update_camera_viewmodel(&mut self, camera: &Camera) {
        self.camera_uniform.update_viewmodel(camera);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
    }

    /// Begin a new frame, returns the command encoder and output view.
    pub fn begin_frame(&mut self) -> Result<(wgpu::SurfaceTexture, wgpu::CommandEncoder)> {
        self.frame_instance_offset = 0; // Reset per-frame instance offset
        let output = self.surface.get_current_texture()?;
        let encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });
        Ok((output, encoder))
    }

    /// Render meshes with instancing.
    pub fn render_instanced(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        mesh: &Mesh,
        instances: &[InstanceData],
    ) {
        if instances.is_empty() {
            return;
        }

        // Allocate a unique region in the instance buffer for this draw call
        let offset = self.frame_instance_offset;
        let remaining = self.max_instances.saturating_sub(offset) as usize;
        let instance_count = instances.len().min(remaining);
        if instance_count == 0 { return; }

        let byte_offset = (offset as usize * std::mem::size_of::<InstanceData>()) as u64;
        self.queue.write_buffer(
            &self.instance_buffer,
            byte_offset,
            bytemuck::cast_slice(&instances[..instance_count]),
        );
        self.frame_instance_offset = offset + instance_count as u32;

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.4,
                        g: 0.35,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
        render_pass.set_bind_group(1, &self.default_texture_bind_group, &[]);
        render_pass.set_bind_group(2, &self.shadow_bind_group, &[]);
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh.num_indices, 0, offset..(offset + instance_count as u32));
    }

    /// Render meshes with instancing, loading existing frame content (no clear).
    pub fn render_instanced_load(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        mesh: &Mesh,
        instances: &[InstanceData],
    ) {
        if instances.is_empty() {
            return;
        }

        // Allocate a unique region in the instance buffer for this draw call
        let offset = self.frame_instance_offset;
        let remaining = self.max_instances.saturating_sub(offset) as usize;
        let instance_count = instances.len().min(remaining);
        if instance_count == 0 { return; }

        let byte_offset = (offset as usize * std::mem::size_of::<InstanceData>()) as u64;
        self.queue.write_buffer(
            &self.instance_buffer,
            byte_offset,
            bytemuck::cast_slice(&instances[..instance_count]),
        );
        self.frame_instance_offset = offset + instance_count as u32;

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass (Load)"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
        render_pass.set_bind_group(1, &self.default_texture_bind_group, &[]);
        render_pass.set_bind_group(2, &self.shadow_bind_group, &[]);
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh.num_indices, 0, offset..(offset + instance_count as u32));
    }

    /// Render viewmodel (gun) with no depth test so it always draws on top. Uses the renderer's own rifle mesh so this pass can never draw a bug mesh.
    pub fn render_viewmodel(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        instances: &[InstanceData],
    ) {
        if instances.is_empty() {
            return;
        }

        // Allocate a unique region in the instance buffer for this draw call
        let offset = self.frame_instance_offset;
        let remaining = self.max_instances.saturating_sub(offset) as usize;
        let instance_count = instances.len().min(remaining);
        if instance_count == 0 { return; }

        let byte_offset = (offset as usize * std::mem::size_of::<InstanceData>()) as u64;
        self.queue.write_buffer(
            &self.instance_buffer,
            byte_offset,
            bytemuck::cast_slice(&instances[..instance_count]),
        );
        self.frame_instance_offset = offset + instance_count as u32;

        let mesh = &self.viewmodel_mesh;
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Viewmodel Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.viewmodel_pipeline);
        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
        render_pass.set_bind_group(1, &self.default_texture_bind_group, &[]);
        render_pass.set_bind_group(2, &self.shadow_bind_group, &[]);
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh.num_indices, 0, offset..(offset + instance_count as u32));
    }

    /// Update terrain uniform (sun, fog, biome colors, time, planet radius, chunk_size, deformation, snow).
    /// Call before render_terrain. Deform origin is world (x,z) center of the deformation texture.
    pub fn update_terrain(
        &mut self,
        time: f32,
        sun_direction: [f32; 4],
        fog_params: [f32; 4],
        biome_colors: [[f32; 4]; 4],
        planet_radius: f32,
        chunk_size: f32,
        deform_origin_x: f32,
        deform_origin_z: f32,
        deform_enabled: bool,
        snow_enabled: bool,
    ) {
        let mut uniform = TerrainUniform::default();
        uniform.biome_params[0] = chunk_size;    // x = chunk_size (for edge blending)
        uniform.biome_params[1] = 2.0;           // y = detail_scale
        uniform.biome_params[2] = time;           // z = time
        uniform.biome_params[3] = planet_radius;  // w = planet radius for curvature
        uniform.sun_direction = sun_direction;
        uniform.fog_params = fog_params;
        uniform.biome_colors = biome_colors;
        uniform.deform_params[0] = deform_origin_x;
        uniform.deform_params[1] = deform_origin_z;
        uniform.deform_params[2] = DEFORM_HALF_SIZE;
        uniform.deform_params[3] = if deform_enabled { 1.0 } else { 0.0 };
        uniform.snow_params[0] = if snow_enabled { 1.0 } else { 0.0 };
        self.queue
            .write_buffer(&self.terrain_buffer, 0, bytemuck::cast_slice(&[uniform]));
    }

    /// Upload terrain deformation heightfield (256x256 f32s). Call when on snow/sand with stamped data.
    pub fn upload_terrain_deformation(&mut self, data: &[f32]) {
        debug_assert_eq!(data.len(), (DEFORM_TEXTURE_SIZE * DEFORM_TEXTURE_SIZE) as usize);
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.deform_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(data),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(DEFORM_TEXTURE_SIZE * 4),
                rows_per_image: Some(DEFORM_TEXTURE_SIZE),
            },
            wgpu::Extent3d {
                width: DEFORM_TEXTURE_SIZE,
                height: DEFORM_TEXTURE_SIZE,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Upload snow accumulation heightfield (256x256 f32s). Call when snow is enabled.
    pub fn upload_terrain_snow(&mut self, data: &[f32]) {
        debug_assert_eq!(data.len(), (DEFORM_TEXTURE_SIZE * DEFORM_TEXTURE_SIZE) as usize);
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.snow_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(data),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(DEFORM_TEXTURE_SIZE * 4),
                rows_per_image: Some(DEFORM_TEXTURE_SIZE),
            },
            wgpu::Extent3d {
                width: DEFORM_TEXTURE_SIZE,
                height: DEFORM_TEXTURE_SIZE,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Update sky uniform for dynamic time of day and weather. Call before render_sky.
    /// `time_of_day`: 0 = dawn, 0.25 = noon, 0.5 = dusk, 0.75 = midnight.
    /// `sun_dir`: pre-computed sun direction (from game sky_weather_params).
    /// `cloud_density`, `dust_amount`: weather-driven smooth values.
    /// `planet_type`: 0 = normal, 1 = toxic (second sun).
    /// `planet_radius`: conceptual planet sphere radius (for space rendering).
    /// `atmo_height`: atmosphere thickness above surface.
    /// `planet_surface_color`: average biome color (orbit, drop, surface — single source).
    /// `atmosphere_color`: planet atmosphere tint for zenith/horizon (same in orbit, drop, surface).
    pub fn update_sky(
        &mut self,
        time_of_day: f32,
        sun_dir: [f32; 3],
        cloud_density: f32,
        dust_amount: f32,
        planet_type: f32,
        planet_radius: f32,
        atmo_height: f32,
        planet_surface_color: [f32; 3],
        atmosphere_color: [f32; 3],
    ) {
        let t = time_of_day;

        // ---- Time-of-day sky color palette ----
        // Dawn  (t ≈ 0.00): warm orange/pink horizon, deep blue-purple zenith
        // Noon  (t ≈ 0.25): bright blue zenith, light blue horizon
        // Dusk  (t ≈ 0.50): rich orange/red horizon, darkening blue zenith
        // Night (t ≈ 0.75): very dark blue/black

        // Helper: smoothstep-based blend between 4 time-of-day keyframes
        fn lerp3(a: [f32; 3], b: [f32; 3], f: f32) -> [f32; 3] {
            [
                a[0] + (b[0] - a[0]) * f,
                a[1] + (b[1] - a[1]) * f,
                a[2] + (b[2] - a[2]) * f,
            ]
        }

        // Keyframe colors: [zenith, horizon, ground]
        // Helldivers 2 / SST Extermination style: saturated, cinematic, dramatic
        let dawn_zenith   = [0.12, 0.08, 0.35]; // deep blue-purple
        let dawn_horizon  = [0.95, 0.50, 0.20]; // intense orange-pink
        let dawn_ground   = [0.30, 0.22, 0.12];

        let noon_zenith   = [0.15, 0.35, 0.85]; // rich blue sky
        let noon_horizon  = [0.50, 0.60, 0.90]; // bright horizon band
        let noon_ground   = [0.45, 0.42, 0.35];

        let dusk_zenith   = [0.12, 0.06, 0.22]; // deep purple
        let dusk_horizon  = [0.98, 0.30, 0.08]; // intense red-orange
        let dusk_ground   = [0.25, 0.12, 0.05];

        // Night: properly dark so night feels like night
        let night_zenith  = [0.008, 0.006, 0.018];
        let night_horizon = [0.018, 0.014, 0.025];
        let night_ground  = [0.012, 0.010, 0.015];

        // Blend between keyframes based on time_of_day
        // t: 0.0=dawn, 0.25=noon, 0.50=dusk, 0.75=night
        let (zenith, horizon, ground) = if t < 0.25 {
            let f = t / 0.25; // 0..1 from dawn to noon
            let f = f * f * (3.0 - 2.0 * f); // smoothstep
            (lerp3(dawn_zenith, noon_zenith, f),
             lerp3(dawn_horizon, noon_horizon, f),
             lerp3(dawn_ground, noon_ground, f))
        } else if t < 0.50 {
            let f = (t - 0.25) / 0.25; // 0..1 from noon to dusk
            let f = f * f * (3.0 - 2.0 * f);
            (lerp3(noon_zenith, dusk_zenith, f),
             lerp3(noon_horizon, dusk_horizon, f),
             lerp3(noon_ground, dusk_ground, f))
        } else if t < 0.75 {
            let f = (t - 0.50) / 0.25; // 0..1 from dusk to night
            let f = f * f * (3.0 - 2.0 * f);
            (lerp3(dusk_zenith, night_zenith, f),
             lerp3(dusk_horizon, night_horizon, f),
             lerp3(dusk_ground, night_ground, f))
        } else {
            let f = (t - 0.75) / 0.25; // 0..1 from night to dawn
            let f = f * f * (3.0 - 2.0 * f);
            (lerp3(night_zenith, dawn_zenith, f),
             lerp3(night_horizon, dawn_horizon, f),
             lerp3(night_ground, dawn_ground, f))
        };

        // Darken sky colors by cloud coverage (overcast dims the sky — Helldivers storm feel)
        let overcast_dim = 1.0 - cloud_density * 0.45;
        let zenith = [zenith[0] * overcast_dim, zenith[1] * overcast_dim, zenith[2] * overcast_dim];

        // Sun intensity: bright during day, fading at dawn/dusk, zero when sun below horizon
        let sun_elev = sun_dir[1].max(0.0);
        let sun_intensity = sun_elev.powf(0.35) * (1.0 - cloud_density * 0.7);

        // Sun color: warm at low elevation, white at high elevation
        let sun_warmth = (1.0 - sun_dir[1].max(0.0)).powf(0.5);
        let sun_r = 1.0;
        let sun_g = 0.85 + (1.0 - sun_warmth) * 0.13;
        let sun_b = 0.6 + (1.0 - sun_warmth) * 0.35;

        // Ground color: blend time-of-day keyframe with actual planet biome surface color
        let ground_final = [
            ground[0] * 0.3 + planet_surface_color[0] * 0.7,
            ground[1] * 0.3 + planet_surface_color[1] * 0.7,
            ground[2] * 0.3 + planet_surface_color[2] * 0.7,
        ];

        // Zenith and horizon tinted by planet atmosphere so drop/surface match orbit
        let atmo_k = 0.35;
        let zenith_tinted = [
            zenith[0] * (1.0 - atmo_k) + atmosphere_color[0] * atmo_k,
            zenith[1] * (1.0 - atmo_k) + atmosphere_color[1] * atmo_k,
            zenith[2] * (1.0 - atmo_k) + atmosphere_color[2] * atmo_k,
        ];
        let horizon_tinted = [
            horizon[0] * (1.0 - atmo_k) + atmosphere_color[0] * atmo_k,
            horizon[1] * (1.0 - atmo_k) + atmosphere_color[1] * atmo_k,
            horizon[2] * (1.0 - atmo_k) + atmosphere_color[2] * atmo_k,
        ];

        let mut u = SkyUniform::default();
        u.sun_direction = [sun_dir[0], sun_dir[1], sun_dir[2], sun_intensity];
        u.sun_color = [sun_r, sun_g, sun_b, 0.02]; // w = disk size
        u.sky_color_zenith = [zenith_tinted[0], zenith_tinted[1], zenith_tinted[2], planet_radius];  // w = planet radius
        u.sky_color_horizon = [horizon_tinted[0], horizon_tinted[1], horizon_tinted[2], atmo_height]; // w = atmo height
        u.ground_color = [ground_final[0], ground_final[1], ground_final[2], dust_amount];
        u.params[0] = time_of_day * 100.0;
        u.params[1] = cloud_density;
        u.params[2] = dust_amount;
        u.params[3] = planet_type;

        self.queue
            .write_buffer(&self.sky_buffer, 0, bytemuck::cast_slice(&[u]));
    }

    /// Render sky (fullscreen). Call first in frame after begin_frame; clears color and depth.
    /// `clear_color`: if Some(r,g,b,a), use for clear (e.g. dark space when in orbit); else default sky blue.
    pub fn render_sky(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        clear_color: Option<[f32; 4]>,
    ) {
        let [r, g, b, a] = clear_color.unwrap_or([0.2, 0.35, 0.5, 1.0]);
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Sky Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: r as f64,
                        g: g as f64,
                        b: b as f64,
                        a: a as f64,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render_pass.set_pipeline(&self.sky_pipeline);
        render_pass.set_bind_group(0, &self.sky_bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    /// Render celestial bodies (stars, planets, moons) as instanced spheres.
    /// Call after render_sky and before render_terrain.
    pub fn render_celestial(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        instances: &[CelestialBodyInstance],
    ) {
        if instances.is_empty() {
            return;
        }

        let instance_count = instances.len().min(self.celestial_max_instances as usize);
        self.queue.write_buffer(
            &self.celestial_instance_buffer,
            0,
            bytemuck::cast_slice(&instances[..instance_count]),
        );

        let mesh = &self.celestial_sphere_mesh;
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Celestial Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.celestial_pipeline);
        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.celestial_instance_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh.num_indices, 0, 0..instance_count as u32);
    }

    /// Render terrain mesh with triplanar procedural shader. Use after render_sky (loads existing color/depth).
    pub fn render_terrain(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        mesh: &Mesh,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Terrain Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.terrain_pipeline);
        render_pass.set_bind_group(0, &self.terrain_bind_group, &[]);
        render_pass.set_bind_group(1, &self.shadow_bind_group, &[]);
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
    }

    /// Render water surface mesh (lakes, streams, ocean). Call after render_terrain.
    pub fn render_water(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        mesh: &Mesh,
    ) {
        if mesh.num_indices == 0 {
            return;
        }
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Water Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.water_pipeline);
        render_pass.set_bind_group(0, &self.terrain_bind_group, &[]);
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
    }

    /// Render screen-space text overlay. Call as the very last pass before end_frame.
    /// Takes pre-built overlay vertices and indices from an `OverlayTextBuilder`.
    pub fn render_overlay(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        vertices: &[OverlayVertex],
        indices: &[u32],
    ) {
        if vertices.is_empty() || indices.is_empty() {
            return;
        }

        let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Overlay Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Overlay Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Overlay Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.overlay_pipeline);
        render_pass.set_bind_group(0, &self.overlay_bind_group, &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
    }

    /// End frame and present.
    pub fn end_frame(&self, output: wgpu::SurfaceTexture, encoder: wgpu::CommandEncoder) {
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    /// Get window dimensions.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// Access the device for mesh creation.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Access texture bind group layout for custom materials.
    pub fn texture_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.texture_bind_group_layout
    }

    /// Access the depth texture view for additional render passes.
    pub fn depth_texture_view(&self) -> &wgpu::TextureView {
        &self.depth_texture.view
    }
}
