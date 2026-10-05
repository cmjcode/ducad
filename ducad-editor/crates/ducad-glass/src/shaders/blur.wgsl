// Blit + blur dual-Kawase untuk backdrop Liquid Glass.
//
// Satu segitiga layar-penuh tanpa vertex buffer; `uv` (0,0) di kiri-atas.

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> VsOut {
    // (-1,-1), (3,-1), (-1,3): menutup seluruh viewport dengan satu segitiga.
    let x = f32(i32(index & 1u) * 4 - 1);
    let y = f32(i32(index >> 1u) * 4 - 1);
    var out: VsOut;
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@group(0) @binding(0) var t_src: texture_2d<f32>;
@group(0) @binding(1) var s_src: sampler;

// Salin apa adanya (viewport = ukuran tekstur, jadi 1:1 per piksel).
@fragment
fn fs_blit(in: VsOut) -> @location(0) vec4<f32> {
    return textureSampleLevel(t_src, s_src, in.uv, 0.0);
}

// Langkah turun: 4× pusat + 4 diagonal setengah-texel sumber.
@fragment
fn fs_down(in: VsOut) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(t_src));
    let o = texel * 1.0;
    var sum = textureSampleLevel(t_src, s_src, in.uv, 0.0) * 4.0;
    sum += textureSampleLevel(t_src, s_src, in.uv + vec2<f32>(-o.x, -o.y), 0.0);
    sum += textureSampleLevel(t_src, s_src, in.uv + vec2<f32>(o.x, -o.y), 0.0);
    sum += textureSampleLevel(t_src, s_src, in.uv + vec2<f32>(-o.x, o.y), 0.0);
    sum += textureSampleLevel(t_src, s_src, in.uv + vec2<f32>(o.x, o.y), 0.0);
    return sum / 8.0;
}

// Langkah naik: 4 sisi (bobot 1) + 4 diagonal (bobot 2).
@fragment
fn fs_up(in: VsOut) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(t_src));
    let o = texel * 1.0;
    var sum = textureSampleLevel(t_src, s_src, in.uv + vec2<f32>(-2.0 * o.x, 0.0), 0.0);
    sum += textureSampleLevel(t_src, s_src, in.uv + vec2<f32>(2.0 * o.x, 0.0), 0.0);
    sum += textureSampleLevel(t_src, s_src, in.uv + vec2<f32>(0.0, -2.0 * o.y), 0.0);
    sum += textureSampleLevel(t_src, s_src, in.uv + vec2<f32>(0.0, 2.0 * o.y), 0.0);
    sum += textureSampleLevel(t_src, s_src, in.uv + vec2<f32>(-o.x, o.y), 0.0) * 2.0;
    sum += textureSampleLevel(t_src, s_src, in.uv + vec2<f32>(o.x, o.y), 0.0) * 2.0;
    sum += textureSampleLevel(t_src, s_src, in.uv + vec2<f32>(-o.x, -o.y), 0.0) * 2.0;
    sum += textureSampleLevel(t_src, s_src, in.uv + vec2<f32>(o.x, -o.y), 0.0) * 2.0;
    return sum / 12.0;
}
