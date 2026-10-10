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
menu-import-stl = STL…
menu-import-dxf = DXF…
menu-export = Ekspor
menu-export-step = STEP… (semua body)
menu-export-stl = STL… (body tampak)
menu-export-obj = OBJ… (body tampak)
menu-export-glb = GLTF / GLB… (Web & AR)
menu-export-dxf = DXF… (sketsa)
menu-export-svg = SVG… (sketsa 2D)
menu-export-drawing-svg = SVG Gambar Teknik…
menu-export-vector-snapshot = Vector Snapshot… (SVG tampilan kamera)
menu-settings = Pengaturan
menu-theme = Tema
menu-theme-dark = Mode Gelap
menu-theme-light = Mode Terang
menu-liquid-glass = Liquid Glass
menu-reduce-transparency = Kurangi transparansi
settings-icon-size = Ukuran Ikon
menu-shortcuts = Pintasan Keyboard
menu-command-palette = Palet Perintah
cmd-no-match = Tidak ada perintah cocok
cmd-search-hint = Cari perintah…
cmd-hint-navigate = navigasi
cmd-hint-run = jalankan
cmd-hint-close = tutup
menu-all-commands = Semua Perintah
cmd-group-ai = AI & Agent
cmd-group-mode = Mode & Bidang Kerja
cmd-group-file = Berkas
cmd-group-sketch = Alat Sketsa
cmd-group-solid = Alat 3D
cmd-group-analysis = Ukur & Analisis
cmd-group-edit = Edit
cmd-group-view = Tampilan & Pengaturan

# Top Bar Actions & Tooltips
topbar-home-tooltip = Dokumen Baru
topbar-saved-tooltip = Dokumen tersimpan
topbar-unsaved-tooltip = Perubahan belum disimpan
topbar-share = Bagikan / Ekspor
topbar-items = Item
topbar-items-tooltip = Pohon Item & Outliner
topbar-search-tooltip = Cari & Palet Perintah (Ctrl/Cmd+Shift+P atau Ctrl/Cmd+K)
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
mode-camera-locked = { $mode }: kamera terkunci ke bidang. Tekan Esc, tombol pensil/kotak di header, atau ⌘+Alt+3 untuk kembali ke 3D.
topbar-unit = Satuan: { $unit }
topbar-zoom-tooltip = Zoom: { $percent }% (100% = ukuran sebenarnya)
topbar-zoom-hint = Klik untuk opsi zoom; gulir atau cubit di viewport untuk zoom
topbar-zoom-in = Perbesar
topbar-zoom-out = Perkecil
topbar-zoom-actual = Ukuran Sebenarnya (100%)
topbar-zoom-fit = Pas ke Layar

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
tool-polygon = Segi-N (Poligon)
tool-polygon-desc = Poligon N-sisi beraturan (Inscribed / Circumscribed)
tool-slot = Slot (Lonjong)
tool-slot-desc = Lubang slot lonjong / rel baut (Antar-Pusat atau Panjang Total)
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
tool-extend = Perpanjang
tool-extend-desc = Perpanjang segmen garis sampai kurva batas terdekat
tool-coincident = Titik Koinsiden
tool-coincident-desc = Gabung dua titik atau tempelkan titik ke kurva
tool-fixed = Titik Tetap
tool-fixed-desc = Kunci posisi titik di ruang
tool-symmetric = Titik Simetris
# Vector Tools
tool-pen-bezier = Pen Bézier
tool-pen-bezier-desc = Gambar kurva dan garis Bézier presisi
tool-node-edit = Edit Node
tool-node-edit-desc = Edit node sudut, halus, dan handle kurva
tool-shape-builder = Pembentuk Bentuk
tool-shape-builder-desc = Operasi boolean gabung, potong, iris bentuk vektor
tool-eyedropper = Pipet Warna
tool-eyedropper-desc = Ambil dan terapkan gaya warna objek
tool-gradient = Gradien
tool-gradient-desc = Atur gradien linier dan radial

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
tool-helix = Helix / Pegas
tool-helix-desc = Buat kurva spiral 3D, pegas kawat, bilah sekrup auger, atau ulir
tool-helix-name = Generator Helix & Pegas
tool-shell = Shell (Rongga Casing)
tool-shell-name = Shell (Rongga Casing)
tool-shell-desc = Buat rongga pada solid dengan ketebalan dinding seragam atau bervariasi (H)
tool-rib = Tulang Penguat (Rib)
tool-rib-name = Tulang Penguat (Rib)
tool-rib-desc = Buat tulang penguat (stiffener ribs) penahan deformasi pada casing hollow (R)
tool-draft-angle = Draft Angle (Kemiringan Cetakan)
tool-draft-angle-desc = Tambahkan sudut kemiringan cetakan plastik (injection molding) pada bidang planar (D)
tool-split-body = Split Body & Face (Potong Benda)
tool-split-body-desc = Potong solid 3D menjadi 2 body terpisah atau bagi face menggunakan bidang/permukaan (S)
tool-pattern = Pattern / Array
tool-pattern-desc = Gandakan objek secara linier (grid X/Y/Z) atau sirkular (melingkar) (P)
tool-boolean = Operasi Boolean
tool-boolean-desc = Gabung, Kurang, atau Irisan benda 3D
tool-section = Tampilan Irisan
tool-section-desc = Bidang penampang interaktif
tool-zebra-stripes = Garis Zebra
tool-zebra-stripes-desc = Proyeksikan garis pantulan specular untuk inspeksi kontinuitas tangensi G1 & kurvatur G2 (Z)
topbar-zebra-tooltip = Beralih Mode Analisis Refleksi Garis Zebra (Z)
zebra-angle = Orientasi:
zebra-horizontal = Horizontal
zebra-vertical = Vertikal
zebra-frequency = Garis:
zebra-blend = Blend:
tool-draft-analysis = Analisis Sudut Lepas
tool-draft-analysis-desc = Inspeksi sudut kemiringan permukaan terhadap arah buka cetakan dengan heatmap DFM real-time
topbar-draft-tooltip = Beralih Mode Analisis Sudut Lepas Cetakan (Draft Heatmap)
draft-pull-dir = Arah Cetakan:
draft-target-angle = Sudut Draft:
draft-blend = Blend:
draft-safe-legend = Aman (≥ { $angle }°)
draft-warning-legend = Draft Kritis (0°–{ $angle }°)
draft-undercut-legend = Undercut (< 0°)
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
guide-shell-prompt = Klik sisi mana pun yang ingin dibuka, atau pilih body lalu tentukan arahnya.
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
param-radius-start = Radius Awal
param-radius-end = Radius Akhir
tool-fillet-name = Fillet
tool-chamfer-name = Chamfer
tool-fillet-variable = Fillet Variabel
inspector-fillet-variable-toggle = Radius Variabel (R_awal != R_akhir)
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
drawer-tab-objects = Objek
drawer-tab-properties = Properti
drawer-tab-layers = Layer
drawer-vector-sketch-inactive = Mode Sketsa Tidak Aktif
drawer-vector-sketch-inactive-desc = Buka atau mulai sketsa 2D untuk mengedit isian, garis tepi, dan gaya vektor.
drawer-vector-layers-inactive-desc = Buka atau mulai sketsa 2D untuk mengelola layer vektor.
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
hud-shell-prompt-select = Klik sisi yang ingin dibuka, atau pilih 1 body
hud-shell-prompt-ready = Sisi Terpilih! Atur Ketebalan & Eksekusi
hud-shell-exec-enter = 🚀 Eksekusi Shell (Enter)
hud-shell-open-dir = Buka ke arah
hud-shell-faces-open = { $count } sisi dibuka
hud-shell-depth-hint = Kedalaman rongga dari sisi terbuka. 0 = rongga penuh.
hud-shell-depth-single-face = Kedalaman hanya berlaku bila satu sisi dibuka
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
inspector-resize-tip = 💡 Resize: aktifkan "Tampilkan Semua Ukuran" (kartu Pengukuran di atas), lalu klik angka X/Y/Z yg muncul langsung di objek » ketik » Enter.
inspector-uniform-scale-note = Catatan: scale seragam (proporsional) — fillet/chamfer bisa ikut berubah bentuk kalau ukurannya besar sekali.
inspector-select-object-hint = Pilih objek di kanvas atau pohon item untuk melihat & mengubah dimensinya.
inspector-revolve-axis = Poros Sumbu:
inspector-axis-y-vert = Sumbu Y (Vertikal)
inspector-axis-x-horiz = Sumbu X (Horizontal)
inspector-axis-sketch-left = Tepi Kiri Sketsa
inspector-axis-sketch-bottom = Tepi Bawah Sketsa
inspector-show-all-dim-tooltip = Tampilkan nominal ukuran tiap garis/rusuk elemen di kanvas
inspector-loft-staged = Profil bawah: ✔ Staged
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
popup-loft-bottom-saved = ✔ Profil Bawah Tersimpan
popup-loft-click-p1 = ○ Klik profil 1 di kanvas lalu simpan:
popup-loft-set-bottom = 📥 Set Profil Bawah dari Seleksi
popup-loft-step-2 = Langkah 2: Profil Atas & Tinggi
popup-loft-click-p2 = Klik profil 2 di kanvas, lalu eksekusi:
popup-sweep-title = Sweep Solid 3D
popup-sweep-desc = Sapu profil 2D menyusuri kurva di bidang berbeda (mis. Top & Front):
popup-sweep-step-1 = Langkah 1: Profil Penampang
popup-sweep-profile-saved = ✔ Profil Penampang Tersimpan
popup-sweep-click-profile = ○ Pilih profil tertutup di bidang pertama (mis. Top):
popup-sweep-set-profile = 📥 Set Profil dari Seleksi
popup-sweep-step-2 = Langkah 2: Jalur Pemandu (Path)
popup-sweep-path-saved = ✔ Jalur Pemandu Tersimpan
popup-sweep-click-path = ○ Ganti bidang (mis. Front) & pilih kurva jalur:
popup-sweep-set-path = 📥 Set Jalur dari Seleksi
popup-sweep-step-3 = Langkah 3: Eksekusi Sweep
popup-sweep-create-btn = 🚀 Buat 3D Sweep
popup-shell-title = Shell 3D Berongga
popup-shell-face-active = ✔ Mode Pilih Wajah (Aktif)
popup-shell-face-enable = ○ Aktifkan Pilih Wajah Terbuka
popup-shell-faces-count = { $count } wajah
popup-draft-title = Draft Angle — Kemiringan Cetakan
popup-draft-desc = Tambahkan kemiringan cetakan injeksi plastik pada sisi (face) datar agar produk mudah dilepas dari cetakan.
popup-draft-face-enable = ○ Pilih Face untuk Di-draft
popup-draft-face-active = ✔ Memilih Face (Aktif)
popup-draft-no-face = Belum ada face datar dipilih
popup-draft-faces-count = { $count } face terpilih
popup-draft-apply = Terapkan Draft Angle
popup-draft-invalid-angle = Sudut harus antara 0° dan 90° (eksklusif)

# Popup & HUD Split Body / Split Face
popup-split-title = Split Body & Split Face
popup-split-desc = Potong solid 3D menjadi dua bagian terpisah di Items Drawer atau bagi face menggunakan bidang pemotong.
popup-split-target-body = Target Body
popup-split-no-body = Belum ada body 3D dipilih
popup-split-plane = Bidang
popup-split-plane-xy = XY (Atas)
popup-split-plane-xz = XZ (Depan)
popup-split-plane-yz = YZ (Kanan)
popup-split-plane-face = Face Terpilih
popup-split-offset = Offset
popup-split-mode-body = Body
popup-split-mode-face = Face
popup-split-apply = Potong Body
popup-split-apply-face = Bagi Face
param-draft-angle = Sudut Draft
param-pull-dir = Arah Bukaan (Pull)

# Popup & HUD Pattern (Array Linier & Sirkular)
popup-pattern-title = Pattern / Array (2D & 3D)
popup-pattern-desc = Duplikasi geometri terpilih dalam susunan kisi linier (X, Y, Z) atau melingkar (poros putar).
pattern-mode-linear = Linier
pattern-mode-circular = Sirkular
param-pattern-mode = Mode
param-count-x = Jumlah X
param-pitch-x = Jarak X (mm)
param-count-y = Jumlah Y
param-pitch-y = Jarak Y (mm)
param-count-z = Jumlah Z
param-pitch-z = Jarak Z (mm)
param-pattern-count = Jumlah Item
param-pattern-angle = Sudut Total
param-pattern-axis = Sumbu Poros
popup-pattern-apply = Terapkan Pattern
popup-pattern-no-selection-2d = Pilih minimal 1 entitas sketsa untuk membuat Pattern
popup-pattern-no-selection-3d = Pilih minimal 1 body 3D untuk membuat Pattern
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
file-glb-filter = Model 3D GLTF Binary (*.glb)
file-no-meshes-glb = Tak ada mesh 3D solid tampak untuk diekspor ke GLB
file-exported-glb = Diekspor ke GLTF/GLB: { $name }
file-export-glb-failed = Gagal ekspor GLB: { $error }
file-sketch-empty-dxf = Sketsa aktif kosong — tak ada entitas untuk diekspor
file-exported-dxf = Diekspor ke DXF: { $name }
file-export-dxf-failed = Gagal ekspor DXF: { $error }
file-svg-filter = Vektor 2D SVG (*.svg)
file-sketch-empty-svg = Sketsa aktif kosong — tak ada entitas untuk diekspor ke SVG
file-exported-svg = Diekspor ke SVG: { $name }
file-export-svg-failed = Gagal ekspor SVG: { $error }
file-dxf-no-entities = File DXF terbaca tapi tidak memuat entitas 2D yang didukung
file-imported-dxf = Diimpor dari { $name }: { $count } entitas
file-import-dxf-failed = Gagal impor DXF: { $error }
file-importing-stl = Mengimpor STL di latar belakang: { $name }…
file-imported-stl = Sukses mengimpor STL: { $name }
file-import-stl-failed = Gagal mengimpor STL: { $error }
file-act-import-dxf = Impor DXF
file-act-import-step = Impor { $name }
file-act-import-stl = Impor STL { $name }

# Interactive Status Bar Tool Prompts
status-prompt-select = Pilih: klik entitas, Shift+klik multi-pilih, Delete hapus
status-prompt-line-0 = Garis: klik titik awal (L)
status-prompt-line-close = Garis: klik titik berikutnya, klik titik awal untuk tutup loop, atau ESC untuk selesai
status-prompt-line-next = Garis: klik titik berikutnya, atau ESC untuk selesai
status-prompt-rect-0 = Persegi: klik sudut pertama (R)
status-prompt-rect-opp = Persegi: klik sudut berlawanan
status-prompt-circle-0 = Lingkaran: klik titik pusat (C)
status-prompt-circle-rad = Lingkaran: klik untuk radius, atau ketik radius lalu Enter
status-prompt-polygon-0 = Poligon Segi-{ $sides }: klik titik pusat (Y)
status-prompt-polygon-1 = Poligon Segi-{ $sides }: klik radius & orientasi sudut, atau ketik nilai lalu Enter
status-prompt-slot-0 = Slot: klik titik pusat / ujung pertama
status-prompt-slot-1 = Slot: klik titik pusat / ujung kedua
status-prompt-slot-2 = Slot: klik atau ketik lebar / diameter lalu tekan Enter
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
status-prompt-extend = Perpanjang: klik garis di dekat ujung yang ingin diperpanjang ke batas terdekat (Shift+E)
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
status-prompt-shell = Shell: klik sisi mana pun untuk dibuka, atur tebal dan kedalaman (S)
status-prompt-draft = Draft Angle: pilih face datar untuk menambahkan kemiringan cetakan (D)
status-prompt-pattern = Pattern / Array: atur jumlah & jarak (Linier) atau sudut putar (Sirkular) di Top HUD lalu klik Terapkan (Enter) (P)
status-prompt-boolean = Boolean: pilih minimal 2 body solid lalu pilih operasi (B)
status-prompt-section = Tampilan Irisan: atur bidang potongan solid 3D
status-prompt-history = Riwayat: lihat jejak langkah modeling dan lakukan Undo / Redo (H)

# Tool Guides Detailed Steps & Tips
guide-sheet-section-header = Panduan Section (Garis Potong):
guide-sheet-section-step-1 = 1. Klik titik awal pada Tampak Depan/Atas/Kanan
guide-sheet-section-step-2 = 2. Klik titik akhir (Shift+klik = potongan bertingkat)
guide-sheet-section-tip = Arah pandang = sisi kiri arah garis. Esc untuk batal.
guide-sheet-dim-header = Panduan Dimensi Asosiatif:
guide-sheet-dim-step-1 = 1. Klik pusat lingkaran atau ujung tepi
guide-sheet-dim-step-2 = 2. Klik titik kedua di tampak yang sama
guide-sheet-dim-tip = Dimensi menempel ke fitur dan ikut berubah saat ukuran diubah.
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
guide-arc-tip = 💡 Urutan: Titik Awal » Lengkungan » Titik Akhir

guide-ellipse-header = Panduan Ellipse (Elips):
guide-ellipse-step-1 = 1. Klik Titik Pusat
guide-ellipse-step-2 = 2. Tarik Radius Mayor (Rx)
guide-ellipse-step-3 = 3. Tarik Radius Minor (Ry)
guide-ellipse-tip = 💡 Rx & Ry mengatur kelonjongan elips

guide-polygon-header = Panduan Segi-N (Polygon):
guide-polygon-step-1 = 1. Klik Titik Pusat Poligon
guide-polygon-step-2 = 2. Tentukan Radius & Sudut
guide-polygon-step-2-active = 2. Tentukan Radius & Sudut (Langkah Aktif)
guide-polygon-tip = 💡 Pilih jumlah sisi (N) & Inscribed/Circumscribed di HUD header

hud-polygon-title = ⬣ Segi-{ $sides } Beraturan
hud-polygon-prompt-radius = ⬣ Segi-{ $sides }: Atur Radius & Sudut
hud-polygon-sides = Sisi (N):
hud-polygon-inscribed = Inscribed (Dalam)
hud-polygon-circumscribed = Circumscribed (Luar)
dim-polygon-inscribed = Inscribed R
dim-polygon-circumscribed = Circumscribed R

guide-slot-header = Panduan Slot (Lubang Lonjong):
guide-slot-step-1 = 1. Klik Titik Pusat/Ujung Pertama
guide-slot-step-2 = 2. Klik Titik Pusat/Ujung Kedua
guide-slot-step-2-active = 2. Klik Titik Pusat/Ujung Kedua (Langkah Aktif)
guide-slot-step-3 = 3. Tentukan Lebar / Diameter
guide-slot-step-3-active = 3. Tentukan Lebar / Diameter (Langkah Aktif)
guide-slot-tip = 💡 Ganti mode Antar-Pusat atau Panjang Total di HUD header

hud-slot-title = ◯ Tool Slot
hud-slot-prompt-p1 = ◯ Slot: Klik titik pusat / ujung pertama
hud-slot-prompt-p2 = ◯ Slot: Klik titik pusat / ujung kedua
hud-slot-prompt-width = ◯ Slot: Tentukan lebar / diameter
hud-slot-mode = Mode:
hud-slot-center-to-center = Antar Pusat
hud-slot-overall = Panjang Total
hud-slot-width = Lebar (Ø):
hud-chain-finish = Selesai
hud-chain-cancel = Batal
hud-spline-prompt-next = Spline: ketuk titik berikutnya
hud-spline-prompt-more = Spline: { $count } titik. Ketuk titik awal untuk menutup, atau Selesai
hud-line-prompt-next = Garis: ketuk titik berikutnya, atau Selesai untuk mengakhiri rantai
dim-slot-c2c = Slot Antar-Pusat
dim-slot-overall = Slot Panjang Total
dim-slot-width = Lebar

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
guide-loft-badge = ✔ Loft 3D

guide-sweep-header = Panduan Sweep 3D:
guide-sweep-step-1 = 1. Buat Profil & Jalur di Bidang Berbeda
guide-sweep-step-2 = 2. Pilih Profil 2D & Klik Sweep
guide-sweep-step-3 = 3. Pilih Kurva Jalur di Kanvas
guide-sweep-step-done = 4. Konfirmasi di HUD (Enter)
guide-sweep-tip = 💡 Klik Sweep pada profil tertutup, lalu langsung pilih kurva jalur di bidang manapun tanpa perlu ganti bidang manual!
guide-sweep-badge = ✔ Sweep 3D
status-prompt-sweep = Sweep: Pilih profil tertutup & kurva jalur pada bidang manapun, lalu konfirmasi di HUD (Enter)
hud-sweep-prompt-profile = 1️⃣ Pilih profil 2D tertutup pada bidang manapun di kanvas
hud-sweep-prompt-path = 2️⃣ Pilih kurva jalur pemandu (garis, busur, atau spline) pada bidang lain
hud-sweep-prompt-ready = ✔ Profil & Jalur terpilih! Klik 'Buat Sweep 3D' atau tekan Enter
hud-sweep-exec-btn = 🚀 Buat Sweep 3D
hud-sweep-reset-profile = ↺ Ganti Profil
hud-sweep-cancel = ✖ Batal

hud-rib-prompt-ready = ✔ Casing/garis terpilih! Atur tebal & kedalaman lalu klik 'Buat Tulang Penguat'
hud-rib-prompt-select = 1️⃣ Pilih body casing 3D atau buat garis sketsa untuk tulang penguat
hud-rib-exec-enter = 🚀 Buat Tulang Penguat (Enter)

param-depth = Kedalaman
param-draft = Kemiringan

guide-shell-header = Panduan Shell (Bodi Berongga & Dinding Bervariasi):
guide-shell-step-1 = 1. Klik Sisi yang Dibuka (arah mana pun)
guide-shell-step-2 = 2. Atur Tebal Dinding & Kedalaman Rongga
guide-shell-step-done = Bodi Berongga Terbentuk
guide-shell-tip = 💡 Mengosongkan bagian dalam benda padat dengan ketebalan seragam atau bervariasi (Pintasan: H)

guide-rib-header = Panduan Rib (Tulang Penguat Casing):
guide-rib-step-1 = 1. Pilih Casing / Garis Rusuk
guide-rib-step-2 = 2. Atur Tebal & Kedalaman Rib
guide-rib-step-done = Tulang Penguat Terpasang
guide-rib-tip = 💡 Memperkuat dinding tipis casing plastik / die-cast (Pintasan: R)

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

guide-split-header = Panduan Split Body & Split Face:
guide-split-step-1 = 1. Pilih body 3D yang ingin dipotong
guide-split-step-2 = 2. Pilih bidang pemotong & offset
guide-split-step-3 = 3. Potong menjadi 2 Body terpisah
guide-split-tip = 💡 Pintasan: S | Tekan Enter untuk potong
guide-split-badge = 2 Bodies

guide-pattern-header = Panduan Pattern (Linier & Sirkular):
guide-pattern-step-1 = 1. Pilih entitas sketsa 2D atau body 3D
guide-pattern-step-2 = 2. Atur jumlah & jarak di Top HUD
guide-pattern-step-3 = 3. Terapkan Pattern (Enter)
guide-pattern-tip = 💡 Pintasan: P | Gunakan preset sudut 360°/180°/90° untuk Sirkular Pattern
guide-pattern-badge = ⊞ Pattern

guide-zebra-header = Panduan Inspeksi Garis Zebra:
guide-zebra-step-1 = 1. Proyeksi Refleksi Specular
guide-zebra-step-2 = 2. Kontinuitas Tangensi G1 (Patahan Sudut)
guide-zebra-step-3 = 3. Kontinuitas Kurvatur G2 (Mulus Class-A)
guide-zebra-tip = 💡 Validasi keluwesan pantulan cahaya pada sambungan permukaan

guide-draft-analysis-header = Panduan Analisis Sudut Lepas (Draft Heatmap):
guide-draft-analysis-step-1 = 1. Tentukan Arah Buka Cetakan (+Z/-Z/+Y/+X)
guide-draft-analysis-step-2 = 2. Sudut Lepas Aman (Hijau ≥ Target)
guide-draft-analysis-step-3 = 3. Draft Kritis (Kuning) & Undercut Terjebak (Merah)
guide-draft-analysis-tip = 💡 Pastikan part dapat lepas dari cetakan injeksi plastik sebelum diproduksi

# Datum Plane Guides
guide-datum-offset-header = Panduan Offset Plane:
guide-datum-offset-step-1 = 1. Pilih Face / Bidang Dasar
guide-datum-offset-step-2 = 2. Atur Jarak Offset (mm)
guide-datum-offset-step-3 = 3. Klik 'Create Plane' (Enter)
guide-datum-offset-tip = 💡 Masukkan nilai negatif atau klik Flip untuk membalik arah

guide-datum-angled-header = Panduan Angled Plane:
guide-datum-angled-step-1 = 1. Pilih Edge / Garis Sumbu Putar
guide-datum-angled-step-2 = 2. Atur Sudut Kemiringan (θ°)
guide-datum-angled-step-3 = 3. Klik 'Create Plane' (Enter)
guide-datum-angled-tip = 💡 Memutar bidang terhadap garis edge 3D atau garis sketsa

guide-datum-3point-header = Panduan 3-Point Plane:
guide-datum-3point-step-1 = 1. Klik Titik P1 (Vertex/Face/Grid)
guide-datum-3point-step-2 = 2. Klik Titik P2
guide-datum-3point-step-3 = 3. Klik Titik P3
guide-datum-3point-step-done = ✔ 3 Titik Terpilih — Klik 'Create Plane'
guide-datum-3point-tip = 💡 Membentuk bidang datar yang melalui 3 titik acuan

# Units
unit-mm = mm (Milimeter)
unit-cm = cm (Sentimeter)
unit-m = m (Meter)
unit-inch = in (Inci)

# CMF & Industrial Material
inspector-cmf-title = CMF & Material Industri
inspector-cmf-presets = Preset Material Industri:
inspector-cmf-color = Pilihan Warna & Tint Kustom:
inspector-cmf-fine-tune = Penyesuaian Parameter Fisik (PBR)
material-matte-plastic = Plastik Matte (ABS/PC)
material-glossy-plastic = Plastik Glossy (Licin)
material-anodized-aluminum = Aluminium Anodisasi Satin
material-polished-chrome = Krom Poles / Stainless
material-translucent-glass = Kaca Tembus Pandang / Akrilik
material-roughness = Kekasaran (Roughness):
material-metallic = Metalisitas (Metallic):
material-clearcoat = Kilau Lapisan (Clearcoat):
material-opacity = Tingkat Transparansi (Opacity):

# Studio Lighting & SSAO Presentation (Fase 4.2)
topbar-studio-lighting = Studio Lighting
topbar-studio-tooltip = Pencahayaan Studio 3-Titik & Bayangan Kontak (SSAO)
hud-studio-title = Studio Lighting & SSAO Visualizer
hud-studio-preset = Preset Pencahayaan Studio:
hud-studio-clean = Clean Studio
hud-studio-warm = Warm Showcase
hud-studio-cool = Cool Tech
hud-studio-dramatic = Dramatic Dark
hud-studio-lights = Keseimbangan Lampu 3-Titik:
hud-studio-key = Lampu Utama (Key):
hud-studio-fill = Lampu Pengisi (Fill):
hud-studio-rim = Lampu Siluet (Rim):
hud-studio-ssao = Screen Space Ambient Occlusion (SSAO):
hud-studio-ssao-desc = Mempertegas kedalaman celah, lekukan, dan rongga part
hud-studio-floor-shadow = Bayangan Kontak Lantai (Floor Shadow)
hud-studio-shadow-intensity = Intensitas Bayangan:
hud-studio-turn-off = Matikan Studio Mode

# 2D Engineering Drawing Sheets (Fase 5)
menu-drawing-sheet = Gambar Kerja 2D (Drawing Sheet)…
menu-export-pdf = PDF Gambar Teknik…
topbar-drawing-sheet = Gambar Kerja 2D
topbar-drawing-sheet-tooltip = Buka Lembar Kerja Gambar Teknik 2D (A4/A3, Tampak Ortogonal, PDF)
drawing-sheet-title = GAMBAR KERJA TEKNIK 2D
drawing-sheet-paper = Ukuran Kertas
drawing-sheet-scale = Skala Gambar
drawing-sheet-hidden-lines = Garis Tersembunyi (Hidden)
drawing-sheet-dimensions = Dimensi Otomatis
drawing-sheet-centerlines = Garis Sumbu (Centerlines)
drawing-sheet-title-block = Kepala Gambar (Title Block)
drawing-sheet-export-pdf = Ekspor PDF (Vektor)
drawing-sheet-export-dxf = Ekspor DXF (2D CAD)
drawing-sheet-fit = Pas ke Layar (Fit)
drawing-view-front = Tampak Depan
drawing-view-top = Tampak Atas
drawing-view-right = Tampak Samping Kanan
drawing-view-isometric = Tampak Isometrik 3D
drawing-view-section = Tampak Potongan A-A
drawing-sheet-section-view = Tampak Potongan A-A
drawing-sheet-hatch = Pola Arsiran (45°)
drawing-sheet-bom = Tabel BOM (Daftar Bagian)
drawing-sheet-bom-tooltip = Tampilkan tabel daftar komponen / Bill of Materials (ISO 7573)
drawing-sheet-balloons = Balon Nomor Penunjuk
drawing-sheet-balloons-tooltip = Tampilkan lingkaran nomor penunjuk yang terhubung ke part pada tampak isometrik
drawing-sheet-add-balloon = Tool Balon
drawing-sheet-add-balloon-tooltip = Klik pada tampak isometrik untuk menempatkan balon penunjuk (B)
bom-table-title = DAFTAR BAGIAN (BOM)
bom-header-item = NO
bom-header-part = NAMA BAGIAN
bom-header-qty = JML
bom-header-material = MATERIAL
bom-header-description = KETERANGAN
bom-add-row = Tambah Baris
bom-delete-row = Hapus Baris
bom-no-items = Tidak ada item BOM
balloon-prompt-place = Klik pada tampak isometrik untuk menaruh balon
file-exported-pdf = Berhasil mengekspor PDF: { $name }
file-export-pdf-failed = Gagal mengekspor PDF: { $error }
file-exported-drawing-dxf = Berhasil mengekspor DXF Gambar Kerja: { $name }
file-export-drawing-dxf-failed = Gagal mengekspor DXF: { $error }
file-exported-drawing-svg = Berhasil mengekspor SVG Gambar Kerja: { $name }
file-export-drawing-svg-failed = Gagal mengekspor SVG: { $error }
file-pdf-filter = Dokumen PDF (*.pdf)
file-drawing-dxf-filter = Gambar AutoCAD DXF (*.dxf)
file-drawing-svg-filter = Format Vektor SVG (*.svg)
file-exported-snapshot = Berhasil mengekspor Vector Snapshot: { $name } ({ $count } garis)
file-export-snapshot-failed = Gagal mengekspor Vector Snapshot: { $error }
file-snapshot-empty = Tidak ada body tampak atau sketsa untuk ditangkap
file-snapshot-offscreen = Tidak ada geometri di dalam bingkai kamera — atur ulang sudut pandang lalu coba lagi
# Hole Wizard & ISO Fasteners (Fase 9.2)
tool-hole-wizard = Hole Wizard
tool-hole-wizard-desc = Buat lubang ulir, counterbore, countersink, dan silinder lurus standar ISO
hole-type = Tipe Lubang:
hole-type-simple = Lubang Silinder (Simple)
hole-type-counterbore = Kepala Baut L (Counterbore)
hole-type-countersink = Kepala Rata (Countersink)
hole-type-tapped = Lubang Ulir (Tapped Thread)
hole-iso-standard = Standar Baut Metrik ISO:
hole-dia = Diameter Lubang:
hole-depth = Kedalaman Lubang:
hole-through-all = Tembus (Through All)
hole-blind = Berkedalaman (Blind)
hole-cbore-dia = Diameter Counterbore:
hole-cbore-depth = Kedalaman Counterbore:
hole-csink-dia = Diameter Countersink:
hole-csink-angle = Sudut Tirus Countersink:
hole-thread-pitch = Kisar Ulir (Pitch):
hole-thread-depth = Kedalaman Ulir:
hole-drill-tip = Ujung Bor Standar 118°
hole-flat-bottom = Dasar Datar (Flat Bottom)
hole-apply = Terapkan Lubang
hole-update-apply = Perbarui Lubang
hole-applied = Berhasil membuat { $callout }
hole-mode = Mode Operasi:
hole-mode-new = Buat Baru
hole-mode-edit = Edit Lubang
hole-pos-offset = Posisi & Geser (Offset):
hole-offset-u = Offset U (X):
hole-offset-v = Offset V (Y):
hole-center-btn = Pusat (Center)
hole-drag-hint = Klik atau drag titik target di viewport untuk menggeser.
hole-select-target = Pilih Lubang yang Diedit:
hole-no-holes-found = Belum ada lubang pada bodi ini untuk diedit. Beralih ke Buat Baru.
selection-face-selected = Face Terpilih
selection-hole-selected = Lubang Terpilih

# 2D Text on Sketch & Emboss/Deboss (Fase 9.5)
tool-text = Teks 2D
tool-text-desc = Buat kurva outline teks 2D pada sketsa menggunakan font TTF/OTF (T)
tool-emboss = Emboss / Deboss
tool-emboss-desc = Ekstrusi timbul (Emboss) atau ukiran tenggelam (Deboss) pada bodi 3D
text-input-label = Isi Teks:
text-input-hint = Ketik teks di sini…
text-font-height = Tinggi Huruf:
text-letter-spacing = Spasi Huruf:
text-align-label = Perataan:
text-font-default = Font Bawaan (Arial / Clean Sans)
text-font-browse = Pilih Berkas TTF…
text-output-mode = Mode Hasil:
text-mode-sketch = Hanya Sketsa
text-mode-emboss = Emboss (Timbul)
text-mode-deboss = Deboss (Ukir)
text-emboss-height = Tinggi Timbul:
text-deboss-depth = Kedalaman Ukir:
text-apply-sketch = Tempatkan di Sketsa
text-apply-emboss = Terapkan Emboss
text-apply-deboss = Terapkan Deboss
status-prompt-text-0 = Teks: klik pada bidang sketsa untuk meletakkan titik acuan teks (T)
guide-text-header = Panduan Teks 2D:
guide-text-step-1 = 1. Ketik teks dan atur tinggi font di popup pojok kanan bawah
guide-text-step-2 = 2. Klik pada bidang sketsa untuk menempatkan teks
guide-text-tip = 💡 Glyph hasil vektorisasi membentuk loop tertutup yang siap di-extrude atau di-emboss
guide-emboss-header = Panduan Emboss / Deboss:
guide-emboss-step-1 = 1. Pilih teks/profil sketsa pada permukaan face bodi 3D
guide-emboss-step-2 = 2. Atur kedalaman & klik Terapkan Emboss (Timbul) atau Deboss (Ukir)
guide-emboss-tip = 💡 Otomatis menggabungkan (Emboss) atau memotong (Deboss) geometri relatif terhadap normal face aktif

tool-datum-plane = Bidang Referensi
tool-datum-plane-desc = Buat bidang kerja referensi 3D bebas (Offset, Angled, 3-Point)
hud-datum-plane-title = Bidang Referensi (Datum)
datum-mode-offset = Offset Plane
datum-mode-angled = Angled Plane
datum-mode-3point = 3-Point Plane
datum-distance = Jarak:
datum-angle = Sudut:
datum-flip = Balik
datum-create = Buat Bidang
datum-cancel = Batal
datum-select-base = Acuan:
datum-select-edge = Garis:
datum-select-points = Titik:
datum-planes-header = Bidang Referensi
datum-plane-new = + Buat Bidang Baru
status-prompt-datum-plane = Bidang Referensi: pilih mode (Offset, Angled, 3-Point), atur parameter, lalu klik Buat Bidang

# Planes Drawer
planes-drawer-title = Bidang Referensi
planes-drawer-new = + Buat Bidang
planes-drawer-active = Aktif
planes-drawer-search = Cari bidang...
planes-drawer-empty = Tidak ada bidang referensi ditemukan
planes-drawer-delete-tooltip = Hapus bidang kustom ini

# Helix / Coil / Spring Tool (Fase 10.2)
status-prompt-helix = Helix / Pegas: atur parameter (pitch, turns, radius) lalu klik 'Buat Solid' atau tekan Enter
hud-helix-title = Generator Helix & Pegas
hud-helix-exec-btn = Buat Solid
hud-helix-path-btn = Buat Jalur Kurva
hud-helix-cancel = Batal
helix-preset-label = Preset:
helix-preset-compression-spring = Pegas Tekan
helix-preset-extension-spring = Pegas Tarik
helix-preset-auger = Bilah Auger
helix-preset-thread = Ulir Botol / V-Thread
helix-preset-custom = Spiral Kustom
helix-param-pitch = Pitch:
helix-param-turns = Putaran:
helix-param-radius = Radius:
helix-param-end-radius = Radius Akhir:
helix-param-taper = Tirus (Kerucut)
helix-param-profile-type = Profil Penampang:
helix-profile-circle = Kawat Bulat (Lingkaran)
helix-profile-rect = Bilah Datar (Persegi)
helix-profile-triangle = Ulir Segitiga (V-Thread)
helix-profile-curve-only = Hanya Kurva Jalur Spiral
helix-param-wire-radius = Radius Kawat:
helix-param-width = Lebar:
helix-param-height = Tinggi:
helix-param-handedness = Arah Putar:
helix-handedness-right = Ulir Kanan (CW)
helix-handedness-left = Ulir Kiri (CCW)
helix-total-height = Total Tinggi: { $height } mm
guide-helix-header = Panduan Helix / Pegas:
guide-helix-step-1 = 1. Atur pitch spiral, jumlah putaran, dan radius
guide-helix-step-2 = 2. Pilih bentuk profil (pegas bulat, bilah auger, atau ulir)
guide-helix-step-done = 3. Klik Buat Solid untuk menyapu kurva spiral 3D
guide-helix-tip = 💡 Menghasilkan pegas solid 3D parametrik, bilah ulir sekrup konveyor, atau kurva spine sweep

# Feature Tree & Riwayat Parametrik (Fase 12.1)
feature-tree-title = Pohon Fitur & Garis Waktu
feature-tree-empty = Belum ada fitur yang terekam
feature-tree-empty-sub = Langkah-langkah parametrik akan muncul di sini saat Anda memodelkan
feature-tree-regenerate = Regenerasi Model 3D
feature-tree-needs-regen = Model telah diubah - klik untuk meregenerasi
feature-tree-regen-success = Model parametrik berhasil diperbarui & diregenerasi
feature-tree-regen-error = Kesalahan regenerasi: { $error }
feature-tree-edit-params = Ubah Parameter
feature-tree-suppress = Nonaktifkan Fitur (Suppress)
feature-tree-unsuppress = Aktifkan Fitur (Unsuppress)
feature-tree-delete = Hapus Fitur
feature-tree-search = Cari fitur…
feature-tree-close = Tutup Pohon Fitur
feature-tree-status-valid = Terkini / Valid
feature-tree-status-modified = Perlu Regenerasi
feature-tree-status-suppressed = Dinonaktifkan (Suppressed)
feature-tree-status-error = Terjadi Kesalahan
feature-edit-title = Edit Parameter Fitur
feature-edit-save = Terapkan & Regenerasi
feature-edit-cancel = Batal
feature-edit-distance = Jarak / Kedalaman:
feature-edit-angle = Sudut:
feature-edit-radius = Radius:
feature-edit-dimension = Dimensi:
feature-edit-thickness = Ketebalan:
topbar-feature-tree-tooltip = Pohon Fitur & Riwayat Parametrik (DAG)

# Assembly Workspace & Mate Constraints (Fase 12.2)
assembly-tree-title = Pohon Perakitan & Hierarki
assembly-tree-empty = Belum ada komponen perakitan
assembly-tree-empty-sub = Buat instance atau tambahkan part untuk mulai merakit
assembly-instance = Instance Part
assembly-sub-assembly = Sub-Perakitan
assembly-new-sub = Sub-Perakitan Baru
assembly-add-component = Tambah Komponen
assembly-ground = Kunci Posisi (Grounded)
assembly-unground = Lepas Kunci (Bebas Bergerak)
assembly-grounded-badge = Terkunci (Grounded)
assembly-dof-badge = { $dof } DOF
assembly-fully-constrained = Terkendala Penuh (0 DOF)
assembly-under-constrained = Belum Terkunci Penuh ({ $dof } DOF)
assembly-mates = Kendala Perakitan (Mates)
assembly-no-mates = Belum ada mate yang diterapkan
assembly-solve = Selesaikan & Perbarui Perakitan
assembly-mate-concentric = Concentric Mate
assembly-mate-coincident = Coincident Mate
assembly-mate-distance = Distance Mate
assembly-mate-angle = Angle Mate
assembly-mate-concentric-desc = Mengunci sumbu silinder poros dengan lubang
assembly-mate-coincident-desc = Menempelkan dua permukaan planar datar
assembly-mate-distance-desc = Mengatur jarak presisi antara dua permukaan atau titik
assembly-mate-angle-desc = Mengatur sudut rotasi engsel antara dua bidang atau sumbu
assembly-flip-alignment = Balik Arah Normal (Sejajar / Berlawanan)
assembly-lock-rotation = Kunci Rotasi Sumbu
assembly-mate-distance-label = Jarak Offset:
assembly-mate-angle-label = Sudut:
assembly-apply-mate = Terapkan Mate
assembly-cancel-mate = Batal
assembly-mate-satisfied = Terpenuhi
assembly-mate-conflicted = Konflik
assembly-mate-suppressed = Dinonaktifkan
topbar-assembly-tooltip = Pohon Perakitan & Mate Constraints
guide-mate-header = Panduan Mate Perakitan:
guide-mate-step-1 = 1. Pilih bidang acuan atau silinder pada part A
guide-mate-step-2 = 2. Shift-pilih bidang target atau silinder pada part B
guide-mate-tip = 💡 Gabungkan Concentric dan Coincident mate untuk perakitan poros dalam lubang sempurna

# Clash & Interference Detection (Phase 12.3)
assembly-clash-tab = Tabrakan
assembly-clash-title = Uji Tabrakan & Interferensi (Clash)
assembly-clash-desc = Uji tabrakan fisik otomatis antar bodi solid menggunakan operasi Boolean interseksi
assembly-clash-run = Periksa Tabrakan
assembly-clash-running = Menguji...
assembly-clash-tolerance = Toleransi Volume:
assembly-clash-tolerance-unit = mm³
assembly-clash-clean = Tidak Ada Tabrakan Terdeteksi
assembly-clash-clean-desc = Seluruh komponen terpasang presisi tanpa tumpang tindih fisik
assembly-clash-detected = { $count } Tabrakan Terdeteksi
assembly-clash-total-volume = Total Volume Tabrakan: { $vol } mm³
assembly-clash-pair = { $part_a } ↔ { $part_b }
assembly-clash-vol-label = Volume: { $vol } mm³
assembly-clash-centroid-label = Pusat: ({ $x }, { $y }, { $z })
assembly-clash-focus = Fokus
assembly-clash-isolate = Isolasi
assembly-clash-create-body = Ubah Jadi Bodi
assembly-clash-clear = Bersihkan Hasil
assembly-clash-created-body-notify = Volume tabrakan berhasil diubah menjadi bodi solid baru '{ $name }'
context-check-clash = Uji Tabrakan

# Checks desain (P7.5)
checks-title = Checks
checks-close = Tutup panel checks
checks-empty = Part ini belum punya check desain.
checks-stale = Sedang dihitung ulang…
checks-summary-tooltip = Hasil check desain (klik untuk detail)

# Cabang histori (P8.5)
history-branch-all = Semua cabang
history-branch-from-here = Buat cabang dari sini
history-branch-created = Cabang { $name } dibuat

# Kartu error operasi (P9.3)
error-card-close = Tutup
error-card-title-fallback = Operasi gagal
fix-use-radius = Pakai radius { $value } mm
fix-use-distance = Pakai jarak { $value } mm
fix-use-thickness = Pakai tebal { $value } mm
fix-use-depth = Pakai kedalaman { $value } mm
fix-use-full-cavity = Pakai rongga penuh (kedalaman 0)
error-invalid_param = Nilai tidak valid
error-unknown_ref = Rujukan tidak dikenal
error-duplicate_id = Id sudah dipakai
error-body_consumed = Body sudah dilebur
error-profile_not_closed = Profil tidak tertutup
error-profile_ambiguous = Titik profil di luar region
error-selector_syntax = Selector tidak valid
error-selector_empty = Selector tidak cocok
error-constraint_unsolved = Constraint tidak terselesaikan
error-over_constrained = Constraint berlebih
error-kernel_failed = Kernel geometri gagal
error-empty_result = Hasil kosong
error-oplog_stale = Oplog basi
error-io = Gagal membaca/menulis berkas
error-unsupported = Belum didukung
error-proposal_stale = Proposal basi
error-fillet_radius_too_large = Radius fillet terlalu besar
error-chamfer_too_large = Chamfer terlalu besar
error-shell_too_thick = Shell terlalu tebal
error-shell_depth_too_deep = Rongga shell terlalu dalam
error-hole_outside_face = Lubang di luar face
error-hole_deeper_than_body = Lubang lebih dalam dari body
error-boolean_no_overlap = Body tidak beririsan
error-profile_open_gap = Profil hampir tertutup

## Asisten AI lokal (P11)
assist-title = Tanya AI…
assist-hint = Mis. "tebal jadi 10 mm" atau "tambah lubang M4 di tengah face atas"
assist-ask = Tanya
assist-cancel = Batal
assist-working = Model sedang bekerja…
assist-apply = Terapkan
assist-reject = Tolak
assist-no-backend = Build ini tanpa backend AI di perangkat.
assist-needs-design = Bagian ini bukan part parametrik (tidak ada oplog), jadi AI belum bisa mengusulkan perubahan.
assist-capability-note = Model di perangkat cocok untuk mengubah ukuran dan menambah fitur sederhana. Untuk membuat part baru yang rumit, pakai agent eksternal.
assist-applied = Usulan AI diterapkan.
assist-rejected = Usulan AI ditolak.
ai-chip-on-device = AI: di perangkat
ai-chip-external = AI: eksternal aktif
ai-privacy-offline = Hanya di perangkat
ai-privacy-external = Izinkan agent eksternal

## Alat Freehand (P12)
tool-freehand = Freehand
tool-freehand-desc = Gambar bebas dengan Pencil/mouse; bentuknya dirapikan dan diberi constraint otomatis
freehand-committed = Coretan diubah menjadi entitas sketsa ber-constraint
freehand-accept = Terima bentuk
freehand-reject = Tolak bentuk

## Mode Tinta & Objek Tertutup
ink-tool-brush = Kuas
ink-tool-eraser = Penghapus
ink-tool-lasso = Lasso
ink-smart-shape = Bentuk Pintar
ink-smart-shape-desc = Coretan langsung dirapikan menjadi garis/lingkaran/kurva sketsa CAD yang tertutup rapat dan siap di-extrude
ink-to-profile-empty = Tidak ada coretan tinta untuk diubah
hud-close-objects = Objek Tertutup
hud-close-objects-desc = Tekan setelah selesai menggambar: garis yang saling bertemu atau berpotongan diubah menjadi objek tertutup siap di-extrude. Bentuk yang saling memotong menjadi objek terpisah; bentuk yang sudah rapi tidak diubah. Berlaku untuk seleksi, atau seluruh sketsa bila tidak ada seleksi.
close-objects-done = { $count } objek tertutup dibuat — ketuk salah satu lalu Ekstrusi
close-objects-kept = Sketsa sudah berupa objek tertutup — ketuk objek lalu Ekstrusi
close-objects-none = Tidak ada wilayah tertutup yang bisa dibentuk — pastikan garis saling bertemu atau berpotongan

## Jembatan agent live (P5) dan kartu proposal (P8.4)
bridge-title = Agent Bridge
bridge-toggle = Agent Bridge (jembatan agent live)
bridge-on = Agent Bridge aktif — agent bisa mengubah dokumen ini
bridge-off = Agent Bridge nonaktif
bridge-failed = Agent Bridge gagal dinyalakan
bridge-blocked-offline = Agent Bridge dimatikan oleh kebijakan privasi "Hanya di perangkat"
bridge-chip = Agent: { $clients }
bridge-activity = Agent: { $count } operasi
proposal-title = Usulan agent
proposal-accept = Terima
proposal-reject = Tolak
proposal-volume = +{ $added } mm³ / −{ $removed } mm³
proposal-accepted = Usulan agent diterapkan
proposal-rejected = Usulan agent ditolak
proposal-timeout = Usulan agent kedaluwarsa

## Chat AI (P13)
chat-title = Chat AI
chat-close = Tutup panel chat
chat-open = Chat AI (agent yang mengendalikan DUCAD)
chat-hint = Mis. "buat bracket L 60×40×5 dengan 2 lubang M5" — Enter untuk kirim, Shift+Enter baris baru
chat-send = Kirim
chat-stop = Hentikan
chat-new = Chat baru
chat-new-confirm = Klik lagi untuk menghapus
chat-working = Agent sedang bekerja…
chat-empty = Minta agent membuat atau mengubah part. Setiap batch perubahan = satu langkah undo.
chat-settings = Pengaturan provider AI
chat-history = Riwayat chat
chat-history-empty = Belum ada chat tersimpan.
chat-provider = Provider
chat-base-url = Base URL
chat-model = Model
chat-api-key = Kunci API
chat-key-saved = tersimpan di Keychain — kosongkan untuk mempertahankan
chat-key-empty = belum ada (atau pakai variabel lingkungan)
chat-confirm-writes = Selalu minta persetujuan sebelum mengubah geometri
chat-allow-external = Izinkan AI eksternal (desain dikirim ke provider jaringan)
chat-privacy-note = Tanpa izin ini hanya provider lokal (localhost) yang bisa dipakai, dan Agent Bridge tetap mati.
chat-save = Simpan
chat-saved = Pengaturan AI disimpan.
chat-blocked-privacy = Privasi "Hanya di perangkat" aktif: provider { $host } ada di jaringan. Aktifkan "Izinkan AI eksternal" di ⚙ atau pakai Ollama lokal.
chat-cancelled = Dibatalkan.
chat-tool-input = Input
chat-tool-output = Hasil
chat-tool-running = berjalan…
chat-usage = token: { $input } masuk · { $output } keluar · { $cached } cache
chat-activity = Chat AI
chat-backend = Backend
chat-backend-api = API
chat-backend-cli = CLI agent
chat-cli-intro = CLI coding agent lokal (agy, claude, gemini, atau perintah kustom) mengendalikan DUCAD lewat server MCP ducad-mcp. Agent yang diaktifkan bisa dipilih di kepala panel.
chat-cli-enable = Aktifkan agent ini untuk chat
chat-cli-command = Perintah / path
chat-cli-command-hint = kosong = cari otomatis
chat-cli-detect = Deteksi
chat-cli-model-default = (bawaan CLI)
chat-cli-default = Bawaan
chat-cli-quick-pick = Pilihan cepat:
chat-cli-effort = Reasoning effort
chat-cli-effort-default = (bawaan)
chat-cli-extra-args = Argumen tambahan
chat-cli-register = Daftarkan MCP DUCAD
chat-cli-register-hint = Menjalankan "<cli> mcp add ducad -- ducad-mcp --attach" (wajib untuk agy dan gemini; claude memakai konfigurasi per giliran).
chat-cli-test = Uji koneksi
chat-cli-working = Menjalankan…
chat-cli-detected = Ditemukan: { $path }
chat-cli-not-found = CLI "{ $name }" tidak ditemukan di PATH maupun lokasi instalasi umum.
chat-cli-no-mcp = ducad-mcp tidak ditemukan. Jalankan "make install-agent-tools" di ducad-editor.
chat-cli-desktop-only = CLI agent hanya tersedia di desktop.
chat-cli-none-enabled = Belum ada CLI agent yang diaktifkan (⚙ » CLI agent).
chat-cli-needs-external = CLI agent mengirim desain ke provider modelnya. Aktifkan "Izinkan AI eksternal" di ⚙ lebih dulu.
chat-cli-bridge-failed = Jembatan agent gagal dinyalakan: { $error }
chat-cli-not-registered = Server MCP "ducad" belum terdaftar di { $name }. Tekan "Daftarkan MCP DUCAD" di ⚙.

# Properti massa (P16)
mass-title = Properti Massa
mass-open = Properti Massa
mass-close = Tutup panel properti massa
mass-copy = Salin tabel
mass-marker = Tampilkan penanda pusat massa
mass-empty = Belum ada body.
mass-units = Satuan
mass-material = Material
mass-material-none = Belum dipilih
mass-material-custom = Kustom…
mass-density = Densitas
mass-young = Modulus Young (GPa)
mass-poisson = Rasio Poisson
mass-yield = Kuat luluh (MPa)
mass-ultimate = Kuat tarik (MPa)
mass-apply = Terapkan
mass-cancel = Batal
mass-mass = Massa
mass-unknown-density = densitas tidak diketahui
mass-volume = Volume
mass-area = Luas permukaan
mass-com = Pusat massa
mass-inertia-com = Inersia di pusat massa
mass-inertia-origin = Inersia di origin
mass-principal-moments = Momen utama
mass-principal-axis = Sumbu utama
mass-gyration = Radius girasi
mass-assembly-mass = Massa gabungan
mass-assembly-com = Pusat massa gabungan
mass-material-applied = Material mekanik diterapkan

# Simulasi statik (P17)
sim-title = Simulasi
sim-open = Simulasi (studi statik)
sim-accuracy = Estimasi (hex)
sim-close = Tutup panel simulasi
sim-new = Studi baru
sim-empty = Belum ada studi. Tekan + untuk membuat.
sim-status-not-run = belum dijalankan
sim-status-running = menghitung…
sim-status-stale = basi
sim-status-done = selesai
sim-status-failed = gagal
sim-run = Jalankan
sim-cancel = Batalkan
sim-delete = Hapus studi
sim-study-summary = { $body } · { $fixtures } tumpuan · { $loads } beban
sim-target-body = Body: { $body }
sim-no-material = Body ini belum punya material mekanik (panel Properti Massa).
sim-no-body = Belum ada body untuk disimulasikan.
sim-study-id = Id
sim-picked-face = Face terpilih: { $selector }
sim-pick-hint = Klik sebuah face di viewport, lalu tekan + pada tumpuan atau beban.
sim-fixtures = Tumpuan
sim-add-fixture = Tambah tumpuan pada face terpilih
sim-fixture-fixed = Terkunci
sim-fixture-roller = Rol
sim-fixture-symmetry = Simetri
sim-loads = Beban
sim-add-load = Tambah beban pada face terpilih
sim-load-force = Gaya
sim-load-pressure = Tekanan
sim-load-gravity = Gravitasi
sim-cell = Ukuran sel
sim-cell-auto = 0 = otomatis
sim-create = Buat studi
sim-discard = Batal
sim-max-stress = von Mises maks
sim-max-displacement = Deformasi maks
sim-safety-factor = Faktor keamanan
sim-reaction = Reaksi
sim-mesh = Elemen / node / sel
sim-iterations = Iterasi solver
sim-show-overlay = Warnai viewport
sim-overlay-stress = Tegangan
sim-overlay-displacement = Deformasi
sim-overlay-safety = Faktor keamanan
sim-deform-auto = Skala otomatis
sim-deform-scale = Skala deformasi
sim-created = Studi dibuat
sim-deleted = Studi dihapus
sim-cancelled = Studi dibatalkan

# Panel Fitur Industri (P18-P20)
ind-title = Fitur Industri
ind-open = Fitur Industri (studi lanjutan, konfigurasi, sheet metal, toleransi, part standar, rakitan)
ind-close = Tutup
ind-tab-study = Studi
ind-tab-config = Konfigurasi
ind-tab-sheet = Sheet metal
ind-tab-tolerance = Toleransi
ind-tab-parts = Part standar
ind-tab-assembly = Rakitan
ind-delete = Hapus
ind-cancel = Batalkan
ind-run = Jalankan
ind-edit = Ubah
ind-create = Buat
ind-discard = Buang
ind-pick-face = pilih face di viewport
ind-target-body = Body sasaran: { $body }
ind-need-material = Body belum punya material mekanik (atur di panel Properti Massa).
ind-study-hint = Frekuensi natural, buckling, dan termal. Hasil berupa estimasi teknik.
ind-study-empty = Belum ada studi lanjutan.
ind-study-new = Studi baru
ind-study-stale = Hasil basi: model atau setup berubah, jalankan ulang.
ind-study-frequency = Frekuensi natural
ind-study-buckling = Buckling
ind-study-thermal = Termal tunak
ind-study-thermal-stress = Tegangan termal
ind-study-fixtures = Tumpuan (jepit)
ind-study-load = Beban gaya
ind-study-thermal-bc = Syarat batas termal
ind-study-mesh = Mesh
ind-study-tet = Tetra (ikuti face lengkung)
ind-study-cell = Ukuran sel
ind-study-cell-hint = 0 = otomatis. Sel lebih kecil lebih teliti tetapi lebih lambat; frekuensi pada mesh tetra halus bisa bermenit-menit.
ind-study-modes = Jumlah mode
ind-add-fixture = Tambah tumpuan: { $face }
ind-add-load = Tambah beban: { $face }
ind-add-bc = Tambah syarat batas: { $face }
ind-bc-temperature = Suhu
ind-bc-flux = Fluks panas
ind-bc-convection = Konveksi
ind-study-created = Studi dibuat
ind-study-cancelled = Studi dibatalkan
ind-result-frequency = Mode { $n }: { $hz } Hz
ind-result-buckling = Faktor tekuk { $n }: { $factor }
ind-result-no-buckling = Beban tidak menimbulkan tekuk
ind-result-thermal = Suhu maks { $max } C, min { $min } C
ind-result-stress = von Mises maks { $stress } MPa, deformasi { $disp } mm, faktor keamanan { $sf }
ind-result-mesh = Mesh { $kind }: { $elements } elemen
ind-cfg-hint = Varian desain: timpa parameter atau lewati op. "Default" adalah desain dasar.
ind-cfg-editor = Tambah / ubah konfigurasi
ind-cfg-name = Nama
ind-cfg-no-params = Desain ini tidak punya parameter bernama.
ind-cfg-suppress = Op yang dilewati
ind-cfg-save = Simpan konfigurasi
ind-cfg-export = Ekspor CSV
ind-cfg-import = Impor CSV
ind-cfg-saved = Konfigurasi disimpan
ind-cfg-activated = Konfigurasi aktif: { $name }
ind-cfg-deleted = Konfigurasi dihapus
ind-cfg-exported = Design table diekspor
ind-cfg-imported = Design table diimpor
ind-sheet-base = Pelat dasar
ind-sheet-base-hint = Persegi panjang di bidang XY dengan sudut di titik asal.
ind-sheet-width = Lebar
ind-sheet-height = Tinggi
ind-sheet-thickness = Tebal
ind-sheet-radius = Radius tekuk
ind-sheet-create-base = Buat pelat dasar
ind-sheet-empty = Belum ada body sheet metal.
ind-sheet-bodies = Body sheet metal
ind-sheet-row = tebal { $thickness } mm, { $flanges } flange
ind-sheet-unfolded = terbentang
ind-sheet-fold = Lipat kembali
ind-sheet-unfold = Bentangkan
ind-sheet-flat = Buat pola datar
ind-sheet-dxf = Ekspor DXF pola datar
ind-sheet-feature = Tambah fitur tepi
ind-sheet-edge-flange = Flange tepi
ind-sheet-hem = Hem
ind-sheet-jog = Jog
ind-sheet-edges = Tepi
ind-sheet-along-x = Sejajar X
ind-sheet-along-y = Sejajar Y
ind-sheet-custom = Selector
ind-sheet-length = Panjang
ind-sheet-angle = Sudut
ind-sheet-gap = Celah
ind-sheet-offset = Offset
ind-sheet-add-feature = Tambah fitur
ind-sheet-limits = Batas: flange di atas flange belum didukung; relief tekuk tidak dipotong.
ind-sheet-exported = Pola datar diekspor
ind-sheet-need-xy = Pilihan sisi otomatis hanya untuk pelat di bidang XY; pakai selector.
ind-tol-stack = Stack-up toleransi
ind-tol-hint = Tiap baris: nominal, lalu suaian ISO (mis. H7) atau deviasi plus/minus.
ind-tol-reverse = arah balik
ind-tol-add-link = Tambah mata rantai
ind-tol-empty = Rantai toleransi kosong
ind-tol-result = Nominal { $nominal } mm, kasus terburuk { $worst } mm, RSS { $rss } mm
ind-tol-max = Batas total
ind-tol-add-check = Jadikan check desain
ind-tol-check-added = Check stack-up ditambahkan
ind-gdt-title = Anotasi GD&T di lembar gambar
ind-gdt-hint = Posisi dalam mm pada kertas. Anotasi ikut diekspor ke PDF dan SVG.
ind-gdt-frame = Bingkai kontrol
ind-gdt-datum = Datum
ind-gdt-dimension = Dimensi
ind-gdt-finish = Kekasaran
ind-gdt-position = Posisi
ind-gdt-diameter = diameter
ind-gdt-datums = Datum acuan
ind-gdt-label = Huruf
ind-gdt-add = Tambah anotasi
ind-gdt-open-sheet = Buka lembar gambar
ind-gdt-added = Anotasi ditambahkan
ind-parts-title = Part standar
ind-parts-length = Panjang
ind-parts-at = Posisi
ind-parts-use-face = Pakai titik face terpilih
ind-parts-insert = Sisipkan
ind-parts-inserted = Part standar disisipkan
ind-thread-title = Ulir
ind-thread-hint = Face silinder: { $face }
ind-thread-pitch = Kisar (0 = ISO kasar)
ind-thread-length = Panjang (0 = penuh)
ind-thread-cosmetic = Kosmetik (hanya catatan)
ind-thread-add = Tambah ulir
ind-thread-slow = Ulir fisik memotong alur heliks: sekitar satu detik per sepuluh lilitan.
ind-thread-added = Ulir ditambahkan
ind-asm-need-two = Butuh minimal dua body untuk kopling.
ind-asm-couplings = Kopling gerak
ind-asm-coupling-hint = Sumbu melewati posisi tiap instance. Kopling bekerja saat penggerak diputar (studi gerak / seret).
ind-asm-gear = Roda gigi
ind-asm-screw = Sekrup
ind-asm-rack = Rack dan pinion
ind-asm-ratio = Rasio
ind-asm-pitch = Kisar (mm/putaran)
ind-asm-pitch-radius = Radius jarak bagi (mm)
ind-asm-driver = Penggerak
ind-asm-driven = Digerakkan
ind-asm-add-coupling = Tambah kopling
ind-asm-coupling-added = Kopling ditambahkan
ind-asm-coupling-failed = Kopling tidak sah (periksa instance dan nilai)
ind-asm-explode = Langkah urai
ind-asm-explode-hint = Langkah dimainkan berurutan saat faktor urai naik dari 0 ke 1.
ind-asm-add-step = Tambah langkah
ind-asm-factor = Faktor urai
ind-failed = Gagal: { $why }

# Tutorial selamat datang (onboarding)
onboard-restart = Tutorial: mulai dari awal
onboard-chat-demo-ask = Buat braket L 40 mm
onboard-chat-demo-reply = Braket dibuat.
onboard-help-title = Tutorial
onboard-help-desc = Buka tur pengenalan DUCAD dari awal
extrude-needs-selection = Extrude: pilih dulu profil sketsa tertutup atau satu sisi solid dengan tool Pilih.
onboard-skip-all = Lewati tutorial
onboard-skip-step = Lewati langkah ini
onboard-back = Kembali
onboard-next = Lanjut
onboard-next-locked = Kerjakan dulu langkahnya pada model, lalu tombol ini aktif.
onboard-finish = Mulai mendesain
onboard-done = Berhasil. Lanjut ke langkah berikutnya.
onboard-progress = { $chapter } - Langkah { $current } dari { $total }
onboard-overview-progress = { $chapter } - Kenali DUCAD { $current }/{ $total }
onboard-next-chapter = Lanjut ke bab berikutnya
onboard-stop-here = Cukup dulu
onboard-bootstrap-done = Geometri awal bab ini dibuat otomatis dari hasil bab sebelumnya.
onboard-bootstrap-failed = Geometri awal bab ini gagal dibuat; kerjakan bab sebelumnya dulu.
onboard-welcome-title = Selamat datang di DUCAD
onboard-welcome-body = Tutorial ini mengerjakan satu part dari awal sampai siap produksi: rem cakram Ø240 mm. Setiap langkah menambah sesuatu pada cakram, dan baru bisa dilanjutkan setelah Anda mengerjakannya sendiri.
onboard-chapter-beginner = Bab 1: Pemula
onboard-chapter-beginner-desc = Kenali layar DUCAD, lalu buat piringan: lingkaran, extrude, lubang poros, chamfer.
onboard-chapter-beginner-start = Mulai Bab 1
onboard-chapter-intermediate = Bab 2: Menengah
onboard-chapter-intermediate-desc = Lima lubang baut dan dua belas ventilasi: sketsa di sisi, pattern, potong, fillet, ukur.
onboard-chapter-intermediate-start = Mulai Bab 2
onboard-chapter-advanced = Bab 3: Mahir
onboard-chapter-advanced-desc = Hub berongga dengan ulir, lalu irisan, material, simulasi, gambar kerja, STEP, agent AI.
onboard-chapter-advanced-start = Mulai Bab 3
onboard-ov-topbar-title = Bilah atas
onboard-ov-topbar-body = Kiri: menu utama (ikon tiga garis: baru, buka, simpan, impor, ekspor, semua perintah, pengaturan) dan nama dokumen. Tengah: tombol mode dan pencarian perintah. Kanan: bantuan, mode sentuh, Chat AI, dan akun.
onboard-ov-toolbar-title = Bilah alat kiri
onboard-ov-toolbar-body = Tool untuk membuat sesuatu yang belum ada. Isinya mengikuti mode: di Sketsa 2D ada garis, persegi, lingkaran, slot; di 3D ada bidang referensi, irisan, sweep, helix. Paling atas selalu tool Pilih (Esc).
onboard-ov-mode-title = Mode Sketsa 2D dan 3D
onboard-ov-mode-body = DUCAD bekerja dalam dua mode. Sketsa 2D untuk menggambar profil datar di sebuah bidang; 3D untuk membentuk solid. Tombol ini (ikon pensil) berpindah mode; { $modkey }+Alt+2 dan { $modkey }+Alt+3 juga bisa. Setelah extrude pertama, DUCAD pindah ke 3D sendiri.
onboard-ov-viewcube-title = Navigasi dan ViewCube
onboard-ov-viewcube-body = Orbit: seret klik tengah (atau klik kiri saat tool Pilih aktif). Geser: Shift + seret. Zoom: gulir. Di trackpad atau iPad pakai dua jari. Klik sisi ViewCube untuk tampak Atas, Depan, atau Kanan.
onboard-ov-context-title = Bilah aksi di bawah
onboard-ov-context-body = Fillet, extrude, shell, Hole Wizard, dan pattern tidak punya ikon di bilah kiri. Semuanya muncul di bilah bawah setelah Anda mengeklik sesuatu dengan tool Pilih: garis atau profil sketsa, sisi solid, tepi, atau body. Pilih dulu, baru aksinya tampil.
onboard-ov-palette-title = Palet perintah
onboard-ov-palette-body = Semua perintah DUCAD bisa dicari dari satu tempat: tekan { $modkey }+Shift+P (atau ikon kaca pembesar), ketik sebagian namanya, lalu Enter. Nanti di Bab 2 Anda memakainya untuk mengukur.
onboard-ov-help-title = Bantuan dan Chat AI
onboard-ov-help-body = Ikon tanda tanya membuka tutorial ini lagi kapan saja. Ikon bintang membuka Chat AI yang bisa memodelkan part dari kalimat biasa; penyiapannya ada di akhir Bab 3. Sekarang mari membuat cakramnya.
onboard-disc-circle-title = Lingkaran piringan Ø240
onboard-disc-circle-body = 1. Pilih tool Lingkaran di bilah kiri (tombol C).
    2. Klik titik pusat di tengah grid (perpotongan sumbu merah dan hijau).
    3. Ketik 120 lalu Enter: radius 120 mm, diameter 240 mm.
onboard-disc-circle-try = Gambar lingkaran radius 120 mm di bidang Top.
onboard-disc-extrude-title = Extrude menjadi piringan
onboard-disc-extrude-body = 1. Tekan Esc untuk kembali ke tool Pilih.
    2. Klik di dalam lingkaran: profil tersorot dan panah gizmo muncul di tengahnya.
    3. Seret panah ke atas, atau klik angkanya dan ketik 6, lalu Enter.
    DUCAD otomatis pindah ke mode 3D.
onboard-disc-extrude-try = Extrude lingkaran setebal 6 mm.
onboard-navigate-title = Lihat piringan dari segala sisi
onboard-navigate-body = Seret dengan klik tengah untuk orbit, Shift + seret untuk geser, gulir untuk zoom. Coba juga klik sisi "Front" pada ViewCube untuk melihat ketebalan 6 mm dari samping, lalu orbit lagi.
onboard-navigate-try = Putar, geser, atau zoom tampilan.
onboard-bore-circle-title = Sketsa lubang poros di sisi atas
onboard-bore-circle-body = 1. Dengan tool Pilih, klik sisi atas piringan: sisi tersorot dan bilah aksi muncul di bawah.
    2. Klik "Sketsa di Face": bidang sketsa pindah ke sisi itu.
    3. Tool Lingkaran (C), klik pusat piringan, ketik 30, Enter.
onboard-bore-circle-try = Buat sketsa di sisi atas dan gambar lingkaran radius 30 mm.
onboard-bore-cut-title = Potong tembus lubang poros
onboard-bore-cut-body = 1. Esc, lalu klik di dalam lingkaran kecil.
    2. Seret panah gizmo ke BAWAH sampai menembus piringan, atau klik angkanya dan ketik -6.
    Arah ke dalam solid berarti memotong: material di dalam profil terbuang.
onboard-bore-cut-try = Potong lubang Ø60 menembus piringan.
onboard-rim-chamfer-title = Chamfer tepi luar
onboard-rim-chamfer-body = 1. Pastikan mode 3D (tombol mode di bilah atas).
    2. Klik tepi luar atas piringan: gagang muncul beserta HUD gaya Fillet/Chamfer.
    3. Pilih Chamfer, lalu seret gagang atau ketik 1 dan Enter.
onboard-rim-chamfer-try = Chamfer 1 mm pada tepi luar piringan.
onboard-save-title = Simpan piringan
onboard-save-body = Tekan { $modkey }+S dan beri nama rem-cakram.ducad. Berkas ini memuat sketsa, solid, dan riwayatnya; Bab 2 melanjutkan dari sini.
onboard-save-try = Simpan dokumen ini.
onboard-end-beginner-title = Bab 1 selesai: piringan jadi
onboard-end-beginner-body = Anda sudah menggambar, meng-extrude, membuat sketsa di sisi solid, memotong tembus, dan memberi chamfer. Di Bab 2 piringan ini mendapat lima lubang baut dan dua belas slot ventilasi.
onboard-bolt-circle-title = Lingkaran baut pertama
onboard-bolt-circle-body = 1. Klik sisi atas piringan dengan tool Pilih, lalu "Sketsa di Face" di bilah bawah.
    2. Tool Lingkaran (C): klik titik 45 mm di kanan pusat (ikuti grid), ketik 5, Enter.
    Lingkaran Ø10 ini adalah pola untuk keempat lubang lainnya.
onboard-bolt-circle-try = Gambar lingkaran radius 5 mm pada radius 45 mm dari pusat.
onboard-bolt-pattern-title = Pattern sirkular lima lubang
onboard-bolt-pattern-body = 1. Esc, klik lingkaran baut tadi.
    2. Klik "Pattern" di bilah bawah.
    3. Di HUD atas pilih Sirkular, jumlah 5, sudut 360°, pusat di pusat piringan, lalu Terapkan (Enter).
onboard-bolt-pattern-try = Perbanyak lingkaran menjadi lima dengan pattern sirkular.
onboard-bolt-cut-title = Potong lima lubang baut
onboard-bolt-cut-body = 1. Esc, lalu pilih kelima lingkaran: Shift + klik satu per satu, atau seret kotak seleksi.
    2. Seret panah gizmo ke bawah menembus piringan (atau ketik -6).
onboard-bolt-cut-try = Potong kelima lingkaran menembus piringan.
onboard-vent-slot-title = Slot ventilasi pertama
onboard-vent-slot-body = 1. Pilih tool Slot di bilah kiri.
    2. Klik titik pada radius 80 mm, lalu titik pada radius 110 mm searah jari-jari.
    3. Ketik lebar 6 lalu Enter.
onboard-vent-slot-try = Gambar satu slot radial sepanjang 30 mm.
onboard-vent-pattern-title = Pattern dua belas ventilasi
onboard-vent-pattern-body = 1. Esc, pilih keempat segmen slot (seret kotak seleksi di sekitarnya).
    2. "Pattern" di bilah bawah: Sirkular, jumlah 12, sudut 360°, pusat piringan, Terapkan.
onboard-vent-pattern-try = Perbanyak slot menjadi dua belas.
onboard-vent-cut-title = Potong semua ventilasi
onboard-vent-cut-body = Pilih semua slot dengan kotak seleksi (jangan ikutkan lingkaran luar), lalu seret panah gizmo ke bawah menembus piringan.
onboard-vent-cut-try = Potong kedua belas slot menembus piringan.
onboard-bore-fillet-title = Fillet tepi lubang poros
onboard-bore-fillet-body = 1. Masuk mode 3D.
    2. Klik tepi atas lubang poros: gagang muncul.
    3. Pastikan gaya Fillet, lalu seret gagang atau ketik 2 dan Enter.
onboard-bore-fillet-try = Fillet 2 mm pada tepi lubang poros.
onboard-measure-title = Ukur jarak antar lubang baut
onboard-measure-body = 1. Buka palet perintah ({ $modkey }+Shift+P), ketik "ukur", jalankan "Ukur Jarak".
    2. Klik pusat dua lubang baut yang bersebelahan.
    Jaraknya tampil di kanvas dan di pil status bawah.
onboard-measure-try = Ukur satu jarak pada piringan.
onboard-rename-title = Beri nama body
onboard-rename-body = 1. Klik tombol folder di baris ikon kanan bawah: daftar Item terbuka.
    2. Klik body piringan, pilih Ganti Nama, ketik "Cakram", Enter.
    Nama ini ikut ke gambar kerja dan ekspor.
onboard-rename-try = Ganti nama body menjadi Cakram.
onboard-end-intermediate-title = Bab 2 selesai: cakram berventilasi
onboard-end-intermediate-body = Cakram kini punya lima lubang baut, dua belas ventilasi, dan tepi yang halus. Di Bab 3 ia mendapat hub berongga, lalu diperiksa dan disiapkan untuk produksi.
onboard-hub-extrude-title = Hub di atas cakram
onboard-hub-extrude-body = 1. Klik sisi atas cakram, "Sketsa di Face".
    2. Lingkaran (C) di pusat, radius 60, Enter.
    3. Esc, klik di dalam lingkaran, seret panah gizmo ke ATAS 25 mm.
onboard-hub-extrude-try = Extrude hub Ø120 setinggi 25 mm ke atas.
onboard-hub-shell-title = Shell: hub berongga
onboard-hub-shell-body = 1. Mode 3D, klik sisi atas hub.
    2. Klik "Shell" di bilah bawah.
    3. Atur tebal dinding 4 mm, lalu Enter. Sisi yang diklik menjadi bukaan.
onboard-hub-shell-try = Shell hub dengan dinding 4 mm.
onboard-hub-hole-title = Lubang ulir dengan Hole Wizard
onboard-hub-hole-body = 1. Klik cincin atas hub (sisi datar yang tersisa).
    2. "Hole Wizard" di bilah bawah: dialog muncul di kanan bawah.
    3. Pilih Tapped M8, tembus, lalu Terapkan.
onboard-hub-hole-try = Buat satu lubang ulir M8 dengan Hole Wizard.
onboard-section-title = Irisan: lihat bagian dalam
onboard-section-body = Di mode 3D klik tool Tampilan Irisan di bilah kiri. Cakram terbelah sehingga rongga hub, lubang ulir, dan ventilasi terlihat. Klik lagi untuk mematikannya.
onboard-section-try = Nyalakan Tampilan Irisan.
onboard-material-title = Material dan massa
onboard-material-body = 1. Palet perintah, jalankan "Properti Massa".
    2. Di panel, pilih material Baja untuk body Cakram.
    Massa, volume, dan pusat massa langsung dihitung.
onboard-material-try = Beri body material mekanik.
onboard-sim-title = Simulasi: beban pengereman
onboard-sim-body = 1. Palet perintah, jalankan "Simulasi (studi statik)".
    2. Tekan +. Klik sisi dalam satu lubang baut, + Tumpuan. Klik zona gesek sisi atas, + Beban 500 N ke bawah.
    3. Buat studi, lalu Jalankan. Lihat tegangan von Mises dan faktor keamanan.
onboard-sim-try = Jalankan satu studi statik sampai selesai.
onboard-drawing-title = Gambar kerja 2D ke PDF
onboard-drawing-body = 1. Menu utama (ikon tiga garis) di bilah atas, pilih "Gambar Kerja 2D".
    2. Lembar dengan tampak atas, depan, samping, dan isometrik dibuat otomatis.
    3. Klik Ekspor PDF, simpan, lalu tutup lembar.
onboard-drawing-try = Ekspor gambar kerja ke PDF.
onboard-step-title = Ekspor STEP untuk manufaktur
onboard-step-body = Menu utama (ikon tiga garis) di bilah atas, submenu Ekspor, pilih "STEP". Berkas STEP dibaca semua CAD/CAM lain; STL untuk cetak 3D ada di menu yang sama.
onboard-step-try = Ekspor cakram ke STEP.
onboard-chat-title = Siapkan agent untuk Chat AI
onboard-chat-body = Chat AI mengubah part dari kalimat biasa, tetapi perlu disiapkan sekali. 1) Buka panel Chat AI. 2) Klik ikon roda gigi di panel, centang "Izinkan AI eksternal". 3) Pilih agent di kiri atas panel: provider API (isi kunci API) atau CLI agent seperti Claude Code. 4) Untuk CLI agent tekan "Deteksi" lalu "Uji koneksi". Setelah siap, coba: "tambah chamfer 0,5 mm pada semua lubang baut".
onboard-chat-try = Buka panel Chat AI. Penyiapan agent bisa dilanjutkan nanti.
onboard-tour-title = Rem cakram siap produksi
onboard-tour-body = Dari satu lingkaran sampai STEP dan gambar kerja: Anda sudah memakai alur lengkap DUCAD. Yang masih bisa dijelajahi:
onboard-tour-shapes = Bentuk lain: Revolve, Loft, Sweep, Helix, Boolean, dan teks timbul.
onboard-tour-vector = Mode Vektor dan Tinta untuk kurva Bezier dan coretan bebas.
onboard-tour-agent = Agent Bridge agar agent AI eksternal memodelkan lewat MCP.
onboard-tour-reopen = Tutorial ini bisa dibuka lagi kapan saja lewat tombol bantuan di kanan atas atau palet perintah.

# Tablet (iPadOS/Android)
file-restored-autosave = Dokumen dipulihkan dari autosave (aplikasi sebelumnya dihentikan sistem)
mobile-memory-trimmed = Memori perangkat menipis: cache dibebaskan, Liquid Glass dimatikan
