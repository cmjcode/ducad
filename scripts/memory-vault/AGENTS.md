# Konvensi Memori DUCAD

Vault ini adalah memori jangka panjang agent yang memodelkan dengan DUCAD.

## Sebelum memodelkan
1. `recall` dengan query berisi jenis part + material + proses (mis. "bracket aluminium CNC"). Anggaran 1500 token.
2. Baca `Preferences/Defaults.md` bila belum ada di hasil recall.

## Yang disimpan, dan di mana
- `Preferences/` — pilihan user yang berlaku umum (satuan, toleransi default, radius fillet favorit, material). Satu fakta per butir.
- `Standards/` — tabel acuan. Jangan diubah tanpa diminta user.
- `Lessons/` — satu catatan per pelajaran: gejala, penyebab, perbaikan, op yang terlibat. HANYA setelah perbaikannya terbukti berhasil (checks hijau).
- `Projects/<nama-part>.md` — tujuan, persyaratan, keputusan + alasan, path file `.ducad`, status.
- `Sessions/YYYY-MM-DD.md` — log singkat: apa yang diminta, apa yang dibuat, apa yang tertunda. Tambah dengan `append_note` (`create_if_missing: true`).
- `BOM/<assembly>.csv` — kolom: item, part, qty, material, file, catatan.

## Paket desain (brief → review → revisi)
- `Standards/firm.json` = profil firma terbaca mesin (lihat `Standards/Firm-Profile.md`); `Standards/Rule-Packs.md` = peta paket aturan.
- Setelah paket diajukan/disetujui: perbarui `Projects/<nama-part>.md` dengan brief, persyaratan + statusnya, dan tabel revisi (`ducad-cli build --package --project-note` menulis drafnya).
- Redline atau proposal yang DITOLAK user: catat di `Lessons/` apa yang diusulkan, mengapa ditolak, dan apa yang akhirnya diterima. Ini satu-satunya pelajaran yang boleh dicatat tanpa checks hijau.

## Aturan
- Jangan menyimpan geometri, STEP, mesh, atau oplog utuh. Simpan path file.
- Tag wajib: `ducad` + salah satu dari `preference|standard|lesson|project|session`.
- Isi argumen `agent` dengan nama harness (mis. `claude-code`).
- Fakta yang sudah tidak berlaku: jangan dihapus; tandai usang sesuai mekanisme di docs/agent-interface.md (supersede) dan tulis penggantinya.
- Sebelum `remember`, `recall` dulu untuk menghindari duplikat; bila mirip, perbarui catatan lama dengan `patch_note` + `if_hash`.
