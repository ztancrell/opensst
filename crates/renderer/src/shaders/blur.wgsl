// Separable Gaussian blur for bloom
// direction: (1,0) horizontal, (0,1) vertical

struct BlurUniform {
    direction: vec2<f32>,
    _pad0: vec2<f32>,
};

@group(0) @binding(0)
var input_tex: texture_2d<f32>;
@group(0) @binding(1)
var input_sampler: sampler;
@group(0) @binding(2)
var<uniform> params: BlurUniform;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    let x = f32((vi << 1u) & 2u);
    let y = f32(vi & 2u);
    var out: VertexOutput;
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

// 9-tap Gaussian (sigma ~2) - good quality/speed tradeoff
const WEIGHTS: array<f32, 9> = array<f32, 9>(
    0.0162, 0.0540, 0.1210, 0.1942, 0.2270, 0.1942, 0.1210, 0.0540, 0.0162
);

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(input_tex));
    let texel_size = 1.0 / dims;
    let dir = params.direction * texel_size;

    var color = vec3<f32>(0.0);
    for (var i = 0; i < 9; i++) {
        let offset = dir * (f32(i) - 4.0);
        color += textureSample(input_tex, input_sampler, in.uv + offset).rgb * WEIGHTS[i];
    }

    return vec4<f32>(color, 1.0);
}
