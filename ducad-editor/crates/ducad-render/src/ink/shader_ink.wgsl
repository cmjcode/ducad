// Shader rendering tinta bertekanan dengan antialiasing strip dan grain pensil (M1.4).

struct Globals {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    light_dir: vec4<f32>,
    fill_light: vec4<f32>,
    rim_light: vec4<f32>,
    studio_params: vec4<f32>,
    shadow_bounds: vec4<f32>,
    clip_plane: vec4<f32>,
    zebra_params: vec4<f32>,
    draft_params: vec4<f32>,
    draft_dir: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) side: f32,
    @location(2) color: vec4<f32>,
    @location(3) soft: f32,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) side: f32,
    @location(1) color: vec4<f32>,
    @location(2) soft: f32,
    @location(3) world_pos: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_pos = globals.view_proj * vec4<f32>(in.pos, 1.0);
    out.side = in.side;
    out.color = in.color;
    out.soft = in.soft;
    out.world_pos = in.pos;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var alpha = in.color.a;

    // Antialiasing di tepian: alpha *= 1 - smoothstep(1 - fw, 1, abs(side))
    let fw = max(fwidth(in.side), 0.001);
    let edge_aa = 1.0 - smoothstep(1.0 - fw, 1.0, abs(in.side));
    alpha = alpha * edge_aa;

    // Pencil (soft ~ 0.5): noise deterministik berbasis koordinat dunia
    if (in.soft > 0.25 && in.soft < 0.75) {
        let p = in.world_pos.xy * 20.0;
        let noise = fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
        let pencil_grain = mix(0.7, 1.0, noise);
        alpha = alpha * pencil_grain;
    } else if (in.soft >= 0.75) {
        // Marker (soft ~ 1.0): alpha rendah untuk efek layering
        alpha = alpha * 0.7;
    }

    let rgb = in.color.rgb;
    return vec4<f32>(rgb * alpha, alpha);
}
