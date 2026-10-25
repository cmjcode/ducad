//! Membaca rilis terbaru dari GitHub Releases dan memilih aset untuk
//! sistem yang sedang berjalan.
//!
//! Nama aset mengikuti `.github/workflows/release.yml` (Windows/Linux) dan
//! `make bundle-macos` (macOS, diunggah manual):
//!
//! - `ducad-<versi>-windows-x86_64.zip` (+ `.sha256`)
//! - `ducad-<versi>-linux-x86_64.tar.gz` (+ `.sha256`)
//! - `DUCAD-<versi>-macos-arm64.dmg` (+ `.sha256`); dmg tanpa label
//!   arsitektur (`DUCAD-<versi>.dmg`, unggahan manual lama) juga diterima,
//!   arsitekturnya diperiksa `lipo` setelah di-mount
//!
//! Tarball `ducad-tools-*` (CLI/MCP headless untuk CI) tidak pernah dipilih.

use std::time::Duration;

use serde::Deserialize;

use crate::version::Version;
use crate::UpdateError;

/// Repositori rilis resmi.
pub const DEFAULT_REPO: &str = "cmjcode/ducad";

/// Env var untuk menunjuk repositori lain (`pemilik/nama`), mis. fork uji.
pub const REPO_ENV: &str = "DUCAD_UPDATE_REPO";

const API_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Os {
    Windows,
    Linux,
    MacOs,
}

/// Sistem tujuan paket: OS + arsitektur (`std::env::consts::ARCH`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub os: Os,
    pub arch: &'static str,
}

impl Target {
    /// `None` di platform tanpa paket desktop (iOS, Android, …).
    pub fn current() -> Option<Self> {
        let os = match std::env::consts::OS {
            "windows" => Os::Windows,
            "linux" => Os::Linux,
            "macos" => Os::MacOs,
            _ => return None,
        };
        Some(Self { os, arch: std::env::consts::ARCH })
    }
}

/// Ejaan arsitektur yang lazim dipakai di nama berkas rilis.
fn arch_aliases(arch: &str) -> &'static [&'static str] {
    match arch {
        "x86_64" => &["x86_64", "x64", "amd64"],
        "aarch64" => &["aarch64", "arm64"],
        _ => &[],
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct Release {
    pub version: Version,
    pub tag: String,
    pub title: String,
    /// Catatan rilis (Markdown mentah dari GitHub).
    pub notes: String,
    /// Halaman rilis di github.com, untuk unduhan manual.
    pub page_url: String,
    pub assets: Vec<Asset>,
}

impl Release {
    /// Paket aplikasi untuk `target`, bila rilis ini menyediakannya.
    pub fn package_for(&self, target: &Target) -> Option<&Asset> {
        select_package(&self.assets, target)
    }

    /// Berkas `<paket>.sha256` pendamping paket.
    pub fn checksum_for(&self, package: &Asset) -> Option<&Asset> {
        let wanted = format!("{}.sha256", package.name);
        self.assets.iter().find(|a| a.name == wanted)
    }
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

/// Normalisasi nama repositori (`pemilik/nama`), menerima format slug atau URL lengkap.
pub fn normalize_repo(raw: &str) -> String {
    let raw = raw.trim();
    if let Some(pos) = raw.find("github.com/") {
        let after = &raw[pos + "github.com/".len()..];
        let parts: Vec<&str> = after.split('/').filter(|p| !p.is_empty()).collect();
        if parts.len() >= 2 {
            return format!("{}/{}", parts[0], parts[1]);
        }
    }
    raw.to_string()
}

/// Repositori yang dipakai: env [`REPO_ENV`] bila valid, selain itu
/// [`DEFAULT_REPO`].
pub fn repo() -> String {
    std::env::var(REPO_ENV)
        .ok()
        .map(|r| normalize_repo(&r))
        .filter(|r| r.split('/').count() == 2 && !r.contains(char::is_whitespace))
        .unwrap_or_else(|| DEFAULT_REPO.to_string())
}

pub(crate) fn user_agent() -> String {
    format!("DUCAD-updater/{}", env!("CARGO_PKG_VERSION"))
}

/// Rilis final terbaru. Mencoba GitHub REST API terlebih dahulu untuk
/// metadata lengkap (catatan rilis, ukuran aset), dan otomatis beralih ke
/// pemeriksaan web `https://github.com/{repo}/releases/latest` bila terkena
/// rate limit GitHub (HTTP 403/429) atau galat API lainnya. Memblokir —
/// panggil dari thread latar.
pub fn fetch_latest(repo: &str) -> Result<Release, UpdateError> {
    let repo = normalize_repo(repo);
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let agent = ureq::AgentBuilder::new().timeout(API_TIMEOUT).build();
    let response = agent
        .get(&url)
        .set("User-Agent", &user_agent())
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .call();
    match response {
        Ok(r) => match r.into_json::<ApiRelease>() {
            Ok(api) => return parse_release(api),
            Err(e) => {
                log::warn!("Gagal parse JSON API rilis ({e}), beralih ke pemeriksaan web...");
            }
        },
        Err(ureq::Error::Status(404, _)) => return Err(UpdateError::NoRelease),
        Err(ureq::Error::Status(403 | 429, _)) => {
            log::info!("GitHub API rate limit tercapai, memeriksa lewat web releases/latest...");
        }
        Err(e) => {
            log::warn!("GitHub API error ({e}), memeriksa lewat web releases/latest...");
        }
    }
    // Fallback kuat ke https://github.com/{repo}/releases/latest
    fetch_latest_from_web(&repo)
}

/// Rilis terbaru dari halaman web GitHub Releases (`https://github.com/{repo}/releases/latest`).
/// Tidak terpengaruh pembatasan rate limit GitHub REST API (60 req/jam).
pub fn fetch_latest_from_web(repo: &str) -> Result<Release, UpdateError> {
    let repo = normalize_repo(repo);
    let url = format!("https://github.com/{repo}/releases/latest");
    let agent = ureq::AgentBuilder::new()
        .timeout(API_TIMEOUT)
        .redirects(5)
        .build();
    let response = agent
        .get(&url)
        .set("User-Agent", &user_agent())
        .call()
        .map_err(|e| UpdateError::Network(e.to_string()))?;

    // Tanpa rilis, GitHub mengalihkan ke `/releases` (tanpa `/tag/…`).
    let final_url = response.get_url().to_string();
    let tag = tag_from_release_url(&final_url).ok_or(UpdateError::NoRelease)?.to_string();
    let version = Version::parse(&tag).ok_or(UpdateError::NoRelease)?;

    // Daftar aset dari fragmen HTML `expanded_assets`. Bila gagal, rilis
    // tetap dilaporkan tanpa aset: UI lalu menawarkan halaman unduhan alih-alih
    // menebak nama berkas (tebakan yang salah = unduhan 404).
    let assets_url = format!("https://github.com/{repo}/releases/expanded_assets/{tag}");
    let assets = match agent.get(&assets_url).set("User-Agent", &user_agent()).call() {
        Ok(resp) => match resp.into_string() {
            Ok(html) => parse_expanded_assets_html(&html, &repo, &tag),
            Err(e) => {
                log::warn!("Daftar aset rilis {tag} tidak terbaca: {e}");
                Vec::new()
            }
        },
        Err(e) => {
            log::warn!("Daftar aset rilis {tag} tidak terbaca: {e}");
            Vec::new()
        }
    };

    Ok(Release {
        version,
        title: tag.clone(),
        tag,
        notes: String::new(),
        page_url: final_url,
        assets,
    })
}

/// Tag dari URL halaman rilis (`…/releases/tag/v0.7.9` → `v0.7.9`).
fn tag_from_release_url(url: &str) -> Option<&str> {
    let (_, rest) = url.split_once("/releases/tag/")?;
    rest.split(['/', '?', '#']).next().filter(|t| !t.is_empty())
}

/// Aset dari fragmen HTML `releases/expanded_assets/{tag}` (tautan
/// `/{repo}/releases/download/{tag}/{nama}`).
pub(crate) fn parse_expanded_assets_html(html: &str, repo: &str, tag: &str) -> Vec<Asset> {
    let prefix = format!("/{repo}/releases/download/{tag}/");
    let mut assets: Vec<Asset> = Vec::new();
    for chunk in html.split(prefix.as_str()).skip(1) {
        let Some(end) = chunk.find(['"', '\'', '<', ' ']) else { continue };
        let name = &chunk[..end];
        if name.is_empty() || name.contains('/') || assets.iter().any(|a| a.name == name) {
            continue;
        }
        assets.push(Asset {
            name: name.to_string(),
            browser_download_url: format!("https://github.com{prefix}{name}"),
            size: 0,
        });
    }
    assets
}

fn parse_release(api: ApiRelease) -> Result<Release, UpdateError> {
    if api.draft || api.prerelease {
        return Err(UpdateError::NoRelease);
    }
    let version = Version::parse(&api.tag_name).ok_or(UpdateError::NoRelease)?;
    Ok(Release {
        version,
        title: api.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| api.tag_name.clone()),
        tag: api.tag_name,
        notes: api.body.unwrap_or_default(),
        page_url: api.html_url,
        assets: api.assets,
    })
}

/// Memilih paket aplikasi (bukan `ducad-tools-*`) untuk `target`.
pub fn select_package<'a>(assets: &'a [Asset], target: &Target) -> Option<&'a Asset> {
    let aliases = arch_aliases(target.arch);
    let names_arch = |a: &Asset, arches: &[&str]| {
        let name = a.name.to_ascii_lowercase();
        arches.iter().any(|arch| name.contains(arch))
    };
    match target.os {
        Os::Windows | Os::Linux => {
            let (os, ext) = match target.os {
                Os::Windows => ("windows", ".zip"),
                _ => ("linux", ".tar.gz"),
            };
            assets.iter().find(|a| {
                let name = a.name.to_ascii_lowercase();
                name.starts_with("ducad-")
                    && !name.starts_with("ducad-tools-")
                    && aliases.iter().any(|arch| name.ends_with(&format!("-{os}-{arch}{ext}")))
            })
        }
        Os::MacOs => {
            let dmgs: Vec<&Asset> =
                assets.iter().filter(|a| a.name.to_ascii_lowercase().ends_with(".dmg")).collect();
            // 1) dmg untuk arsitektur ini atau universal.
            if let Some(a) = dmgs.iter().find(|a| names_arch(a, aliases) || names_arch(a, &["universal"])) {
                return Some(a);
            }
            // 2) dmg tanpa label arsitektur (`DUCAD-0.4.6.dmg`). Dmg yang jelas
            //    untuk arsitektur LAIN dilewati.
            let other: Vec<&str> = ["x86_64", "aarch64"]
                .into_iter()
                .filter(|arch| *arch != target.arch)
                .flat_map(|arch| arch_aliases(arch).iter().copied())
                .collect();
            dmgs.into_iter().find(|a| !names_arch(a, &other))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> Asset {
        Asset { name: name.to_string(), browser_download_url: format!("https://x/{name}"), size: 1 }
    }

    fn assets(names: &[&str]) -> Vec<Asset> {
        names.iter().map(|n| asset(n)).collect()
    }

    const CI_ASSETS: &[&str] = &[
        "DUCAD-0.4.6.dmg",
        "ducad-0.4.6-linux-x86_64.tar.gz",
        "ducad-0.4.6-linux-x86_64.tar.gz.sha256",
        "ducad-0.4.6-windows-x86_64.zip",
        "ducad-0.4.6-windows-x86_64.zip.sha256",
        "ducad-tools-linux-x86_64.tar.gz",
        "ducad-tools-windows-x86_64.zip",
        "ducad-tools-macos-arm64.tar.gz",
    ];

    fn pick(os: Os, arch: &'static str, names: &[&str]) -> Option<String> {
        let list = assets(names);
        select_package(&list, &Target { os, arch }).map(|a| a.name.clone())
    }

    #[test]
    fn picks_app_package_not_tools() {
        assert_eq!(
            pick(Os::Linux, "x86_64", CI_ASSETS).as_deref(),
            Some("ducad-0.4.6-linux-x86_64.tar.gz")
        );
        assert_eq!(
            pick(Os::Windows, "x86_64", CI_ASSETS).as_deref(),
            Some("ducad-0.4.6-windows-x86_64.zip")
        );
        assert_eq!(pick(Os::MacOs, "aarch64", CI_ASSETS).as_deref(), Some("DUCAD-0.4.6.dmg"));
    }

    #[test]
    fn ci_mac_dmg_is_arch_labelled() {
        let names = ["DUCAD-0.5.3-macos-arm64.dmg", "ducad-tools-macos-arm64.tar.gz"];
        assert_eq!(pick(Os::MacOs, "aarch64", &names).as_deref(), Some("DUCAD-0.5.3-macos-arm64.dmg"));
        assert_eq!(pick(Os::MacOs, "x86_64", &names), None);
    }

    #[test]
    fn missing_arch_yields_none() {
        assert_eq!(pick(Os::Linux, "aarch64", CI_ASSETS), None);
        assert_eq!(pick(Os::Windows, "aarch64", &["ducad-tools-windows-aarch64.zip"]), None);
    }

    #[test]
    fn mac_prefers_matching_arch_and_skips_other_arch() {
        let names = ["DUCAD-1.0.0-x86_64.dmg", "DUCAD-1.0.0-arm64.dmg"];
        assert_eq!(pick(Os::MacOs, "aarch64", &names).as_deref(), Some("DUCAD-1.0.0-arm64.dmg"));
        assert_eq!(pick(Os::MacOs, "x86_64", &names).as_deref(), Some("DUCAD-1.0.0-x86_64.dmg"));
        assert_eq!(pick(Os::MacOs, "x86_64", &["DUCAD-1.0.0-arm64.dmg"]), None);
        assert_eq!(
            pick(Os::MacOs, "x86_64", &["DUCAD-1.0.0-universal.dmg"]).as_deref(),
            Some("DUCAD-1.0.0-universal.dmg")
        );
    }

    #[test]
    fn checksum_is_paired_by_name() {
        let release = Release {
            version: Version::parse("0.4.6").unwrap(),
            tag: "v0.4.6".into(),
            title: "v0.4.6".into(),
            notes: String::new(),
            page_url: String::new(),
            assets: assets(CI_ASSETS),
        };
        let target = Target { os: Os::Linux, arch: "x86_64" };
        let pkg = release.package_for(&target).unwrap();
        assert_eq!(
            release.checksum_for(pkg).map(|a| a.name.as_str()),
            Some("ducad-0.4.6-linux-x86_64.tar.gz.sha256")
        );
        let mac = release.package_for(&Target { os: Os::MacOs, arch: "aarch64" }).unwrap();
        assert!(release.checksum_for(mac).is_none());
    }

    #[test]
    fn parses_github_api_payload() {
        let json = r#"{
            "tag_name": "v0.4.6", "name": "", "body": "Fixes",
            "html_url": "https://github.com/cmjcode/ducad/releases/tag/v0.4.6",
            "draft": false, "prerelease": false,
            "assets": [{"name": "DUCAD-0.4.6.dmg", "size": 42,
                        "browser_download_url": "https://github.com/x/DUCAD-0.4.6.dmg"}]
        }"#;
        let release = parse_release(serde_json::from_str(json).unwrap()).unwrap();
        assert_eq!(release.version.to_string(), "0.4.6");
        assert_eq!(release.title, "v0.4.6");
        assert_eq!(release.assets[0].size, 42);

        let pre = r#"{"tag_name": "v0.5.0-rc.1", "html_url": "h", "prerelease": true}"#;
        assert!(matches!(
            parse_release(serde_json::from_str(pre).unwrap()),
            Err(UpdateError::NoRelease)
        ));
    }

    #[test]
    fn test_normalize_repo() {
        assert_eq!(normalize_repo("cmjcode/ducad"), "cmjcode/ducad");
        assert_eq!(
            normalize_repo("https://github.com/cmjcode/ducad/releases/latest"),
            "cmjcode/ducad"
        );
        assert_eq!(
            normalize_repo("https://github.com/cmjcode/ducad"),
            "cmjcode/ducad"
        );
    }

    #[test]
    fn test_parse_expanded_assets_html() {
        let html = r#"
            <div>
              <a href="/cmjcode/ducad/releases/download/v0.7.9/DUCAD-0.7.9.dmg">DUCAD-0.7.9.dmg</a>
              <a href="/cmjcode/ducad/releases/download/v0.7.9/DUCAD-0.7.9.dmg.sha256">DUCAD-0.7.9.dmg.sha256</a>
            </div>
        "#;
        let assets = parse_expanded_assets_html(html, "cmjcode/ducad", "v0.7.9");
        assert_eq!(assets.len(), 2);
        assert_eq!(assets[0].name, "DUCAD-0.7.9.dmg");
        assert_eq!(
            assets[0].browser_download_url,
            "https://github.com/cmjcode/ducad/releases/download/v0.7.9/DUCAD-0.7.9.dmg"
        );
        assert_eq!(assets[1].name, "DUCAD-0.7.9.dmg.sha256");
    }

    #[test]
    fn tag_from_release_url_handles_redirects() {
        assert_eq!(
            tag_from_release_url("https://github.com/cmjcode/ducad/releases/tag/v0.7.9"),
            Some("v0.7.9")
        );
        assert_eq!(tag_from_release_url("https://github.com/o/r/releases/tag/v1.0.0?x=1"), Some("v1.0.0"));
        // Repo tanpa rilis: GitHub mengalihkan ke daftar rilis.
        assert_eq!(tag_from_release_url("https://github.com/cmjcode/ducad/releases"), None);
        assert_eq!(tag_from_release_url("https://github.com/o/r/releases/tag/"), None);
    }

    #[test]
    fn expanded_assets_feed_package_selection() {
        let html = r#"<a href="/o/r/releases/download/v1.2.3/ducad-1.2.3-windows-x86_64.zip" rel="nofollow">
            <a href="/o/r/releases/download/v1.2.3/ducad-1.2.3-windows-x86_64.zip.sha256">
            <a href="/o/r/releases/download/v1.2.3/ducad-1.2.3-linux-x86_64.tar.gz">
            <a href="/o/r/releases/download/v1.2.3/DUCAD-1.2.3.dmg">
            <a href="/o/r/archive/refs/tags/v1.2.3.zip">"#;
        let list = parse_expanded_assets_html(html, "o/r", "v1.2.3");
        assert_eq!(list.len(), 4);
        let win = select_package(&list, &Target { os: Os::Windows, arch: "x86_64" }).unwrap();
        assert_eq!(win.name, "ducad-1.2.3-windows-x86_64.zip");
        let lin = select_package(&list, &Target { os: Os::Linux, arch: "x86_64" }).unwrap();
        assert_eq!(lin.name, "ducad-1.2.3-linux-x86_64.tar.gz");
        let mac = select_package(&list, &Target { os: Os::MacOs, arch: "aarch64" }).unwrap();
        assert_eq!(mac.name, "DUCAD-1.2.3.dmg");
    }

    #[test]
    #[ignore]
    fn live_web_latest_release_fetch() {
        let release = fetch_latest_from_web(DEFAULT_REPO).unwrap();
        println!("Web check tag: {}, assets: {}", release.tag, release.assets.len());
        assert!(!release.tag.is_empty());
        assert!(!release.assets.is_empty());
    }
}
