// High-quality terrain shader with PBR-style triplanar texturing
// Starship Troopers aesthetic: harsh alien worlds, arid wastelands, volcanic hellscapes
// Features: multi-octave noise, erosion channels, micro-detail, per-biome materials

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
    biome_params: vec4<f32>,           // x = blend_sharpness, y = detail_scale, z = time, w = planet_radius
    sun_direction: vec4<f32>,
    fog_params: vec4<f32>,            // x = density, y = height_falloff, z = start, w = end
    deform_params: vec4<f32>,         // x = origin_x, y = origin_z, z = half_size, w = enabled
    snow_params: vec4<f32>,           // x = snow_enabled, yzw unused
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(0) @binding(1)
var<uniform> terrain: TerrainUniform;

@group(0) @binding(2)
var deform_tex: texture_2d<f32>;

@group(0) @binding(3)
var deform_sampler: sampler;  // Unused in vertex; textureLoad used for vertex-stage compatibility

@group(0) @binding(4)
var snow_tex: texture_2d<f32>;

@group(0) @binding(5)
var snow_sampler: sampler;

struct ShadowUniform {
    light_view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    planet_radius: f32,
    _pad: vec3<f32>,
}

@group(1) @binding(0)
var<uniform> shadow: ShadowUniform;

@group(1) @binding(1)
var shadow_tex: texture_depth_2d;

@group(1) @binding(2)
var shadow_sampler: sampler_comparison;

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
    @location(3) biome_color: vec4<f32>,
};

const PI: f32 = 3.14159265359;

// ============================================================================
// HIGH-QUALITY NOISE FUNCTIONS
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

fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

// Gradient noise (better quality than value noise)
fn grad_noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    // Quintic interpolation for smoother derivatives
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);

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

// Domain-warped FBM for more organic patterns
fn fbm(p: vec3<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var pp = p;

    for (var i = 0; i < octaves; i++) {
        value += amplitude * grad_noise(pp * frequency);
        frequency *= 2.07; // slight detuning to reduce repetition
        amplitude *= 0.48;
    }
    return value;
}

// Domain-warped FBM (warps the input coords for organic look)
fn warp_fbm(p: vec3<f32>, octaves: i32, warp_strength: f32) -> f32 {
    let q = vec3<f32>(
        fbm(p, 3),
        fbm(p + vec3<f32>(5.2, 1.3, 7.8), 3),
        fbm(p + vec3<f32>(2.6, 8.1, 3.4), 3)
    );
    return fbm(p + q * warp_strength, octaves);
}

// Voronoi-based crack/cell patterns
fn voronoi(p: vec3<f32>) -> vec2<f32> {
    let i = floor(p);
    let f = fract(p);

    var min_dist = 10.0;
    var second_dist = 10.0;

    for (var z = -1; z <= 1; z++) {
        for (var y = -1; y <= 1; y++) {
            for (var x = -1; x <= 1; x++) {
                let neighbor = vec3<f32>(f32(x), f32(y), f32(z));
                let point = hash33(i + neighbor);
                let diff = neighbor + point - f;
                let d = dot(diff, diff);

                if (d < min_dist) {
                    second_dist = min_dist;
                    min_dist = d;
                } else if (d < second_dist) {
                    second_dist = d;
                }
            }
        }
    }
    return vec2<f32>(sqrt(min_dist), sqrt(second_dist));
}

// ============================================================================
// TRIPLANAR MAPPING
// ============================================================================

fn triplanar_weights(normal: vec3<f32>) -> vec3<f32> {
    var weights = abs(normal);
    weights = pow(weights, vec3<f32>(8.0)); // very sharp blend for clean transitions
    weights /= max(weights.x + weights.y + weights.z, 0.001);
    return weights;
}

// Parallax offset: view-dependent height displacement for depth illusion
fn parallax_offset(p: vec3<f32>, n: vec3<f32>, view_dir: vec3<f32>, height_scale: f32, time: f32) -> vec3<f32> {
    let height = fbm(p * 2.0, 3) * 0.5 + 0.5;
    // Offset in tangent plane (simplified: use view xy in world)
    let tangent = normalize(vec3<f32>(-n.z, 0.0, n.x) + vec3<f32>(0.001));
    let bitangent = cross(n, tangent);
    let view_tan = vec2<f32>(dot(view_dir, tangent), dot(view_dir, bitangent));
    let offset = view_tan * height * height_scale;
    return vec3<f32>(offset.x, 0.0, offset.y);
}

// Sample a 3D procedural texture with triplanar projection + parallax
fn triplanar_sample(p: vec3<f32>, n: vec3<f32>, tex_fn_id: i32, scale: f32, time: f32) -> vec3<f32> {
    return triplanar_sample_with_parallax(p, n, tex_fn_id, scale, time, vec3<f32>(0.0));
}
fn triplanar_sample_with_parallax(p: vec3<f32>, n: vec3<f32>, tex_fn_id: i32, scale: f32, time: f32, view_dir: vec3<f32>) -> vec3<f32> {
    let w = triplanar_weights(n);

    // Parallax: offset sample position by view for depth illusion
    let parallax = parallax_offset(p, n, view_dir, 0.08, time);
    let pp = p + parallax;

    var col_xy: vec3<f32>;
    var col_xz: vec3<f32>;
    var col_yz: vec3<f32>;

    let p_xy = vec3<f32>(pp.x, pp.y, 0.0) * scale;
    let p_xz = vec3<f32>(pp.x, 0.0, pp.z) * scale;
    let p_yz = vec3<f32>(0.0, pp.y, pp.z) * scale;

    // Dispatch to biome-aware texture
    switch tex_fn_id {
        case 0: {
            col_xy = desert_material(p_xy, time);
            col_xz = desert_material(p_xz, time);
            col_yz = desert_material(p_yz, time);
        }
        case 1: {
            col_xy = rock_material(p_xy, time);
            col_xz = rock_material(p_xz, time);
            col_yz = rock_material(p_yz, time);
        }
        case 2: {
            col_xy = volcanic_material(p_xy, time);
            col_xz = volcanic_material(p_xz, time);
            col_yz = volcanic_material(p_yz, time);
        }
        case 3: {
            col_xy = organic_material(p_xy, time);
            col_xz = organic_material(p_xz, time);
            col_yz = organic_material(p_yz, time);
        }
        case 4: {
            col_xy = frozen_material(p_xy, time);
            col_xz = frozen_material(p_xz, time);
            col_yz = frozen_material(p_yz, time);
        }
        case 5: {
            col_xy = swamp_material(p_xy, time);
            col_xz = swamp_material(p_xz, time);
            col_yz = swamp_material(p_yz, time);
        }
        case 6: {
            col_xy = crystal_material(p_xy, time);
            col_xz = crystal_material(p_xz, time);
            col_yz = crystal_material(p_yz, time);
        }
        case 7: {
            col_xy = ash_material(p_xy, time);
            col_xz = ash_material(p_xz, time);
            col_yz = ash_material(p_yz, time);
        }
        case 8: {
            col_xy = jungle_material(p_xy, time);
            col_xz = jungle_material(p_xz, time);
            col_yz = jungle_material(p_yz, time);
        }
        case 9: {
            col_xy = wasteland_material(p_xy, time);
            col_xz = wasteland_material(p_xz, time);
            col_yz = wasteland_material(p_yz, time);
        }
        default: {
            col_xy = rock_material(p_xy, time);
            col_xz = rock_material(p_xz, time);
            col_yz = rock_material(p_yz, time);
        }
    }

    return col_xy * w.z + col_xz * w.y + col_yz * w.x;
}

// ============================================================================
// BIOME-SPECIFIC MATERIAL FUNCTIONS
// Each returns a high-quality procedural surface color
// ============================================================================

// DESERT: wind-sculpted dunes, packed sand, scattered pebbles, heat shimmer
fn desert_material(p: vec3<f32>, time: f32) -> vec3<f32> {
    // Large dune ripple pattern
    let dune_scale = p * 0.3;
    let dune_warp = fbm(dune_scale * 0.5, 3) * 2.0;
    let dune_ripple = sin(dune_scale.x * 8.0 + dune_warp) * 0.5 + 0.5;

    // Wind-eroded streaks
    let wind_streak = fbm(p * vec3<f32>(0.8, 2.0, 0.8), 4);

    // Fine grain detail
    let grain = fbm(p * 12.0, 3) * 0.15;

    // Pebble scatter (voronoi cells)
    let pebbles = voronoi(p * 4.0);
    let pebble_mask = smoothstep(0.15, 0.1, pebbles.x) * 0.3;

    // Color palette: warm desert tones with variation
    let sand_light  = vec3<f32>(0.82, 0.68, 0.48);
    let sand_mid    = vec3<f32>(0.72, 0.58, 0.40);
    let sand_dark   = vec3<f32>(0.55, 0.42, 0.30);
    let sand_orange = vec3<f32>(0.80, 0.52, 0.32);

    var color = mix(sand_mid, sand_light, dune_ripple);
    color = mix(color, sand_dark, wind_streak * 0.4);
    color = mix(color, sand_orange, smoothstep(0.4, 0.6, fbm(p * 1.5, 3)));
    color += vec3<f32>(grain);
    color = mix(color, sand_dark * 0.8, pebble_mask);

    // Subtle heat distortion / dust patina
    let dust = fbm(p * 0.8 + vec3<f32>(time * 0.01, 0.0, 0.0), 3);
    color = mix(color, vec3<f32>(0.75, 0.65, 0.50), dust * 0.1);

    return color;
}

// ROCK: layered sediment, cracks, lichen, weathering
fn rock_material(p: vec3<f32>, time: f32) -> vec3<f32> {
    // Layered sedimentary strata
    let strata = sin(p.y * 3.0 + fbm(p * 0.5, 3) * 4.0) * 0.5 + 0.5;
    let strata_sharp = pow(strata, 2.0);

    // Crack network (voronoi edges)
    let cracks = voronoi(p * 2.5);
    let crack_edge = smoothstep(0.05, 0.0, cracks.y - cracks.x); // edge detection
    let crack_fill = smoothstep(0.08, 0.04, cracks.x) * 0.5;

    // Weathering erosion channels
    let erosion = warp_fbm(p * 1.5, 5, 1.5);
    let erosion_channel = smoothstep(0.35, 0.5, erosion);

    // Surface detail: lichen, mineral deposits
    let micro_detail = fbm(p * 20.0, 3) * 0.08;
    let lichen = smoothstep(0.6, 0.65, fbm(p * 3.0, 4)) * 0.2;

    // Color palette: realistic stone
    let rock_base   = vec3<f32>(0.42, 0.38, 0.35);
    let rock_light  = vec3<f32>(0.58, 0.54, 0.48);
    let rock_dark   = vec3<f32>(0.22, 0.20, 0.18);
    let rock_warm   = vec3<f32>(0.50, 0.40, 0.32);
    let lichen_col  = vec3<f32>(0.35, 0.42, 0.28);

    var color = mix(rock_base, rock_light, strata_sharp);
    color = mix(color, rock_warm, smoothstep(0.3, 0.7, fbm(p * 0.8, 3)));
    color = mix(color, rock_dark, crack_edge * 0.8 + crack_fill);
    color = mix(color, rock_dark * 0.7, erosion_channel * 0.5);
    color += vec3<f32>(micro_detail);
    color = mix(color, lichen_col, lichen);

    return color;
}

// VOLCANIC: lava cracks, cooled obsidian, glowing fissures, ash
fn volcanic_material(p: vec3<f32>, time: f32) -> vec3<f32> {
    // Cooled basalt base
    let basalt = fbm(p * 3.0, 4);
    let basalt_pattern = smoothstep(0.3, 0.7, basalt);

    // Lava crack network
    let lava_cracks = voronoi(p * 1.8);
    let crack_width = lava_cracks.y - lava_cracks.x;
    let is_crack = smoothstep(0.08, 0.02, crack_width);

    // Flowing lava inside cracks (animated)
    let flow = fbm(p * 2.0 + vec3<f32>(0.0, -time * 0.03, 0.0), 4);
    let lava_temp = is_crack * flow;

    // Obsidian glass patches
    let obsidian = smoothstep(0.65, 0.7, fbm(p * 2.5, 3));

    // Ash layer
    let ash = fbm(p * 8.0, 3) * 0.1;

    // Color palette
    let basalt_dark  = vec3<f32>(0.12, 0.10, 0.10);
    let basalt_mid   = vec3<f32>(0.25, 0.20, 0.18);
    let obsidian_col = vec3<f32>(0.08, 0.08, 0.10);
    let lava_hot     = vec3<f32>(1.0, 0.65, 0.12);
    let lava_warm    = vec3<f32>(0.85, 0.25, 0.05);
    let lava_cool    = vec3<f32>(0.50, 0.10, 0.02);

    var color = mix(basalt_dark, basalt_mid, basalt_pattern);
    color = mix(color, obsidian_col, obsidian * 0.6);
    color += vec3<f32>(ash * 0.5);

    // Lava glow: temperature-based color
    let lava_color = mix(lava_cool, mix(lava_warm, lava_hot, flow), flow);
    // Emissive glow (HDR)
    let emission = lava_temp * lava_temp * 3.0;
    color = mix(color, lava_color, lava_temp);
    color += lava_color * emission;

    // Heat shimmer on edges of cracks
    let heat_halo = smoothstep(0.15, 0.05, crack_width) * (1.0 - is_crack);
    color += vec3<f32>(0.3, 0.08, 0.02) * heat_halo * flow;

    return color;
}

// ORGANIC / HIVE: pulsing flesh, vein networks, chitin plates, mucus
fn organic_material(p: vec3<f32>, time: f32) -> vec3<f32> {
    // Chitin plate pattern (voronoi cells)
    let plates = voronoi(p * 2.0);
    let plate_edge = smoothstep(0.06, 0.0, plates.y - plates.x);
    let plate_center = smoothstep(0.5, 0.1, plates.x);

    // Vein network (warp-distorted lines)
    let vein_pattern = warp_fbm(p * 1.5, 4, 2.0);
    let veins = smoothstep(0.42, 0.48, vein_pattern) * smoothstep(0.58, 0.52, vein_pattern);

    // Pulsing animation
    let pulse_phase = time * 1.5 + fbm(p * 0.5, 2) * 6.0;
    let pulse = sin(pulse_phase) * 0.5 + 0.5;
    let slow_pulse = sin(time * 0.5 + plates.x * 4.0) * 0.5 + 0.5;

    // Mucus / wet sheen
    let mucus = smoothstep(0.5, 0.6, fbm(p * 4.0 + vec3<f32>(time * 0.02, 0.0, 0.0), 3));

    // Fine membrane texture
    let membrane = fbm(p * 15.0, 3) * 0.06;

    // Color palette: alien hive
    let flesh_base = vec3<f32>(0.32, 0.22, 0.18);
    let flesh_dark = vec3<f32>(0.18, 0.10, 0.08);
    let vein_color = vec3<f32>(0.55, 0.20, 0.12);
    let chitin_col = vec3<f32>(0.25, 0.18, 0.15);
    let mucus_col  = vec3<f32>(0.40, 0.35, 0.22);

    var color = mix(flesh_base, flesh_dark, plate_center * 0.4);
    color = mix(color, chitin_col, plate_edge * 0.7);
    color = mix(color, vein_color, veins * (0.6 + pulse * 0.4));
    color += vec3<f32>(membrane);

    // Mucus wet sheen
    color = mix(color, mucus_col, mucus * 0.3);

    // Subtle pulsing glow in veins
    color += vein_color * veins * pulse * 0.15;

    // Organism breathing: slight brightness variation
    color *= 0.9 + slow_pulse * 0.1;

    return color;
}

// FROZEN: ice crystals, frost patterns, packed snow, glacial crevasses
fn frozen_material(p: vec3<f32>, time: f32) -> vec3<f32> {
    // Ice crystal structure (voronoi)
    let crystals = voronoi(p * 3.0);
    let crystal_edge = smoothstep(0.05, 0.0, crystals.y - crystals.x);
    let crystal_body = smoothstep(0.4, 0.1, crystals.x);

    // Frost patterns (delicate branching)
    let frost = warp_fbm(p * 4.0, 5, 1.0);
    let frost_pattern = smoothstep(0.45, 0.55, frost);

    // Snow drift
    let snow_drift = fbm(p * 0.5, 3);
    let snow_layer = smoothstep(0.3, 0.6, snow_drift);

    // Glacial blue deep inside
    let depth_factor = fbm(p * 1.5, 3);

    // Subsurface sparkle (tiny crystal facets catching light)
    let sparkle = pow(hash31(floor(p * 50.0)), 20.0) * 0.3;

    // Color palette
    let ice_surface = vec3<f32>(0.82, 0.88, 0.94);
    let ice_deep    = vec3<f32>(0.35, 0.55, 0.75);
    let snow_white  = vec3<f32>(0.92, 0.94, 0.96);
    let frost_col   = vec3<f32>(0.75, 0.82, 0.90);
    let crevasse    = vec3<f32>(0.15, 0.25, 0.40);

    var color = mix(ice_surface, ice_deep, depth_factor * 0.5);
    color = mix(color, snow_white, snow_layer * 0.7);
    color = mix(color, frost_col, frost_pattern * 0.3);
    color = mix(color, crevasse, crystal_edge * 0.5);
    color += vec3<f32>(sparkle);

    // Subtle blue shifting in shadows
    color = mix(color, ice_deep, crystal_body * 0.2);

    return color;
}

// SWAMP: murky wetland, standing water, mud, decaying matter
fn swamp_material(p: vec3<f32>, time: f32) -> vec3<f32> {
    // Mud base with moisture variation
    let mud = fbm(p * 2.0, 4);
    let wet_areas = smoothstep(0.35, 0.5, fbm(p * 0.8, 3));

    // Standing water pools (dark reflective patches)
    let pool_noise = warp_fbm(p * 1.2, 4, 1.5);
    let pools = smoothstep(0.55, 0.6, pool_noise);

    // Decomposing organic matter
    let organic_detail = fbm(p * 8.0, 3) * 0.1;
    let roots = smoothstep(0.5, 0.55, fbm(p * 5.0, 3)) * 0.2;

    // Color palette
    let mud_dark  = vec3<f32>(0.15, 0.18, 0.10);
    let mud_mid   = vec3<f32>(0.22, 0.26, 0.15);
    let mud_wet   = vec3<f32>(0.12, 0.15, 0.08);
    let water_col = vec3<f32>(0.06, 0.10, 0.05);
    let root_col  = vec3<f32>(0.20, 0.15, 0.08);

    var color = mix(mud_mid, mud_dark, mud * 0.5);
    color = mix(color, mud_wet, wet_areas * 0.6);
    color = mix(color, water_col, pools * 0.8);
    color = mix(color, root_col, roots);
    color += vec3<f32>(organic_detail);

    // Subtle surface ripple on water
    if (pools > 0.3) {
        let ripple = sin(p.x * 20.0 + time * 0.5) * sin(p.z * 20.0 + time * 0.3) * 0.02;
        color += vec3<f32>(ripple);
    }

    return color;
}

// CRYSTAL: faceted alien crystal formations, color-shifting, reflective
fn crystal_material(p: vec3<f32>, time: f32) -> vec3<f32> {
    // Crystal facet structure (voronoi)
    let facets = voronoi(p * 3.0);
    let facet_edge = smoothstep(0.04, 0.0, facets.y - facets.x);
    let facet_body = smoothstep(0.3, 0.05, facets.x);

    // Internal refraction patterns
    let internal = warp_fbm(p * 2.0, 4, 2.0);
    let refraction = smoothstep(0.3, 0.7, internal);

    // Iridescent color shift based on angle (simulated with position)
    let hue_shift = sin(p.x * 3.0 + p.z * 2.0) * 0.5 + 0.5;

    // Sparkle (micro-facet highlights)
    let sparkle = pow(hash31(floor(p * 40.0)), 15.0) * 0.4;

    // Color palette
    let crystal_purple = vec3<f32>(0.35, 0.18, 0.55);
    let crystal_blue   = vec3<f32>(0.20, 0.30, 0.60);
    let crystal_pink   = vec3<f32>(0.55, 0.20, 0.45);
    let crystal_edge   = vec3<f32>(0.70, 0.65, 0.80);

    var color = mix(crystal_purple, crystal_blue, hue_shift);
    color = mix(color, crystal_pink, refraction * 0.4);
    color = mix(color, crystal_edge, facet_edge * 0.6);
    color += vec3<f32>(sparkle);

    // Deep glow within crystals
    color += crystal_purple * facet_body * 0.15;

    return color;
}

// ASH: post-eruption grey powder, scattered ember glow, collapsed terrain
fn ash_material(p: vec3<f32>, time: f32) -> vec3<f32> {
    // Ash powder base
    let ash_base = fbm(p * 3.0, 4);
    let fine_ash = fbm(p * 15.0, 3) * 0.05;

    // Buried debris (darker patches)
    let debris = smoothstep(0.55, 0.65, fbm(p * 1.5, 3));

    // Ember spots (still glowing)
    let ember_noise = fbm(p * 4.0 + vec3<f32>(time * 0.01, 0.0, 0.0), 3);
    let embers = smoothstep(0.7, 0.75, ember_noise);
    let ember_pulse = sin(time * 2.0 + ember_noise * 10.0) * 0.3 + 0.7;

    // Wind-blown patterns
    let wind = fbm(p * vec3<f32>(0.5, 1.0, 0.5) + vec3<f32>(time * 0.005, 0.0, 0.0), 3);

    // Color palette
    let ash_light = vec3<f32>(0.42, 0.40, 0.38);
    let ash_dark  = vec3<f32>(0.25, 0.24, 0.23);
    let ash_mid   = vec3<f32>(0.33, 0.32, 0.30);
    let ember_col = vec3<f32>(0.90, 0.35, 0.08);

    var color = mix(ash_mid, ash_light, ash_base * 0.5);
    color = mix(color, ash_dark, debris * 0.5);
    color += vec3<f32>(fine_ash);
    color = mix(color, ash_dark * 0.8, wind * 0.15);

    // Ember glow (HDR)
    color = mix(color, ember_col, embers * 0.5);
    color += ember_col * embers * ember_pulse * 0.8;

    return color;
}

// JUNGLE: dense alien root networks, mossy ground, spore-laden soil
fn jungle_material(p: vec3<f32>, time: f32) -> vec3<f32> {
    // Root network pattern
    let roots = voronoi(p * 2.5);
    let root_edge = smoothstep(0.06, 0.0, roots.y - roots.x);
    let root_body = smoothstep(0.4, 0.1, roots.x);

    // Moss and lichen coverage
    let moss = fbm(p * 4.0, 4);
    let moss_layer = smoothstep(0.3, 0.6, moss);

    // Spore patches (bioluminescent)
    let spore_noise = fbm(p * 6.0, 3);
    let spores = smoothstep(0.65, 0.7, spore_noise);
    let spore_glow = sin(time * 1.0 + spore_noise * 8.0) * 0.3 + 0.7;

    // Damp soil base
    let soil = fbm(p * 8.0, 3) * 0.08;

    // Color palette
    let soil_base  = vec3<f32>(0.15, 0.22, 0.10);
    let soil_dark  = vec3<f32>(0.10, 0.12, 0.06);
    let root_col   = vec3<f32>(0.25, 0.18, 0.10);
    let moss_col   = vec3<f32>(0.12, 0.30, 0.08);
    let spore_col  = vec3<f32>(0.10, 0.50, 0.25);

    var color = mix(soil_base, soil_dark, root_body * 0.4);
    color = mix(color, root_col, root_edge * 0.6);
    color = mix(color, moss_col, moss_layer * 0.5);
    color += vec3<f32>(soil);

    // Bioluminescent spore patches
    color = mix(color, spore_col * 0.5, spores * 0.4);
    color += spore_col * spores * spore_glow * 0.15;

    return color;
}

// WASTELAND: cracked irradiated earth, chemical staining, dead flat terrain
fn wasteland_material(p: vec3<f32>, time: f32) -> vec3<f32> {
    // Cracked earth pattern (voronoi)
    let cracks = voronoi(p * 2.0);
    let crack_edge = smoothstep(0.06, 0.0, cracks.y - cracks.x);
    let plate = smoothstep(0.5, 0.15, cracks.x);

    // Chemical staining (sickly patches)
    let stain = warp_fbm(p * 1.0, 4, 1.5);
    let chemical = smoothstep(0.4, 0.6, stain);

    // Radiation glow spots
    let rad_noise = fbm(p * 3.0, 3);
    let rad_spots = smoothstep(0.68, 0.72, rad_noise);
    let rad_pulse = sin(time * 0.8 + rad_noise * 6.0) * 0.2 + 0.8;

    // Fine dust
    let dust = fbm(p * 12.0, 3) * 0.06;

    // Color palette
    let earth_base = vec3<f32>(0.48, 0.44, 0.35);
    let earth_dark = vec3<f32>(0.30, 0.28, 0.22);
    let crack_col  = vec3<f32>(0.20, 0.18, 0.14);
    let chem_col   = vec3<f32>(0.45, 0.42, 0.20);
    let rad_col    = vec3<f32>(0.40, 0.60, 0.15);

    var color = mix(earth_base, earth_dark, plate * 0.3);
    color = mix(color, crack_col, crack_edge * 0.7);
    color = mix(color, chem_col, chemical * 0.25);
    color += vec3<f32>(dust);

    // Radiation glow (subtle HDR)
    color = mix(color, rad_col * 0.5, rad_spots * 0.3);
    color += rad_col * rad_spots * rad_pulse * 0.1;

    return color;
}

// ============================================================================
// VERTEX SHADER
// ============================================================================

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    var pos = vertex.position;

    // Planetary curvature: drop = d^2 / (2R)
    let planet_radius = terrain.biome_params.w;
    if (planet_radius > 0.0) {
        let dx = pos.x - camera.position.x;
        let dz = pos.z - camera.position.z;
        let horiz_dist_sq = dx * dx + dz * dz;
        let curvature_drop = horiz_dist_sq / (2.0 * planet_radius);
        pos.y -= curvature_drop;
    }

    // Snow accumulation (weather-driven): knee-deep snow, same region as deform.
    var world_normal = vertex.normal;
    let snow_enabled = terrain.snow_params.x > 0.5;
    if (snow_enabled) {
        let origin_x = terrain.deform_params.x;
        let origin_z = terrain.deform_params.y;
        let half_size = terrain.deform_params.z;
        let u = (pos.x - origin_x) / (2.0 * half_size) + 0.5;
        let v = (pos.z - origin_z) / (2.0 * half_size) + 0.5;
        let tex_size = 256;
        let tex_size_f = f32(tex_size);
        let u_clamped = clamp(u, 0.0, 1.0);
        let v_clamped = clamp(v, 0.0, 1.0);
        let px = clamp(i32(floor(u_clamped * tex_size_f)), 0, tex_size - 1);
        let py = clamp(i32(floor(v_clamped * tex_size_f)), 0, tex_size - 1);
        let fx = fract(u_clamped * tex_size_f);
        let fy = fract(v_clamped * tex_size_f);
        let s00 = textureLoad(snow_tex, vec2<i32>(px, py), 0).r;
        let s10 = textureLoad(snow_tex, vec2<i32>(min(px + 1, tex_size - 1), py), 0).r;
        let s01 = textureLoad(snow_tex, vec2<i32>(px, min(py + 1, tex_size - 1)), 0).r;
        let s11 = textureLoad(snow_tex, vec2<i32>(min(px + 1, tex_size - 1), min(py + 1, tex_size - 1)), 0).r;
        let snow_val = mix(mix(s00, s10, fx), mix(s01, s11, fx), fy);
        let edge = 0.06;
        let fade_u = smoothstep(0.0, edge, u) * smoothstep(0.0, edge, 1.0 - u);
        let fade_v = smoothstep(0.0, edge, v) * smoothstep(0.0, edge, 1.0 - v);
        let snow_fade = fade_u * fade_v;
        pos.y += snow_val * snow_fade;
    }

    // Terrain deformation: footprints/trails in snow and sand (Helldivers 2 / Dune style).
    // Bilinear sampling + recomputed normals so depressions look curved, not flat.
    let deform_enabled = terrain.deform_params.w > 0.5;
    if (deform_enabled) {
        let origin_x = terrain.deform_params.x;
        let origin_z = terrain.deform_params.y;
        let half_size = terrain.deform_params.z;
        let u = (pos.x - origin_x) / (2.0 * half_size) + 0.5;
        let v = (pos.z - origin_z) / (2.0 * half_size) + 0.5;
        let tex_size = 256;
        let tex_size_f = f32(tex_size);
        let u_clamped = clamp(u, 0.0, 1.0);
        let v_clamped = clamp(v, 0.0, 1.0);
        let px = clamp(i32(floor(u_clamped * tex_size_f)), 0, tex_size - 1);
        let py = clamp(i32(floor(v_clamped * tex_size_f)), 0, tex_size - 1);
        let fx = fract(u_clamped * tex_size_f);
        let fy = fract(v_clamped * tex_size_f);

        // Bilinear sample for displacement (smoother depressions)
        let o00 = textureLoad(deform_tex, vec2<i32>(px, py), 0).r;
        let o10 = textureLoad(deform_tex, vec2<i32>(min(px + 1, tex_size - 1), py), 0).r;
        let o01 = textureLoad(deform_tex, vec2<i32>(px, min(py + 1, tex_size - 1)), 0).r;
        let o11 = textureLoad(deform_tex, vec2<i32>(min(px + 1, tex_size - 1), min(py + 1, tex_size - 1)), 0).r;
        let ox = mix(mix(o00, o10, fx), mix(o01, o11, fx), fy);

        // Smooth fade at region edges so no hard boundary
        let edge = 0.06;
        let fade_u = smoothstep(0.0, edge, u) * smoothstep(0.0, edge, 1.0 - u);
        let fade_v = smoothstep(0.0, edge, v) * smoothstep(0.0, edge, 1.0 - v);
        let fade = fade_u * fade_v;
        let offset = ox * fade;

        pos.y -= offset;

        // Recompute normal from height gradient so depressions are properly lit (satisfying curvature)
        let world_cell = (2.0 * half_size) / tex_size_f;
        let d_du = mix(o10 - o00, o11 - o01, fy);
        let d_dv = mix(o01 - o00, o11 - o10, fx);
        let d_offset_dx = d_du / world_cell;
        let d_offset_dz = d_dv / world_cell;
        let deformed_n = normalize(vec3<f32>(d_offset_dx, 1.0, d_offset_dz));
        world_normal = normalize(mix(vertex.normal, deformed_n, fade));
    }

    out.world_position = pos;
    out.clip_position = camera.view_proj * vec4<f32>(pos, 1.0);
    out.world_normal = world_normal;
    out.uv = vertex.uv;
    out.biome_color = vertex.color;
    return out;
}

// ============================================================================
// FRAGMENT SHADER
// ============================================================================

// ============================================================================
// CHUNK EDGE BLENDING
// Computes a blend factor that is 0.0 in the interior and rises to 1.0 near
// chunk edges. Used to cross-fade between fine detail and a smooth large-scale
// texture at chunk boundaries, hiding any seams.
// ============================================================================

fn chunk_edge_blend(world_pos: vec3<f32>, chunk_size: f32) -> f32 {
    if (chunk_size <= 0.0) {
        return 0.0;
    }

    // Distance from the nearest chunk edge in X and Z
    // fract(pos/chunk_size) gives position within chunk [0,1]
    let chunk_frac_x = fract(world_pos.x / chunk_size + 0.5); // 0..1 within chunk
    let chunk_frac_z = fract(world_pos.z / chunk_size + 0.5);

    // Distance to nearest edge: 0.0 at edge, 0.5 at center
    let edge_dist_x = min(chunk_frac_x, 1.0 - chunk_frac_x);
    let edge_dist_z = min(chunk_frac_z, 1.0 - chunk_frac_z);
    let edge_dist = min(edge_dist_x, edge_dist_z);

    // Blend zone: ramp from 0 (at ~5% from edge) to 1 (at edge)
    // This is ~3.2 world units for a 64-unit chunk
    return 1.0 - smoothstep(0.0, 0.05, edge_dist);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let chunk_size = terrain.biome_params.x;
    let time = terrain.biome_params.z;

    // World-space position for texturing (scaled for detail)
    let world_p = in.world_position;
    let p = world_p * 0.1;

    // ---- CHUNK EDGE BLEND FACTOR ----
    // Near chunk borders, blend toward a smoother, lower-frequency texture
    // to hide any remaining seam artifacts from normals/vertex interpolation.
    let edge_blend = chunk_edge_blend(world_p, chunk_size);

    // ---- SLOPE & HEIGHT ANALYSIS ----
    let slope = 1.0 - n.y; // 0 = flat, 1 = vertical cliff
    let slope_factor = smoothstep(0.15, 0.55, slope);
    let height = world_p.y;

    // ---- BIOME TINT FROM VERTEX COLOR ----
    // Vertex color carries per-vertex biome blend from procgen; use it as primary tint.
    // Fallback to uniform when vertex color is missing (alpha 0) or effectively black (uninitialized).
    // Earth override: when uniform is Earth-like (green-dominant), prefer it so Mountain vertex grey doesn't produce ash/blue artifacts.
    let vertex_rgb = in.biome_color.rgb;
    let uniform_base = terrain.biome_colors[0].rgb;
    let is_earth_palette = uniform_base.g > 0.45 && uniform_base.g >= uniform_base.r && uniform_base.g >= uniform_base.b;
    let has_vertex_color = in.biome_color.a > 0.01 && (vertex_rgb.r + vertex_rgb.g + vertex_rgb.b) > 0.05;
    let use_uniform = !has_vertex_color || is_earth_palette;
    let biome_tint = select(vertex_rgb, uniform_base, use_uniform);

    // Classify biome from color heuristics (for procedural material selection)
    let warmth = biome_tint.r - biome_tint.b;
    let greenness = biome_tint.g - max(biome_tint.r, biome_tint.b);
    let coldness = biome_tint.b - biome_tint.r;
    let avg_brightness = (biome_tint.r + biome_tint.g + biome_tint.b) / 3.0;
    let darkness = 1.0 - avg_brightness;
    let purple = biome_tint.b > biome_tint.g && biome_tint.r > 0.25 && biome_tint.b > 0.4;
    let is_grey = abs(biome_tint.r - biome_tint.g) < 0.04 && abs(biome_tint.g - biome_tint.b) < 0.04;

    // ---- DETERMINE BIOME MATERIAL ID ----
    // 0=desert, 1=rock, 2=volcanic, 3=organic/hive, 4=frozen
    // 5=swamp, 6=crystal, 7=ash, 8=jungle, 9=wasteland
    var biome_id: i32;
    if (purple) {
        biome_id = 6; // crystalline (purple-blue hues)
    } else if (is_grey && avg_brightness > 0.25 && avg_brightness < 0.45) {
        biome_id = 7; // ashlands (neutral grey)
    } else if (coldness > 0.1) {
        biome_id = 4; // frozen
    } else if (greenness > 0.06 && darkness > 0.55) {
        biome_id = 8; // jungle (dark green)
    } else if (greenness > 0.02 && darkness > 0.5) {
        biome_id = 5; // swamp (dark yellow-green)
    } else if (greenness > 0.02 && darkness < 0.5) {
        biome_id = 3; // organic/hive
    } else if (warmth > 0.15 && darkness > 0.4) {
        biome_id = 2; // volcanic
    } else if (warmth > 0.05 && avg_brightness > 0.4) {
        biome_id = 9; // wasteland (pale warm)
    } else if (warmth > 0.05) {
        biome_id = 0; // desert
    } else {
        biome_id = 1; // rock/mountain
    }

    // ---- MULTI-LAYER MATERIAL SAMPLING (with parallax) ----
    let view_dir = normalize(camera.position.xyz - world_p);

    // Layer 1: Flat ground material (biome-specific) - fine detail + parallax
    let flat_color = triplanar_sample_with_parallax(p, n, biome_id, 1.0, time, view_dir);

    // Layer 1b: Same material at a coarser scale (for edge blending)
    let coarse_p = world_p * 0.04; // much lower frequency
    let flat_color_coarse = triplanar_sample_with_parallax(coarse_p, n, biome_id, 1.0, time, view_dir);

    // Blend fine and coarse at chunk edges to smooth over any discontinuities
    let blended_flat = mix(flat_color, flat_color_coarse, edge_blend * 0.6);

    // Layer 2: Cliff/slope material (always rocky) + parallax
    let cliff_color = triplanar_sample_with_parallax(p, n, 1, 1.2, time, view_dir);
    let cliff_color_coarse = triplanar_sample_with_parallax(coarse_p, n, 1, 1.2, time, view_dir);
    let blended_cliff = mix(cliff_color, cliff_color_coarse, edge_blend * 0.6);

    // Layer 3: Micro-detail overlay (adds fine grain at close range)
    let dist_to_cam = length(camera.position.xyz - world_p);
    let detail_fade = 1.0 - smoothstep(20.0, 80.0, dist_to_cam);
    // Reduce micro detail at chunk edges to further smooth seams
    let micro_noise = fbm(world_p * 2.0, 3) * 0.08 * detail_fade * (1.0 - edge_blend * 0.8);

    // ---- BLEND LAYERS ----
    var albedo = mix(blended_flat, blended_cliff, slope_factor);
    albedo += vec3<f32>(micro_noise);

    // Height-based color shift (lower = darker/damper, higher = lighter/drier)
    let height_tint = smoothstep(-5.0, 20.0, height);
    albedo *= mix(0.85, 1.08, height_tint);

    // Vertex biome color is the main driver: blend procedural with vertex tint so terrain shows
    // clear biome variation (sand, rock, volcanic, etc.) instead of grey/white/black.
    let procedural_only = albedo;
    let tinted = albedo * biome_tint;
    albedo = mix(procedural_only, tinted, 0.75);
    albedo *= 1.15;

    // Erosion streaks on slopes (vertical dark lines from water erosion)
    // Also reduce at chunk edges
    if (slope_factor > 0.2 && edge_blend < 0.8) {
        let erosion_streak = fbm(vec3<f32>(world_p.x * 0.5, world_p.y * 2.0, world_p.z * 0.5), 3);
        let streak = smoothstep(0.45, 0.55, erosion_streak) * slope_factor * (1.0 - edge_blend);
        albedo *= 1.0 - streak * 0.25;
    }

    // ---- PBR-STYLE LIGHTING ----
    let light_dir = normalize(terrain.sun_direction.xyz);
    let sun_intensity = terrain.sun_direction.w;
    // view_dir already defined above for parallax

    // Day factor
    let day_factor = clamp(light_dir.y * 3.0, 0.0, 1.0);

    // Ambient occlusion from normal (crevices darker)
    let ao = smoothstep(-0.2, 0.3, n.y) * 0.5 + 0.5;

    // Hemisphere ambient: raised so shadows stay visible and colored, not pure black
    let sky_ambient = vec3<f32>(0.28, 0.30, 0.36) * day_factor;
    let ground_bounce = vec3<f32>(0.12, 0.10, 0.08) * day_factor;
    let night_ambient = vec3<f32>(0.08, 0.06, 0.09);
    let ambient_light = mix(night_ambient, mix(ground_bounce, sky_ambient, n.y * 0.5 + 0.5), day_factor) * ao;

    // Diffuse: warm at golden hour, neutral at noon
    let golden_hour = smoothstep(0.0, 0.12, light_dir.y) * smoothstep(0.35, 0.08, light_dir.y);
    let noon_light = vec3<f32>(1.0, 0.96, 0.88);
    let warm_light = vec3<f32>(1.0, 0.65, 0.35);
    let sun_color = mix(noon_light, warm_light, golden_hour);
    let n_dot_l = max(dot(n, light_dir), 0.0);

    // Softer toon shading: more bands + smooth ramp so terrain has midtones, not just white/black
    let half_lambert = n_dot_l * 0.65 + 0.35;
    let toon_bands = 6.0;
    let toon_lambert = floor(half_lambert * toon_bands + 0.5) / toon_bands;
    let smooth_lambert = mix(toon_lambert, half_lambert, 0.35);
    var diffuse = sun_color * smooth_lambert * sun_intensity;

    // Shadow map: sample sun shadow (directional light)
    let light_clip = shadow.light_view_proj * vec4<f32>(world_p, 1.0);
    let light_ndc = light_clip.xyz / light_clip.w;
    let shadow_uv = light_ndc.xy * 0.5 + 0.5;
    let depth_compare = light_ndc.z * 0.5 + 0.5 + 0.002;
    let in_bounds = all(shadow_uv >= vec2<f32>(0.0)) && all(shadow_uv <= vec2<f32>(1.0));
    let shadow_factor = select(1.0, textureSampleCompare(shadow_tex, shadow_sampler, shadow_uv, depth_compare), in_bounds);
    diffuse *= shadow_factor;

    // Specular: Blinn-Phong with roughness variation (slightly sharper for stylized look)
    let h = normalize(light_dir + view_dir);
    let n_dot_h = max(dot(n, h), 0.0);
    let roughness = mix(0.85, 0.5, slope_factor); // cliffs are rougher
    let spec_power = mix(16.0, 64.0, 1.0 - roughness);
    let spec = pow(n_dot_h, spec_power) * sun_intensity;

    // Fresnel term for specular
    let fresnel = 0.04 + 0.96 * pow(1.0 - max(dot(view_dir, h), 0.0), 5.0);
    let spec_final = spec * fresnel * mix(0.05, 0.2, 1.0 - roughness);

    // Combine lighting
    var color = albedo * (ambient_light + diffuse) + vec3<f32>(spec_final) * sun_color;

    // MIRO-style bold rim light: stylized edge glow (day + golden hour)
    let rim = pow(1.0 - max(dot(n, view_dir), 0.0), 3.0);
    let rim_color = mix(vec3<f32>(0.25, 0.30, 0.45), warm_light, golden_hour * 0.8);
    color += rim_color * rim * (0.25 + golden_hour * 0.2);

    // ---- ATMOSPHERIC FOG ----
    let dist = length(camera.position.xyz - world_p);
    let fog_start = terrain.fog_params.z;
    let fog_end = terrain.fog_params.w;
    let fog_amount = clamp((dist - fog_start) / max(fog_end - fog_start, 1.0), 0.0, 0.88);

    // Fog color: biome-tinted atmosphere (slot 2 = fog tint, slot 3 = ambient tint)
    // Night fog reflects the warm amber/orange nebula glow from the SST-style space
    let biome_fog_tint = terrain.biome_colors[2].rgb;
    let biome_amb_tint = terrain.biome_colors[3].rgb;
    let day_fog = mix(
        biome_fog_tint * 0.7 + biome_tint * 0.15 + vec3<f32>(0.08, 0.06, 0.05),
        biome_amb_tint * 0.5 + vec3<f32>(0.25, 0.24, 0.22),
        0.4
    );
    // Night fog: warm amber glow from nebulae visible overhead
    let nebula_tint = vec3<f32>(0.025, 0.015, 0.008); // warm amber from space
    let night_fog = biome_fog_tint * 0.08 + nebula_tint + vec3<f32>(0.012, 0.014, 0.025);
    let fog_color = mix(night_fog, day_fog, day_factor);

    // Height fog: denser near ground level, scaled by biome fog density
    let height_fog = exp(-max(world_p.y, 0.0) * 0.004) * 0.18 * day_factor;
    let total_fog = min(fog_amount + height_fog, 0.92);
    color = mix(color, fog_color, total_fog);

    // ---- TONE MAPPING (ACES filmic) ----
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    color = clamp((color * (color * a + b)) / (color * (color * c + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));

    // Gamma correction
    color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, 1.0);
}
