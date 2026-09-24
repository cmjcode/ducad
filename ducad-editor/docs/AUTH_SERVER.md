# Server auth DUCAD — sisi klien

Diperbarui: 2026-09-22.

Aplikasi DUCAD tidak mengurus OAuth sendiri. Ia membuka browser ke server
auth, dan server itu yang berurusan dengan Google/GitHub/Apple lalu
menerbitkan JWT DUCAD.

**Runbook servernya ada di [`../../ducad-server/README.md`](../../ducad-server/README.md)**
— variabel environment, penyiapan Sign in with Apple, dan deploy. Dokumen ini
sengaja tidak menduplikasinya; yang di bawah hanyalah hal-hal yang berada di
sisi `ducad-cloud`.

Keputusan di balik bentuk ini ada di `docs/adr/0003-identitas-auth-ducad.md`.

---

## 1. Bagaimana klien menemukan server

`ducad-cloud` mencari URL server dalam urutan ini:

| Urutan | Sumber |
|---|---|
| 1 | env `DUCAD_SERVER_URL` |
| 2 | env `CMJCODE_SERVER_URL` (nama lama, fallback) |
| 3 | env `SERVER_BASE_URL` |
| 4 | berkas `.env` di direktori kerja, kunci yang sama berurutan |
| 5 | default `https://api.ducad.app` |

Untuk menunjuk ke server lokal saat pengembangan:

```bash
DUCAD_SERVER_URL=http://127.0.0.1:8430 cargo run -p ducad-app
```

Ingat: Google dan GitHub bisa diuji lewat `http://` lokal, **Apple tidak** —
Apple menolak Return URL non-HTTPS, termasuk `localhost`.

## 2. Endpoint yang dipakai klien

| Metode | Path | Dipakai untuk |
|---|---|---|
| GET | `/api/v1/auth/login/{google,github,apple}` | URL yang dibuka di browser. Klien menambahkan `?client=ducad&ticket=…[&port=…]` |
| POST | `/api/v1/auth/ticket/poll` | Body `{"ticket":"…"}`. Balasan: `status` `pending` / `completed` (+`token`) / `error` (+`error`) |

Daftar lengkap endpoint server ada di README server §3.

`client=ducad` masih dikirim demi kompatibilitas, tapi `ducad-server` tidak
memakainya: server itu hanya melayani DUCAD, jadi halaman suksesnya selalu
bermerek DUCAD. Nilai tersebut boleh hilang tanpa akibat apa pun.

## 3. Tier lisensi

Balasan token memuat `user.license_tier`. Klien memakai nilai itu; bila
server tidak menyebutkannya (server lama), `token_to_account` jatuh ke `"Pro"`
seperti perilaku sebelumnya — lihat `FALLBACK_LICENSE_TIER` di
`crates/ducad-cloud/src/auth.rs`.

## 4. Verifikasi cepat

```bash
# Ganti dengan URL server yang sedang diuji.
BASE=https://api.ducad.app

# 1. Server hidup dan mengenali provider
curl -sI "$BASE/api/v1/auth/login/apple?client=ducad&ticket=uji123" | head -3
#   307 + header Location ke appleid.apple.com → konfigurasi Apple OK
#   502 berisi "belum dikonfigurasi"           → APPLE_CLIENT_ID belum diset

# 2. Endpoint poll menjawab
curl -s -X POST "$BASE/api/v1/auth/ticket/poll" \
  -H 'Content-Type: application/json' -d '{"ticket":"uji123"}'
#   {"success":true,"data":{"status":"pending"}}
```

Pemeriksaan yang lebih lengkap: `ducad-server/smoke_test.sh`.

Uji end-to-end dari GUI ada di `docs/CEKLIS_UJI_GUI.md`, bagian login.
