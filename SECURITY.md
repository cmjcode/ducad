# Security Policy / Kebijakan Keamanan

## Supported Versions / Versi yang Didukung

| Version / Versi | Supported / Didukung |
|---|---|
| `0.8.x` (latest) | Yes / Ya |
| `< 0.8.0` | No / Tidak |

---

## Reporting a Vulnerability (English)

Please **do not** report security vulnerabilities through public GitHub issues, discussions, or pull requests.

Send vulnerability reports privately via email to:
- **security / primary**: `support@ducad.app` / `nunung.pamungkas@vneu.co.id`
- **maintainer**: `yulius.jayuda@gmail.com`

Include where possible:
1. Component affected (`ducad-editor`, `ducad-cli`, `ducad-mcp`, `ducad-update`, or `ducad-server`).
2. DuCAD version, operating system, and minimal reproduction steps or proof-of-concept file (`.ducad`, `.ops.json`, `.step`, `.zip`).
3. Impact assessment (crash/abort, path traversal, arbitrary file write, credential exposure, privilege escalation).

We acknowledge reports within **72 hours** and aim to publish a fix and security advisory within **14 days** for critical/high severity issues.

### Security Controls in Repository
- Dependencies are audited continuously via `cargo deny check` (blocking in CI) and automated Dependabot updates.
- Kernel operations run inside panic guards (`catch_op_panic`) and bounded compute budgets.
- Local secrets and signing credentials must never be committed; use `.env` (ignored by Git) based on `.env.example`.

---

## Melaporkan Kerentanan (Bahasa Indonesia)

Mohon **jangan** melaporkan celah keamanan melalui GitHub Issues publik.

Kirim laporan secara privat melalui email ke:
- `support@ducad.app` / `nunung.pamungkas@vneu.co.id`
- `yulius.jayuda@gmail.com`

Sertakan komponen terdampak, versi aplikasi, sistem operasi, langkah reproduksi minimal, serta dampak keamanan. Kami akan mengonfirmasi penerimaan laporan maksimal dalam **72 jam**.

