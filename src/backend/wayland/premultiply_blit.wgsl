struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    // Generates UV coordinates for a full-screen triangle procedurally:
    // Index 0: (0.0, 0.0)
    // Index 1: (2.0, 0.0)
    // Index 2: (0.0, 2.0)
    let uv = vec2<f32>(
        f32((vertex_index << 1u) & 2u),
        f32(vertex_index & 2u)
    );
    
    // Map UV coordinates (0.0 to 1.0) into Clip Space (-1.0 to 1.0)
    out.position = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    
    // WebGPU clip-space Y-axis goes down, but texture space goes up.
    // Invert the position Y coordinate so the triangle renders upright.
    out.position.y = -out.position.y;
    
    out.uv = uv;
    return out;
}

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var s_source: sampler;

@fragment
fn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    // If your Wayland compositor flips the texture vertically,
    // you can invert the Y channel here instead: vec2<f32>(uv.x, 1.0 - uv.y)
    var color = textureSample(t_source, s_source, uv);
    
    // Mathematically premultiply the texture colors before sending to the surface
    color.r *= color.a;
    color.g *= color.a;
    color.b *= color.a;
    
    return color;
}
