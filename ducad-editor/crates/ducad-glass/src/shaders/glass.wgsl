// Material Liquid Glass: satu panel = satu segitiga layar-penuh di viewport
// panel. Semua panjang dalam piksel framebuffer. Rumus SDF/lensa/komposit di
// sini harus tetap sama dengan `sdf.rs` dan `GlassMaterial::composite`.
//
// Tampilan yang dikejar (WWDC25 "Meet Liquid Glass"): lempeng bening tebal.
// Tengah hampir tak berubah; pita rim membelokkan latar kuat dengan profil
// lensa cembung, memisahkan warna (dispersi), lebih gelap karena ketebalan,
// dan membawa dua garis sorot tipis: kuat di sisi cahaya, lemah di seberang.

struct Panel {
    rect: vec4<f32>,   // min.xy, size.xy
    shape: vec4<f32>,  // radius, rim_width, refraction, dispersion
    look: vec4<f32>,   // frost, tint_strength, specular, saturation
    tint: vec4<f32>,   // rgb, luma_limit
    light: vec4<f32>,  // light_dir.xy (satuan), press.xy
    fx: vec4<f32>,     // rim_shade, -, -, -
    extra: vec4<f32>,  // pixels_per_point, press_strength, opacity, mode (1 = terang)
    scene: vec4<f32>,  // origin.xy, size.xy tekstur scene di framebuffer
};

@group(0) @binding(0) var t_sharp: texture_2d<f32>;
@group(0) @binding(1) var t_mild: texture_2d<f32>;
@group(0) @binding(2) var t_strong: texture_2d<f32>;
@group(0) @binding(3) var s_backdrop: sampler;
@group(1) @binding(0) var<uniform> panel: Panel;

const LUMA: vec3<f32> = vec3<f32>(0.2126, 0.7152, 0.0722);

@vertex
fn vs_glass(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(index & 1u) * 4 - 1);
    let y = f32(i32(index >> 1u) * 4 - 1);
    return vec4<f32>(x, y, 0.0, 1.0);
}

fn sd_rrect(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

// Latar pada posisi piksel `px`: tajam → buram ringan → buram penuh.
fn backdrop(px: vec2<f32>, frost: f32) -> vec3<f32> {
    let uv = clamp((px - panel.scene.xy) / panel.scene.zw, vec2<f32>(0.001), vec2<f32>(0.999));
    let sharp = textureSampleLevel(t_sharp, s_backdrop, uv, 0.0).rgb;
    let mild = textureSampleLevel(t_mild, s_backdrop, uv, 0.0).rgb;
    let strong = textureSampleLevel(t_strong, s_backdrop, uv, 0.0).rgb;
    let a = clamp(frost * 3.0, 0.0, 1.0);
    let b = clamp((frost - 0.33) / 0.67, 0.0, 1.0);
    return mix(mix(sharp, mild, a), strong, b);
}

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

// Kedalaman lensa: jarak ke sisi terdekat, digabung smooth-min selebar `k`
// supaya arah tarikan berpadu mulus di sudut (gradien SDF persegi-bulat
// berbelok mendadak di diagonal sudut dan meninggalkan lipatan). Lihat
// sdf::smooth_inner_depth.
fn inner_depth(p: vec2<f32>, half: vec2<f32>, k: f32) -> f32 {
    let dx = half.x - abs(p.x);
    let dy = half.y - abs(p.y);
    let h = clamp(0.5 + 0.5 * (dy - dx) / k, 0.0, 1.0);
    return mix(dy, dx, h) - k * h * (1.0 - h);
}

// Profil lensa cembung (lihat sdf::lens_profile).
fn lens_profile(depth: f32, rim: f32) -> f32 {
    let t = clamp(depth / max(rim, 1.0), 0.0, 1.0);
    let u = 1.0 - t;
    return 1.0 - sqrt(max(1.0 - u * u, 0.0));
}

@fragment
fn fs_glass(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let half = panel.rect.zw * 0.5;
    let center = panel.rect.xy + half;
    let p = frag.xy - center;
    let radius = clamp(panel.shape.x, 0.0, min(half.x, half.y));
    let ppp = max(panel.extra.x, 0.5);

    let d = sd_rrect(p, half, radius);
    // Tepi halus selebar satu piksel, dikali opasitas UI (fade-in Area).
    let coverage = clamp(0.5 - d, 0.0, 1.0) * panel.extra.z;
    if (coverage <= 0.0) {
        discard;
    }

    // Normal ke luar = gradien medan kedalaman halus (beda hingga terpusat).
    let rim = max(panel.shape.y, 1.0);
    let smooth_k = rim * 0.75;
    let e = vec2<f32>(0.5, 0.0);
    let grad = vec2<f32>(
        inner_depth(p - e.xy, half, smooth_k) - inner_depth(p + e.xy, half, smooth_k),
        inner_depth(p - e.yx, half, smooth_k) - inner_depth(p + e.yx, half, smooth_k),
    );
    let glen = length(grad);
    var n = vec2<f32>(0.0);
    if (glen > 1e-6) {
        n = grad / glen;
    }

    // Lensa: hanya pita rim yang membelokkan latar; tengah tetap lurus.
    // Kedalaman halus hanya untuk lensa; garis sorot dan penggelapan rim
    // memakai jarak tepi sebenarnya agar tetap setipis satu garis di sudut.
    let depth = max(-d, 0.0);
    let lens_depth = max(min(inner_depth(p, half, smooth_k), depth), 0.0);
    let lens = lens_profile(lens_depth, rim);
    var disp = -n * lens * panel.shape.z;

    // Gelembung di bawah kursor/jari: latar tertarik ke titik tekan.
    let to_press = frag.xy - panel.light.zw;
    let press_radius = 64.0 * ppp;
    let bulge = exp(-dot(to_press, to_press) / (press_radius * press_radius)) * panel.extra.y;
    disp += -to_press * bulge * 0.25;

    // Rim disampel tajam supaya pembelokannya terlihat jelas.
    let frost = panel.look.x * (1.0 - lens);
    // Dispersi: merah dibelokkan paling kuat, biru paling lemah (seperti prisma).
    let k = panel.shape.w * lens;
    var color = vec3<f32>(
        backdrop(frag.xy + disp * (1.0 + k), frost).r,
        backdrop(frag.xy + disp, frost).g,
        backdrop(frag.xy + disp * (1.0 - k), frost).b,
    );

    // Saturasi → batas kecerahan → tint (lihat GlassMaterial::composite).
    let gray = dot(color, LUMA);
    color = clamp(mix(vec3<f32>(gray), color, panel.look.w), vec3<f32>(0.0), vec3<f32>(1.0));
    let light_mode = panel.extra.w > 0.5;
    if (light_mode) {
        color = vec3<f32>(1.0) - color;
    }
    let bright = max(dot(color, LUMA), 0.9 * max(color.r, max(color.g, color.b)));
    color = color * min(1.0, panel.tint.w / max(bright, 1e-4));
    if (light_mode) {
        color = vec3<f32>(1.0) - color;
    }
    color = mix(color, panel.tint.rgb, clamp(panel.look.y, 0.0, 1.0));

    // Ketebalan kaca: pita rim lebih gelap (cahaya terserap di sisi lensa).
    let thick = smoothstep(0.0, 1.0, lens);
    let shade = 1.0 - panel.fx.x * thick * (0.55 + 0.45 * (1.0 - abs(dot(n, panel.light.xy))));
    color = color * shade;

    // Sorotan spekular. Satu garis tipis di tepi: terang di sisi yang
    // menghadap cahaya, lebih lemah di seberang (pantulan kedua), plus pendar
    // halus di pita rim pada sisi cahaya.
    let facing = dot(n, panel.light.xy);
    let key = pow(max(facing, 0.0), 1.2);
    let fill = pow(max(-facing, 0.0), 2.0) * 0.5;
    let edge_w = 1.8 * ppp;
    let line = (1.0 - smoothstep(0.0, edge_w, depth)) * smoothstep(-1.0, 0.3, depth);
    let glow = lens * lens;
    let highlight = panel.look.z * (
        line * (0.18 + 0.82 * max(key, fill))
        + glow * (0.04 + 0.16 * key)
    );
    color += vec3<f32>(highlight + bulge * 0.10);

    // Dither tipis melawan banding pada gradien buram.
    color += vec3<f32>((hash(frag.xy) - 0.5) / 255.0);
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));

    // Keluaran premultiplied (konvensi blending egui).
    return vec4<f32>(color * coverage, coverage);
}
