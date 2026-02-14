// High-quality celestial body shader: stars with corona, planets with surface detail,
// atmosphere rim glow, proper ring systems, and moons.
// Uses instanced rendering with per-instance data.

struct CameraUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    position: vec4<f32>,
    planet_radius: f32,
    _pad: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
};

struct InstanceInput {
    @location(3) inst_position: vec3<f32>,
    @location(4) inst_radius: f32,
    @location(5) inst_color: vec4<f32>,
    @location(6) inst_star_dir: vec4<f32>,   // xyz = direction to star, w = has_atmosphere
    @location(7) inst_atmo_color: vec4<f32>, // rgb = atmosphere color, w = ring_system
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) star_direction: vec4<f32>,
    @location(3) atmo_color: vec4<f32>,
    @location(4) view_dir: vec3<f32>,
    @location(5) local_pos: vec3<f32>,
};

const PI: f32 = 3.14159265359;

// ============================================================================
// NOISE FUNCTIONS
// ============================================================================

fn hash31(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash33(p: vec3<f32>) -> vec3<f32> {
    var p3 = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yxz + 33.33);
    return fract((p3.xxy + p3.yxx) * p3.zyx);
}

fn noise3d(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    return mix(
        mix(
            mix(hash31(i), hash31(i + vec3<f32>(1.0, 0.0, 0.0)), u.x),
            mix(hash31(i + vec3<f32>(0.0, 1.0, 0.0)), hash31(i + vec3<f32>(1.0, 1.0, 0.0)), u.x),
            u.y
        ),
        mix(
            mix(hash31(i + vec3<f32>(0.0, 0.0, 1.0)), hash31(i + vec3<f32>(1.0, 0.0, 1.0)), u.x),
            mix(hash31(i + vec3<f32>(0.0, 1.0, 1.0)), hash31(i + vec3<f32>(1.0, 1.0, 1.0)), u.x),
            u.y
        ),
        u.z
    );
}

fn fbm(p: vec3<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var pp = p;
    for (var i = 0; i < octaves; i++) {
        value += amplitude * noise3d(pp);
        pp *= 2.03;
        amplitude *= 0.49;
    }
    return value;
}

// Voronoi for surface detail
fn voronoi_dist(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    var min_dist = 10.0;

    for (var z = -1; z <= 1; z++) {
        for (var y = -1; y <= 1; y++) {
            for (var x = -1; x <= 1; x++) {
                let neighbor = vec3<f32>(f32(x), f32(y), f32(z));
                let point = hash33(i + neighbor);
                let diff = neighbor + point - f;
                let d = dot(diff, diff);
                min_dist = min(min_dist, d);
            }
        }
    }
    return sqrt(min_dist);
}

// ============================================================================
// VERTEX SHADER
// ============================================================================

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = vertex.position * instance.inst_radius + instance.inst_position;

    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_normal = vertex.normal;
    out.color = instance.inst_color;
    out.star_direction = instance.inst_star_dir;
    out.atmo_color = instance.inst_atmo_color;
    out.view_dir = normalize(world_pos - camera.position.xyz);
    out.local_pos = vertex.position; // unit sphere position

    return out;
}

// ============================================================================
// FRAGMENT SHADER
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.world_normal);
    let is_emissive = in.color.w > 0.5;
    let view_dot = abs(dot(normal, -in.view_dir));

    if is_emissive {
        // ===== STAR =====
        let base_color = in.color.rgb;

        // Limb darkening (center brighter than edges -- like real stars)
        let limb = pow(view_dot, 0.4);

        // Core: white-hot center
        let core_factor = pow(view_dot, 2.5);
        let core_color = vec3<f32>(1.0, 1.0, 0.98);

        // Surface: convection cells (granulation)
        let granulation = fbm(in.local_pos * 12.0, 3) * 0.1;
        let surface_variation = base_color * (0.95 + granulation);

        // Sunspots (dark patches)
        let sunspot = smoothstep(0.72, 0.75, fbm(in.local_pos * 5.0, 3));
        let spot_darken = 1.0 - sunspot * 0.4;

        // Final star surface
        var star_color = mix(surface_variation, core_color, core_factor * 0.6) * limb * spot_darken;

        // Chromosphere / corona glow at edges
        let edge_glow = pow(1.0 - view_dot, 2.5);
        let corona_color = base_color * 1.5 + vec3<f32>(0.2, 0.1, 0.0);
        star_color += corona_color * edge_glow * 0.6;

        // Prominences (bright arcs at edge -- noise-based)
        let prom_noise = fbm(in.local_pos * 8.0 + vec3<f32>(0.0, 0.0, 0.0), 3);
        let prominence = smoothstep(0.55, 0.65, prom_noise) * edge_glow;
        star_color += base_color * 2.0 * prominence * 0.3;

        // HDR intensity
        star_color *= 2.5;

        return vec4<f32>(star_color, 1.0);

    } else {
        // ===== PLANET / MOON =====
        let star_dir = normalize(in.star_direction.xyz);
        let has_atmosphere = in.star_direction.w > 0.5;
        let has_rings = in.atmo_color.w > 0.5;
        let atmo_color = in.atmo_color.rgb;

        // ---- Surface detail ----
        // Continent/terrain noise
        let surface_pos = in.local_pos;
        let continent_noise = fbm(surface_pos * 4.0, 4);
        let detail_noise = fbm(surface_pos * 12.0, 3) * 0.15;
        let surface_value = continent_noise + detail_noise;

        // Surface color variation
        let base = in.color.rgb;
        let highland = base * 1.15;
        let lowland = base * 0.7;
        let surface_color = mix(lowland, highland, smoothstep(0.35, 0.65, surface_value));

        // Ice caps (polar regions)
        let polar_factor = abs(surface_pos.y);
        let ice_cap = smoothstep(0.7, 0.85, polar_factor);
        let ice_color = vec3<f32>(0.88, 0.90, 0.95);
        let colored_surface = mix(surface_color, ice_color, ice_cap * 0.6);

        // Cloud layer
        let cloud_noise = fbm(surface_pos * 6.0, 3);
        let clouds = smoothstep(0.48, 0.6, cloud_noise) * 0.35;
        let cloud_color = vec3<f32>(0.9, 0.92, 0.95);

        // Combine surface + clouds
        var planet_surface = mix(colored_surface, cloud_color, clouds);

        // ---- Lighting ----
        let ndl = max(dot(normal, star_dir), 0.0);
        let ambient = 0.06;

        // Half-lambert for softer shading
        let half_lambert = ndl * 0.7 + 0.3;
        let diffuse = half_lambert * 0.9 + ambient;

        var planet_color = planet_surface * diffuse;

        // Specular (subtle glint on oceans/ice)
        let h = normalize(star_dir + (-in.view_dir));
        let spec = pow(max(dot(normal, h), 0.0), 48.0) * 0.15;
        let ocean_mask = smoothstep(0.5, 0.4, surface_value) * (1.0 - ice_cap);
        planet_color += vec3<f32>(1.0, 0.98, 0.9) * spec * (ocean_mask * 0.5 + ice_cap * 0.3);

        // ---- Atmosphere rim glow (Fresnel) ----
        if has_atmosphere {
            let rim = pow(1.0 - view_dot, 3.5);

            // Atmosphere is bright on the lit side, subtle on dark side
            let lit_rim = rim * (ndl * 0.5 + 0.4);
            let dark_rim = rim * 0.08;
            let atmo_intensity = mix(dark_rim, lit_rim, smoothstep(-0.1, 0.2, ndl));

            // Scatter light color shift
            let scatter_color = mix(atmo_color, vec3<f32>(1.0, 0.6, 0.3), pow(max(dot(-in.view_dir, star_dir), 0.0), 3.0) * 0.3);
            planet_color = mix(planet_color, scatter_color, atmo_intensity);
        }

        // ---- Ring system ----
        if has_rings {
            // Ring plane: y = 0 in local space
            let ring_inner = 1.2; // inner radius (beyond sphere surface)
            let ring_outer = 2.0; // outer radius

            // We're rendering on the sphere surface, so rings visible as projected band
            let y = in.local_pos.y;
            let ring_r = length(vec2<f32>(in.local_pos.x, in.local_pos.z));

            // Check if this fragment is near the ring plane intersection
            let equator_dist = abs(y);
            if (equator_dist < 0.15 && ring_r > 0.85) {
                // Ring band visible on the sphere's edge
                let ring_alpha = smoothstep(0.15, 0.03, equator_dist);

                // Ring detail: concentric bands with gaps
                let ring_noise = fbm(vec3<f32>(ring_r * 20.0, 0.0, ring_r * 5.0), 3);
                let ring_bands = sin(ring_r * 60.0) * 0.5 + 0.5;
                let ring_opacity = ring_bands * smoothstep(0.0, 0.1, ring_noise) * 0.7;

                // Ring color: slightly different from planet
                let ring_base = base * 0.8 + vec3<f32>(0.15, 0.12, 0.08);
                let ring_lit = ring_base * (ndl * 0.6 + 0.4);

                // Ring shadow on planet
                let shadow_factor = smoothstep(0.05, 0.0, equator_dist) * ring_opacity * 0.3;
                planet_color *= (1.0 - shadow_factor);

                // Blend ring over planet at edges
                planet_color = mix(planet_color, ring_lit, ring_alpha * ring_opacity);
            }
        }

        // ---- Night side: bioluminescence / volcanic glow ----
        let night_side = smoothstep(0.05, -0.15, ndl);
        if (night_side > 0.0) {
            let bio_noise = fbm(surface_pos * 15.0, 3);
            let city_lights = smoothstep(0.68, 0.72, bio_noise) * night_side;

            // Mix of orange (volcanic) and green (bioluminescent)
            let glow_color = mix(
                vec3<f32>(0.8, 0.35, 0.05), // volcanic
                vec3<f32>(0.1, 0.5, 0.2),   // bio
                smoothstep(0.5, 0.7, hash31(floor(surface_pos * 10.0)))
            );
            planet_color += glow_color * city_lights * 0.25;
        }

        // ---- Terminator (day/night boundary softening) ----
        let terminator = smoothstep(-0.08, 0.12, ndl);
        planet_color *= terminator + (1.0 - terminator) * 0.12;

        return vec4<f32>(planet_color, 1.0);
    }
}
