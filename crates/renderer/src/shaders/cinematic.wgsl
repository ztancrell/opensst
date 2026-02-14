// Cinematic post-process: MIRO-style stylized + Starship Troopers military aesthetic
// Colorful, saturated, atmospheric — cel-shaded terrain + bold palette

struct CinematicUniform {
    time: f32,
    dither_strength: f32,
    vignette_strength: f32,
    bloom_strength: f32,
    lift: vec3<f32>,   // shadow lift (warm)
    ssao_scale: f32,
    inv_gamma: vec3<f32>,
    ssao_radius: f32,
    gain: vec3<f32>,   // highlight punch
    ssao_bias: f32,
};

@group(0) @binding(0)
var scene_tex: texture_2d<f32>;
@group(0) @binding(1)
var scene_sampler: sampler;
@group(0) @binding(2)
var<uniform> cinematic: CinematicUniform;
@group(0) @binding(3)
var bloom_tex: texture_2d<f32>;
@group(0) @binding(4)
var depth_tex: texture_depth_2d;
@group(0) @binding(5)
var depth_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    // Fullscreen triangle
    let x = f32((vi << 1u) & 2u);
    let y = f32(vi & 2u);
    var out: VertexOutput;
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

// 4x4 Bayer matrix for ordered dithering (classic print / retro look)
fn bayer4(px: u32, py: u32) -> f32 {
    let i = px % 4u;
    let j = py % 4u;
    // Standard 4x4 Bayer, row-major: [0,8,2,10, 12,4,14,6, 3,11,1,9, 15,7,13,5] / 16
    let m = array<f32, 16>(
        0.0, 8.0, 2.0, 10.0,
        12.0, 4.0, 14.0, 6.0,
        3.0, 11.0, 1.0, 9.0,
        15.0, 7.0, 13.0, 5.0
    );
    return m[j * 4u + i] / 16.0;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let res = vec2<f32>(textureDimensions(scene_tex));
    let uv = in.uv;
    var color = textureSample(scene_tex, scene_sampler, uv).rgb;

    // --- SSAO (screen-space ambient occlusion): darken crevices/contacts ---
    let depth_center = textureSample(depth_tex, depth_sampler, uv);
    let texel = 1.0 / res;
    let d0 = textureSample(depth_tex, depth_sampler, uv + vec2<f32>(texel.x, 0.0));
    let d1 = textureSample(depth_tex, depth_sampler, uv + vec2<f32>(-texel.x, 0.0));
    let d2 = textureSample(depth_tex, depth_sampler, uv + vec2<f32>(0.0, texel.y));
    let d3 = textureSample(depth_tex, depth_sampler, uv + vec2<f32>(0.0, -texel.y));
    let depth_diff = max(max(abs(depth_center - d0), abs(depth_center - d1)),
                         max(abs(depth_center - d2), abs(depth_center - d3)));
    let ao = 1.0 - smoothstep(cinematic.ssao_bias, cinematic.ssao_radius, depth_diff) * cinematic.ssao_scale;
    color *= ao;

    // --- Bloom (additive glow from bright pass) ---
    let bloom = textureSample(bloom_tex, scene_sampler, uv).rgb * cinematic.bloom_strength;
    color += bloom;

    // --- Ordered dither (Bayer 4x4, classic look — no TV static) ---
    let px = u32(in.uv.x * res.x);
    let py = u32(in.uv.y * res.y);
    let bayer = bayer4(px, py);
    let dither = (bayer - 0.5) * cinematic.dither_strength;
    color += dither;

    // --- Lift / Gamma / Gain (MIRO + SST color grading) ---
    // Lift: warm saturated shadows (orange/amber military palette)
    color = color + cinematic.lift * (1.0 - color);
    // Gamma: slight rolloff for stylized midtones
    color = pow(max(color, vec3<f32>(0.0001)), cinematic.inv_gamma);
    // Gain: punch highlights
    color = color * cinematic.gain;

    // --- Saturation boost (MIRO-style colorful, stylized) ---
    let luma = dot(color, vec3<f32>(0.299, 0.587, 0.114));
    color = mix(vec3<f32>(luma, luma, luma), color, 1.22);

    // --- Vignette (softer edges for atmospheric look) ---
    let ndc = in.uv * 2.0 - 1.0;
    let dist = length(ndc);
    let vig = 1.0 - smoothstep(0.4, 1.2, dist) * cinematic.vignette_strength;
    color *= vig;

    // --- Final tone map (soft clip, film-like) ---
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    color = clamp((color * (color * a + b)) / (color * (color * c + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(color, 1.0);
}
