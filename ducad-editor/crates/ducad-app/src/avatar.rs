//! Pemuat foto profil akun login untuk avatar di header dan popup Akun.
//!
//! Unduhan (atau baca cache) berjalan di thread latar lewat
//! `ducad_cloud::fetch_avatar`; frame GUI hanya memeriksa kanal tanpa
//! memblokir. Selama foto belum ada — atau akun Apple yang memang tidak
//! memberi foto, atau unduhan gagal — avatar jatuh ke inisial.

use std::io::Cursor;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use ducad_cloud::DucadAccount;
use image::{DynamicImage, ImageDecoder};

type FetchResult = Result<Vec<u8>, String>;
type UploadResult = Result<DucadAccount, String>;

/// Sisi foto profil yang diunggah (px): cukup tajam untuk avatar 40 pt @3x.
const UPLOAD_PIXELS: u32 = 256;

/// Batas berkas sumber yang mau dibaca dari disk sebelum diperkecil.
const MAX_SOURCE_BYTES: u64 = 25 * 1024 * 1024;

#[derive(Default)]
pub struct AvatarLoader {
    /// URL yang sedang/terakhir dimuat; berubah = muat ulang.
    url: Option<String>,
    rx: Option<Receiver<FetchResult>>,
    texture: Option<egui::TextureHandle>,
    upload_rx: Option<Receiver<UploadResult>>,
    /// Galat unggahan terakhir, ditampilkan di popup Akun.
    upload_error: Option<String>,
}

impl AvatarLoader {
    /// Tekstur foto profil bila sudah termuat.
    pub fn texture(&self) -> Option<&egui::TextureHandle> {
        self.texture.as_ref()
    }

    /// Keadaan untuk popup Akun.
    pub fn ui_state(&self) -> ducad_ui::AccountAvatar<'_> {
        ducad_ui::AccountAvatar {
            photo: self.texture.as_ref(),
            loading: self.rx.is_some(),
            uploading: self.upload_rx.is_some(),
            error: self.upload_error.as_deref(),
        }
    }

    /// Membaca berkas gambar pilihan pengguna, memperkecilnya, lalu
    /// mengunggahnya ke server di thread latar. Hasilnya diambil `poll_upload`.
    pub fn start_upload(
        &mut self,
        ctx: &egui::Context,
        server_url: &str,
        account: &DucadAccount,
        path: &std::path::Path,
    ) {
        self.upload_error = None;
        let image = match read_source(path).and_then(|raw| prepare_avatar_image(&raw)) {
            Ok(image) => image,
            Err(err) => {
                self.upload_error = Some(err);
                return;
            }
        };
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        let server_url = server_url.to_string();
        let account = account.clone();
        let spawned = std::thread::Builder::new()
            .name("ducad-avatar-upload".into())
            .spawn(move || {
                let _ = tx.send(ducad_cloud::upload_avatar(&server_url, &account, &image));
                ctx.request_repaint();
            });
        match spawned {
            Ok(_) => self.upload_rx = Some(rx),
            Err(err) => self.upload_error = Some(format!("Gagal memulai unggahan: {err}")),
        }
    }

    /// Akun terbaru (token + `avatar_url` baru) begitu unggahan berhasil.
    /// Galat disimpan untuk ditampilkan di popup, bukan dikembalikan.
    pub fn poll_upload(&mut self) -> Option<DucadAccount> {
        let rx = self.upload_rx.as_ref()?;
        match rx.try_recv() {
            Ok(Ok(account)) => {
                self.upload_rx = None;
                Some(account)
            }
            Ok(Err(err)) => {
                self.upload_rx = None;
                self.upload_error = Some(err);
                None
            }
            Err(TryRecvError::Disconnected) => {
                self.upload_rx = None;
                self.upload_error = Some("Unggahan foto profil terhenti".to_string());
                None
            }
            Err(TryRecvError::Empty) => None,
        }
    }

    /// Dipanggil sekali per frame: memulai unduhan saat URL akun berubah dan
    /// mengambil hasil thread latar bila sudah selesai.
    pub fn update(&mut self, ctx: &egui::Context, account: Option<&DucadAccount>) {
        let wanted = account
            .and_then(|acc| acc.avatar_url.as_deref())
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(str::to_owned);

        if wanted != self.url {
            self.url = wanted.clone();
            self.texture = None;
            self.rx = wanted.and_then(|url| spawn_fetch(ctx, url));
        }

        let Some(rx) = &self.rx else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(bytes)) => {
                self.texture =
                    crate::chat_ui::texture_from_image_bytes(ctx, &bytes, "ducad-account-avatar");
                if self.texture.is_none() {
                    log::info!("Foto profil tidak bisa didekode; memakai inisial");
                }
                self.rx = None;
            }
            Ok(Err(err)) => {
                log::info!("Foto profil tidak dimuat: {err}");
                self.rx = None;
            }
            Err(TryRecvError::Disconnected) => self.rx = None,
            Err(TryRecvError::Empty) => {}
        }
    }
}

fn spawn_fetch(ctx: &egui::Context, url: String) -> Option<Receiver<FetchResult>> {
    let (tx, rx) = mpsc::channel();
    let ctx = ctx.clone();
    let spawned = std::thread::Builder::new()
        .name("ducad-avatar".into())
        .spawn(move || {
            let _ = tx.send(ducad_cloud::fetch_avatar(&url));
            ctx.request_repaint();
        });
    match spawned {
        Ok(_) => Some(rx),
        Err(err) => {
            log::warn!("Thread foto profil gagal dibuat: {err}");
            None
        }
    }
}

fn read_source(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let too_big = std::fs::metadata(path).map(|m| m.len() > MAX_SOURCE_BYTES).unwrap_or(false);
    if too_big {
        return Err("Berkas foto terlalu besar (maksimal 25 MB)".to_string());
    }
    std::fs::read(path).map_err(|e| format!("Gagal membaca foto: {e}"))
}

/// Menyiapkan foto untuk diunggah: putar sesuai orientasi EXIF, potong
/// persegi di tengah, perkecil ke 256 px, lalu tulis ulang sebagai JPEG.
/// Menulis ulang sekaligus membuang metadata (lokasi GPS, kamera) dari foto
/// asli, dan memastikan server hanya menerima PNG/JPEG yang sah.
pub fn prepare_avatar_image(raw: &[u8]) -> Result<Vec<u8>, String> {
    let unreadable = |e: image::ImageError| format!("Berkas bukan gambar yang didukung: {e}");
    let mut decoder = image::ImageReader::new(Cursor::new(raw))
        .with_guessed_format()
        .map_err(|e| format!("Gagal membaca foto: {e}"))?
        .into_decoder()
        .map_err(unreadable)?;
    let orientation = decoder.orientation().map_err(unreadable)?;
    let mut img = DynamicImage::from_decoder(decoder).map_err(unreadable)?;
    img.apply_orientation(orientation);

    let side = img.width().min(img.height());
    if side == 0 {
        return Err("Foto kosong".to_string());
    }
    let square = img.crop_imm((img.width() - side) / 2, (img.height() - side) / 2, side, side);
    let target = side.min(UPLOAD_PIXELS);
    let rgb = square
        .resize_exact(target, target, image::imageops::FilterType::Lanczos3)
        .to_rgb8();

    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 88)
        .encode_image(&rgb)
        .map_err(|e| format!("Gagal menyiapkan foto: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(width, height, image::Rgba([200, 100, 50, 255]));
        let mut out = Vec::new();
        DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn large_photo_becomes_square_jpeg_256() {
        let jpeg = prepare_avatar_image(&png(1200, 800)).unwrap();
        assert!(jpeg.starts_with(&[0xFF, 0xD8, 0xFF]), "hasil harus JPEG");
        let back = image::load_from_memory(&jpeg).unwrap();
        assert_eq!((back.width(), back.height()), (256, 256));
    }

    #[test]
    fn small_photo_is_not_upscaled() {
        let back = image::load_from_memory(&prepare_avatar_image(&png(90, 120)).unwrap()).unwrap();
        assert_eq!((back.width(), back.height()), (90, 90));
    }

    #[test]
    fn non_image_rejected() {
        assert!(prepare_avatar_image(b"bukan gambar sama sekali").is_err());
        assert!(prepare_avatar_image(b"").is_err());
    }
}
