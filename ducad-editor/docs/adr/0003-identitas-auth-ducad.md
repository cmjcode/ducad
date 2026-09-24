# ADR 0003 — Identitas otentikasi DUCAD & Sign in with Apple

**Status**: Diterima · **Tanggal**: 2026-09-22 · **Cakupan**: `ducad-cloud`, `ducad-ui`, packaging Apple

## Konteks

DUCAD menawarkan login pihak ketiga (Google, GitHub) lewat basis kode server
auth yang sama dengan aplikasi lain di tim. Dua masalah menumpuk:

1. **App Store Guideline 4.8.** Aplikasi yang menyediakan login pihak ketiga
   **wajib** juga menyediakan layanan yang membatasi pengumpulan data ke nama
   dan email, serta mengizinkan pengguna menyembunyikan alamat aslinya. Sign in
   with Apple memenuhi itu. Tanpa opsi tersebut, build iPad DUCAD berisiko
   ditolak review.
2. **Identitas.** Server dengan satu set kredensial OAuth membuat layar consent
   Google/GitHub menyebut nama aplikasi lain, bukan DUCAD. Pengguna DUCAD
   melihat merek yang tidak ia kenal pada saat paling sensitif dalam alur
   login.

## Keputusan

### 1. Server auth DUCAD terpisah, bukan kredensial bersama

DUCAD memakai instance server auth sendiri (`https://api.ducad.app`) dengan
kredensial OAuth milik DUCAD: Google Client bernama "DUCAD", GitHub OAuth App
"DUCAD", dan Services ID Apple `id.ducad.studio.signin`.

**Amandemen 2026-09-22 — basis kodenya juga terpisah.** Versi pertama
keputusan ini menyatakan "basis kode servernya tetap sama; hanya `.env`-nya
berbeda". Itu tidak bertahan, dengan dua alasan:

1. Server bersama membawa 16 tabel — koneksi basis data, tim, kolaborasi
   CRDT, vault enkripsi ujung-ke-ujung, moderasi — yang tidak satu pun dipakai
   DUCAD, sementara DUCAD hanya butuh identitas.
2. Halaman suksesnya sudah memuat percabangan merek
   `match client_app { "ducad" => …, "tabular" => … }` — persis percabangan
   yang alinea "alternatif yang ditolak" di bawah menolak, hanya saja ia masuk
   lewat halaman HTML alih-alih lewat pemilihan kredensial.

Server DUCAD sekarang berdiri sendiri di `ducad-server/` (Cargo project
terpisah, bukan member workspace `ducad-editor`, karena axum/tokio/sqlx tidak
boleh masuk graf dependensi editor) dengan empat tabel: `users`, `sessions`,
`oauth_states`, `auth_tickets`. Kontrak HTTP-nya tidak berubah, sehingga
`ducad-cloud` tidak perlu disentuh selain menerima field `license_tier` yang
kini dilaporkan server alih-alih di-hardcode di klien.

Pemisahan ini juga membuat klaim di bagian "Konsekuensi" di bawah — bahwa
identitas akun dikunci ke `sub`, bukan email — akhirnya benar secara harfiah:
server bersama meng-upsert dengan `WHERE (provider, provider_id) OR email`,
yang menggabungkan akun berbeda provider yang emailnya sama.

Runbook variabel environment dan penyiapan Apple ada di
[`../../../ducad-server/README.md`](../../../ducad-server/README.md);
`docs/AUTH_SERVER.md` kini hanya memuat sisi klien.

Alternatif yang ditolak: menambahkan kredensial per-klien ke server (memilih
set kredensial berdasarkan query `client`). Itu menaruh percabangan merek di
jalur paling sensitif keamanan di server, demi menghemat satu deployment.

Klien mengirim `client=ducad` pada URL login, yang membuat halaman sukses di
browser tampil dengan identitas DUCAD.

### 2. Web flow lewat server, bukan `ASAuthorization` native

Sign in with Apple dipakai lewat alur web (Services ID + `form_post` ke
callback server), bukan `ASAuthorizationAppleIDProvider` native iOS/macOS.

Alasannya:

- Satu jalur kode untuk ketiga provider di ketiga platform (macOS, iPad, dan
  Linux/Windows untuk build pengembangan). `ASAuthorization` hanya ada di
  platform Apple, sehingga jalur native berarti dua implementasi yang harus
  dijaga sinkron.
- Pertukaran kode dan penerbitan JWT tetap berada di server, tempat `.p8`
  disimpan. Jalur native tetap butuh server untuk memverifikasi `id_token`,
  jadi ia menambah kode tanpa menghapus komponen apa pun.

Konsekuensi yang harus diingat: alur web memakai **Services ID**, bukan bundle
ID, dan **tidak** membutuhkan entitlement `com.apple.developer.applesignin`
(entitlement itu khusus jalur native). Karena itu
`apple/ios/DUCAD-iOS.entitlements` sengaja dibiarkan tanpa kunci tersebut —
jangan ditambahkan karena mengira ada yang kurang.

### 3. Ticket polling HTTPS sebagai jalur utama pengambilan token

Klien membuat ticket acak 128-bit, menyertakannya di URL login, lalu
menjemput token dengan poll `POST /api/v1/auth/ticket/poll`. Callback loopback
`127.0.0.1:{port}` tetap dipakai bila tersedia, tapi hanya sebagai jalur cepat
opsional.

Ini bukan redundansi; tiap jalur menutup lubang nyata:

| Kondisi | Loopback | Polling |
|---|---|---|
| macOS App Sandbox tanpa `network.server` | ❌ `bind` ditolak | ✅ |
| iOS / iPadOS | ❌ tak bisa listen | ✅ |
| Apple sign-in (`form_post` ke server) | ❌ tak ada redirect ke loopback | ✅ |
| Safari, HTTPS → `127.0.0.1` | ❌ `fetch` lintas-origin diblokir | ✅ |
| Desktop non-sandbox | ✅ lebih cepat | ✅ |

Maka **kegagalan `TcpListener::bind` wajib non-fatal.** Versi pertama
`ducad-cloud::auth` langsung `return Err` saat bind gagal, dan itu membuat
login mati total di build Mac App Store — semua provider, bukan hanya Apple.
Tes `test_login_url_without_loopback_port` menjaga agar jalur tanpa port tetap
menghasilkan URL login yang sah.

### 4. `com.apple.security.network.server` sengaja TIDAK diminta

Entitlement itu akan memulihkan loopback di sandbox, tapi ia menyatakan
"aplikasi ini menerima koneksi masuk" — hal yang ditanyai reviewer App Store.
Ticket polling membuatnya tidak perlu, jadi `apple/macos/DUCAD.entitlements`
tetap tanpa entitlement server.

### 5. `ureq`, bukan `reqwest`; `getrandom`, bukan `rand`

Poller berjalan blocking di thread sendiri, jadi runtime async tidak
dibutuhkan. Workspace ini belum menarik tokio/hyper sama sekali; memakai
`reqwest` akan menjadikannya penambahan graf dependensi terbesar di repo hanya
untuk satu POST per 1,5 detik. `ureq` 2 dipakai dengan rustls
(`default-features = false`, fitur `tls` + `json`).

Ticket dibuat dengan `getrandom` langsung, bukan `rand`: yang dibutuhkan hanya
mengisi 16 byte dari CSPRNG sistem, dan `getrandom` sudah ada di pohon
dependensi — menarik `rand` akan menambah salinan ketiganya (0.9 dan 0.10 sudah
masuk lewat dependensi lain). Kegagalan CSPRNG **membatalkan login** alih-alih
jatuh ke sumber acak yang lebih lemah, karena ticket adalah kredensial pembawa:
siapa pun yang menebaknya lebih dulu bisa menjemput token korban.

**Catatan lisensi.** `ureq` menarik `webpki-roots`, yang berlisensi
`CDLA-Permissive-2.0` dan karena itu perlu ditambahkan ke daftar izin
`deny.toml`. Crate tersebut berisi daftar akar CA Mozilla — data, bukan kode —
dan CDLA-Permissive-2.0 bersifat permisif tanpa copyleft. Penambahan itu
dilakukan sadar, bukan agar pemeriksa berhenti mengeluh; `ureq` sendiri
MIT OR Apache-2.0.

Bila suatu saat vendoring daftar CA dianggap tidak diinginkan, alternatifnya
adalah `ureq` dengan trust store OS (`native-tls`, atau `ureq` 3 dengan
`platform-verifier`). Itu menghapus `webpki-roots`, tapi `native-tls` menuntut
OpenSSL saat build di Linux — biaya yang tidak sebanding hanya untuk menghindari
satu baris di `deny.toml`.

### 6. `DUCAD_SERVER_URL`, dengan `CMJCODE_SERVER_URL` sebagai fallback

Urutan resolusi: `DUCAD_SERVER_URL` → `CMJCODE_SERVER_URL` → `SERVER_BASE_URL`
→ `.env` → default. Nama lama dipertahankan agar setup pengembangan lokal yang
sudah ada tidak rusak, tapi yang baru menang bila keduanya diset.

## Konsekuensi

- Email Apple bisa berupa alamat relay privat (`@privaterelay.appleid.com`)
  yang berotasi. Server mengunci identitas ke klaim `sub`, bukan email, jadi
  klien tidak perlu menanganinya — tapi jangan pernah memakai email sebagai
  kunci akun.
- Apple menolak Return URL non-HTTPS, sehingga Sign in with Apple **tidak bisa
  diuji lewat `http://127.0.0.1:3000`**. Uji Apple butuh server ber-HTTPS;
  Google/GitHub tetap bisa diuji lokal. Tes
  `test_default_server_url_is_https` mencegah default kembali ke `http://`.
- Menambah provider baru cukup dengan satu varian di `OAuthProvider` dan satu
  lengan di `render_login_button`; urutan tampilnya diatur
  `OAuthProvider::all()`, yang menjaga Apple tetap di posisi pertama.
