# Changelog

All notable changes to **DuCAD Studio** (`ducad`, `ducad-cli`, `ducad-mcp`, and workspace crates) are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Security
- Unified repository license metadata to `AGPL-3.0-only` with bundled third-party attribution (`NOTICE` for SolidCraft, truck, matext4cgmath, and TextureCraft) across all archives and Windows MSI installers.
- Added `SECURITY.md` vulnerability disclosure policy, `.github/dependabot.yml`, and blocking `cargo deny check` + `release-gate.sh` verification in `.github/workflows/release.yml` and `scripts/release-tag.sh`.
- Upgraded `zip` (`9.5.1`) and `rustls` / `rustls-webpki` / `webpki-roots` to resolve security advisories.
- Hardened `ducad-cloud` client session storage (`0700` config dir, `0600` `session.json` permissions on Unix) and removed OAuth ticket/URL logging at `info` level.
- Added Ed25519 / Minisign manifest signature verification (`<package>.sha256.minisig` / `.sig`) in `ducad-update`.

### Added
- In-app permanent account deletion (**Delete Account** with explicit confirmation in `AccountDrawer` and `CloudSession::delete_account` calling `DELETE /api/v1/users/me`).
- English user, CLI, MCP, and Hardware CI documentation (`ducad-editor/docs/GUIDE_EN.md`).
- Cross-platform structural golden verification for 2D engineering drawing sheets (`drawing_golden`) alongside per-platform byte hashes.

### Changed
- Scoped active release targets to Linux `x86_64`, Windows `x86_64`, and macOS `arm64`; marked `ducad-android` and `aur/ducad` as paused, and labeled `ducad-photoreal`, modal/buckling/thermal FEA, `ducad-ink` freehand, Vector mode, and Cloud Sync as **Experimental**.
- Removed dead legacy UI modules (`feature_inspector`, `radial_menu`, unused `tool_popups` modals, and unused `history_drawer` widget) and hid the unimplemented desktop `palm_rejection` toggle.
- Replaced inaccurate `"CAD/CAM"` title-block and SVG generator strings with `"DUCAD Studio"` / `"DuCAD"` (DuCAD does not generate CNC G-code toolpaths).

---

## [0.8.0] - 2026-10-30

### Added
- Pure-Rust B-rep solid modeling kernel integration (**SolidCraft** on **truck**, vendored in-tree) replacing C++ OpenCASCADE dependencies.
- Shared atomic parametric oplog engine (`ducad-engine`) powering the desktop GUI (`ducad-app`), headless CLI (`ducad-cli`), and MCP server (`ducad-mcp`).
- Semantic topology selector DSL (`faces`, `edges`, directional/geometric filters) for resilient parametric history replay.
- 2D Engineering Drawing workbench with Hidden Line Removal (HLR), section views, detail views, BOM tables, callout balloons, and `SVG` / `PDF` / `DXF` export.
- Engineering assertion checks, DFM printability analysis, clash detection, and linear static FEA (`ducad-sim`).
- Complete bilingual localization (`en-US` and `id-ID` via `ducad-i18n`) and self-updater (`ducad-update`) for Windows, Linux, and macOS.

