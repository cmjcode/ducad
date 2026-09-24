//! Profil pengguna: membaca dan menghapus akun sendiri.

use axum::Json;
use axum::extract::State;

use crate::AppState;
use crate::auth::handler::fetch_user;
use crate::error::{AppError, Result};
use crate::models::{ApiResponse, DeleteAccountResponse, JwtClaims, UserResponse};

/// `GET /api/v1/users/me`
pub async fn get_me(
    claims: JwtClaims,
    State(app): State<AppState>,
) -> Result<Json<ApiResponse<UserResponse>>> {
    let user = fetch_user(&app.db, &claims.sub).await?;
    Ok(Json(ApiResponse::ok(UserResponse::from(user))))
}

/// `DELETE /api/v1/users/me`
///
/// Diwajibkan App Store Review Guideline 5.1.1(v): aplikasi yang
/// memungkinkan pembuatan akun harus memungkinkan penghapusannya dari dalam
/// aplikasi, bukan hanya lewat situs web.
///
/// Satu DELETE cukup: `sessions.user_id` memakai `ON DELETE CASCADE`, jadi
/// seluruh sesi perangkat ikut terhapus dan refresh token yang masih
/// dipegang perangkat lain langsung tidak berlaku.
pub async fn delete_me(
    claims: JwtClaims,
    State(app): State<AppState>,
) -> Result<Json<ApiResponse<DeleteAccountResponse>>> {
    // Dibaca lebih dulu supaya balasannya bisa menyebut alamat yang dihapus,
    // dan supaya token yang hidup lebih lama dari barisnya menjawab 404
    // alih-alih melaporkan sukses atas penghapusan yang tidak terjadi.
    let user = fetch_user(&app.db, &claims.sub).await?;

    let result = sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(&claims.sub)
        .execute(&app.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Akun tidak ditemukan".to_string()));
    }

    tracing::info!(user_id = %claims.sub, "akun dihapus atas permintaan pemiliknya");

    Ok(Json(ApiResponse::ok(DeleteAccountResponse {
        deleted: true,
        email: user.email,
    })))
}
