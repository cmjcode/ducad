//! Verifikasi tanda tangan kriptografis Ed25519 / Minisign atas berkas manifest
//! checksum `<paket>.sha256` sebelum nilai SHA-256 di dalamnya dipercayai.
//!
//! Mendukung dua format tanda tangan standar di GitHub Releases:
//! 1. **Minisign** (`<paket>.sha256.minisig`, dari `minisign -Sm`): mendukung
//!    mode modern `ED` (Ed25519 atas BLAKE2b-512), mode murni `Ed` (`minisign -l`),
//!    kecocokan `keynum` 8-byte, serta tanda tangan global pada `trusted comment`.
//! 2. **Ed25519 mentah** (`<paket>.sha256.sig`): tanda tangan 64-byte dalam
//!    encoding Base64 standar atau 128 karakter hex atas byte mentah `.sha256`.
//!
//! Format kunci publik yang diterima (lewat [`DEFAULT_UPDATE_PUBKEY`] atau
//! variabel lingkungan [`PUBKEY_ENV`]):
//! - Kunci publik Minisign (`RW...` Base64 42-byte, atau berkas `.pub` 2 baris),
//! - Kunci publik Ed25519 mentah 32-byte dalam Base64 (44 karakter) atau hex
//!   (64 karakter).

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use ring::signature::{UnparsedPublicKey, ED25519};

use crate::UpdateError;

/// Variabel lingkungan untuk mengganti atau mengaktifkan kunci publik Ed25519
/// verifikasi manifest pembaruan (misal untuk pengujian rilis atau kunci baru).
pub const PUBKEY_ENV: &str = "DUCAD_UPDATE_PUBKEY";

/// Kunci publik rilis resmi DUCAD (Minisign `RW...` atau Ed25519 32-byte).
///
/// Diisi `Some("RW...")` begitu pasangan kunci penandatanganan rilis produksi
/// dibuat dan `DUCAD_UPDATE_MINISIGN_SECKEY` dipasang di GitHub Actions Secrets
/// (lihat `docs/PACKAGING.md`). Selama bernilai `None` dan `DUCAD_UPDATE_PUBKEY`
/// tidak diset, updater mempertahankan kompatibilitas mundur dengan rilis lama
/// yang hanya menyediakan `.sha256`.
pub const DEFAULT_UPDATE_PUBKEY: Option<&str> = None;

/// Kunci publik aktif dari [`PUBKEY_ENV`] atau [`DEFAULT_UPDATE_PUBKEY`].
pub fn configured_pubkey() -> Option<String> {
    if let Ok(val) = std::env::var(PUBKEY_ENV) {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    DEFAULT_UPDATE_PUBKEY
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedPublicKey {
    key_id: Option<[u8; 8]>,
    bytes: [u8; 32],
}

/// Mengurai kunci publik Minisign (`RW...` atau berkas `.pub` dua baris) maupun
/// kunci publik Ed25519 mentah 32-byte (Base64 atau hex).
fn parse_public_key(raw: &str) -> Result<ParsedPublicKey, UpdateError> {
    let candidate = raw
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("untrusted comment:"))
        .ok_or_else(|| UpdateError::Signature("kunci publik pembaruan kosong".to_string()))?;

    if candidate.len() == 64 && candidate.bytes().all(|b| b.is_ascii_hexdigit()) {
        let decoded = hex::decode(candidate)
            .map_err(|e| UpdateError::Signature(format!("kunci publik hex rusak: {e}")))?;
        let bytes: [u8; 32] = decoded
            .try_into()
            .map_err(|_| UpdateError::Signature("panjang kunci publik hex bukan 32 byte".into()))?;
        return Ok(ParsedPublicKey { key_id: None, bytes });
    }

    let decoded = B64
        .decode(candidate)
        .map_err(|e| UpdateError::Signature(format!("kunci publik Base64 tidak valid: {e}")))?;

    match decoded.as_slice() {
        // Minisign public key: b"Ed" (2) || key_id (8) || pubkey (32) = 42 byte.
        [b'E', b'd', rest @ ..] if rest.len() == 40 => {
            let mut key_id = [0u8; 8];
            key_id.copy_from_slice(&rest[..8]);
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(&rest[8..40]);
            Ok(ParsedPublicKey {
                key_id: Some(key_id),
                bytes,
            })
        }
        raw_pk if raw_pk.len() == 32 => {
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(raw_pk);
            Ok(ParsedPublicKey { key_id: None, bytes })
        }
        _ => Err(UpdateError::Signature(
            "format kunci publik pembaruan tidak dikenali (bukan Minisign RW... atau Ed25519 32-byte)"
                .to_string(),
        )),
    }
}

/// Memverifikasi tanda tangan `sig_text` (`.minisig` atau `.sig`) terhadap
/// isi mentah `manifest_bytes` menggunakan `pubkey_str`.
pub fn verify_manifest_signature(
    manifest_bytes: &[u8],
    sig_text: &str,
    pubkey_str: &str,
) -> Result<(), UpdateError> {
    let pubkey = parse_public_key(pubkey_str)?;
    let lines: Vec<&str> = sig_text
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim().is_empty())
        .collect();

    if lines.is_empty() {
        return Err(UpdateError::Signature(
            "berkas tanda tangan manifest kosong".to_string(),
        ));
    }

    if lines[0].starts_with("untrusted comment:") {
        verify_minisign(manifest_bytes, &lines, &pubkey)
    } else {
        verify_raw_ed25519(manifest_bytes, lines[0].trim(), &pubkey)
    }
}

fn verify_minisign(
    manifest_bytes: &[u8],
    lines: &[&str],
    pubkey: &ParsedPublicKey,
) -> Result<(), UpdateError> {
    let sig_b64 = lines
        .get(1)
        .copied()
        .map(str::trim)
        .ok_or_else(|| UpdateError::Signature("berkas .minisig tidak memuat baris tanda tangan".into()))?;

    let decoded = B64
        .decode(sig_b64)
        .map_err(|e| UpdateError::Signature(format!("tanda tangan Minisign rusak: {e}")))?;

    if decoded.len() != 74 {
        return Err(UpdateError::Signature(format!(
            "panjang tanda tangan Minisign tidak valid ({} byte, harus 74)",
            decoded.len()
        )));
    }

    let alg = &decoded[0..2];
    let sig_key_id = &decoded[2..10];
    let sig_bytes = &decoded[10..74];

    if let Some(expected_id) = &pubkey.key_id {
        if sig_key_id != expected_id {
            return Err(UpdateError::Signature(
                "ID kunci tanda tangan Minisign tidak cocok dengan kunci publik DUCAD".to_string(),
            ));
        }
    }

    let verifier = UnparsedPublicKey::new(&ED25519, &pubkey.bytes);
    match alg {
        b"Ed" => verifier.verify(manifest_bytes, sig_bytes).map_err(|_| {
            UpdateError::Signature("verifikasi tanda tangan Ed25519 (Minisign Ed) gagal".into())
        })?,
        b"ED" => {
            let prehashed = blake2b_512(manifest_bytes);
            verifier.verify(&prehashed, sig_bytes).map_err(|_| {
                UpdateError::Signature(
                    "verifikasi tanda tangan Ed25519-BLAKE2b (Minisign ED) gagal".into(),
                )
            })?;
        }
        _ => {
            return Err(UpdateError::Signature(
                "algoritma tanda tangan Minisign tidak didukung".to_string(),
            ));
        }
    }

    // Jika baris `trusted comment:` tersedia (standar Minisign), tanda tangan
    // global pada baris ke-4 WAJIB ada dan sah.
    if let Some(trusted_line) = lines.get(2) {
        let comment = trusted_line
            .strip_prefix("trusted comment: ")
            .or_else(|| trusted_line.strip_prefix("trusted comment:"))
            .ok_or_else(|| {
                UpdateError::Signature("baris trusted comment Minisign tidak valid".to_string())
            })?;
        let global_b64 = lines.get(3).copied().map(str::trim).ok_or_else(|| {
            UpdateError::Signature("tanda tangan global trusted comment Minisign hilang".to_string())
        })?;
        let global_sig = B64.decode(global_b64).map_err(|e| {
            UpdateError::Signature(format!("tanda tangan global Minisign rusak: {e}"))
        })?;
        if global_sig.len() != 64 {
            return Err(UpdateError::Signature(
                "panjang tanda tangan global Minisign harus 64 byte".to_string(),
            ));
        }
        let mut global_msg = Vec::with_capacity(64 + comment.len());
        global_msg.extend_from_slice(sig_bytes);
        global_msg.extend_from_slice(comment.as_bytes());
        verifier.verify(&global_msg, &global_sig).map_err(|_| {
            UpdateError::Signature(
                "verifikasi tanda tangan global trusted comment Minisign gagal".to_string(),
            )
        })?;
    }

    Ok(())
}

fn verify_raw_ed25519(
    manifest_bytes: &[u8],
    token: &str,
    pubkey: &ParsedPublicKey,
) -> Result<(), UpdateError> {
    let sig_bytes = if token.len() == 128 && token.bytes().all(|b| b.is_ascii_hexdigit()) {
        hex::decode(token)
            .map_err(|e| UpdateError::Signature(format!("tanda tangan hex rusak: {e}")))?
    } else {
        B64.decode(token)
            .map_err(|e| UpdateError::Signature(format!("tanda tangan Base64 rusak: {e}")))?
    };

    if sig_bytes.len() != 64 {
        return Err(UpdateError::Signature(format!(
            "panjang tanda tangan Ed25519 tidak valid ({} byte, harus 64)",
            sig_bytes.len()
        )));
    }

    let verifier = UnparsedPublicKey::new(&ED25519, &pubkey.bytes);
    verifier.verify(manifest_bytes, &sig_bytes).map_err(|_| {
        UpdateError::Signature("verifikasi tanda tangan Ed25519 manifest gagal".to_string())
    })
}

// ---------------------------------------------------------------------------
// RFC 7693 BLAKE2b-512 murni Rust (untuk Minisign mode `ED`)

const BLAKE2B_IV: [u64; 8] = [
    0x6a09e667f3bcc908,
    0xbb67ae8584caa73b,
    0x3c6ef372fe94f82b,
    0xa54ff53a5f1d36f1,
    0x510e527fade682d1,
    0x9b05688c2b3e6c1f,
    0x1f83d9abfb41bd6b,
    0x5be0cd19137e2179,
];

const BLAKE2B_SIGMA: [[usize; 16]; 12] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
    [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
    [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
    [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
    [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
    [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
    [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
    [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
];

#[inline]
fn blake2b_g(v: &mut [u64; 16], a: usize, b: usize, c: usize, d: usize, x: u64, y: u64) {
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
    v[d] = (v[d] ^ v[a]).rotate_right(32);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(24);
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
    v[d] = (v[d] ^ v[a]).rotate_right(16);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(63);
}

fn blake2b_compress(h: &mut [u64; 8], block: &[u8; 128], t: u128, last: bool) {
    let mut m = [0u64; 16];
    for (i, slot) in m.iter_mut().enumerate() {
        let mut word = [0u8; 8];
        word.copy_from_slice(&block[i * 8..(i + 1) * 8]);
        *slot = u64::from_le_bytes(word);
    }

    let mut v = [0u64; 16];
    v[..8].copy_from_slice(h);
    v[8..16].copy_from_slice(&BLAKE2B_IV);
    v[12] ^= t as u64;
    v[13] ^= (t >> 64) as u64;
    if last {
        v[14] = !v[14];
    }

    for s in &BLAKE2B_SIGMA {
        blake2b_g(&mut v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
        blake2b_g(&mut v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
        blake2b_g(&mut v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
        blake2b_g(&mut v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
        blake2b_g(&mut v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
        blake2b_g(&mut v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
        blake2b_g(&mut v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
        blake2b_g(&mut v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
    }

    for i in 0..8 {
        h[i] ^= v[i] ^ v[i + 8];
    }
}

fn blake2b_512(input: &[u8]) -> [u8; 64] {
    let mut h = BLAKE2B_IV;
    // Parameter block: digest_length = 64, fanout = 1, depth = 1 -> 0x01010040
    h[0] ^= 0x0101_0040;

    if input.is_empty() {
        let empty = [0u8; 128];
        blake2b_compress(&mut h, &empty, 0, true);
    } else {
        let mut offset = 0usize;
        while input.len() - offset > 128 {
            let mut block = [0u8; 128];
            block.copy_from_slice(&input[offset..offset + 128]);
            offset += 128;
            blake2b_compress(&mut h, &block, offset as u128, false);
        }
        let mut last_block = [0u8; 128];
        let rem = &input[offset..];
        last_block[..rem.len()].copy_from_slice(rem);
        blake2b_compress(&mut h, &last_block, input.len() as u128, true);
    }

    let mut out = [0u8; 64];
    for (i, word) in h.iter().enumerate() {
        out[i * 8..(i + 1) * 8].copy_from_slice(&word.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::rand::SystemRandom;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    fn make_keypair() -> Ed25519KeyPair {
        let rng = SystemRandom::new();
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap()
    }

    fn minisign_pubkey_str(kp: &Ed25519KeyPair, key_id: [u8; 8]) -> String {
        let mut raw = Vec::with_capacity(42);
        raw.extend_from_slice(b"Ed");
        raw.extend_from_slice(&key_id);
        raw.extend_from_slice(kp.public_key().as_ref());
        format!(
            "untrusted comment: ducad release public key\n{}\n",
            B64.encode(raw)
        )
    }

    fn sign_minisign(
        kp: &Ed25519KeyPair,
        key_id: [u8; 8],
        manifest: &[u8],
        prehashed: bool,
    ) -> String {
        let (alg, sig) = if prehashed {
            let digest = blake2b_512(manifest);
            (b"ED", kp.sign(&digest))
        } else {
            (b"Ed", kp.sign(manifest))
        };
        let mut line2 = Vec::with_capacity(74);
        line2.extend_from_slice(alg);
        line2.extend_from_slice(&key_id);
        line2.extend_from_slice(sig.as_ref());

        let trusted = "timestamp:1762850000\tfile:ducad-0.8.0-linux-x86_64.tar.gz.sha256";
        let mut global_msg = Vec::new();
        global_msg.extend_from_slice(sig.as_ref());
        global_msg.extend_from_slice(trusted.as_bytes());
        let global_sig = kp.sign(&global_msg);

        format!(
            "untrusted comment: signature from minisign secret key\n{}\ntrusted comment: {}\n{}\n",
            B64.encode(line2),
            trusted,
            B64.encode(global_sig.as_ref()),
        )
    }

    #[test]
    fn blake2b_512_matches_rfc7693_vector() {
        let digest = hex::encode(blake2b_512(b"abc"));
        assert_eq!(
            digest,
            "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d1\
             7d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923"
        );
    }

    #[test]
    fn verifies_minisign_prehashed_and_pure_ed25519() {
        let kp = make_keypair();
        let key_id = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        let pubkey = minisign_pubkey_str(&kp, key_id);
        let manifest = b"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  ducad-0.8.0-linux-x86_64.tar.gz\n";

        let sig_ed = sign_minisign(&kp, key_id, manifest, true);
        verify_manifest_signature(manifest, &sig_ed, &pubkey).unwrap();

        let sig_legacy = sign_minisign(&kp, key_id, manifest, false);
        verify_manifest_signature(manifest, &sig_legacy, &pubkey).unwrap();
    }

    #[test]
    fn rejects_tampered_manifest_and_wrong_key() {
        let kp = make_keypair();
        let wrong_kp = make_keypair();
        let key_id = [1, 2, 3, 4, 5, 6, 7, 8];
        let pubkey = minisign_pubkey_str(&kp, key_id);
        let wrong_pubkey = minisign_pubkey_str(&wrong_kp, key_id);

        let manifest = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  ducad.zip\n";
        let tampered = b"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb  ducad.zip\n";
        let sig = sign_minisign(&kp, key_id, manifest, true);

        assert!(matches!(
            verify_manifest_signature(tampered, &sig, &pubkey),
            Err(UpdateError::Signature(_))
        ));
        assert!(matches!(
            verify_manifest_signature(manifest, &sig, &wrong_pubkey),
            Err(UpdateError::Signature(_))
        ));
    }

    #[test]
    fn verifies_raw_ed25519_sig_with_hex_or_base64_pubkey() {
        let kp = make_keypair();
        let manifest = b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  ducad.dmg\n";
        let sig = kp.sign(manifest);

        let pk_hex = hex::encode(kp.public_key().as_ref());
        let sig_b64 = B64.encode(sig.as_ref());
        verify_manifest_signature(manifest, &sig_b64, &pk_hex).unwrap();

        let pk_b64 = B64.encode(kp.public_key().as_ref());
        let sig_hex = hex::encode(sig.as_ref());
        verify_manifest_signature(manifest, &sig_hex, &pk_b64).unwrap();
    }
}

