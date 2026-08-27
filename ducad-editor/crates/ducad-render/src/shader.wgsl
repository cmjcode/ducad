struct Globals {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    // Key light (pencahayaan utama): xyz = normal direction, w = intensity
    light_dir: vec4<f32>,
    // Fill light (pencahayaan pengisi sekunder, Fase 4.2): xyz = normal direction, w = intensity
    fill_light: vec4<f32>,
    // Rim / Back light (pencahayaan kontur siluet, Fase 4.2): xyz = normal direction, w = intensity
    rim_light: vec4<f32>,
    // Studio & SSAO params (Fase 4.2):
    // x: enabled (> 0.5 = aktif), y: ssao_intensity (0.0..2.0),
    // z: floor_shadow_intensity (0.0..1.0), w: ground_z
    studio_params: vec4<f32>,
    // Shadow projection bounds (Fase 4.2):
    // x: center_x, y: center_y, z: radius_x, w: radius_y
    shadow_bounds: vec4<f32>,
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

// ---------- Floor Contact Soft Shadow (Fase 4.2) ----------

struct FloorIn {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct FloorOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
};

@vertex
fn vs_floor(in: FloorIn) -> FloorOut {
    var out: FloorOut;
    out.clip = globals.view_proj * vec4<f32>(in.position, 1.0);
    out.world = in.position;
    return out;
}

@fragment
fn fs_floor(in: FloorOut) -> @location(0) vec4<f32> {
    let enabled = globals.studio_params.x;
    let shadow_intensity = globals.studio_params.z;
    if (enabled <= 0.5 || shadow_intensity <= 0.001) {
        discard;
    }

    let center = globals.shadow_bounds.xy;
    let radius = max(globals.shadow_bounds.zw, vec2<f32>(1.0, 1.0));

    let d_vec = (in.world.xy - center) / radius;
    let dist_sq = dot(d_vec, d_vec);
    if (dist_sq > 4.0) {
        discard;
    }

    // Soft Gaussian-like dropoff with tight contact core beneath object
    let contact_core = exp(-dist_sq * 4.0);
    let soft_penumbra = exp(-dist_sq * 0.9);
    let shadow_alpha = (contact_core * 0.70 + soft_penumbra * 0.30) * shadow_intensity * 0.60;

    let shadow_color = vec3<f32>(0.07, 0.08, 0.10);
    return vec4<f32>(shadow_color, shadow_alpha);
}

// ---------- Mesh solid (body B-rep ter-tessellasi & highlight profil 2D) ----------

struct MeshIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) material_params: vec4<f32>, // x: roughness, y: metallic, z: clearcoat, w: reserved
};

struct MeshOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) world: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) material_params: vec4<f32>,
};

@vertex
fn vs_mesh(in: MeshIn) -> MeshOut {
    var out: MeshOut;
    out.clip = globals.view_proj * vec4<f32>(in.position, 1.0);
    out.normal = in.normal;
    out.world = in.position;
    out.color = in.color;
    out.material_params = in.material_params;
    return out;
}

@fragment
fn fs_mesh(in: MeshOut) -> @location(0) vec4<f32> {
    let clip_side = dot(globals.clip_plane.xyz, in.world) - globals.clip_plane.w;
    if (clip_side > 0.0) {
        discard;
    }

    let albedo = in.color.rgb;
    let alpha = in.color.a;
    let roughness = clamp(in.material_params.x, 0.02, 1.0);
    let metallic = clamp(in.material_params.y, 0.0, 1.0);
    let clearcoat = clamp(in.material_params.z, 0.0, 1.0);

    let n = normalize(in.normal);
    let v = normalize(globals.eye.xyz - in.world);
    let r = reflect(-v, n);
    let ndotv = max(dot(n, v), 0.001);

    // Fresnel Schlick: Dielectric F0 = 0.04, Metal F0 = albedo
    let f0 = mix(vec3<f32>(0.04, 0.04, 0.04), albedo, metallic);
    let rough_sq = roughness * roughness;
    let spec_exp = max(2.0 / (rough_sq * rough_sq + 0.0001) - 2.0, 1.0);

    // 1. Key Light (Lampu Utama - Tajam & Dominan)
    let l_key = normalize(globals.light_dir.xyz);
    let key_int = max(globals.light_dir.w, 0.0);
    let ndotl_key = max(dot(n, l_key), 0.0);
    let h_key = normalize(v + l_key);
    let ndoth_key = max(dot(n, h_key), 0.0);
    let vdoth_key = max(dot(v, h_key), 0.0);

    let fresnel_key = f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - vdoth_key, 5.0);
    let d_key = pow(ndoth_key, spec_exp) * ((spec_exp + 2.0) / 8.0);
    let key_spec = fresnel_key * d_key * (0.25 + 0.75 * ndotl_key) * key_int;
    let key_diffuse = albedo * (0.35 + 0.65 * ndotl_key) * (1.0 - metallic) * key_int;
    let key_cc_spec = pow(ndoth_key, 256.0) * clearcoat * 0.45 * key_int;

    // 2. Fill Light (Lampu Pengisi - Lembut & Mengurangi Bayangan Kasar)
    let l_fill = normalize(globals.fill_light.xyz);
    let fill_int = max(globals.fill_light.w, 0.0);
    let ndotl_fill = max(dot(n, l_fill), 0.0);
    let fill_tone = vec3<f32>(0.88, 0.93, 1.0); // Soft cool studio fill
    let fill_diffuse = albedo * fill_tone * (0.25 + 0.75 * ndotl_fill) * (1.0 - metallic) * fill_int * 0.6;
    let h_fill = normalize(v + l_fill);
    let ndoth_fill = max(dot(n, h_fill), 0.0);
    let fill_spec = f0 * pow(ndoth_fill, max(spec_exp * 0.5, 1.0)) * 0.15 * fill_int;

    // 3. Rim / Back Light (Lampu Kontur Siluet)
    let l_rim = normalize(globals.rim_light.xyz);
    let rim_int = max(globals.rim_light.w, 0.0);
    let rim_dot = max(dot(n, l_rim), 0.0);
    let rim_fresnel = pow(1.0 - ndotv, 3.5);
    let rim_highlight = mix(vec3<f32>(1.0), albedo, metallic * 0.5) * rim_fresnel * (0.30 + 0.70 * rim_dot) * rim_int * 0.65;

    // 4. Studio Environment IBL Reflection
    let env_top = vec3<f32>(0.92, 0.94, 0.98);
    let env_bottom = vec3<f32>(0.20, 0.22, 0.26);
    let env_refl = mix(env_bottom, env_top, clamp(r.y * 0.5 + 0.5, 0.0, 1.0));
    let refl_intensity = (1.0 - roughness * 0.8) * (metallic * 0.65 + clearcoat * 0.35);
    let refl_color = env_refl * mix(vec3<f32>(1.0), albedo, metallic) * refl_intensity;

    // 5. Fresnel Rim & Glass Edge Glow
    let glass_rim = pow(1.0 - ndotv, 3.5) * (0.15 + (1.0 - alpha) * 0.55);

    // 6. Screen Space & Cavity Curvature Ambient Occlusion (SSAO)
    let is_studio = globals.studio_params.x > 0.5;
    let ssao_strength = globals.studio_params.y;
    let n_deriv = length(fwidth(n));
    let p_deriv = length(fwidth(in.world));
    let cavity = clamp(n_deriv / max(p_deriv * 0.08 + 0.0001, 0.001), 0.0, 1.0);
    let grazing_ao = pow(1.0 - ndotv, 2.0) * 0.25;
    let ao_factor = 1.0 - clamp((cavity * 0.60 + grazing_ao * 0.30) * ssao_strength, 0.0, 0.85);

    // Gabungkan seluruh komponen pencahayaan studio
    var total_diffuse: vec3<f32>;
    var total_specular: vec3<f32>;

    if (is_studio) {
        total_diffuse = (key_diffuse + fill_diffuse) * ao_factor;
        total_specular = (key_spec + fill_spec + vec3<f32>(key_cc_spec) + rim_highlight) * (0.35 + 0.65 * ao_factor);
    } else {
        total_diffuse = key_diffuse;
        total_specular = key_spec + vec3<f32>(key_cc_spec);
    }

    let standard_color = total_diffuse + total_specular + refl_color + vec3<f32>(glass_rim);

    // Zebra Stripes Reflection Inspection (Fase 3.1)
    if (globals.zebra_params.x > 0.5) {
        let freq = globals.zebra_params.y;
        let angle = globals.zebra_params.z;
        let cos_a = cos(angle);
        let sin_a = sin(angle);

        // Koordinat refleksi silindris / sferis
        let u = atan2(r.y, r.x) / 3.14159265;
        let v_coord = clamp(r.z, -1.0, 1.0);

        let coord = (cos_a * v_coord + sin_a * u) * freq * 3.14159265;
        let wave = sin(coord);

        let edge = max(fwidth(wave) * 1.5, 0.001);
        let stripe = smoothstep(-edge, edge, wave);

        let zebra_dark = vec3<f32>(0.04, 0.04, 0.05);
        let zebra_light = vec3<f32>(0.96, 0.96, 0.98);
        let zebra_pattern = mix(zebra_dark, zebra_light, stripe);

        let spec = pow(max(dot(r, normalize(globals.light_dir.xyz)), 0.0), 16.0) * 0.25;
        let zebra_shaded = zebra_pattern * (0.75 + 0.25 * ndotl_key) + vec3<f32>(spec);

        let blend = clamp(globals.zebra_params.w, 0.0, 1.0);
        let final_color = mix(standard_color, zebra_shaded, blend);
        return vec4<f32>(final_color, alpha);
    }

    // Draft Angle Heatmap Inspection (Fase 3.2)
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

        let heatmap_shaded = heatmap_color * (0.70 + 0.30 * ndotl_key) + vec3<f32>(glass_rim * 0.4);
        let blend = clamp(globals.draft_params.z, 0.0, 1.0);
        let final_color = mix(standard_color, heatmap_shaded, blend);
        return vec4<f32>(final_color, alpha);
    }

    return vec4<f32>(standard_color, alpha);
}

// Fragment shader khusus gizmo 3D (selalu menampilkan warna asli in.color dengan diffuse + rim shading)
@fragment
fn fs_gizmo(in: MeshOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let view_dir = normalize(globals.eye.xyz - in.world);
    let diffuse = max(dot(n, normalize(globals.light_dir.xyz)), 0.0);
    let rim = pow(1.0 - max(dot(n, view_dir), 0.0), 3.0) * 0.15;

    let shaded = in.color.rgb * (0.45 + 0.55 * diffuse) + vec3<f32>(rim);
    return vec4<f32>(shaded, in.color.a);
}

