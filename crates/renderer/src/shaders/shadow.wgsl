// Depth-only shadow pass: terrain and instanced meshes from sun's POV.
// Outputs depth to shadow map; no color target.

struct ShadowUniform {
    light_view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    planet_radius: f32,
    _pad: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> shadow: ShadowUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}

@vertex
fn vs_terrain(vertex: VertexInput) -> VertexOutput {
    var world_pos = vertex.position;
    if (shadow.planet_radius > 0.0) {
        let dx = world_pos.x - shadow.camera_pos.x;
        let dz = world_pos.z - shadow.camera_pos.z;
        let horiz_dist_sq = dx * dx + dz * dz;
        let curvature_drop = horiz_dist_sq / (2.0 * shadow.planet_radius);
        world_pos.y -= curvature_drop;
    }
    var out: VertexOutput;
    out.clip_position = shadow.light_view_proj * vec4<f32>(world_pos, 1.0);
    return out;
}

@fragment
fn fs_terrain() {
    // Depth-only: no color output; depth comes from vertex position
}

// ---- Instanced shadow (bugs, props) ----
struct InstanceInput {
    @location(3) model_matrix_0: vec4<f32>,
    @location(4) model_matrix_1: vec4<f32>,
    @location(5) model_matrix_2: vec4<f32>,
    @location(6) model_matrix_3: vec4<f32>,
}

@vertex
fn vs_instanced(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );
    var world_pos = (model_matrix * vec4<f32>(vertex.position, 1.0)).xyz;
    if (shadow.planet_radius > 0.0) {
        let dx = world_pos.x - shadow.camera_pos.x;
        let dz = world_pos.z - shadow.camera_pos.z;
        let horiz_dist_sq = dx * dx + dz * dz;
        let curvature_drop = horiz_dist_sq / (2.0 * shadow.planet_radius);
        world_pos.y -= curvature_drop;
    }
    var out: VertexOutput;
    out.clip_position = shadow.light_view_proj * vec4<f32>(world_pos, 1.0);
    return out;
}

@fragment
fn fs_instanced() {
}
