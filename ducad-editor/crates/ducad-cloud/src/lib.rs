//! `ducad-cloud`: Modul integrasi cloud, otentikasi SSO CMJCode, dan sinkronisasi untuk Ducad CAD.

pub mod auth;
pub mod client;
pub mod types;

pub use auth::{clear_account, load_account, open_url, save_account, session_file_path, start_oauth_flow, token_to_account};
pub use client::{detect_server_url, CloudClient, DEFAULT_SERVER_URL};
pub use types::{AuthStatus, DucadAccount, OAuthProvider, TokenResponse, UserResponse};
