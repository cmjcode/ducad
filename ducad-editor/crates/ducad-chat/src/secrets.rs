//! Penyimpanan kunci API. Urutan baca: variabel lingkungan → Keychain
//! (macOS/iOS) → berkas `~/.ducad/secrets.json` bermode 0600 (platform lain).
//! Kunci tidak pernah masuk log maupun `ai-chat.json`.

use crate::ProviderConfig;

/// Nama layanan Keychain.
pub const SERVICE: &str = "id.ducad.ai-chat";

/// Isi `cfg.api_key` bila ada kunci tersimpan.
pub fn load_into(cfg: &mut ProviderConfig) {
    if cfg.api_key.is_none() {
        cfg.api_key = load(cfg);
    }
}

pub fn load(cfg: &ProviderConfig) -> Option<String> {
    if let Some(var) = cfg.key_env() {
        if let Ok(v) = std::env::var(var) {
            if !v.trim().is_empty() {
                return Some(v.trim().to_string());
            }
        }
    }
    store::get(&cfg.key_account())
}

/// Simpan kunci untuk host provider; string kosong = hapus.
pub fn save(cfg: &ProviderConfig, key: &str) -> anyhow::Result<()> {
    let account = cfg.key_account();
    if key.trim().is_empty() {
        return store::delete(&account);
    }
    store::set(&account, key.trim())
}

/// `true` bila ada kunci tersimpan (tanpa membaca variabel lingkungan).
pub fn has_stored(cfg: &ProviderConfig) -> bool {
    store::get(&cfg.key_account()).is_some()
}

#[cfg(target_vendor = "apple")]
mod store {
    use security_framework::passwords;

    pub fn get(account: &str) -> Option<String> {
        let bytes = passwords::get_generic_password(super::SERVICE, account).ok()?;
        String::from_utf8(bytes).ok()
    }

    pub fn set(account: &str, key: &str) -> anyhow::Result<()> {
        passwords::set_generic_password(super::SERVICE, account, key.as_bytes())
            .map_err(|e| anyhow::anyhow!("gagal menyimpan ke Keychain: {e}"))
    }

    pub fn delete(account: &str) -> anyhow::Result<()> {
        match passwords::delete_generic_password(super::SERVICE, account) {
            Ok(()) => Ok(()),
            // Item tidak ada = sudah terhapus.
            Err(e) if e.code() == -25300 => Ok(()),
            Err(e) => Err(anyhow::anyhow!("gagal menghapus dari Keychain: {e}")),
        }
    }
}

#[cfg(not(target_vendor = "apple"))]
mod store {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn path() -> PathBuf {
        match std::env::var_os("HOME") {
            Some(h) => PathBuf::from(h).join(".ducad").join("secrets.json"),
            None => PathBuf::from("ducad-secrets.json"),
        }
    }

    fn read() -> BTreeMap<String, String> {
        std::fs::read_to_string(path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn write(map: &BTreeMap<String, String>) -> anyhow::Result<()> {
        let p = path();
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&p, serde_json::to_string(map)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    pub fn get(account: &str) -> Option<String> {
        read().remove(account)
    }

    pub fn set(account: &str, key: &str) -> anyhow::Result<()> {
        let mut m = read();
        m.insert(account.to_string(), key.to_string());
        write(&m)
    }

    pub fn delete(account: &str) -> anyhow::Result<()> {
        let mut m = read();
        if m.remove(account).is_some() {
            write(&m)?;
        }
        Ok(())
    }
}
