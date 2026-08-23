# DUCAD - Indonesian (id-ID) Translations

# General App & Branding
app-name = DUCAD
app-title = { $name } - DUCAD

# Language
lang-en = English
lang-id = Bahasa Indonesia
lang-current = Bahasa

# Top Bar & Menus
menu-file = Berkas
menu-new = Dokumen Baru
menu-open = Buka…
menu-save = Simpan
menu-save-as = Simpan Sebagai…
menu-import = Impor
menu-import-step = STEP…
menu-import-dxf = DXF…
menu-export = Ekspor
menu-export-step = STEP… (semua body)
menu-export-stl = STL… (body tampak)
menu-export-obj = OBJ… (body tampak)
menu-export-dxf = DXF… (sketsa)
menu-settings = Pengaturan
menu-theme = Tema
menu-theme-dark = Mode Gelap
menu-theme-light = Mode Terang
menu-shortcuts = Pintasan Keyboard
menu-command-palette = Palet Perintah
cmd-no-match = Tidak ada perintah cocok

# Top Bar Actions & Tooltips
topbar-home-tooltip = Dokumen Baru
topbar-saved-tooltip = Dokumen tersimpan
topbar-unsaved-tooltip = Perubahan belum disimpan
topbar-share = Bagikan / Ekspor
topbar-items = Item
topbar-items-tooltip = Pohon Item & Outliner
topbar-search-tooltip = Cari & Palet Perintah (Ctrl/Cmd+K)
topbar-sketch-mode = Mode Sketsa
topbar-solid-mode = Mode Solid 3D
topbar-enter-sketch = Sketsa
topbar-exit-sketch = Selesai Sketsa
topbar-sketch-plane = Bidang: { $plane }
topbar-section-view = Tampilan Irisan
topbar-measurements = Pengukuran
topbar-delete-tooltip = Hapus Pilihan (Del / Backspace)
topbar-switch-to-sketch = Beralih ke Mode Sketsa 2D
topbar-switch-to-solid = Beralih ke Mode Solid 3D
topbar-unit = Satuan: { $unit }

# Planes
plane-top = Atas (XY)
plane-front = Depan (XZ)
plane-right = Kanan (YZ)
plane-bottom = Bawah
plane-back = Belakang
plane-left = Kiri
plane-isometric = Isometrik

# Tools & Actions
tool-select = Pilih
tool-select-desc = Seleksi entitas atau elemen
tool-line = Garis
tool-line-desc = Buat segmen garis bersambung
tool-arc = Busur
tool-arc-desc = Busur lingkaran 3 titik
tool-rectangle = Persegi
tool-rectangle-desc = Persegi sudut 2 titik
tool-circle = Lingkaran
tool-circle-desc = Lingkaran pusat-radius
tool-ellipse = Elips
tool-ellipse-desc = Elips pusat & semi-sumbu
tool-spline = Spline
tool-spline-desc = Gambar kurva halus multi-titik (Catmull-Rom)
tool-fillet-2d = Fillet 2D
tool-fillet-2d-desc = Bulatkan sudut antara dua garis dengan radius busur halus
tool-chamfer-2d = Chamfer 2D
tool-chamfer-2d-desc = Potong miring sudut pertemuan dua garis
tool-offset = Offset
tool-offset-desc = Offset kurva paralel
tool-mirror = Cermin
tool-mirror-desc = Cerminkan sketsa terhadap sumbu
tool-trim = Pangkas
tool-trim-desc = Pangkas segmen kurva tumpang tindih
tool-coincident = Titik Koinsiden
tool-coincident-desc = Gabung dua titik atau tempelkan titik ke kurva
tool-fixed = Titik Tetap
tool-fixed-desc = Kunci posisi titik di ruang
tool-symmetric = Titik Simetris
tool-symmetric-desc = Batasi dua titik simetris terhadap sumbu

# 3D Tools
tool-extrude = Ekstrusi
tool-extrude-desc = Ekstrusi sketsa 2D atau face 3D menjadi solid
tool-revolve = Putar (Revolve)
tool-revolve-desc = Putar profil mengelilingi sumbu menjadi solid
tool-loft = Loft
tool-loft-desc = Hubungkan dua profil melintasi bidang
tool-sweep = Sweep
tool-sweep-desc = Sapu profil 2D menyusuri kurva jalur (spine path) menjadi solid 3D
tool-sweep-name = Sweep 3D
tool-shell = Shell (Rongga)
tool-shell-desc = Buat rongga pada solid dengan ketebalan dinding seragam
tool-boolean = Operasi Boolean
tool-boolean-desc = Gabung, Kurang, atau Irisan benda 3D
tool-section = Tampilan Irisan
tool-section-desc = Bidang penampang interaktif
tool-measure = Ukur Jarak
tool-measure-desc = Ukur jarak antara titik, rusuk, atau bidang
tool-measure-angle = Ukur Sudut
tool-measure-angle-desc = Ukur sudut antara dua garis atau rusuk
tool-history = Riwayat
tool-history-desc = Riwayat operasi, pohon undo & redo

# Tool Guides
guide-step = Langkah { $current } dari { $total }
guide-next = Lanjut
guide-finish = Selesai
guide-cancel = Batal (Esc)
guide-select-title = Mode Seleksi
guide-select-prompt = Klik untuk memilih entitas, seret untuk seleksi kotak, atau klik dua kali untuk mengedit.
guide-line-title = Gambar Garis (L)
guide-line-p1 = Klik kanvas untuk titik AWAL.
guide-line-p2 = Klik untuk titik AKHIR (atau ketik panjang lalu tekan Enter).
guide-arc-title = Busur 3-Titik (A)
guide-arc-p1 = Klik kanvas untuk titik awal.
guide-arc-p2 = Klik kanvas untuk titik akhir.
guide-arc-p3 = Seret atau klik untuk menentukan radius/lengkungan.
guide-rect-title = Persegi (R)
guide-rect-p1 = Klik sudut pertama.
guide-rect-p2 = Klik sudut yang berlawanan.
guide-circle-title = Lingkaran (C)
guide-circle-p1 = Klik titik pusat.
guide-circle-p2 = Seret atau klik untuk menentukan radius.
guide-ellipse-title = Elips (E)
guide-ellipse-p1 = Klik titik pusat.
guide-ellipse-p2 = Tentukan radius mayor dan minor.
guide-offset-title = Offset Kurva (O)
guide-offset-prompt = Pilih kurva yang akan di-offset, lalu atur jarak di HUD.
guide-mirror-title = Cermin (M)
guide-mirror-prompt = Pilih entitas yang akan dicerminkan dan tentukan sumbu cermin.
guide-trim-title = Pangkas Kurva (T)
guide-trim-prompt = Klik segmen yang ingin dipangkas.
guide-extrude-title = Ekstrusi Solid
guide-extrude-prompt = Pilih profil sketsa tertutup atau bidang 3D untuk diekstrusi.
guide-revolve-title = Putar Solid (V)
guide-revolve-prompt = Pilih profil dan sumbu putar.
guide-loft-title = Loft Solid
guide-loft-prompt = Pilih profil bawah dan bidang/profil target.
guide-shell-title = Shell Solid
guide-shell-prompt = Pilih body dan bidang opsional yang akan dihilangkan.
guide-boolean-title = Operasi Boolean
guide-boolean-prompt = Pilih body target dan body alat, lalu pilih operasi.
guide-measure-title = Alat Ukur Jarak
guide-measure-prompt = Klik dua titik atau elemen untuk mengukur jarak.
guide-measure-angle-title = Pengukuran Sudut
guide-measure-angle-prompt = Klik dua garis/tepi untuk mengukur sudut.

# Parameters & Labels
param-distance = Jarak
param-distance-val = Jarak: { $val }
param-height = Tinggi
param-angle = Sudut
param-angle-val = Sudut: { $val }
measure-angle-undefined = Sudut: tidak terdefinisi (titik berimpit)
param-thickness = Tebal
param-radius = Radius
param-length = Panjang
param-width = Lebar
param-axis = Sumbu
param-direction = Arah
param-inside = Ke Dalam
param-outside = Ke Luar
param-symmetric = Simetris
param-preset = Preset
param-apply = Terapkan
param-close = Tutup
param-delete = Hapus
param-rename = Ganti Nama
param-visibility = Visibilitas
param-lock = Kunci

# Boolean Operations
boolean-union = Gabung
boolean-union-desc = Gabungkan body menjadi satu solid utuh
boolean-subtract = Kurang
boolean-subtract-desc = Potong body target dengan body alat
boolean-intersect = Irisan
boolean-intersect-desc = Simpan hanya volume yang tumpang tindih

# Revolve Presets
axis-x = Sumbu X
axis-y = Sumbu Y
axis-z = Sumbu Z
axis-custom = Garis Kustom

# Items Drawer
drawer-items-title = Item
drawer-bodies = Body 3D ({ $count })
drawer-sketches = Sketsa 2D ({ $count })
drawer-dimensions = Dimensi ({ $count })
drawer-no-items = Tidak ada item dalam dokumen
drawer-empty-bodies = Belum ada body 3D yang dibuat
drawer-empty-sketches = Belum ada sketsa 2D
drawer-search-placeholder = Cari objek…
drawer-rename-placeholder = Masukkan nama baru…
drawer-group = Kelompokkan Terpilih
drawer-ungroup = Lepas Grup
drawer-hide-all = Sembunyikan Semua
drawer-show-all = Tampilkan Semua

# History Drawer
drawer-history-title = Riwayat & Aktivitas
drawer-history-empty = Belum ada aktivitas yang tercatat
drawer-undo = Undo
drawer-redo = Redo
drawer-clear-history = Bersihkan Riwayat
history-search-placeholder = Cari riwayat aktivitas…
history-clear-search = Hapus pencarian
history-close = Tutup Riwayat
history-no-match = Tidak ditemukan hasil pencarian
history-auto-record = Aktivitas 2D & 3D akan tercatat otomatis
history-jump-tooltip = Klik untuk memulihkan keadaan pada { $time }

# Feature Inspector
inspector-title = Inspektor
inspector-properties = Properti
inspector-constraints = Batasan
inspector-dimensions = Dimensi
inspector-geometry = Geometri
inspector-no-selection = Tidak ada yang dipilih
inspector-multi-selection = { $count } item dipilih
inspector-anchor = Titik Acuan
inspector-coincident = Koinsiden
inspector-horizontal = Horizontal
inspector-vertical = Vertikal
inspector-parallel = Paralel
inspector-perpendicular = Tegak Lurus
inspector-tangent = Tangen
inspector-equal = Sama Panjang
inspector-fix = Kunci Posisi

# HUD & Dimension Pills
hud-extrude-btn = Ekstrusi
hud-revolve-btn = Putar
hud-loft-btn = Loft
hud-shell-btn = Shell
hud-boolean-btn = Boolean
hud-show-dimensions = Tampilkan Semua Ukuran
hud-hide-dimensions = Sembunyikan Ukuran
hud-click-to-edit = Klik untuk ubah ukuran
hud-normal-to-sketch = Normal ke Sketsa
hud-section-banner = Matikan Tampilan Irisan untuk melihat bagian tersembunyi
hud-turn-off = Matikan
hud-copy = Salin
hud-apply-enter = Terapkan (Enter)
hud-revolve-prompt-select = Pilih profil sketsa 2D tertutup dulu
hud-revolve-prompt-ready = Sumbu Siap! Atur Sudut & Terapkan
hud-revolve-prompt-step-1 = Langkah 1: Klik Titik 1 Sumbu Poros
hud-revolve-prompt-step-2 = Langkah 2: Klik Titik 2 Sumbu Poros
hud-loft-prompt-0 = Pilih 2 profil sketsa 2D (klik / drag kotak)
hud-loft-prompt-1 = Pilih profil ke-2 untuk menyelesaikan Loft
hud-loft-prompt-ready = Profil Siap! Atur Ketinggian & Buat 3D
hud-loft-create-enter = Buat 3D Loft (Enter)
hud-loft-warn-unaligned = ⚠️ Titik Tengah Belum Menyatu
hud-loft-align-question = Ingin satukan titik tengah (simetris) atau biarkan menceng (offset)?
hud-loft-align-center = 🎯 Satukan Titik Tengah
hud-loft-keep-offset = Biarkan Menceng (Offset)
hud-shell-prompt-select = Pilih salah satu sisi objek 3D
hud-shell-prompt-ready = Sisi Terpilih! Atur Ketebalan & Eksekusi
hud-shell-exec-enter = 🚀 Eksekusi Shell (Enter)
hud-boolean-prompt-select = Pilih min 2 body (Tahan Shift + Klik)
hud-boolean-prompt-ready = 2 Body Terpilih! Siap Diproses

# Feature Inspector Details
inspector-start-point = Titik Awal (Start):
inspector-end-point = Titik Akhir (End):
inspector-center-point = Pusat (Center):
inspector-apply-coords = Terapkan Koordinat
inspector-quick-constraints = Constraint Cepat:
inspector-horiz = — Horiz
inspector-vert = | Vert
inspector-radius-diameter = Radius (R) / Diameter (Ø), mm:
inspector-apply-dimensions = Terapkan Dimensi
inspector-length-p = Panjang (P):
inspector-width-w = Lebar (L):
inspector-anchor-help = Anchor (titik yg tetap diam saat resize):
inspector-apply-joint-constraints = Terapkan Constraint Bersama:
inspector-measure-hint = Klik 2 titik untuk jarak, 3 titik untuk sudut
inspector-clear-all = Hapus Semua
inspector-resize-tip = 💡 Resize: aktifkan "Tampilkan Semua Ukuran" (kartu Pengukuran di atas), lalu klik angka X/Y/Z yg muncul langsung di objek → ketik → Enter.
inspector-uniform-scale-note = Catatan: scale seragam (proporsional) — fillet/chamfer bisa ikut berubah bentuk kalau ukurannya besar sekali.
inspector-select-object-hint = Pilih objek di kanvas atau pohon item untuk melihat & mengubah dimensinya.
inspector-revolve-axis = Poros Sumbu:
inspector-axis-y-vert = Sumbu Y (Vertikal)
inspector-axis-x-horiz = Sumbu X (Horizontal)
inspector-axis-sketch-left = Tepi Kiri Sketsa
inspector-axis-sketch-bottom = Tepi Bawah Sketsa
inspector-show-all-dim-tooltip = Tampilkan nominal ukuran tiap garis/rusuk elemen di kanvas
inspector-loft-staged = Profil bawah: ✓ Staged
inspector-loft-unstaged = Profil bawah: Belum diset
inspector-set-bottom-profile = Set Profil Bawah
inspector-exec-loft = Eksekusi Loft
inspector-edge-pick-active = [x] Mode Pilih Tepi (Aktif)
inspector-edge-pick-manual = [ ] Mode Pilih Tepi Manual
inspector-edge-count = { $count } tepi
inspector-reset-edge-pick = Reset Seleksi Tepi
inspector-delete-selected-bodies = Hapus Body Terpilih
inspector-enable-section = Aktifkan Potongan
inspector-invert-direction = Balik arah
inspector-model-history = Riwayat Model 3D:
inspector-entities-count = • 2D Entitas: { $count } objek
inspector-bodies-count = • 3D Bodies: { $count } objek
inspector-revolve-3d = Revolve 3D (Benda Putar)
inspector-draw-2-points-manual = ✏️ Gambar 2 Titik Manual
inspector-click-2-points-canvas = ✏️ Klik 2 Titik di Kanvas
inspector-exec-revolve = 🚀 Eksekusi Revolve

# Revolve Dialog & 3D Popups
revolve-dialog-title = Revolve Solid 3D
revolve-dialog-subtitle = Bentuk solid 3D putar terhadap sumbu poros
revolve-dialog-select-hint = Pilih sketsa tertutup (lingkaran, persegi, atau loop garis) terlebih dahulu.
revolve-dialog-execute = Putar Profil
revolve-dialog-reverse = Balik Arah
revolve-dialog-window-title = ✨ Fitur Revolve (Putar 3D)
revolve-dialog-header-title = Revolve 3D — Buat Benda Putar
revolve-dialog-header-desc = Memutar sketsa 2D mengelilingi poros sumbu.
revolve-dialog-profile-ready = Profil Sketsa Siap ({ $count } entitas terpilih)
revolve-dialog-no-profile = Belum Ada Profil Tertutup Terpilih
revolve-dialog-select-axis-prompt = 1. Pilih Poros Sumbu Putar:
revolve-dialog-axis-y-origin = Sumbu Y (Vertikal Origin X=0)
revolve-dialog-axis-x-origin = Sumbu X (Horizontal Origin Y=0)
revolve-dialog-axis-bbox-left = Tepi Kiri Sketsa (Poros Silinder/Tabung)
revolve-dialog-axis-bbox-bottom = Tepi Bawah Sketsa
revolve-dialog-axis-manual = ✏️ Gambar Manual (Klik 2 Titik di Kanvas)
revolve-dialog-select-angle-prompt = 2. Sudut Putaran (Derajat):
revolve-dialog-angle-360 = 360° Penuh
revolve-dialog-angle-180 = 180° Setengah
revolve-dialog-angle-90 = 90° Siku
revolve-dialog-custom-deg = Kustom Derajat:
revolve-dialog-tip = Tips: Garis poros sumbu tidak boleh memotong bagian dalam profil.
revolve-dialog-start-manual-btn = ✏️ Mulai Klik 2 Titik Sumbu
alert-modal-default-title = Peringatan Operasi
alert-modal-tips-title = 💡 Tips Solusi:
alert-modal-dismiss-btn =   Mengerti  
popup-extrude-profile-title = Extrude Profil (3D)
popup-extrude-face-title = Extrude Sisi (Push-Pull)
popup-extrude-face-desc = Tarik atau dorong sisi model 3D:
popup-sketch-on-face = ✏ Sketsa di Sisi
popup-extrude-profile-desc = Tarik kurva / profil 2D menjadi solid 3D:
popup-loft-title = Loft Solid 3D
popup-loft-desc = Transisi bodi 3D dari 2 profil sketsa:
popup-loft-step-1 = Langkah 1: Profil Bawah
popup-loft-bottom-saved = ✓ Profil Bawah Tersimpan
popup-loft-click-p1 = ○ Klik profil 1 di kanvas lalu simpan:
popup-loft-set-bottom = 📥 Set Profil Bawah dari Seleksi
popup-loft-step-2 = Langkah 2: Profil Atas & Tinggi
popup-loft-click-p2 = Klik profil 2 di kanvas, lalu eksekusi:
popup-sweep-title = Sweep Solid 3D
popup-sweep-desc = Sapu profil 2D menyusuri kurva di bidang berbeda (mis. Top & Front):
popup-sweep-step-1 = Langkah 1: Profil Penampang
popup-sweep-profile-saved = ✓ Profil Penampang Tersimpan
popup-sweep-click-profile = ○ Pilih profil tertutup di bidang pertama (mis. Top):
popup-sweep-set-profile = 📥 Set Profil dari Seleksi
popup-sweep-step-2 = Langkah 2: Jalur Pemandu (Path)
popup-sweep-path-saved = ✓ Jalur Pemandu Tersimpan
popup-sweep-click-path = ○ Ganti bidang (mis. Front) & pilih kurva jalur:
popup-sweep-set-path = 📥 Set Jalur dari Seleksi
popup-sweep-step-3 = Langkah 3: Eksekusi Sweep
popup-sweep-create-btn = 🚀 Buat 3D Sweep
popup-shell-title = Shell 3D Berongga
popup-shell-face-active = ✓ Mode Pilih Wajah (Aktif)
popup-shell-face-enable = ○ Aktifkan Pilih Wajah Terbuka
popup-shell-faces-count = { $count } wajah
popup-boolean-title = Operasi Boolean 3D
popup-boolean-desc = Body terpilih: { $count } objek (butuh minimal 2)
revolve-axis-too-short-title = Revolve Gagal: Sumbu Terlalu Pendek
revolve-axis-too-short-desc = Dua titik sumbu yang Anda klik berada di posisi yang sama atau terlalu dekat.
revolve-axis-tip-1 = Klik dua titik yang berjarak jelas untuk membentuk garis sumbu.
revolve-axis-tip-2 = Atau gunakan preset 'Sumbu Y' / 'Sumbu X' di jendela opsi Revolve.
revolve-axis-staged-status = Sumbu poros terpasang. Sesuaikan sudut & arah lalu klik Terapkan (atau tekan Enter).

# Notifications & Status
status-ready = Siap
status-saved = Dokumen berhasil disimpan
status-saved-to = Tersimpan ke { $name }
status-exported = Berhasil diekspor ke { $format }
status-imported = Berhasil mengimpor { $count } body
status-error-export = Gagal mengekspor file: { $error }
status-error-import = Gagal mengimpor file: { $error }
status-error-save = Gagal menyimpan dokumen: { $error }
status-error-open = Gagal membuka dokumen: { $error }
status-error-op = Operasi gagal: { $error }
status-doc-filter = Dokumen DUCAD

# File I/O Operations & Dialogs
file-doc-ducad = Dokumen DUCAD
file-step-filter = STEP 3D CAD
file-stl-filter = STL Mesh
file-obj-filter = Wavefront OBJ
file-dxf-filter = AutoCAD DXF
file-saved-to = Tersimpan ke { $name }
file-save-failed = Gagal menyimpan: { $error }
file-opened = Dibuka: { $name }
file-open-failed = Gagal membuka: { $error }
file-act-open = Buka Berkas
file-act-open-desc = Membuka dokumen { $name }
file-no-bodies-step = Tak ada body 3D untuk diekspor ke STEP
file-exported-step = Diekspor ke STEP: { $name }
file-export-step-failed = Gagal ekspor STEP: { $error }
file-importing-step = Mengimpor STEP di latar belakang: { $name }…
file-imported-step = Sukses mengimpor STEP: { $name }
file-import-step-build-failed = Gagal membangun solid dari STEP: { $error }
file-import-step-failed = Gagal mengimpor STEP: { $error }
file-no-meshes-stl = Tak ada mesh 3D tampak untuk diekspor ke STL
file-exported-stl = Diekspor ke STL: { $name }
file-export-stl-failed = Gagal ekspor STL: { $error }
file-no-meshes-obj = Tak ada mesh 3D tampak untuk diekspor ke OBJ
file-exported-obj = Diekspor ke OBJ: { $name }
file-export-obj-failed = Gagal ekspor OBJ: { $error }
file-sketch-empty-dxf = Sketsa aktif kosong — tak ada entitas untuk diekspor
file-exported-dxf = Diekspor ke DXF: { $name }
file-export-dxf-failed = Gagal ekspor DXF: { $error }
file-dxf-no-entities = File DXF terbaca tapi tidak memuat entitas 2D yang didukung
file-imported-dxf = Diimpor dari { $name }: { $count } entitas
file-import-dxf-failed = Gagal impor DXF: { $error }
file-act-import-dxf = Impor DXF
file-act-import-step = Impor { $name }

# Interactive Status Bar Tool Prompts
status-prompt-select = Pilih: klik entitas, Shift+klik multi-pilih, Delete hapus
status-prompt-line-0 = Garis: klik titik awal (L)
status-prompt-line-close = Garis: klik titik berikutnya, klik titik awal untuk tutup loop, atau ESC untuk selesai
status-prompt-line-next = Garis: klik titik berikutnya, atau ESC untuk selesai
status-prompt-rect-0 = Persegi: klik sudut pertama (R)
status-prompt-rect-opp = Persegi: klik sudut berlawanan
status-prompt-circle-0 = Lingkaran: klik titik pusat (C)
status-prompt-circle-rad = Lingkaran: klik untuk radius, atau ketik radius lalu Enter
status-prompt-ellipse-0 = Elips: klik titik pusat (E)
status-prompt-ellipse-box = Elips: klik sudut kotak pembatas
status-prompt-arc-0 = Busur: klik titik awal (A)
status-prompt-arc-1 = Busur: klik titik lengkungan busur
status-prompt-arc-2 = Busur: klik titik akhir busur
status-prompt-offset-none = Offset: klik entitas sumber (O)
status-prompt-offset-side = Offset: klik sisi & jarak hasil offset
status-prompt-mirror-empty = Cermin: pilih entitas di tool Pilih dulu, lalu tekan M
status-prompt-mirror-p1 = Cermin: klik titik 1 sumbu cermin ({ $count } entitas terpilih)
status-prompt-mirror-p2 = Cermin: klik titik 2 sumbu cermin
status-prompt-trim = Pangkas: klik segmen garis yang mau dipotong (T)
status-prompt-fillet-2d = Fillet 2D: klik titik sudut atau pilih garis untuk membulatkan sudut (F)
status-prompt-chamfer-2d = Chamfer 2D: klik titik sudut atau pilih garis untuk memotong miring sudut
status-prompt-revolve-empty = Putar: pilih profil di tool Pilih dulu, lalu tekan V
status-prompt-revolve-p1 = Putar: klik titik 1 sumbu ({ $count } entitas terpilih, 360°)
status-prompt-revolve-p2 = Putar: klik titik 2 sumbu
status-prompt-coincident-0 = Koinsiden: klik titik pertama (endpoint/center)
status-prompt-coincident-1 = Koinsiden: klik titik kedua
status-prompt-fixed = Tetap: klik titik (endpoint/center) untuk mengunci di posisi sekarang
status-prompt-symmetric-axis = Simetris: pilih 1 Garis jadi sumbu di tool Pilih dulu
status-prompt-symmetric-0 = Simetris: klik titik pertama (endpoint/center)
status-prompt-symmetric-1 = Simetris: klik titik kedua
status-prompt-measure-0 = Ukur: klik titik pertama
status-prompt-measure-1 = Ukur: klik titik kedua
status-prompt-measure-ang-0 = Ukur Sudut: klik titik awal
status-prompt-measure-ang-1 = Ukur Sudut: klik titik sudut (vertex)
status-prompt-measure-ang-2 = Ukur Sudut: klik titik akhir
status-prompt-extrude = Ekstrusi: tarik panah gizmo atau klik angka dimensi ruler untuk atur ketinggian
status-prompt-loft = Loft: atur profil bawah & tinggi pada popup kanan bawah
status-prompt-shell = Shell: pilih sisi terbuka lalu atur ketebalan dinding (S)
status-prompt-boolean = Boolean: pilih minimal 2 body solid lalu pilih operasi (B)
status-prompt-section = Tampilan Irisan: atur bidang potongan solid 3D
status-prompt-history = Riwayat: lihat jejak langkah modeling dan lakukan Undo / Redo (H)

# Tool Guides Detailed Steps & Tips
guide-line-header = Panduan Line (Garis):
guide-line-step-1 = 1. Klik Titik Awal
guide-line-step-2 = 2. Tarik & Klik Titik Akhir
guide-line-step-2-active = 2. Tarik & Klik Titik Akhir (Langkah Aktif)
guide-line-tip = 💡 Tahan Shift untuk snap garis lurus 0°/45°/90°

guide-rect-header = Panduan Rectangle (Kotak):
guide-rect-step-1 = 1. Klik Sudut Pertama
guide-rect-step-2 = 2. Tarik ke Sudut Diagonal
guide-rect-step-2-active = 2. Tarik ke Sudut Lawan (Langkah Aktif)
guide-rect-tip = 💡 Sudut awal menjadi jangkar posisi kotak

guide-circle-header = Panduan Circle (Lingkaran):
guide-circle-step-1 = 1. Klik Titik Pusat Lingkaran
guide-circle-step-2 = 2. Tarik & Tentukan Radius (R)
guide-circle-step-2-active = 2. Tarik Radius Jari-Jari (Langkah Aktif)
guide-circle-tip = 💡 Ukuran radius dapat disesuaikan di popup

guide-arc-header = Panduan Arc (Busur 3-Titik):
guide-arc-step-1 = 1. Klik Titik Awal Busur
guide-arc-step-2 = 2. Klik Titik Lengkungan (Kurva)
guide-arc-step-2-active = 2. Klik Titik Lengkungan (Langkah Aktif)
guide-arc-step-3 = 3. Klik Titik Akhir Busur
guide-arc-step-3-active = 3. Klik Titik Akhir Busur (Langkah Aktif)
guide-arc-step-done = Busur Terbentuk (3 Titik)
guide-arc-tip = 💡 Urutan: Titik Awal → Lengkungan → Titik Akhir

guide-ellipse-header = Panduan Ellipse (Elips):
guide-ellipse-step-1 = 1. Klik Titik Pusat
guide-ellipse-step-2 = 2. Tarik Radius Mayor (Rx)
guide-ellipse-step-3 = 3. Tarik Radius Minor (Ry)
guide-ellipse-tip = 💡 Rx & Ry mengatur kelonjongan elips

guide-spline-header = Panduan Spline (Kurva Organik):
guide-spline-step-1 = 1. Klik Titik Awal Kurva
guide-spline-step-2 = 2. Klik Titik-Titik Kurva Berikutnya
guide-spline-step-3 = 3. Tekan Enter / Dobel Klik untuk Selesai
guide-spline-step-active = Titik Kurva (Langkah Aktif)
guide-spline-tip = 💡 Klik kembali ke titik awal untuk menutup loop kurva menjadi profil

guide-offset-header = Panduan Offset Sketsa:
guide-offset-step-1 = 1. Klik Kurva Sumber
guide-offset-step-2 = 2. Geser Jarak & Sisi Offset
guide-offset-tip = 💡 Arah geser mouse menentukan sisi luar/dalam

guide-mirror-header = Panduan Mirror (Cermin):
guide-mirror-step-1 = 1. Pilih Sketsa Sumber
guide-mirror-step-2 = 2. Klik 2 Titik Sumbu Cermin
guide-mirror-step-3 = 3. Hasil Cermin Terduplikasi
guide-mirror-tip = 💡 Garis sumbu mendefinisikan bidang simetri
guide-mirror-symmetric = ⇄ Simetris

guide-trim-header = Panduan Trim (Gunting):
guide-trim-step-1 = 1. Arahkan ke Garis Berpotongan
guide-trim-step-2 = 2. Klik Segmen yang Mau Dipotong
guide-trim-tip = 💡 Memotong segmen garis hingga titik potong terdekat
guide-trim-badge = ✂ Terpotong

guide-coincident-header = Panduan Coincident (Penyatuan Titik):
guide-coincident-step-1 = 1. Klik Titik 1
guide-coincident-step-2 = 2. Klik Titik 2 atau Garis
guide-coincident-step-done = Titik Tersambung
guide-coincident-tip = 💡 Menempelkan 2 titik atau titik ke garis secara permanen
guide-coincident-badge = 🔗 Menyatu

guide-fixed-header = Panduan Fixed (Kunci Posisi):
guide-fixed-step-1 = 1. Klik Titik untuk Mengunci
guide-fixed-step-done = Titik Terkunci (Fixed)
guide-fixed-tip = 💡 Titik fixed tidak akan bergeser oleh solver sketsa
guide-fixed-badge = ⚓ Terkunci

guide-symmetric-header = Panduan Symmetric (Simetris):
guide-symmetric-step-1 = 1. Pilih 1 Garis Jadi Sumbu
guide-symmetric-step-2 = 2. Klik Titik 1 & 2
guide-symmetric-step-done = Simetri Diterapkan
guide-symmetric-tip = 💡 Menjaga jarak kedua titik seimbang terhadap sumbu
guide-symmetric-badge = ⇄ Simetris

guide-extrude-header = Panduan Extrude (Tarik Padat 3D):
guide-extrude-step-1 = 1. Pilih Profil Tertutup
guide-extrude-step-2 = 2. Tarik Panah Ketinggian
guide-extrude-step-done = Solid 3D Terbentuk
guide-extrude-tip = 💡 Tarik panah gizmo atau klik dimensi ruler

guide-loft-header = Panduan Loft 3D (Mode 2D):
guide-loft-step-1 = 1. Pilih Profil 1
guide-loft-step-2 = 2. Pilih Profil 2
guide-loft-step-done = Solid Loft Terbentuk
guide-loft-tip = 💡 Pilih 2 profil di kanvas -> atur tinggi di Top Bar -> Enter
guide-loft-badge = ✓ Loft 3D

guide-sweep-header = Panduan Sweep 3D:
guide-sweep-step-1 = 1. Pilih Profil 2D (Bidang 1)
guide-sweep-step-2 = 2. Pilih Jalur Kurva (Bidang 2)
guide-sweep-step-done = Solid Sweep Terbentuk
guide-sweep-tip = 💡 Gambar Profil di bidang 1 (mis. Top), lalu ganti ke bidang lain (mis. Front) untuk Jalur!
guide-sweep-badge = ✓ Sweep 3D
status-prompt-sweep = Sweep: tetapkan profil penampang & jalur kurva di popup kanan bawah


guide-shell-header = Panduan Shell (Bodi Berongga):
guide-shell-step-1 = 1. Pilih Sisi Terbuka
guide-shell-step-2 = 2. Atur Ketebalan Dinding
guide-shell-step-done = Bodi Berongga Terbentuk
guide-shell-tip = 💡 Mengosongkan bagian dalam benda padat dengan ketebalan t

guide-boolean-header = Panduan Boolean 3D:
guide-boolean-step-1 = 1. Pilih Bodi Target & Alat
guide-boolean-step-2 = 2. Pilih Operasi (Gabung/Potong)
guide-boolean-step-done = Operasi Selesai
guide-boolean-tip = 💡 Pilih mode di Top HUD lalu klik Terapkan (Enter)
boolean-union-badge = ∪ Gabung
boolean-subtract-badge = - Potong
boolean-intersect-badge = ∩ Irisan

guide-section-header = Panduan Section View (Irisan Dalam):
guide-section-step-1 = 1. Pilih Bidang Irisan (X/Y/Z)
guide-section-step-2 = 2. Atur Pergeseran Potongan
guide-section-tip = 💡 Menginspeksi rongga internal tanpa merusak 3D
guide-section-badge = 🔍 Potongan

guide-measure-header = Panduan Measure (Ukur Jarak):
guide-measure-step-1 = 1. Klik Elemen 1
guide-measure-step-2 = 2. Klik Elemen 2
guide-measure-step-2-active = 2. Klik Elemen 2 (Langkah Aktif)
guide-measure-tip = 💡 Pengukuran non-destruktif untuk inspeksi dimensi

guide-measure-angle-header = Panduan Measure Angle (Ukur Sudut):
guide-measure-angle-step-1 = 1. Klik Garis 1
guide-measure-angle-step-2 = 2. Klik Titik Sudut
guide-measure-angle-step-3 = 3. Klik Garis 2
guide-measure-angle-tip = 💡 Mengukur sudut presisi dalam satuan derajat (°)

# Units
unit-mm = mm (Milimeter)
unit-cm = cm (Sentimeter)
unit-m = m (Meter)
unit-inch = in (Inci)
