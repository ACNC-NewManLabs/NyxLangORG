struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

struct Uniforms {
    transform: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var texture_diffuse: texture_2d<f32>;

@group(0) @binding(2)
var sampler_diffuse: sampler;

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.transform * vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let texture_color = textureSample(texture_diffuse, sampler_diffuse, in.uv);
    return in.color * texture_color;
}
