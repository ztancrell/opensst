// High-quality procedural sky shader for alien planets
// Inspired by Starship Troopers (2005 game) UI background:
// Dense, populated space with warm amber nebulae, rich star clusters, gas filaments
// Features: realistic atmosphere, Rayleigh/Mie scattering, volumetric clouds,
// multi-layer starfield, warm nebulae, milky way band, planet from orbit

struct CameraUniform {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    position: vec4<f32>,
    planet_radius: f32,
    _pad: vec3<f32>,
};

struct SkyUniform {
    sun_direction: vec4<f32>,     // xyz = direction, w = intensity
    sun_color: vec4<f32>,         // rgb = color, w = sun disk size
    sky_color_zenith: vec4<f32>,  // rgb = zenith color, w = planet_radius
    sky_color_horizon: vec4<f32>, // rgb = horizon color, w = atmo_height
    ground_color: vec4<f32>,      // rgb = planet surface color, w = haze amount
    params: vec4<f32>,            // x = time, y = cloud_density, z = dust_amount, w = planet_type
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(0) @binding(1)
var<uniform> sky: SkyUniform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) view_dir: vec3<f32>,
};

const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;

// ============================================================================
// NOISE FUNCTIONS
// ============================================================================

fn hash(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash2(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash3(p: vec3<f32>) -> vec3<f32> {
    var p3 = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yxz + 33.33);
    return fract((p3.xxy + p3.yxx) * p3.zyx);
}

fn noise3d(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0); // quintic

    return mix(
        mix(
            mix(hash(i), hash(i + vec3<f32>(1.0, 0.0, 0.0)), u.x),
            mix(hash(i + vec3<f32>(0.0, 1.0, 0.0)), hash(i + vec3<f32>(1.0, 1.0, 0.0)), u.x),
            u.y
        ),
        mix(
            mix(hash(i + vec3<f32>(0.0, 0.0, 1.0)), hash(i + vec3<f32>(1.0, 0.0, 1.0)), u.x),
            mix(hash(i + vec3<f32>(0.0, 1.0, 1.0)), hash(i + vec3<f32>(1.0, 1.0, 1.0)), u.x),
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
        pp *= 2.07;
        amplitude *= 0.47;
    }
    return value;
}

fn fbm_high(p: vec3<f32>) -> f32 {
    return fbm(p, 6);
}

// ============================================================================
// STAR FIELD - Dense, crisp multi-magnitude field
// Starfield / Star Citizen / Helldivers 2: varied brightness, cooler palette
// ============================================================================

fn starfield(view_dir: vec3<f32>, time: f32) -> vec3<f32> {
    var total_stars = vec3<f32>(0.0);

    // Layer 1: Bright stars (few, large, crisp with subtle halos)
    {
        let grid = view_dir * 220.0;
        let cell = floor(grid);
        let h1 = hash(cell);
        let h2 = hash(cell + vec3<f32>(17.3, 43.7, 71.1));
        let h3 = hash(cell + vec3<f32>(91.2, 13.5, 57.8));

        if (h1 > 0.988) {
            let frac_pos = fract(grid) - 0.5;
            let star_offset = (hash3(cell) - 0.5) * 0.4;
            let d = length(frac_pos - star_offset);
            let core = smoothstep(0.05, 0.0, d) * 4.0;
            let halo = smoothstep(0.15, 0.0, d) * 0.4;
            let brightness = core + halo;

            // Cooler star colors (Starfield / Star Citizen)
            var star_col: vec3<f32>;
            if (h2 < 0.15) {
                star_col = vec3<f32>(0.7, 0.8, 1.0);   // blue-white
            } else if (h2 < 0.35) {
                star_col = vec3<f32>(1.0, 1.0, 1.0);   // white (common)
            } else if (h2 < 0.55) {
                star_col = vec3<f32>(1.0, 0.98, 0.92); // slight warm
            } else if (h2 < 0.75) {
                star_col = vec3<f32>(1.0, 0.92, 0.82); // yellow-white
            } else if (h2 < 0.90) {
                star_col = vec3<f32>(1.0, 0.85, 0.7);  // soft orange
            } else {
                star_col = vec3<f32>(1.0, 0.7, 0.6);   // orange-red
            }

            let twinkle = sin(time * 1.2 + h3 * 100.0) * 0.08 + 0.92;
            total_stars += star_col * brightness * twinkle;
        }
    }

    // Layer 2: Medium stars (many, crisp)
    {
        let grid = view_dir * 520.0;
        let cell = floor(grid);
        let h1 = hash(cell);
        let h2 = hash(cell + vec3<f32>(23.5, 67.1, 41.9));

        if (h1 > 0.972) {
            let frac_pos = fract(grid) - 0.5;
            let star_offset = (hash3(cell) - 0.5) * 0.3;
            let d = length(frac_pos - star_offset);
            let brightness = smoothstep(0.06, 0.0, d) * 1.1;

            let star_col = mix(vec3<f32>(0.9, 0.92, 1.0), vec3<f32>(1.0, 0.98, 0.95), h2);
            let twinkle = sin(time * 2.0 + h2 * 200.0) * 0.08 + 0.92;
            total_stars += star_col * brightness * twinkle;
        }
    }

    // Layer 3: Dense dim star field
    {
        let grid = view_dir * 1100.0;
        let cell = floor(grid);
        let h1 = hash(cell);

        if (h1 > 0.942) {
            let frac_pos = fract(grid) - 0.5;
            let d = length(frac_pos);
            let brightness = smoothstep(0.09, 0.0, d) * 0.35;
            let twinkle = sin(time * 2.8 + h1 * 300.0) * 0.06 + 0.94;
            total_stars += vec3<f32>(0.95, 0.96, 1.0) * brightness * twinkle;
        }
    }

    // Layer 4: Star dust (numerous faint points)
    {
        let grid = view_dir * 2000.0;
        let cell = floor(grid);
        let h1 = hash(cell);

        if (h1 > 0.928) {
            let brightness = (h1 - 0.928) / 0.072 * 0.1;
            total_stars += vec3<f32>(0.88, 0.9, 0.98) * brightness;
        }
    }

    // Layer 5: Ultra-faint background
    {
        let grid = view_dir * 3200.0;
        let cell = floor(grid);
        let h1 = hash(cell);

        if (h1 > 0.918) {
            let brightness = (h1 - 0.918) / 0.082 * 0.05;
            total_stars += vec3<f32>(0.8, 0.85, 0.95) * brightness;
        }
    }

    return total_stars;
}

// ============================================================================
// NEBULA - Starfield / Star Citizen: subtle blue/purple/violet gas clouds
// ============================================================================

fn nebula(view_dir: vec3<f32>, time: f32) -> vec3<f32> {
    var total_nebula = vec3<f32>(0.0);

    // --- Layer 1: Primary nebula (large, cool-toned) ---
    {
        let nebula_pos = view_dir * 2.2;
        let warp1 = vec3<f32>(
            fbm(nebula_pos + vec3<f32>(0.0, 0.0, 0.0), 3),
            fbm(nebula_pos + vec3<f32>(5.2, 1.3, 0.0), 3),
            fbm(nebula_pos + vec3<f32>(0.0, 7.8, 3.4), 3)
        );
        let warped = nebula_pos + warp1 * 1.8;
        let density = fbm(warped, 5);

        let arm_angle = atan2(view_dir.z, view_dir.x);
        let arm_mask = smoothstep(0.2, 0.7, sin(arm_angle * 1.2 + 1.5) * 0.5 + 0.5);
        let vert_mask = smoothstep(-0.3, 0.1, view_dir.y) * smoothstep(0.9, 0.3, view_dir.y);

        let nebula_strength = smoothstep(0.30, 0.60, density) * arm_mask * vert_mask * 0.28;

        // Cool palette: deep blue, violet, soft purple
        let color_noise = fbm(warped * 2.0, 3);
        let neb_blue   = vec3<f32>(0.08, 0.12, 0.28);
        let neb_violet = vec3<f32>(0.12, 0.06, 0.22);
        let neb_purple = vec3<f32>(0.15, 0.08, 0.25);
        let neb_soft   = vec3<f32>(0.06, 0.10, 0.20);

        var neb_color = mix(neb_soft, neb_blue, smoothstep(0.3, 0.6, color_noise));
        neb_color = mix(neb_color, neb_violet, smoothstep(0.4, 0.7, color_noise) * 0.6);
        neb_color = mix(neb_color, neb_purple, smoothstep(0.65, 0.85, density) * 0.4);

        let core_glow = smoothstep(0.50, 0.70, density) * 0.5;
        neb_color += neb_color * core_glow;

        total_nebula += neb_color * nebula_strength;
    }

    // --- Layer 2: Secondary nebula (different region) ---
    {
        let nebula_pos = view_dir * 1.6 + vec3<f32>(3.0, 1.0, 5.0);
        let warp2 = vec3<f32>(
            fbm(nebula_pos + vec3<f32>(10.0, 3.0, 0.0), 3),
            fbm(nebula_pos + vec3<f32>(2.5, 8.0, 0.0), 3),
            fbm(nebula_pos + vec3<f32>(0.0, 5.5, 7.0), 3)
        );
        let warped = nebula_pos + warp2 * 1.5;
        let density = fbm(warped, 4);

        let arm_angle = atan2(view_dir.x, view_dir.z);
        let arm_mask = smoothstep(0.25, 0.65, sin(arm_angle * 0.8 + 3.0) * 0.5 + 0.5);

        let nebula_strength = smoothstep(0.35, 0.65, density) * arm_mask * 0.18;

        let color_noise = fbm(warped * 1.5, 3);
        let neb_deep  = vec3<f32>(0.04, 0.06, 0.14);
        let neb_mid   = vec3<f32>(0.06, 0.04, 0.12);
        let neb_edge  = vec3<f32>(0.08, 0.05, 0.18);

        var neb_color = mix(neb_deep, neb_mid, smoothstep(0.3, 0.7, color_noise));
        neb_color = mix(neb_color, neb_edge, smoothstep(0.6, 0.9, density) * 0.4);

        total_nebula += neb_color * nebula_strength;
    }

    // --- Layer 3: Faint wispy filaments ---
    {
        let fil_pos = view_dir * 4.0 + vec3<f32>(7.0, 2.0, 3.0);
        let fil_noise = fbm(fil_pos, 4);
        let fil_warp = fbm(fil_pos * 0.5, 3);
        let filament = smoothstep(0.45, 0.55, fil_noise + fil_warp * 0.3) *
                        smoothstep(0.65, 0.55, fil_noise + fil_warp * 0.3);
        let fil_color = vec3<f32>(0.04, 0.06, 0.14) * filament * 0.25;
        total_nebula += fil_color;
    }

    return total_nebula;
}

// ============================================================================
// MILKY WAY BAND - Starfield / Star Citizen: cooler, blue-white band
// ============================================================================

fn milky_way(view_dir: vec3<f32>, time: f32) -> vec3<f32> {
    let galactic_tilt = mat3x3<f32>(
        vec3<f32>(0.866, 0.0, 0.5),
        vec3<f32>(0.0, 1.0, 0.0),
        vec3<f32>(-0.5, 0.0, 0.866)
    );
    let gal_dir = galactic_tilt * view_dir;

    let band_dist = abs(gal_dir.y);
    let band_strength = exp(-band_dist * band_dist * 7.0);

    if (band_strength < 0.01) {
        return vec3<f32>(0.0);
    }

    let mw_pos = gal_dir * vec3<f32>(4.0, 8.0, 4.0);
    let mw_noise = fbm(mw_pos, 5);
    let dust_lane = smoothstep(0.4, 0.55, fbm(gal_dir * vec3<f32>(3.0, 6.0, 3.0), 4));

    let brightness = band_strength * (mw_noise * 0.65 + 0.35) * (1.0 - dust_lane * 0.55);

    let cluster_noise = smoothstep(0.55, 0.75, fbm(gal_dir * 8.0, 3));
    let extra_stars = cluster_noise * 0.5;

    // Cool palette: blue-white core, subtle violet edges
    let mw_core   = vec3<f32>(0.15, 0.18, 0.28); // blue-white core
    let mw_mid    = vec3<f32>(0.10, 0.12, 0.22); // mid
    let mw_edge   = vec3<f32>(0.06, 0.08, 0.16); // blue edge
    let mw_center = mix(mw_mid, mw_core, smoothstep(0.2, 0.0, band_dist));
    let mw_color  = mix(mw_edge, mw_center, smoothstep(0.4, 0.0, band_dist));

    return mw_color * (brightness * 0.7 + extra_stars) + vec3<f32>(brightness * 0.06);
}

// ============================================================================
// VOLUMETRIC CLOUDS
// ============================================================================

fn cloud_layer(view_dir: vec3<f32>, time: f32, cloud_density: f32, day_factor: f32,
               night_factor: f32, sun_dir: vec3<f32>, golden_hour: f32) -> vec3<f32> {
    if (cloud_density < 0.01 || view_dir.y < -0.02) {
        return vec3<f32>(-1.0); // sentinel: no cloud
    }

    let cloud_alt = 200.0;
    let t_ray = cloud_alt / max(view_dir.y, 0.02);
    let cloud_pos = camera.position.xyz + view_dir * t_ray;

    let wind = vec2<f32>(time * 0.008, time * 0.003);
    let cloud_uv = cloud_pos.xz * 0.0003 + wind;

    let base_cloud = fbm(vec3<f32>(cloud_uv.x * 4.0, cloud_uv.y * 4.0, time * 0.01), 5);
    let detail = fbm(vec3<f32>(cloud_uv.x * 16.0, cloud_uv.y * 16.0, time * 0.02), 3) * 0.3;
    let cloud_noise = base_cloud + detail;

    let threshold = mix(0.48, 0.28, cloud_density);
    let cloud_shape = smoothstep(threshold, threshold + 0.18, cloud_noise);

    if (cloud_shape < 0.01) {
        return vec3<f32>(-1.0);
    }

    let shadow_offset = sun_dir.xz * 0.003;
    let shadow_uv = cloud_uv + shadow_offset;
    let shadow_noise = fbm(vec3<f32>(shadow_uv.x * 4.0, shadow_uv.y * 4.0, time * 0.01), 3);
    let self_shadow = smoothstep(threshold, threshold + 0.15, shadow_noise);

    let sun_elevation = max(sun_dir.y, 0.0);
    let lit = mix(0.4, 1.0, sun_elevation) * (1.0 - self_shadow * 0.4);

    let cloud_bright = vec3<f32>(1.0, 0.97, 0.92) * lit;
    let cloud_shadow = vec3<f32>(0.30, 0.30, 0.38) * mix(0.5, 1.0, sun_elevation);
    let cloud_night = vec3<f32>(0.06, 0.06, 0.10);

    var cloud_color = mix(cloud_shadow, cloud_bright, lit);
    cloud_color = mix(cloud_color, cloud_night, night_factor * 0.85);

    if (golden_hour > 0.05) {
        let sunset_tint = vec3<f32>(1.0, 0.55, 0.20);
        cloud_color = mix(cloud_color, sunset_tint * lit, golden_hour * 0.6);
    }

    let back_lit = pow(max(dot(normalize(vec3<f32>(view_dir.x, 0.0, view_dir.z)), sun_dir), 0.0), 8.0);
    let edge_brightness = (1.0 - cloud_shape) * back_lit * 0.5;
    cloud_color += vec3<f32>(1.0, 0.95, 0.85) * edge_brightness * day_factor;

    let cloud_fade = smoothstep(-0.02, 0.12, view_dir.y);

    return mix(vec3<f32>(-1.0), cloud_color, cloud_shape * cloud_fade * cloud_density);
}

// ============================================================================
// VERTEX SHADER
// ============================================================================

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    let x = f32((vertex_index & 1u) << 2u) - 1.0;
    let y = f32((vertex_index & 2u) << 1u) - 1.0;

    out.clip_position = vec4<f32>(x, y, 0.9999, 1.0);

    let view_space_dir = vec3<f32>(
        x / camera.proj[0][0],
        y / camera.proj[1][1],
        -1.0
    );

    let inv_view = transpose(mat3x3<f32>(
        camera.view[0].xyz,
        camera.view[1].xyz,
        camera.view[2].xyz
    ));
    out.view_dir = inv_view * view_space_dir;

    return out;
}

// ============================================================================
// FRAGMENT SHADER
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let view_dir = normalize(in.view_dir);
    let sun_dir = normalize(sky.sun_direction.xyz);
    let sun_intensity = sky.sun_direction.w;
    let time = sky.params.x;
    let cloud_density = sky.params.y;
    let dust_amount = sky.params.z;
    let planet_type = sky.params.w;

    let planet_radius = sky.sky_color_zenith.w;
    let atmo_height = sky.sky_color_horizon.w;
    let altitude = camera.position.y;

    // ===== ATMOSPHERE / SPACE BLEND =====
    // When atmo_height < 0.001 we are in "force space" mode (approach/open orbit) — always full space
    let atmo_start = atmo_height * 0.15;
    let atmo_end = atmo_height * 0.8;
    var space_blend = smoothstep(atmo_start, atmo_end, altitude);
    if (atmo_height < 0.001) {
        space_blend = 1.0;
    }

    let day_factor = clamp(sun_dir.y * 3.0, 0.0, 1.0);
    let night_factor = 1.0 - day_factor;
    let golden_hour = smoothstep(0.0, 0.12, sun_dir.y) * smoothstep(0.35, 0.08, sun_dir.y);

    // ===== ATMOSPHERIC SKY (surface) =====
    let up_dot = view_dir.y;
    let zenith_blend = pow(max(up_dot, 0.0), mix(0.35, 0.6, day_factor));

    var atmo_color = mix(sky.sky_color_horizon.rgb, sky.sky_color_zenith.rgb, zenith_blend);

    // Mie scattering — stronger, more cinematic sun halo
    let sun_dot = dot(view_dir, sun_dir);
    let mie_phase = pow(max(sun_dot, 0.0), 6.0);
    let mie_color = mix(vec3<f32>(1.0, 0.95, 0.85), vec3<f32>(1.0, 0.5, 0.2), golden_hour);
    atmo_color += mie_color * mie_phase * sun_intensity * 0.4;

    // Ozone absorption at horizon
    let horizon_proximity = pow(1.0 - abs(up_dot), 6.0);
    let ozone_tint = vec3<f32>(0.15, 0.08, 0.25) * horizon_proximity * day_factor * 0.15;
    atmo_color += ozone_tint;

    // Below horizon: ground color blend
    if (up_dot < 0.0 && space_blend < 0.3) {
        let ground_blend = smoothstep(0.0, -0.2, up_dot) * (1.0 - space_blend * 3.33);
        atmo_color = mix(atmo_color, sky.ground_color.rgb * 0.6, ground_blend);
    }

    // ===== SPACE BACKGROUND (default everywhere until in atmosphere) =====
    // Unified space skybox: pitch black + twinkling stars (Starship Troopers aesthetic).
    // Used for: main menu, ship interior, orbit, approach, and drop pod above atmosphere.
    // Only when descending into atmosphere (space_blend decreases) do we blend to the
    // time/weather/effects-driven atmospheric sky below.
    let twinkle_time = time * 1.8;
    var space_color = vec3<f32>(0.0, 0.0, 0.0);
    space_color += starfield(view_dir, twinkle_time) * 1.2;
    {
        let grid = view_dir * 1400.0;
        let cell = floor(grid);
        let h1 = hash(cell);
        let h2 = hash(cell + vec3<f32>(31.1, 17.3, 53.7));
        if (h1 > 0.94) {
            let frac_pos = fract(grid) - 0.5;
            let d = length(frac_pos);
            let sparkle = smoothstep(0.12, 0.0, d) * 0.4;
            let twinkle = sin(twinkle_time * 2.5 + h2 * 400.0) * 0.25 + 0.75;
            space_color += vec3<f32>(0.92, 0.95, 1.0) * sparkle * twinkle;
        }
    }

    // ===== BLEND ATMOSPHERE <-> SPACE =====
    var sky_color = mix(atmo_color, space_color, space_blend);

    // ===== SUN (realistic disk, corona, limb darkening) =====
    let sun_disk_size = sky.sun_color.w * 0.018; // Slightly larger for visibility
    let sun_disk = smoothstep(1.0 - sun_disk_size, 1.0 - sun_disk_size * 0.25, sun_dot);

    // Limb darkening: sun is brighter at center (realistic)
    let limb_darken = 0.4 + 0.6 * pow(max(sun_dot, 0.0), 0.5);
    let sun_core_color = vec3<f32>(1.0, 1.0, 0.98);
    let sun_limb_color = vec3<f32>(0.95, 0.92, 0.85);

    let corona_noise = fbm(view_dir * 20.0 + sun_dir * 5.0, 3);
    let corona_atmo = pow(max(sun_dot, 0.0), 14.0) * (0.85 + corona_noise * 0.5) * 0.18 * (1.0 - space_blend);
    let corona_space = pow(max(sun_dot, 0.0), 70.0) * 0.45 * space_blend; // Sharper in space
    let corona = corona_atmo + corona_space;

    let glow_power = mix(2.8, 28.0, space_blend); // Slightly softer glow for more realistic sun
    let glow_atmo = mix(1.0, 0.08, space_blend);
    let sun_glow = pow(max(sun_dot, 0.0), glow_power) * mix(0.85, 0.15, day_factor) * glow_atmo;

    let sun_vis = mix(sun_intensity, 1.0, space_blend);

    let disk_brightness = mix(4.0, 8.0, space_blend); // Brighter, more visible sun
    let sun_disk_color = mix(sun_limb_color, sun_core_color, limb_darken);
    sky_color += sun_disk_color * sun_disk * disk_brightness * sun_vis;
    sky_color += sky.sun_color.rgb * (sun_glow + corona) * sun_vis;

    // Lens flare (stronger at dawn/dusk)
    if (sun_dot > 0.88 && space_blend < 0.75) {
        let flare = pow(max(sun_dot - 0.88, 0.0) * 8.33, 2.5) * (0.12 + golden_hour * 0.15) * (1.0 - space_blend);
        sky_color += sky.sun_color.rgb * flare * sun_vis;
    }

    // ===== ATMOSPHERIC SCATTERING / SUNSET =====
    {
        let scatter_strength = pow(max(sun_dot, 0.0), 1.5);
        let sunset_color = mix(
            vec3<f32>(1.0, 0.30, 0.05),
            vec3<f32>(1.0, 0.55, 0.18),
            clamp(sun_dir.x * 0.5 + 0.5, 0.0, 1.0)
        );
        let scatter_fade = 1.0 - space_blend;
        sky_color += sunset_color * horizon_proximity * scatter_strength * golden_hour * 1.4 * scatter_fade;

        let horizon_band = pow(1.0 - abs(up_dot), 10.0) * golden_hour * 0.7 * scatter_fade;
        sky_color += vec3<f32>(0.85, 0.35, 0.08) * horizon_band;

        let anti_sun = max(dot(view_dir, -sun_dir), 0.0);
        let anti_glow = pow(anti_sun, 3.0) * golden_hour * 0.1 * scatter_fade;
        sky_color += vec3<f32>(0.15, 0.08, 0.25) * anti_glow * horizon_proximity;
    }

    // ===== MOON (realistic with phases, crater detail, earthshine) =====
    // Moon opposite sun; time/100 = time_of_day for phase (full at midnight, new at noon)
    let time_of_day = time * 0.01;
    let moon_dir = normalize(vec3<f32>(-sun_dir.x, max(0.15, -sun_dir.y * 0.7 + 0.25), -sun_dir.z));
    let moon_dot = dot(view_dir, moon_dir);
    // Phase from time: full at dawn (t=0), new at dusk (t=0.5)
    let moon_sun_dot = cos((time_of_day - 0.5) * TAU); // -1 (full) to 1 (new)

    if (moon_dot > 0.0 && space_blend < 0.85) {
        let moon_disk = smoothstep(0.9994, 0.99992, moon_dot);
        if (moon_disk > 0.0) {
            // Crater detail (maria + highlands)
            let moon_uv = view_dir * 180.0;
            let craters = fbm(moon_uv, 4) * 0.25;
            let maria = smoothstep(0.45, 0.55, fbm(moon_uv * 0.5, 3));
            let moon_base = vec3<f32>(0.68, 0.70, 0.76);
            let moon_dark = vec3<f32>(0.35, 0.38, 0.45); // Maria
            var moon_surface = mix(moon_base, moon_dark, maria * 0.4);
            moon_surface *= (0.85 + craters);

            // Phase shading: full when moon opposite sun, crescent/new when same side
            let phase_lit = 1.0 - smoothstep(-0.2, 0.85, moon_sun_dot); // Lit portion (1=full, 0=new)
            let moon_lit = mix(moon_surface * 0.12, moon_surface, phase_lit); // Dark side faint (earthshine)
            sky_color = mix(sky_color, moon_lit, moon_disk);
        }

        let moon_glow = pow(max(moon_dot, 0.0), 90.0) * 0.22;
        let moon_halo = pow(max(moon_dot, 0.0), 18.0) * 0.06;
        let moon_color = vec3<f32>(0.5, 0.55, 0.75);
        let moon_vis = smoothstep(0.15, 0.45, night_factor) * (1.0 - cloud_density * 0.5);
        sky_color += moon_color * (moon_glow + moon_halo) * moon_vis;
    }

    // ===== CLOUDS =====
    let cloud_vis = (1.0 - space_blend) * cloud_density;
    if (cloud_vis > 0.01) {
        let cloud_result = cloud_layer(view_dir, time, cloud_density, day_factor,
                                        night_factor, sun_dir, golden_hour);
        if (cloud_result.r >= 0.0) {
            let cloud_alpha = clamp(length(cloud_result) / 2.0, 0.0, 0.95) * (1.0 - space_blend);
            sky_color = mix(sky_color, cloud_result, cloud_alpha);
        }
    }

    // ===== VOLUMETRIC DUST / HAZE (biome-tinted) =====
    if (dust_amount > 0.01 && space_blend < 0.8) {
        let dust_warp = vec3<f32>(
            fbm(view_dir * 3.0 + time * 0.1, 3),
            fbm(view_dir * 3.0 + vec3<f32>(5.0, 0.0, 0.0) + time * 0.1, 3),
            fbm(view_dir * 2.5 + vec3<f32>(0.0, 7.0, 0.0) + time * 0.07, 2)
        );
        let dust_noise = fbm(view_dir * 8.0 + dust_warp * 0.5, 4);
        let dust_detail = fbm(view_dir * 16.0 + dust_warp * 0.3 + time * 0.05, 3) * 0.4;
        let combined_dust = (dust_noise + dust_detail) * dust_amount * 0.05 * (1.0 - space_blend);
        let biome_dust_col = mix(sky.ground_color.rgb * 0.8 + vec3<f32>(0.2), sky.sun_color.rgb, 0.25);
        let dust_col = mix(vec3<f32>(0.70, 0.50, 0.30), biome_dust_col, 0.6);
        sky_color += dust_col * combined_dust * day_factor;
    }

    // Horizon haze — Helldivers/SST Extermination: thicker volumetric atmosphere at horizon
    if (space_blend < 0.8) {
        let haze_falloff = pow(1.0 - abs(up_dot), 4.0);
        let base_haze = 0.15 * (1.0 - space_blend); // always some atmospheric haze
        let dust_haze = dust_amount * 0.5 * (1.0 - space_blend);
        let haze_amount = haze_falloff * (base_haze + dust_haze);
        let biome_haze = mix(sky.ground_color.rgb, sky.sky_color_horizon.rgb, 0.5);
        let haze_color = mix(biome_haze, sky.sun_color.rgb * 0.6, day_factor * 0.3);
        sky_color = mix(sky_color, haze_color, haze_amount);
    }

    // God rays — Helldivers 2 / SST Extermination style: stronger, more visible crepuscular rays
    if (space_blend < 0.5 && day_factor > 0.05) {
        let sun_alignment = max(dot(view_dir, normalize(sky.sun_direction.xyz)), 0.0);
        let shaft_noise = fbm(view_dir * 3.0 + vec3<f32>(time * 0.02, 0.0, time * 0.015), 4);
        let shaft_strength = pow(sun_alignment, 12.0) * (0.4 + shaft_noise * 0.6) * 0.35 * day_factor * (1.0 - space_blend);
        let shaft_color = mix(sky.sun_color.rgb, vec3<f32>(1.0, 0.9, 0.7), 0.4);
        sky_color += shaft_color * shaft_strength * (1.0 - cloud_density * 0.6);
    }

    // ===== PLANET SPHERE FROM ORBIT =====
    // Enhanced: biome colors are much more visible, atmosphere tint matches biome
    if (space_blend > 0.05 && planet_radius > 0.0) {
        let cam_h = planet_radius + altitude;
        let oc = vec3<f32>(0.0, cam_h, 0.0);

        let b_planet = dot(oc, view_dir);
        let c_planet = dot(oc, oc) - planet_radius * planet_radius;
        let disc_planet = b_planet * b_planet - c_planet;

        let atmo_r = planet_radius + atmo_height;
        let c_atmo = dot(oc, oc) - atmo_r * atmo_r;
        let disc_atmo = b_planet * b_planet - c_atmo;

        if (disc_planet >= 0.0) {
            let t_hit = -b_planet - sqrt(disc_planet);
            if (t_hit > 0.0) {
                let hit_point = oc + view_dir * t_hit;
                let surface_normal = normalize(hit_point);

                // Planet surface: multi-octave detail with STRONG biome color
                let surface_noise = fbm(surface_normal * 8.0, 5) * 0.25;
                let surface_detail = fbm(surface_normal * 20.0, 3) * 0.10;
                let surface_base = sky.ground_color.rgb * (0.80 + surface_noise + surface_detail);

                // Continental features (different biome regions)
                let continent = fbm(surface_normal * 3.0, 4);
                let coast = smoothstep(0.42, 0.48, continent);
                // Secondary biome accent color (slightly different from base)
                let accent_color = sky.ground_color.rgb * vec3<f32>(0.85, 1.1, 0.9);
                let surface_varied = mix(surface_base, accent_color, coast * 0.3);

                // Oceans (dark regions in low areas, tinted by biome)
                let ocean_mask = smoothstep(0.44, 0.38, continent);
                let ocean_color = mix(
                    vec3<f32>(0.04, 0.10, 0.22),
                    sky.ground_color.rgb * 0.3,
                    0.2
                );
                let surface_with_ocean = mix(surface_varied, ocean_color, ocean_mask * 0.35);

                // Cloud layer on planet — driven by realtime weather (cloud_density)
                let cloud_noise = fbm(surface_normal * 4.0 + vec3<f32>(time * 0.001, 0.0, 0.0), 4);
                let base_clouds = smoothstep(0.45, 0.6, cloud_noise);
                let planet_clouds = base_clouds * (0.25 + cloud_density * 0.75); // weather scales coverage

                // Diffuse lighting
                let planet_ndotl = max(dot(surface_normal, sun_dir), 0.0);
                let planet_ambient = 0.08;
                var planet_lit = surface_with_ocean * (planet_ambient + 0.92 * planet_ndotl);
                // Clouds are white-ish but tinted by biome atmosphere
                let cloud_col = mix(vec3<f32>(0.88, 0.90, 0.92), sky.sky_color_horizon.rgb * 0.5 + vec3<f32>(0.5), 0.15);
                planet_lit = mix(planet_lit, cloud_col * (planet_ambient + planet_ndotl), planet_clouds);

                // Atmosphere limb effect (tinted by biome horizon color)
                let view_normal_dot = max(dot(surface_normal, normalize(-view_dir)), 0.0);
                let fresnel = pow(1.0 - view_normal_dot, 3.5);
                let atmo_tint = mix(sky.sky_color_horizon.rgb, sky.ground_color.rgb, 0.2) * 0.6;
                let planet_color_final = mix(planet_lit, atmo_tint, fresnel * 0.65);

                // Night side: city lights / bioluminescence
                let night_side = smoothstep(0.05, -0.1, planet_ndotl);
                let bio_noise = fbm(surface_normal * 20.0, 3);
                let bio_lights = smoothstep(0.7, 0.75, bio_noise) * night_side * 0.18;
                let bio_color = mix(
                    vec3<f32>(0.1, 0.3, 0.15),  // green biolum
                    sky.ground_color.rgb * 0.4,   // biome-tinted
                    0.3
                );
                let planet_final = planet_color_final + bio_color * bio_lights;

                let planet_vis = smoothstep(0.05, 0.4, space_blend);
                sky_color = mix(sky_color, planet_final, planet_vis);
            }
        } else if (disc_atmo >= 0.0) {
            // Atmosphere rim glow - Starfield/HD2: vivid blue-white limb
            let t_atmo_entry = -b_planet - sqrt(disc_atmo);
            let t_atmo_exit = -b_planet + sqrt(disc_atmo);
            if (t_atmo_entry > 0.0) {
                let atmo_path = (t_atmo_exit - t_atmo_entry) / (atmo_height * 2.0);
                let glow_strength = pow(clamp(atmo_path, 0.0, 1.0), 0.35);

                let atmo_base = mix(sky.sky_color_horizon.rgb, sky.ground_color.rgb, 0.1);
                let sun_side = max(dot(view_dir, sun_dir), 0.0);
                let lit_rim = mix(vec3<f32>(0.4, 0.6, 1.0), vec3<f32>(0.9, 0.85, 0.95), sun_side * 0.6);
                let atmo_glow = mix(atmo_base * 0.5, lit_rim, 0.7);

                let rim_vis = smoothstep(0.05, 0.35, space_blend);
                sky_color += atmo_glow * glow_strength * 0.85 * rim_vis;
            }
        }
    }

    // ===== SECOND SUN (alien planets) =====
    if (planet_type > 0.5) {
        let sun2_dir = normalize(vec3<f32>(-sun_dir.x, sun_dir.y * 0.5 + 0.1, -sun_dir.z));
        let sun2_dot = dot(view_dir, sun2_dir);
        let sun2_disk = smoothstep(0.998, 1.0, sun2_dot);
        let sun2_glow = pow(max(sun2_dot, 0.0), 20.0) * 0.12;
        let sun2_corona = pow(max(sun2_dot, 0.0), 8.0) * 0.05;
        let sun2_color = vec3<f32>(1.0, 0.25, 0.05);
        sky_color += sun2_color * (sun2_disk * 2.0 + sun2_glow + sun2_corona) * 0.7;
    }

    // ===== TONE MAPPING (ACES filmic) =====
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    sky_color = clamp((sky_color * (sky_color * a + b)) / (sky_color * (sky_color * c + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(sky_color, 1.0);
}
