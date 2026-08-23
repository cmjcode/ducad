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
guide-measure-angle-title = Alat Ukur Sudut
guide-measure-angle-prompt = Klik dua garis/rusuk untuk mengukur sudut.

# Popups & Dialogs
popup-extrude-title = Parameter Ekstrusi
popup-revolve-title = Parameter Revolve
popup-loft-title = Parameter Loft
popup-shell-title = Parameter Shell
popup-boolean-title = Operasi Boolean
popup-measure-title = Rincian Pengukuran
popup-history-title = Riwayat Operasi
popup-entity-title = Properti Entitas

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
hud-click-to-edit = Klik untuk edit ukuran
hud-normal-to-sketch = Tegak Lurus ke Sketsa
hud-section-banner = Matikan Tampilan Irisan untuk menampilkan bagian tersembunyi
hud-turn-off = Matikan
hud-copy = Salin

# Notifications & Status
status-ready = Siap
status-saved = Dokumen berhasil disimpan
status-exported = Berhasil diekspor ke { $format }
status-imported = Berhasil mengimpor { $count } body
status-error-export = Gagal mengekspor file: { $error }
status-error-import = Gagal mengimpor file: { $error }
status-error-save = Gagal menyimpan dokumen: { $error }
status-error-open = Gagal membuka dokumen: { $error }
status-error-op = Operasi gagal: { $error }

# Units
unit-mm = mm (Milimeter)
unit-cm = cm (Sentimeter)
unit-m = m (Meter)
unit-inch = in (Inci)
