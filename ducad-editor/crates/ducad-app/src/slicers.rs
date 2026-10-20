//! Slicer terpasang (Paritas 3.12): deteksi PrusaSlicer, Bambu Studio,
//! OrcaSlicer, UltiMaker Cura di lokasi standar tiap OS, lalu "Buka di
//! slicer" = ekspor 3MF ke folder sementara dan jalankan slicer dengan
//! berkas itu.

use std::path::{Path, PathBuf};

/// Satu slicer yang ditemukan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slicer {
    pub name: &'static str,
    /// Lokasi app/biner yang ditemukan.
    pub path: PathBuf,
}

/// (nama, bundle macOS, exe Windows relatif `Program Files`, biner Linux).
const KNOWN: &[(&str, &str, &str, &str)] = &[
    ("PrusaSlicer", "PrusaSlicer.app", "Prusa3D/PrusaSlicer/prusa-slicer.exe", "prusa-slicer"),
    ("Bambu Studio", "BambuStudio.app", "Bambu Studio/bambu-studio.exe", "bambu-studio"),
    ("OrcaSlicer", "OrcaSlicer.app", "OrcaSlicer/orca-slicer.exe", "orca-slicer"),
    ("UltiMaker Cura", "UltiMaker Cura.app", "UltiMaker Cura/UltiMaker-Cura.exe", "cura"),
];

/// Cari slicer di folder kandidat (dipisah agar bisa dites).
pub fn detect_in(app_dirs: &[PathBuf], bin_dirs: &[PathBuf]) -> Vec<Slicer> {
    let mut out = Vec::new();
    for (name, mac, win, bin) in KNOWN {
        let hit = app_dirs
            .iter()
            .flat_map(|d| [d.join(mac), d.join(win)])
            .chain(bin_dirs.iter().map(|d| d.join(bin)))
            .find(|p| p.exists());
        if let Some(path) = hit {
            out.push(Slicer { name, path });
        }
    }
    out
}

/// Folder standar tempat app dipasang di OS ini.
fn standard_dirs() -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut apps = Vec::new();
    let mut bins = Vec::new();
    if cfg!(target_os = "macos") {
        apps.push(PathBuf::from("/Applications"));
        if let Some(home) = std::env::var_os("HOME") {
            apps.push(Path::new(&home).join("Applications"));
        }
    } else if cfg!(target_os = "windows") {
        for var in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            if let Some(v) = std::env::var_os(var) {
                apps.push(PathBuf::from(v));
            }
        }
    } else {
        bins.extend(["/usr/bin", "/usr/local/bin", "/snap/bin"].map(PathBuf::from));
        if let Some(home) = std::env::var_os("HOME") {
            bins.push(Path::new(&home).join(".local/bin"));
        }
        if let Some(path) = std::env::var_os("PATH") {
            bins.extend(std::env::split_paths(&path));
        }
    }
    (apps, bins)
}

/// Slicer yang terpasang di komputer ini.
pub fn detect() -> Vec<Slicer> {
    let (apps, bins) = standard_dirs();
    detect_in(&apps, &bins)
}

/// Jalankan slicer dengan berkas `file`.
pub fn open_with(slicer: &Slicer, file: &Path) -> anyhow::Result<()> {
    let is_bundle = slicer.path.extension().is_some_and(|e| e == "app");
    let mut cmd = if cfg!(target_os = "macos") && is_bundle {
        let mut c = std::process::Command::new("open");
        c.arg("-a").arg(&slicer.path).arg(file);
        c
    } else {
        let mut c = std::process::Command::new(&slicer.path);
        c.arg(file);
        c
    };
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("gagal menjalankan {}: {e}", slicer.name))
}

impl crate::app::DuCADApp {
    /// Ekspor body terlihat ke 3MF sementara lalu buka di slicer pertama
    /// yang terdeteksi (atau ke-`index`).
    pub fn open_in_slicer(&mut self, index: usize) {
        let slicers = detect();
        let Some(slicer) = slicers.get(index).or(slicers.first()) else {
            self.model_status = Some(ducad_i18n::t!("slicer-none"));
            return;
        };
        let bodies: Vec<(String, ducad_kernel::KernelMesh)> = self
            .model
            .doc
            .bodies
            .iter()
            .filter(|(_, b)| b.visible)
            .filter_map(|(id, b)| {
                self.model
                    .geometry
                    .get(id)
                    .map(|g| (b.name.clone(), g.mesh.as_ref().clone()))
            })
            .collect();
        if bodies.is_empty() {
            self.model_status = Some(ducad_i18n::t!("body-tool-need-body"));
            return;
        }
        let refs: Vec<(&str, &ducad_kernel::KernelMesh, Option<[f32; 3]>)> =
            bodies.iter().map(|(n, m)| (n.as_str(), m, None)).collect();
        let file = std::env::temp_dir().join(format!("ducad-{}.3mf", std::process::id()));
        let res = ducad_io::mesh_formats::write_3mf(&refs, &file)
            .and_then(|_| open_with(slicer, &file));
        self.model_status = Some(match res {
            Ok(()) => ducad_i18n::t!("slicer-opened", name = slicer.name),
            Err(e) => ducad_i18n::t!("slicer-failed", error = e.to_string()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_only_existing_installs() {
        let dir = std::env::temp_dir().join(format!("ducad-slicer-test-{}", std::process::id()));
        let apps = dir.join("apps");
        let bins = dir.join("bin");
        std::fs::create_dir_all(apps.join("OrcaSlicer.app")).unwrap();
        std::fs::create_dir_all(&bins).unwrap();
        std::fs::write(bins.join("prusa-slicer"), b"").unwrap();
        let found = detect_in(&[apps], &[bins]);
        let names: Vec<&str> = found.iter().map(|s| s.name).collect();
        assert_eq!(names, vec!["PrusaSlicer", "OrcaSlicer"]);
        let _ = std::fs::remove_dir_all(dir);
        assert!(detect_in(&[], &[]).is_empty());
    }
}
