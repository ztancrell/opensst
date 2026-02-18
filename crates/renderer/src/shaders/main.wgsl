// Main shader for Bug Horde Engine
// Supports instanced rendering with PBR-lite lighting

struct CameraUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    position: vec4<f32>,
    planet_radius: f32,
    _pad: vec3<f32>,  // vec3 has size 12, alignment 16 — matches Rust _pad[7] for 240-byte struct
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0)
var t_albedo: texture_2d<f32>;
@group(1) @binding(1)
var s_albedo: sampler;

struct ShadowUniform {
    light_view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    planet_radius: f32,
    _pad: vec3<f32>,
}

@group(2) @binding(0)
var<uniform> shadow: ShadowUniform;

@group(2) @binding(1)
var shadow_tex: texture_depth_2d;

@group(2) @binding(2)
var shadow_sampler: sampler_comparison;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct InstanceInput {
    @location(3) model_matrix_0: vec4<f32>,
    @location(4) model_matrix_1: vec4<f32>,
    @location(5) model_matrix_2: vec4<f32>,
    @location(6) model_matrix_3: vec4<f32>,
    @location(7) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

@vertex
fn vs_main(
    vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );

    // Extract rotation/scale for normal transformation (ignoring translation)
    let normal_matrix = mat3x3<f32>(
        model_matrix[0].xyz,
        model_matrix[1].xyz,
        model_matrix[2].xyz,
    );

    var out: VertexOutput;
    var world_pos = (model_matrix * vec4<f32>(vertex.position, 1.0)).xyz;
    // Planetary curvature: match terrain shader so objects sit on curved surface
    let planet_radius = camera.planet_radius;
    if (planet_radius > 0.0) {
        let dx = world_pos.x - camera.position.x;
        let dz = world_pos.z - camera.position.z;
        let horiz_dist_sq = dx * dx + dz * dz;
        let curvature_drop = horiz_dist_sq / (2.0 * planet_radius);
        world_pos.y -= curvature_drop;
    }
    out.world_position = world_pos;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_normal = normalize(normal_matrix * vertex.normal);
    out.uv = vertex.uv;
    out.color = instance.color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample texture
    let albedo = textureSample(t_albedo, s_albedo, in.uv);

    // Use camera Y to determine if we're on a planet surface or in space.
    // On a planet surface, reduce lighting at night (camera at low altitude).
    // The terrain shader has its own dynamic sun direction; this shader approximates it.
    let altitude = camera.position.y;

    // Emissive objects (color alpha > 0.9 AND very bright channel) bypass normal lighting.
    // This lets embers, fireflies, tracers, muzzle flashes glow properly.
    let max_channel = max(max(in.color.r, in.color.g), in.color.b);
    let is_emissive = (max_channel > 1.5);

    if (is_emissive) {
        // Emissive particles: just output the color directly, no lighting
        let final_alpha = albedo.a * in.color.a;
        if (final_alpha < 0.15) { discard; }
        let emit_color = in.color.rgb * albedo.rgb;
        return vec4<f32>(clamp(emit_color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
    }

    // MIRO + Starship Troopers: cel/toon lighting (stylized, colorful)
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let light_color = vec3<f32>(1.0, 0.92, 0.85); // Slightly warmer
    let ambient = vec3<f32>(0.12, 0.12, 0.18); // Brighter ambient for stylized look

    // Cel shading: 3-step diffuse
    let ndotl = max(dot(in.world_normal, light_dir), 0.0);
    let half_lambert = ndotl * 0.7 + 0.3;
    let toon_lambert = floor(half_lambert * 3.0 + 0.5) / 3.0;
    var diffuse = light_color * toon_lambert * 0.85;

    // Shadow map: sample sun shadow
    let light_clip = shadow.light_view_proj * vec4<f32>(in.world_position, 1.0);
    let light_ndc = light_clip.xyz / light_clip.w;
    let shadow_uv = light_ndc.xy * 0.5 + 0.5;
    let depth_compare = light_ndc.z * 0.5 + 0.5 + 0.002;
    let in_bounds = all(shadow_uv >= vec2<f32>(0.0)) && all(shadow_uv <= vec2<f32>(1.0));
    let shadow_factor = select(1.0, textureSampleCompare(shadow_tex, shadow_sampler, shadow_uv, depth_compare), in_bounds);
    diffuse *= shadow_factor;

    // View direction for specular and rim
    let view_dir = normalize(camera.position.xyz - in.world_position);
    let half_dir = normalize(light_dir + view_dir);
    let spec_power = 32.0; // Softer spec for stylized
    let spec = pow(max(dot(in.world_normal, half_dir), 0.0), spec_power) * 0.06;

    // MIRO-style bold rim: colorful edge glow
    let rim = pow(1.0 - max(dot(in.world_normal, view_dir), 0.0), 2.5);
    let rim_color = vec3<f32>(0.22, 0.28, 0.42) * rim;
    let rim_spec = pow(max(dot(in.world_normal, half_dir), 0.0), 12.0) * rim * 0.15;

    // Combine lighting with color
    let base_color = albedo.rgb * in.color.rgb;
    let lit_color = base_color * (ambient + diffuse + rim_color) + vec3<f32>(spec + rim_spec) * base_color;

    // Simple distance fog (MIRO-style: slightly more saturated)
    let fog_color = vec3<f32>(0.38, 0.34, 0.32);
    let fog_start = 60.0;
    let fog_end = 400.0;
    let dist = length(camera.position.xyz - in.world_position);
    let fog_factor = clamp((dist - fog_start) / (fog_end - fog_start), 0.0, 0.85);

    let final_color = mix(lit_color, fog_color, fog_factor);
    let final_alpha = albedo.a * in.color.a;

    // Discard fragments with low alpha (particles that would be transparent)
    if (final_alpha < 0.15) {
        discard;
    }

    return vec4<f32>(final_color, 1.0);
}
