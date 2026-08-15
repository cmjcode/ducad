struct Globals {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    light_dir: vec4<f32>,
    // Section view (Fase 7): bidang potong `dot(xyz, world) - w`, fragment
    // mesh dengan hasil > 0 dibuang. Nonaktif = `xyz` nol vektor + `w`
    // sangat besar, jadi hasilnya selalu sangat negatif (tidak pernah
    // memotong apa pun) — menghindari field "enabled" terpisah.
    clip_plane: vec4<f32>,
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

// ---------- Mesh solid (body B-rep ter-tessellasi) ----------

struct MeshIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct MeshOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) world: vec3<f32>,
};

@vertex
fn vs_mesh(in: MeshIn) -> MeshOut {
    var out: MeshOut;
    out.clip = globals.view_proj * vec4<f32>(in.position, 1.0);
    out.normal = in.normal;
    out.world = in.position;
    return out;
}

@fragment
fn fs_mesh(in: MeshOut) -> @location(0) vec4<f32> {
    let clip_side = dot(globals.clip_plane.xyz, in.world) - globals.clip_plane.w;
    if (clip_side > 0.0) {
        discard;
    }
    let base = vec3<f32>(0.62, 0.68, 0.76); // baja muda, netral khas CAD
    let n = normalize(in.normal);
    let diffuse = max(dot(n, normalize(globals.light_dir.xyz)), 0.0);
    let view_dir = normalize(globals.eye.xyz - in.world);
    let rim = pow(1.0 - max(dot(n, view_dir), 0.0), 3.0) * 0.15;
    let color = base * (0.35 + 0.65 * diffuse) + vec3<f32>(rim);
    return vec4<f32>(color, 1.0);
}
