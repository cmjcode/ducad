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

    return vec4<f32>(standard_color, in.color.a);
}
