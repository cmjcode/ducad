# Tutorial Registrasi & Mendapatkan Kredensial OAuth: GitHub, Google, dan Apple

Panduan lengkap langkah demi langkah untuk mendaftarkan aplikasi DUCAD dan memperoleh kredensial (Client ID, Client Secret, Key ID, Team ID, dan Private Key) untuk ketiga penyedia otentikasi di `ducad-server`.

---

## Daftar Isi

1. [Pemetaan Variabel Lingkungan (`.env`)](#1-pemetaan-variabel-lingkungan-env)
2. [GitHub — OAuth App](#2-github--oauth-app)
3. [Google Cloud — OAuth 2.0 Client](#3-google-cloud--oauth-20-client)
4. [Apple Developer — Sign in with Apple](#4-apple-developer--sign-in-with-apple)
5. [Contoh Lengkap Pengisian `.env`](#5-contoh-lengkap-pengisian-env)
6. [Pengujian & Troubleshooting Galat Umum](#6-pengujian--troubleshooting-galat-umum)

---

## 1. Pemetaan Variabel Lingkungan (`.env`)

Sebelum mendaftar, pahami bagaimana endpoint callback `ducad-server` diturunkan:

- Base URL server ditentukan oleh variabel `SERVER_BASE_URL` (contoh produksi: `https://api.ducad.app`, atau lokal: `http://localhost:8430` / `http://127.0.0.1:8430`).
- Callback URL bawaan masing-masing provider:
  - **Google**: `{SERVER_BASE_URL}/api/v1/auth/callback/google`
  - **GitHub**: `{SERVER_BASE_URL}/api/v1/auth/callback/github`
  - **Apple**: `{SERVER_BASE_URL}/api/v1/auth/callback/apple`

| Variabel di `.env` | Provider | Keterangan |
|---|---|---|
| `GITHUB_CLIENT_ID` | GitHub | Client ID dari GitHub OAuth App |
| `GITHUB_CLIENT_SECRET` | GitHub | Client Secret dari GitHub OAuth App |
| `GITHUB_REDIRECT_URI` | GitHub | *(Opsional)* Diisi jika berbeda dari base URL |
| `GOOGLE_CLIENT_ID` | Google | Client ID dari Google Cloud Console (`...apps.googleusercontent.com`) |
| `GOOGLE_CLIENT_SECRET` | Google | Client Secret dari Google Cloud Console |
| `GOOGLE_REDIRECT_URI` | Google | *(Opsional)* Diisi jika berbeda dari base URL |
| `APPLE_CLIENT_ID` | Apple | **Services ID** (contoh: `id.ducad.studio.signin`), **BUKAN** Bundle ID aplikasi! |
| `APPLE_TEAM_ID` | Apple | 10 karakter Team ID dari akun Apple Developer |
| `APPLE_KEY_ID` | Apple | 10 karakter Key ID dari kunci Sign in with Apple |
| `APPLE_PRIVATE_KEY_PATH` | Apple | Path ke berkas kunci privat `AuthKey_XXXXXXXXXX.p8` (disarankan) |
| `APPLE_PRIVATE_KEY` | Apple | Alternatif: isi berkas `.p8` sebagai string PEM satu baris dengan `\n` |
| `APPLE_REDIRECT_URI` | Apple | *(Opsional)* Diisi jika berbeda dari base URL |

> [!IMPORTANT]
> **Identitas Aplikasi**: Kredensial di ketiga provider harus didaftarkan atas nama **DUCAD** (bukan aplikasi lain atau nama pribadi tim), agar layar izin (*consent screen*) menampilkan merek dan logo DUCAD dengan benar kepada pengguna.

---

## 2. GitHub — OAuth App

Registrasi GitHub paling cepat dan mendukung pengujian langsung di alamat `http://localhost` atau `http://127.0.0.1`.

### Langkah-langkah Pembuatan:

1. **Masuk ke Pengaturan Akun / Organisasi GitHub**:
   - Buka [GitHub](https://github.com/) dan login.
   - Klik foto profil di pojok kanan atas → **Settings**.
   - Di menu sebelah kiri, scroll ke paling bawah dan pilih **Developer settings** (atau langsung buka: [https://github.com/settings/apps](https://github.com/settings/apps)).

2. **Buka OAuth Apps**:
   - Di sidebar kiri, klik **OAuth Apps**.
   - Klik tombol **New OAuth App** (atau **Register a new application**).

3. **Isi Formulir Aplikasi**:
   - **Application name**: `DUCAD` (atau `DUCAD (Local Dev)` jika membuat aplikasi khusus dev).
   - **Homepage URL**:
     - Produksi: `https://ducad.app`
     - Lokal/Pengujian: `http://localhost:8430` atau `http://127.0.0.1:8430`
   - **Application description**: *(Opsional)* `Server otentikasi resmi DUCAD`.
   - **Authorization callback URL**:
     - Produksi: `https://api.ducad.app/api/v1/auth/callback/github`
     - Lokal: `http://127.0.0.1:8430/api/v1/auth/callback/github`
   - *(Opsional)* Unggah logo DUCAD pada bagian Application logo.

4. **Daftarkan Aplikasi & Dapatkan Kunci**:
   - Klik tombol **Register application**.
   - Anda akan diarahkan ke halaman detail aplikasi.
   - Salin **Client ID** yang ditampilkan → masukkan ke `GITHUB_CLIENT_ID`.
   - Di bagian **Client secrets**, klik tombol **Generate a new client secret**.
   - Konfirmasi kata sandi atau 2FA jika diminta.
   - **Segera salin nilai Client Secret yang muncul** (karena tidak akan ditampilkan lagi secara penuh) → masukkan ke `GITHUB_CLIENT_SECRET`.

> [!NOTE]
> Scope yang diminta oleh `ducad-server` secara otomatis adalah `read:user` dan `user:email`. Anda tidak perlu mengonfigurasi scope tambahan di portal GitHub karena scope tersebut dikirimkan via query otorisasi.

---

## 3. Google Cloud — OAuth 2.0 Client

Google OAuth memerlukan pembuatan proyek di Google Cloud Platform dan konfigurasi layar persetujuan (*OAuth consent screen*).

### Langkah 1: Buat Proyek Google Cloud

1. Buka [Google Cloud Console](https://console.cloud.google.com/).
2. Login dengan akun Google yang mengelola DUCAD.
3. Di bilah navigasi atas, klik pemilih proyek (*project selector*) → klik **New Project**.
4. Masukkan nama proyek: `DUCAD` (atau `ducad-auth`).
5. Klik **Create**, lalu pastikan proyek tersebut aktif dipilih.

### Langkah 2: Konfigurasi Layar Persetujuan (OAuth consent screen)

1. Di menu navigasi samping kiri, buka **APIs & Services** → **OAuth consent screen** (atau buka [https://console.cloud.google.com/apis/credentials/consent](https://console.cloud.google.com/apis/credentials/consent)).
2. Pilih **User Type**:
   - Pilih **External** (agar dapat diakses oleh semua pemilik akun Google).
   - Klik **Create**.
3. **App information**:
   - **App name**: `DUCAD`
   - **User support email**: Pilih email kontak/dukungan Anda.
   - **App logo**: *(Opsional)* Unggah logo DUCAD.
4. **App domain**:
   - **Application home page**: `https://ducad.app`
   - **Application privacy policy link**: `https://ducad.app/privacy` *(jika sudah ada)*
   - **Application terms of service link**: `https://ducad.app/terms` *(jika sudah ada)*
   - **Authorized domains**: Tambahkan domain utama, misalnya `ducad.app`.
5. **Developer contact information**:
   - Masukkan alamat email developer/admin DUCAD.
   - Klik **Save and Continue**.
6. **Scopes**:
   - Klik tombol **Add or Remove Scopes**.
   - Pilih tiga scope dasar berikut:
     - `.../auth/userinfo.email`
     - `.../auth/userinfo.profile`
     - `openid`
   - Klik **Update**, lalu klik **Save and Continue**.
7. **Test users** (Mode Pengujian):
   - Selama aplikasi berstatus *Testing*, hanya email yang didaftarkan di sini yang dapat login.
   - Klik **Add Users** dan masukkan email pengujian Anda.
   - Klik **Save and Continue**, lalu kembali ke Dashboard.

> [!TIP]
> Jika siap rilis publik ke pengguna umum, klik tombol **Publish App** di tab OAuth consent screen agar pengguna Google di luar *Test users* bisa login tanpa pembatasan.

### Langkah 3: Buat Kredensial OAuth Client ID

1. Di menu sidebar kiri, klik **Credentials**.
2. Klik tombol **+ CREATE CREDENTIALS** di bagian atas → pilih **OAuth client ID**.
3. Isi detail kredensial:
   - **Application type**: Pilih **Web application**.
   - **Name**: `DUCAD Server Auth`.
   - **Authorized JavaScript origins**:
     - Produksi: `https://api.ducad.app`
     - Lokal: `http://localhost:8430` dan `http://127.0.0.1:8430`
   - **Authorized redirect URIs** *(Sangat penting)*:
     - Produksi: `https://api.ducad.app/api/v1/auth/callback/google`
     - Lokal: `http://127.0.0.1:8430/api/v1/auth/callback/google` dan `http://localhost:8430/api/v1/auth/callback/google`
4. Klik tombol **Create**.
5. Modal dialog akan muncul menampilkan:
   - **Your Client ID** (berakhir dengan `.apps.googleusercontent.com`) → masukkan ke `GOOGLE_CLIENT_ID`.
   - **Your Client Secret** → masukkan ke `GOOGLE_CLIENT_SECRET`.

---

## 4. Apple Developer — Sign in with Apple

Sign in with Apple memiliki alur registrasi yang paling ketat dan memerlukan keanggotaan **Apple Developer Program** berbayar.

> [!WARNING]
> **Kesalahan Konfigurasi Terbanyak**:
> `APPLE_CLIENT_ID` **HARUS** berupa **Services ID** (contoh: `id.ducad.studio.signin`), **BUKAN** Bundle ID aplikasi iOS/iPad (`id.ducad.studio`).
> Jika salah memasukkan Bundle ID, Safari akan langsung menampilkan pesan galat: **"Sign Up Not Complete"**.

> [!NOTE]
> **Apple Mewajibkan HTTPS**:
> Apple menolak protokol `http://` dan domain `localhost`. Anda **tidak bisa** menguji Sign in with Apple menggunakan server lokal biasa tanpa reverse proxy HTTPS berdomain publik (misal domain staging atau Cloudflare Tunnel / ngrok ber-HTTPS yang domainnya terdaftar).

### Langkah 1: Pastikan App ID Utama Mendukung "Sign in with Apple"

1. Masuk ke [Apple Developer Portal](https://developer.apple.com/account/).
2. Buka menu **Certificates, Identifiers & Profiles** → **Identifiers**.
3. Di dropdown sebelah kanan, pilih **App IDs**.
4. Cari App ID aplikasi DUCAD Anda (misal `id.ducad.studio`).
   - *Jika belum ada, buat App ID baru dengan tipe App, Description "DUCAD", dan Bundle ID `id.ducad.studio`.*
5. Klik App ID tersebut, lalu di daftar Capabilities, centang **Sign in with Apple**.
6. Klik **Edit** di samping Sign in with Apple:
   - Pilih opsi **Enable as a primary App ID**.
   - Klik **Save**.
7. Klik **Save** di pojok kanan atas untuk menyimpan perubahan pada App ID.

### Langkah 2: Buat Services ID (Inilah Client ID Anda!)

1. Kembali ke halaman **Identifiers**.
2. Klik tombol tambah berwarna biru (**+**).
3. Pilih opsi **Services IDs** → klik **Continue**.
4. Isi form:
   - **Description**: `DUCAD Server Auth Service`
   - **Identifier**: Masukkan pengenal unik, biasanya turunan dari bundle ID dengan akhiran `.signin` atau `.service`, contoh:
     ```text
     id.ducad.studio.signin
     ```
5. Klik **Continue** → klik **Register**.
6. Nilai Identifier inilah (`id.ducad.studio.signin`) yang menjadi **`APPLE_CLIENT_ID`**.

### Langkah 3: Konfigurasi Domain & Return URL pada Services ID

1. Cari dan klik Services ID yang baru Anda buat (`id.ducad.studio.signin`) dari daftar Identifiers.
2. Centang kotak **Sign in with Apple**.
3. Klik tombol **Configure** di sebelah kanannya:
   - **Primary App ID**: Pilih App ID utama dari Langkah 1 (contoh: `id.ducad.studio`).
   - **Domains and Subdomains**: Masukkan domain server auth Anda **tanpa** skema `https://` dan tanpa garis miring:
     ```text
     api.ducad.app
     ```
     *(Catatan: Apple mungkin memerlukan verifikasi kepemilikan domain via penempatan berkas jika diminta).*
   - **Return URLs**: Masukkan URL callback endpoint Apple lengkap ber-HTTPS:
     ```text
     https://api.ducad.app/api/v1/auth/callback/apple
     ```
4. Klik **Next** → klik **Done**.
5. Klik **Continue** → klik **Save** pada halaman Services ID.

### Langkah 4: Buat Kunci Privat (Private Key `.p8`)

Berbeda dengan Google dan GitHub yang memiliki Client Secret statis, `ducad-server` secara dinamis mencetak token JWT ES256 berumur 1 jam menggunakan kunci privat Apple.

1. Di menu sidebar kiri, buka **Keys** (di bawah *Certificates, Identifiers & Profiles*).
2. Klik tombol tambah (**+**) untuk membuat kunci baru.
3. **Key Name**: `DUCAD Sign in with Apple Key`.
4. Di daftar opsi, centang **Sign in with Apple**.
5. Klik tombol **Configure** di sampingnya:
   - **Primary App ID**: Pilih App ID utama Anda (`id.ducad.studio`).
   - Klik **Save**.
6. Klik **Continue** → klik **Register**.
7. Anda akan melihat halaman unduh:
   - **Key ID**: Catat 10 karakter alfanumerik (contoh: `YYYYYYYYYY`) → masukkan ke `APPLE_KEY_ID`.
   - Klik tombol **Download** untuk mengunduh berkas kunci (`AuthKey_YYYYYYYYYY.p8`).

> [!CAUTION]
> **BERKAS `.p8` HANYA BISA DIUNDUH SATU KALI DARI APPLE!**
> Simpan berkas ini di tempat yang aman dan jangan pernah commit berkas ini ke dalam repository git. Jika berkas hilang, Anda harus mencabut (*revoke*) kunci lama dan membuat kunci baru.

### Langkah 5: Dapatkan Team ID Apple

1. Lihat di pojok kanan atas portal Apple Developer (di samping nama Anda/organisasi).
2. Terdapat kode 10 karakter yang tertera sebagai **Team ID** (contoh: `XXXXXXXXXX`).
3. Anda juga dapat melihatnya di tab **Membership Details**.
4. Masukkan nilai ini ke `APPLE_TEAM_ID`.

### Langkah 6: Pasang Kunci Privat pada `ducad-server`

Terdapat 2 cara konfigurasi kunci Apple di `ducad-server`:

#### Cara A: Menggunakan Berkas Rahasia (Direkomendasikan)
Simpan berkas `AuthKey_YYYYYYYYYY.p8` di server hosting (misalnya di folder rahasia `/run/secrets/AuthKey.p8` atau `./secrets/AuthKey.p8`):
```dotenv
APPLE_PRIVATE_KEY_PATH=/run/secrets/AuthKey.p8
```

#### Cara B: Menggunakan Environment Variable Satu Baris
Jika server/PaaS (seperti Heroku, Railway, dsb.) hanya mendukung env variable berupa teks satu baris, buka berkas `.p8` dengan text editor dan ubah semua karakter *newline* menjadi `\n` literal:
```dotenv
APPLE_PRIVATE_KEY="-----BEGIN PRIVATE KEY-----\nMIGTAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBHkwdwIBAQQg...\n-----END PRIVATE KEY-----"
```

---

## 5. Contoh Lengkap Pengisian `.env`

Berikut adalah contoh isi berkas `.env` di `ducad-server` setelah semua kredensial diperoleh:

```dotenv
# ==============================================================================
# Konfigurasi ducad-server (.env)
# ==============================================================================

# ─── Konfigurasi Dasar ────────────────────────────────────────────────────────
DATABASE_URL=mysql://ducad:sandi_rahasia_db@127.0.0.1:3306/ducad_server
SERVER_PORT=8430
SERVER_BASE_URL=https://api.ducad.app

# ─── JWT (Token Sesi DUCAD) ───────────────────────────────────────────────────
# Buat dengan perintah: openssl rand -hex 32
JWT_SECRET=ganti-dengan-32-byte-acak-hex-string
JWT_ACCESS_EXPIRY_MINUTES=60
JWT_REFRESH_EXPIRY_DAYS=30

# ─── Google OAuth ─────────────────────────────────────────────────────────────
GOOGLE_CLIENT_ID=your-client-id.apps.googleusercontent.com
GOOGLE_CLIENT_SECRET=your-google-client-secret-here
# Kosongkan jika memakai default: {SERVER_BASE_URL}/api/v1/auth/callback/google
GOOGLE_REDIRECT_URI=

# ─── GitHub OAuth ────────────────────────────────────────────────────────────
GITHUB_CLIENT_ID=your-github-client-id
GITHUB_CLIENT_SECRET=your-github-client-secret
# Kosongkan jika memakai default: {SERVER_BASE_URL}/api/v1/auth/callback/github
GITHUB_REDIRECT_URI=

# ─── Sign in with Apple ───────────────────────────────────────────────────────
# WAJIB Services ID, BUKAN Bundle ID!
APPLE_CLIENT_ID=id.ducad.studio.signin
APPLE_TEAM_ID=XXXXXXXXXX
APPLE_KEY_ID=YYYYYYYYYY
# Path ke berkas .p8 di server
APPLE_PRIVATE_KEY_PATH=/run/secrets/AuthKey.p8
# Kosongkan jika memakai default: {SERVER_BASE_URL}/api/v1/auth/callback/apple
APPLE_REDIRECT_URI=

# ─── CORS & Lisensi ───────────────────────────────────────────────────────────
ALLOWED_ORIGINS=https://ducad.app,https://www.ducad.app
DEFAULT_LICENSE_TIER=Pro
RUST_LOG=ducad_server=info,tower_http=info
```

---

## 6. Pengujian & Troubleshooting Galat Umum

### Cara Menguji Server

1. **Jalankan Server**:
   ```bash
   cargo run
   ```
   Server akan memverifikasi koneksi database dan menjalankan migrasi skema tabel secara otomatis.

2. **Periksa Endpoint Health**:
   ```bash
   curl -i http://127.0.0.1:8430/health
   # Balasan harus HTTP 200 dengan body "ok"
   ```

3. **Uji Inisiasi Login di Browser**:
   Buka salah satu URL berikut di browser untuk menguji pengalihan ke masing-masing provider:
   - Google: `http://127.0.0.1:8430/api/v1/auth/login/google`
   - GitHub: `http://127.0.0.1:8430/api/v1/auth/login/github`
   - Apple: `https://api.ducad.app/api/v1/auth/login/apple` *(memerlukan domain publik HTTPS)*

---

### Diagnosa Masalah Umum

#### 1. Masalah Google
- **Penyebab**: `redirect_uri_mismatch` (Galat 400).
  - **Solusi**: URL callback yang dituju oleh browser (`{SERVER_BASE_URL}/api/v1/auth/callback/google`) belum didaftarkan di daftar **Authorized redirect URIs** pada Google Cloud Console. Pastikan tidak ada perbedaan protokol (`http` vs `https`) atau port (`8430`).
- **Penyebab**: `Access blocked: This app has not been verified`.
  - **Solusi**: Jika aplikasi masih dalam status *Testing*, pastikan email yang dipakai untuk login sudah ditambahkan ke daftar **Test users** di tab OAuth consent screen.

#### 2. Masalah GitHub
- **Penyebab**: `The redirect_uri MUST match the registered callback URL for this application`.
  - **Solusi**: Periksa kembali **Authorization callback URL** di GitHub Developer Settings. GitHub sangat sensitif terhadap perbedaan subdomain dan port.

#### 3. Masalah Apple
- **Penyebab**: Tampilan Safari: `Sign Up Not Complete` atau `invalid_client`.
  - **Solusi 1**: Nilai `APPLE_CLIENT_ID` yang dipasang adalah Bundle ID (misal `id.ducad.studio`). Segera ganti dengan **Services ID** (misal `id.ducad.studio.signin`).
  - **Solusi 2**: Domain server Anda belum terdaftar di kolom **Domains and Subdomains** pada konfigurasi Services ID Apple.
- **Penyebab**: Nama pengguna tidak muncul di database/profil (bernama kosong).
  - **Fakta**: Apple **hanya mengirimkan nama lengkap pengguna pada saat otorisasi pertama kali**. Pada otorisasi berikutnya, Apple tidak lagi menyertakan objek nama. Untuk menguji ulang dari awal, masuk ke [https://appleid.apple.com](https://appleid.apple.com), pilih *Sign in with Apple*, lalu cabut akses DUCAD sebelum mencoba login kembali.
- **Penyebab**: Alamat email pengguna berupa `@privaterelay.appleid.com`.
  - **Fakta**: Ini adalah fitur privasi bawaan Apple (*Hide My Email*). `ducad-server` secara otomatis mengaitkan akun berdasarkan klaim `sub` permanen dari Apple, sehingga email relay tetap valid sebagai pengenal pengguna.
