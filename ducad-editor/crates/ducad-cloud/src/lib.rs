//! `ducad-cloud`: Modul integrasi cloud, otentikasi SSO DUCAD, dan sinkronisasi untuk DUCAD CAD.

// Prinsip "jangan pernah crash" (ADR 0009): tidak ada unwrap/expect/panic di
// kode produksi; modul tes dikecualikan.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::todo, clippy::unimplemented)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
pub mod auth;
pub mod avatar;
pub mod client;
pub mod sync;
pub mod types;

pub use sync::{CloudFile, CloudSession, FileList, RemotePrefs, SyncError};

pub use avatar::{clear_avatar_cache, fetch_avatar, upload_avatar};
pub use auth::{clear_account, load_account, open_url, save_account, session_file_path, start_oauth_flow, token_to_account};
pub use client::{detect_server_url, CloudClient, DEFAULT_SERVER_URL};
pub use types::{AuthStatus, DucadAccount, OAuthProvider, TokenResponse, UserResponse};
