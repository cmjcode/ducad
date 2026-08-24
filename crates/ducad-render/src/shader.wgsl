struct Globals {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    light_dir: vec4<f32>,
    // Section view (Fase 7): bidang potong `dot(xyz, world) - w`, fragment
    // mesh dengan hasil > 0 dibuang. Nonaktif = `xyz` nol vektor + `w`
    // sangat besar, jadi hasilnya selalu sangat negatif (tidak pernah
    // memotong apa pun) — menghindari field "enabled" terpisah.
    clip_plane: vec4<f32>,
    // Zebra stripes reflection inspection (Fase 3.1):
    // x: enabled (> 0.5 = aktif), y: frequency (jumlah garis),
    // z: angle (orientasi garis radian), w: blend factor (0.0..1.0).
    zebra_params: vec4<f32>,
    // Draft angle heatmap inspection (Fase 3.2):
    // x: enabled (> 0.5 = aktif), y: target_angle_rad, z: blend factor (0.0..1.0), w: reserved.
    draft_params: vec4<f32>,
    // Pull direction vector (arah buka cetakan mold):
    // xyz: normalized pull direction (e.g. [0.0, 0.0, 1.0]), w: unused.
    draft_dir: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

// ---------- Garis (grid, sumbu, nanti edge sketch) ----------

struct LineIn {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct LineOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) world: vec3<f32>,
};

@vertex
fn vs_line(in: LineIn) -> LineOut {
    var out: LineOut;
    out.clip = globals.view_proj * vec4<f32>(in.position, 1.0);
    out.color = in.color;
    out.world = in.position;
    return out;
}

@fragment
fn fs_line(in: LineOut) -> @location(0) vec4<f32> {
    // Fade grid berdasarkan jarak dari kamera agar horizon tidak "ramai".
    let dist = distance(in.world, globals.eye.xyz);
    let fade = clamp(1.0 - dist / 2000.0, 0.0, 1.0);
    return vec4<f32>(in.color.rgb, in.color.a * fade);
}

// ---------- Mesh solid (body B-rep ter-tessellasi & highlight profil 2D) ----------

struct MeshIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct MeshOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) world: vec3<f32>,
    @location(2) color: vec4<f32>,
};

@vertex
fn vs_mesh(in: MeshIn) -> MeshOut {
    var out: MeshOut;
    out.clip = globals.view_proj * vec4<f32>(in.position, 1.0);
    out.normal = in.normal;
    out.world = in.position;
    out.color = in.color;
    return out;
}

@fragment
fn fs_mesh(in: MeshOut) -> @location(0) vec4<f32> {
    let clip_side = dot(globals.clip_plane.xyz, in.world) - globals.clip_plane.w;
    if (clip_side > 0.0) {
        discard;
    }
    let base = in.color.rgb;
    let n = normalize(in.normal);
    let view_dir = normalize(globals.eye.xyz - in.world);
    let diffuse = max(dot(n, normalize(globals.light_dir.xyz)), 0.0);
    let rim = pow(1.0 - max(dot(n, view_dir), 0.0), 3.0) * 0.15;
    let standard_color = base * (0.35 + 0.65 * diffuse) + vec3<f32>(rim);

    // Zebra Stripes Reflection Inspection (Fase 3.1)
    // Memproyeksikan pantulan specular dari silinder/lingkungan bergaris virtual
    // untuk memvalidasi kontinuitas permukaan:
    // - G0 (Posisi): Garis zebra bersambung tanpa celah / gap.
    // - G1 (Tangensi): Garis zebra bersambung tetapi bersudut tajam di sambungan.
    // - G2 (Kurvatur): Garis zebra mengalir dengan lengkungan mulus tanpa sudut tajam.
    if (globals.zebra_params.x > 0.5) {
        let r = reflect(-view_dir, n);

        let freq = globals.zebra_params.y;
        let angle = globals.zebra_params.z;
        let cos_a = cos(angle);
        let sin_a = sin(angle);

        // Koordinat refleksi silindris / sferis
        let u = atan2(r.y, r.x) / 3.14159265; // rentang [-1.0, 1.0]
        let v = clamp(r.z, -1.0, 1.0);       // rentang [-1.0, 1.0]

        let coord = (cos_a * v + sin_a * u) * freq * 3.14159265;
        let wave = sin(coord);

        // Antialiasing berbasis fwidth untuk garis tajam dan bersih bebas flickering
        let edge = max(fwidth(wave) * 1.5, 0.001);
        let stripe = smoothstep(-edge, edge, wave);

        // Kontras tinggi zebra (hitam dan putih keperakan khas CAD industrial)
        let zebra_dark = vec3<f32>(0.04, 0.04, 0.05);
        let zebra_light = vec3<f32>(0.96, 0.96, 0.98);
        let zebra_pattern = mix(zebra_dark, zebra_light, stripe);

        // Shading lembut dan specular highlight agar bentuk geometris 3D tetap jelas
        let spec = pow(max(dot(r, normalize(globals.light_dir.xyz)), 0.0), 16.0) * 0.25;
        let zebra_shaded = zebra_pattern * (0.75 + 0.25 * diffuse) + vec3<f32>(spec);

        let blend = clamp(globals.zebra_params.w, 0.0, 1.0);
        let final_color = mix(standard_color, zebra_shaded, blend);
        return vec4<f32>(final_color, in.color.a);
    }

    // Draft Angle Heatmap Inspection (Fase 3.2)
    // Mengevaluasi sudut kemiringan permukaan terhadap arah buka cetakan (pull direction)
    // untuk validasi DFM (Design for Manufacturing) cetakan injeksi plastik / die-cast:
    // - Hijau: Sudut aman (>= target_angle, e.g. >= 1.0°)
    // - Kuning: Sudut kritis / butuh kemiringan draft (0° s/d target_angle)
    // - Merah: Undercut / kemiringan terbalik (< 0°) yang menjebak part di dalam cetakan
    if (globals.draft_params.x > 0.5) {
        let pull_dir = normalize(globals.draft_dir.xyz);
        let dot_nd = clamp(dot(n, pull_dir), -1.0, 1.0);
        let alpha_rad = asin(dot_nd);
        let target_rad = globals.draft_params.y;

        let color_safe = vec3<f32>(0.18, 0.80, 0.44);     // Hijau terang CAD (#2ecc71)
        let color_warning = vec3<f32>(0.98, 0.78, 0.12);  // Kuning peringatan (#f1c40f)
        let color_undercut = vec3<f32>(0.92, 0.24, 0.20); // Merah undercut (#e74c3c)

        var heatmap_color: vec3<f32>;
        if (alpha_rad >= target_rad) {
            let t = clamp((alpha_rad - target_rad) / (1.5707963 - target_rad), 0.0, 1.0);
            heatmap_color = mix(color_safe, vec3<f32>(0.10, 0.62, 0.32), t * 0.25);
        } else if (alpha_rad >= 0.0) {
            let t = clamp(alpha_rad / max(target_rad, 0.0001), 0.0, 1.0);
            heatmap_color = mix(color_warning, color_safe, t * 0.85);
        } else {
            let t = clamp(-alpha_rad / 1.5707963, 0.0, 1.0);
            heatmap_color = mix(color_undercut, vec3<f32>(0.72, 0.12, 0.12), t * 0.35);
        }

        // Shading pencahayaan diffuse + rim lembut agar kedalaman 3D tetap tampak tajam
        let heatmap_shaded = heatmap_color * (0.70 + 0.30 * diffuse) + vec3<f32>(rim * 0.4);
        let blend = clamp(globals.draft_params.z, 0.0, 1.0);
        let final_color = mix(standard_color, heatmap_shaded, blend);
        return vec4<f32>(final_color, in.color.a);
    }

    return vec4<f32>(standard_color, in.color.a);
}

// Fragment shader khusus gizmo 3D (selalu menampilkan warna asli in.color dengan diffuse + rim shading,
// tanpa terpengaruh oleh inspection shaders seperti Zebra maupun Draft Angle Heatmap).
@fragment
fn fs_gizmo(in: MeshOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let view_dir = normalize(globals.eye.xyz - in.world);
    let diffuse = max(dot(n, normalize(globals.light_dir.xyz)), 0.0);
    let rim = pow(1.0 - max(dot(n, view_dir), 0.0), 3.0) * 0.15;

    let shaded = in.color.rgb * (0.45 + 0.55 * diffuse) + vec3<f32>(rim);
    return vec4<f32>(shaded, in.color.a);
}
