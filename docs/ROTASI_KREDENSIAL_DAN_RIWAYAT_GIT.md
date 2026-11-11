# Panduan Rotasi Kredensial & Pembersihan Riwayat Git

Dokumen ini menindaklanjuti temuan keamanan kritis pada **Tinjauan Produk DUCAD (Tahap 1 — Tangani Hari Ini)**.

> [!IMPORTANT]
> Kredensial pada working tree `DUCAD-STUDIO/build_macos.sh` telah dipindahkan ke berkas lokal `.env` (izin `0600`, di-ignore oleh Git) dan digantikan oleh template `.env.example`. Namun, **menghapus kredensial pada commit baru tidak menghapusnya dari riwayat commit lama**. Rotasi kredensial di penyedia layanan tetap wajib dilakukan segera.

---

## 1. Daftar Kredensial yang Wajib Dirotasi Hari Ini

### A. Apple Developer / App Store Connect (`DUCAD-STUDIO`)
Sebelumnya terekspos di `build_macos.sh` (`APPLE_PASSWORD` / `PASSWORD` untuk `nunung.pamungkas@vneu.co.id`):
1. Buka [https://appleid.apple.com](https://appleid.apple.com) -> **Sign-In and Security** -> **App-Specific Passwords**.
2. Cabut (*Revoke*) app-specific password lama yang pernah dipakai skrip build/notarisasi.
3. Buat App-Specific Password baru khusus mesin build lokal, lalu perbarui nilainya **hanya** di `/Users/jayuda/Documents/PROJECT/DUCAD/DUCAD-STUDIO/.env` (`APPLE_PASSWORD='...'`).

### B. Server Otentikasi & Cloud (`DUCAD-SERVER`)
Sebelumnya berkas `.env` sempat ter-commit pada `ad9f86f0` (26 Agu) dan dihapus pada `d4a4e4fa` (10 Sep):
1. **`DATABASE_URL` (MySQL)**: Ganti kata sandi user database produksi, perbarui `.env` di server produksi, lalu restart layanan `ducad-server`.
2. **`JWT_SECRET`**: Bangkitkan kunci acak baru minimal 32 byte (`openssl rand -hex 32`), pasang di `.env` server produksi (seluruh sesi lama akan otomatis diminta login ulang dengan aman).
3. **`GOOGLE_CLIENT_SECRET`**: Buka Google Cloud Console -> *APIs & Services -> Credentials* -> pilih OAuth 2.0 Client ID DUCAD -> *Add secret* lalu hapus secret lama setelah `.env` server diperbarui.
4. **`GITHUB_CLIENT_SECRET`**: Buka GitHub *Settings -> Developer settings -> OAuth Apps* -> DUCAD -> *Generate a new client secret* lalu cabut (*Delete*) secret lama.
5. **`AuthKey.p8` (Sign in with Apple)**: Pastikan berkas lokal tetap tidak ter-track (`git ls-files AuthKey.p8` kosong); bila pernah terekspos di luar server, cabut Key di Apple Developer Portal (*Certificates, Identifiers & Profiles -> Keys*) dan buat kunci baru.

---

## 2. Prosedur Opsional Pembersihan Riwayat Git (`git filter-repo`)

Sesuai keputusan eksekusi, riwayat Git **tidak** ditulis ulang secara otomatis agar tidak memutus working tree/clone lain tanpa koordinasi. Setelah seluruh kredensial di atas dirotasi dan semua perubahan aktif telah di-push, jalankan prosedur berikut pada clone segar bila ingin membersihkan riwayat publik:

### A. Pasang `git-filter-repo`
```bash
brew install git-filter-repo
```

### B. Bersihkan riwayat `.env` di `DUCAD-SERVER`
```bash
git clone --mirror git@github.com:<org>/ducad-server.git ducad-server-clean.git
cd ducad-server-clean.git
git filter-repo --path .env --invert-paths
git push --force --mirror
```

### C. Sensor string rahasia lama di `DUCAD-STUDIO`
Buat berkas sementara `/tmp/ducad-replacements.txt` berisi:
```text
<password-app-specific-lama>==>***REMOVED***
```
Lalu jalankan pada mirror clone:
```bash
git clone --mirror git@github.com:cmjcode/ducad.git ducad-clean.git
cd ducad-clean.git
git filter-repo --replace-text /tmp/ducad-replacements.txt
rm -f /tmp/ducad-replacements.txt
git push --force --mirror
```

### D. Aktifkan Perlindungan GitHub
Di pengaturan repositori GitHub (`Settings -> Code security and analysis`):
1. Aktifkan **Secret scanning**.
2. Aktifkan **Push protection**.
3. Di `Settings -> Branches`, tambahkan aturan proteksi cabang `main` yang mewajibkan status check workflow **CI** hijau sebelum merge.

