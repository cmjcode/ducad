# ADR 0005 — Chat agent di dalam DUCAD (P13–P15)

**Status**: Diterima · **Tanggal**: 2026-09-30 · **Cakupan**: P13, P14, P15
(`.claude/plan/ducad-agent-harness/P13-P15-chat-agent.md`)

## Konteks

DUCAD sudah bisa dikendalikan agent eksternal lewat MCP (20 tool, 12 `Op`)
dan punya asisten di perangkat (ADR 0002) yang hanya cocok untuk ubahan
kecil. Yang belum ada: chat di dalam aplikasi yang bisa membuat dan
mengubah part lewat percakapan, dan cakupan operasi yang setara dengan GUI.

## Keputusan

1. **Satu permukaan tool.** Chat di aplikasi memakai definisi tool dari
   `ducad_mcp::tools::chat_tools(true)` dan dieksekusi oleh kode jembatan
   live yang sama dengan `ducad-mcp --attach` (`agent_bridge.rs`). Agent
   internal dan eksternal melihat tool yang identik; tidak ada daftar
   kedua yang bisa menyimpang.
2. **Kanal in-process.** Loop chat berjalan di thread latar dan mengirim
   `BridgeRequest` ke kanal lokal jembatan (tanpa soket). Eksekusi tetap di
   UI thread (`KernelShape` bukan `Send`), satu batch = satu langkah undo,
   `propose_ops` menunggu Terima/Tolak, `accept_proposal` tidak tersedia.
   Desain ini juga jalan di iPadOS, yang tidak bisa menjalankan proses MCP.
3. **Crate `ducad-chat` tanpa GUI/kernel.** Provider Anthropic (Messages
   API, SSE) dan OpenAI-compatible (OpenAI, Ollama, LM Studio, OpenRouter)
   lewat `ureq` blocking, tanpa tokio — keputusan workspace yang sama
   dengan `ducad-cloud`. Pola loop, model percakapan, batas 8 putaran,
   pemotongan hasil 20 KB, dan pembatalan `AtomicBool` mengikuti agent
   TABULAR (`ai_tool_chat.rs`).
4. **Anthropic sebagai bawaan** (`claude-opus-5`): blok konten asisten
   (termasuk thinking bertanda tangan) dikirim ulang apa adanya,
   `cache_control` di tool terakhir dan prompt sistem, `fallbacks:
   "default"` untuk keluarga Opus 5/Fable di API langsung, dan
   `stop_reason: "refusal"` dilaporkan ke pengguna.
5. **Privasi tetap "hanya di perangkat" secara bawaan.** Provider jaringan
   dan Agent Bridge baru bisa dipakai setelah pengguna mencentang "Izinkan
   AI eksternal". Provider di `localhost` selalu boleh. Kunci API di
   Keychain (macOS/iOS) atau berkas 0600, atau variabel lingkungan; tidak
   pernah di `ai-chat.json` maupun log.
6. **Cakupan op diperluas** (P14): `loft`, `sweep`, `helix`, `draft`,
   `mirror`, `scale`, `split`, fillet variabel (`radius_end`). Emboss tidak
   jadi op tersendiri: sketch di face + extrude `add`/`cut` sudah setara.
7. **Tool baru** (P15): `drawing`, `import_step` (STEP disimpan di
   `base_bodies` agar replay mandiri), `diff`, `list_parts`, MCP resources
   dan prompts, serta tool live `document_info`, `get_view`, `set_view`,
   `get_selection`, `select`, `screenshot`.

## Tambahan: CLI agent sebagai backend (P13.5)

8. **CLI coding agent lokal** (Antigravity `agy`, Claude Code `claude`,
   Gemini CLI `gemini`, atau perintah kustom) bisa dipilih sebagai backend
   chat, pola harness TABULAR (`agent/harness.rs`). Agent berjalan dalam
   mode print + `stream-json` di `~/.ducad/agent-workspace` (folder kosong)
   dan memakai `ducad-mcp --attach` yang tersambung ke soket jembatan, jadi
   tool, undo, dan kartu proposal sama dengan chat API.
   - Claude Code: `--mcp-config` per giliran + `--strict-mcp-config` +
     `--allowedTools mcp__ducad` (tool bawaan lain ditolak), sesi lanjut
     dengan `--resume`.
   - agy: `--dangerously-skip-permissions`, sesi lanjut `--conversation`,
     server MCP didaftarkan global (`agy mcp add ducad -- ducad-mcp --attach`).
   - Gemini: `--approval-mode yolo` + `--allowed-mcp-server-names ducad`,
     `GEMINI_CLI_TRUST_WORKSPACE=true` (mode headless menolak folder yang
     belum dipercaya), tanpa resume: riwayat teks disisipkan di prompt.
   - Butuh "Izinkan AI eksternal": CLI mengirim desain ke provider modelnya.
   - "Selalu minta persetujuan" kini dipaksa di jembatan (`force_propose`),
     jadi berlaku untuk CLI agent dan agent eksternal mana pun.
   Diverifikasi dengan Claude Code sungguhan (haiku): `new_part` →
   `run_ops` → `inspect` → jawaban 6000 mm³; rekamannya menjadi fixture
   `ducad-chat/tests/fixtures/claude_code_stream.ndjson`. agy menjawab
   prompt uji. Gemini di mesin pengembang belum login (401), sehingga
   jalur tool Gemini belum teruji langsung.

## Temuan saat implementasi

- **Bug helix di kernel**: spine dari rangkaian segmen garis membuat
  `MakePipe` menghasilkan solid ber-volume ≈ 0 (2,4 mm³ untuk pegas yang
  seharusnya ±593 mm³). Tes kernel lama hanya memeriksa mesh. Spine kini
  satu kurva B-spline interpolasi; tes `helix_spring_has_real_volume`
  menjaga volumenya. Fitur Helix di GUI ikut diperbaiki.
- **Mirror bidang** tidak ada di binding OCCT (hanya mirror sumbu =
  rotasi 180°). Disusun dari skala −1 lalu rotasi 180° mengelilingi normal
  (I − 2nnᵀ); tes memverifikasi bbox dan volume.
- **Privasi tidak pernah bisa diubah** sebelum ini: `AiPrivacy` tidak punya
  UI, sehingga Agent Bridge selalu terblokir. Sakelar kini ada di ⚙ panel
  Chat AI dan tersimpan.

## Yang belum dikerjakan (jujur)

- Tidak ada uji langsung ke API Anthropic/OpenAI di sesi ini (tanpa kunci
  API). Jalur HTTP diuji dengan server SSE palsu di localhost
  (`ducad-cli/tests/chat.rs`) dan pengurai dengan rekaman aliran.
- Op P14 belum punya adapter GUI baru; GUI tetap memakai implementasi
  fitur lamanya sendiri.
- `screenshot` bergantung pada dukungan tangkapan layar renderer wgpu
  eframe; tanpa renderer (tes) jalurnya berakhir dengan error setelah 10 s.
- Shell variabel belum menjadi op.
- Eval tingkat lulus provider eksternal (`evals/run_eval.py` dengan
  `ducad-cli chat`) belum dijalankan karena butuh kunci API.
