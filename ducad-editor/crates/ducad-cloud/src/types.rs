use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Provider OAuth yang didukung oleh CMJCode Auth Server
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OAuthProvider {
    Google,
    GitHub,
}

impl OAuthProvider {
    pub fn label(&self) -> &'static str {
        match self {
            OAuthProvider::Google => "Google",
            OAuthProvider::GitHub => "GitHub",
        }
    }

    pub fn path(&self) -> &'static str {
        match self {
            OAuthProvider::Google => "google",
            OAuthProvider::GitHub => "github",
        }
    }
}

/// Representasi profil pengguna yang dikembalikan server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub username: Option<String>,
    pub phone: Option<String>,
}

/// Response payload otentikasi dari server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub user: UserResponse,
}

/// Akun pengguna Ducad yang tersimpan di sesi lokal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DucadAccount {
    pub user_id: String,
    pub email: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub username: Option<String>,
    pub phone: Option<String>,
    pub access_token: String,
    pub refresh_token: String,
    pub token_expires_at: i64,
    #[serde(default = "default_tier")]
    pub license_tier: String,
}

fn default_tier() -> String {
    "Pro".to_string()
}

impl DucadAccount {
    pub fn is_token_expired(&self) -> bool {
        let now = Utc::now().timestamp();
        // Buffer 60 detik sebelum benar-benar expired
        now >= (self.token_expires_at - 60)
    }

    pub fn display_title(&self) -> String {
        if let Some(ref name) = self.display_name {
            if !name.trim().is_empty() {
                return name.clone();
            }
        }
        if let Some(ref username) = self.username {
            if !username.trim().is_empty() {
                return format!("@{}", username);
            }
        }
        self.email.clone()
    }

    pub fn initials(&self) -> String {
        let title = self.display_title();
        let parts: Vec<&str> = title.split_whitespace().collect();
        if parts.len() >= 2 {
            let first = parts[0].chars().next().unwrap_or('?');
            let second = parts[1].chars().next().unwrap_or('?');
            format!("{}{}", first, second).to_uppercase()
        } else {
            title.chars().take(2).collect::<String>().to_uppercase()
        }
    }
}

/// Status otentikasi saat ini pada aplikasi Ducad
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthStatus {
    LoggedOut,
    Authenticating { provider: OAuthProvider },
    LoggedIn,
    Error(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_initials_and_display_title() {
        let acc = DucadAccount {
            user_id: "usr_123".to_string(),
            email: "yulius.jayuda@gmail.com".to_string(),
            display_name: Some("Yulius Jayuda".to_string()),
            avatar_url: None,
            username: Some("yjayuda".to_string()),
            phone: None,
            access_token: "token".to_string(),
            refresh_token: "refresh".to_string(),
            token_expires_at: Utc::now().timestamp() + 3600,
            license_tier: "Pro".to_string(),
        };

        assert_eq!(acc.display_title(), "Yulius Jayuda");
        assert_eq!(acc.initials(), "YJ");
        assert!(!acc.is_token_expired());
    }

    #[test]
    fn test_account_fallback_initials() {
        let acc = DucadAccount {
            user_id: "usr_456".to_string(),
            email: "engineer@company.com".to_string(),
            display_name: None,
            avatar_url: None,
            username: None,
            phone: None,
            access_token: "token".to_string(),
            refresh_token: "refresh".to_string(),
            token_expires_at: 0,
            license_tier: "Free".to_string(),
        };

        assert_eq!(acc.display_title(), "engineer@company.com");
        assert_eq!(acc.initials(), "EN");
        assert!(acc.is_token_expired());
    }
}
