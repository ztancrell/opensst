// Screen-space text overlay shader with procedural bitmap font
// Minecraft-style on-screen text rendering

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@group(0) @binding(0)
var font_tex: texture_2d<f32>;
@group(0) @binding(1)
var font_sampler: sampler;

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) color: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    // position is already in NDC (-1..1)
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.tex_coords = tex_coords;
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // tex_coords.x < 0 signals a solid background quad (no texture lookup)
    if (in.tex_coords.x < 0.0) {
        return in.color;
    }
    let alpha = textureSample(font_tex, font_sampler, in.tex_coords).r;
    if (alpha < 0.3) {
        discard;
    }
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
