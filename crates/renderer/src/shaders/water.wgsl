// Procedural water shader: lakes, streams, ocean
// Uses same bind group layout as terrain (camera + terrain uniform for sun/fog)

struct CameraUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    position: vec4<f32>,
    planet_radius: f32,
    _pad: vec3<f32>,
};

struct TerrainUniform {
    biome_colors: array<vec4<f32>, 4>,
    biome_params: vec4<f32>,
    sun_direction: vec4<f32>,
    fog_params: vec4<f32>,
    deform_params: vec4<f32>,
    snow_params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(0) @binding(1)
var<uniform> terrain: TerrainUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) water_color: vec4<f32>,
};

// Animated wave displacement (Gerstner-style: position + normal)
fn water_wave(pos: vec3<f32>, time: f32) -> vec3<f32> {
    let freq1 = 0.15;
    let freq2 = 0.22;
    let amp1 = 0.12;
    let amp2 = 0.08;
    let speed1 = 1.2;
    let speed2 = 0.9;
    let wave1 = sin(pos.x * freq1 + time * speed1) * cos(pos.z * freq1 * 0.7 + time * speed1 * 0.5) * amp1;
    let wave2 = sin(pos.z * freq2 + time * speed2) * cos(pos.x * freq2 * 0.8 + time * speed2 * 0.6) * amp2;
    return vec3<f32>(0.0, wave1 + wave2, 0.0);
}

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    let time = terrain.biome_params.z;
    let wave_offset = water_wave(vertex.position, time);
    var world_pos = vertex.position + wave_offset;

    // Planetary curvature: same as terrain so water conforms to planet surface
    let planet_radius = terrain.biome_params.w;
    if (planet_radius > 0.0) {
        let dx = world_pos.x - camera.position.x;
        let dz = world_pos.z - camera.position.z;
        let horiz_dist_sq = dx * dx + dz * dz;
        let curvature_drop = horiz_dist_sq / (2.0 * planet_radius);
        world_pos.y -= curvature_drop;
    }

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_position = world_pos;
    // Approximate wave normal from gradient (for lighting)
    let eps = 0.1;
    let ddx = water_wave(vertex.position + vec3<f32>(eps, 0.0, 0.0), time) - wave_offset;
    let ddz = water_wave(vertex.position + vec3<f32>(0.0, 0.0, eps), time) - wave_offset;
    let wave_normal = normalize(vec3<f32>(-ddx.y / eps, 1.0, -ddz.y / eps));
    out.world_normal = wave_normal;
    out.uv = vertex.uv;
    out.water_color = vertex.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let time = terrain.biome_params.z;

    // Water type from color.a: 0=ocean, 1=lake, 2=stream
    let water_type = in.water_color.a;

    // Base water color (deep blue-green)
    var base_color = in.water_color.rgb;
    if (water_type < 0.5) {
        base_color = vec3<f32>(0.04, 0.12, 0.28);  // Ocean: deeper
    } else if (water_type < 1.5) {
        base_color = vec3<f32>(0.06, 0.18, 0.32);   // Lake
    } else {
        base_color = vec3<f32>(0.10, 0.24, 0.38);  // Stream: lighter
    }

    // Subtle wave/ripple pattern (additive only — negative values caused black moving shadows)
    let ripple_raw = sin(in.uv.x * 40.0 + time * 0.5) * sin(in.uv.y * 40.0 + time * 0.3);
    let ripple = (ripple_raw * 0.5 + 0.5) * 0.02; // remap [-1,1] -> [0, 0.02]
    base_color += vec3<f32>(ripple);

    // Lighting
    let light_dir = normalize(terrain.sun_direction.xyz);
    let sun_intensity = terrain.sun_direction.w;
    let view_dir = normalize(camera.position.xyz - in.world_position);

    let n_dot_l = max(dot(n, light_dir), 0.0);
    let half_lambert = n_dot_l * 0.6 + 0.4;

    let day_factor = clamp(light_dir.y * 3.0, 0.0, 1.0);
    let ambient = mix(
        vec3<f32>(0.02, 0.03, 0.05),
        vec3<f32>(0.08, 0.12, 0.18),
        day_factor
    );

    var color = base_color * (ambient + vec3<f32>(0.4, 0.5, 0.6) * half_lambert * sun_intensity);

    // Specular highlight (water reflection)
    let h = normalize(light_dir + view_dir);
    let n_dot_h = max(dot(n, h), 0.0);
    let spec = pow(n_dot_h, 64.0) * sun_intensity * 0.4;
    color += vec3<f32>(0.7, 0.8, 0.9) * spec;

    // Fresnel: brighter at grazing angles
    let fresnel = pow(1.0 - max(dot(n, view_dir), 0.0), 4.0);
    color = mix(color, vec3<f32>(0.15, 0.25, 0.4), fresnel * 0.5);

    // Fog
    let dist = length(camera.position.xyz - in.world_position);
    let fog_start = terrain.fog_params.z;
    let fog_end = terrain.fog_params.w;
    let fog_amount = clamp((dist - fog_start) / max(fog_end - fog_start, 1.0), 0.0, 0.85);
    let fog_color = mix(
        vec3<f32>(0.02, 0.03, 0.05),
        vec3<f32>(0.15, 0.18, 0.22),
        day_factor
    );
    color = mix(color, fog_color, fog_amount);

    // Tone mapping
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, 0.95); // Slightly more opaque so water is always visible
}
