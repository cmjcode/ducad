// Shaders GPU untuk elemen grafis vektor 2D/3D (M1.3)
// Mendukung pewarnaan solid dan gradien (linier & radial) dengan output premultiplied alpha.

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

struct GradientStop {
    offset: f32,
    pad0: f32,
    pad1: f32,
    pad2: f32,
    color: vec4<f32>,
};

struct GradientUniform {
    kind: u32,
    count: u32,
    p0: vec2<f32>,
    p1: vec2<f32>,
    _pad: vec2<f32>,
    stops: array<GradientStop, 8>,
};

struct GradientBatch {
    gradients: array<GradientUniform, 64>,
};

@group(1) @binding(0) var<uniform> batch_gradients: GradientBatch;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) paint: u32,
    @location(3) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) @interpolate(flat) paint: u32,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_pos = globals.view_proj * vec4<f32>(in.pos, 1.0);
    out.color = in.color;
    out.paint = in.paint;
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if (in.paint == 0u) {
        let c = in.color;
        return vec4<f32>(c.rgb * c.a, c.a);
    }

    let grad_idx = in.paint - 1u;
    if (grad_idx >= 64u) {
        let c = in.color;
        return vec4<f32>(c.rgb * c.a, c.a);
    }

    let grad = batch_gradients.gradients[grad_idx];
    let stop_count = min(grad.count, 8u);
    if (stop_count == 0u) {
        let c = in.color;
        return vec4<f32>(c.rgb * c.a, c.a);
    }
    if (stop_count == 1u) {
        let c = grad.stops[0].color;
        return vec4<f32>(c.rgb * c.a, c.a);
    }

    var t: f32 = 0.0;
    if (grad.kind == 0u) {
        // Linier: t = dot(uv - p_start, dir) / len_sq
        let p_start = grad.p0;
        let p_end = grad.p1;
        let dir = p_end - p_start;
        let len_sq = dot(dir, dir);
        if (len_sq > 1e-6) {
            t = clamp(dot(in.uv - p_start, dir) / len_sq, 0.0, 1.0);
        }
    } else {
        // Radial: t = length(uv - center) / radius
        let center = grad.p0;
        let radius = max(grad.p1.x, 1e-6);
        t = clamp(length(in.uv - center) / radius, 0.0, 1.0);
    }

    var color = grad.stops[0].color;
    if (t <= grad.stops[0].offset) {
        color = grad.stops[0].color;
    } else if (t >= grad.stops[stop_count - 1u].offset) {
        color = grad.stops[stop_count - 1u].color;
    } else {
        for (var i = 0u; i < stop_count - 1u; i = i + 1u) {
            let s0 = grad.stops[i];
            let s1 = grad.stops[i + 1u];
            if (t >= s0.offset && t <= s1.offset) {
                let span = max(s1.offset - s0.offset, 1e-6);
                let factor = clamp((t - s0.offset) / span, 0.0, 1.0);
                color = mix(s0.color, s1.color, factor);
                break;
            }
        }
    }

    return vec4<f32>(color.rgb * color.a, color.a);
}
