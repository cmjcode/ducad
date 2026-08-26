//! API Client untuk berkomunikasi dengan CMJCode Server.

use std::fs;
use std::path::Path;

pub const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:3000";

/// Mendeteksi URL server secara otomatis dari environment variable atau file `.env`.
pub fn detect_server_url() -> String {
    // 1. Cek environment variables
    if let Ok(url) = std::env::var("CMJCODE_SERVER_URL") {
        if !url.trim().is_empty() {
            return url.trim().trim_end_matches('/').to_string();
        }
    }
    if let Ok(url) = std::env::var("SERVER_BASE_URL") {
        if !url.trim().is_empty() {
            return url.trim().trim_end_matches('/').to_string();
        }
    }

    // 2. Cek file .env di direktori saat ini
    if let Some(url) = read_env_var_from_file(".env", "CMJCODE_SERVER_URL")
        .or_else(|| read_env_var_from_file(".env", "SERVER_BASE_URL"))
    {
        return url.trim_end_matches('/').to_string();
    }

    DEFAULT_SERVER_URL.to_string()
}

fn read_env_var_from_file(path: impl AsRef<Path>, key: &str) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                let val = v.trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct CloudClient {
    pub server_url: String,
}

impl Default for CloudClient {
    fn default() -> Self {
        Self {
            server_url: detect_server_url(),
        }
    }
}

impl CloudClient {
    pub fn new(server_url: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into(),
        }
    }
}
