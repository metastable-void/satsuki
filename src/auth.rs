//! Basic-auth based authentication extractor plus password helpers.
use axum::{
    Extension,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use std::future::Future;

use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use rand_core::OsRng;

use crate::SharedState;
use crate::db::user_repo::User;

/// Axum extractor that verifies Basic credentials against the database.
pub struct Authenticated(pub User);

impl<S> FromRequestParts<S> for Authenticated
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> {
        Box::pin(async move {
            // The state is SharedState via Extension
            let Extension(app_state): axum::extract::Extension<SharedState> =
                Extension::from_request_parts(parts, state)
                    .await
                    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "missing state"))?;

            let auth_header = parts
                .headers
                .get(axum::http::header::AUTHORIZATION)
                .ok_or((StatusCode::UNAUTHORIZED, "missing Authorization header"))?
                .to_str()
                .map_err(|_| (StatusCode::BAD_REQUEST, "invalid Authorization header"))?;

            if !auth_header.starts_with("Basic ") {
                return Err((StatusCode::UNAUTHORIZED, "expected Basic auth"));
            }

            let b64 = &auth_header[6..];
            let decoded = BASE64
                .decode(b64)
                .map_err(|_| (StatusCode::BAD_REQUEST, "invalid Base64"))?;
            let decoded = String::from_utf8(decoded)
                .map_err(|_| (StatusCode::BAD_REQUEST, "invalid UTF-8"))?;

            let (username, password) = decoded
                .split_once(':')
                .ok_or((StatusCode::BAD_REQUEST, "invalid Basic payload"))?;

            // lookup user by subdomain (username)
            let user = crate::db::user_repo::find_by_subdomain(&app_state.db, username)
                .await
                .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid credentials"))?
                .ok_or((StatusCode::UNAUTHORIZED, "invalid credentials"))?;

            // verify password
            if !crate::auth::verify_password(&user.password_hash, password)
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "hash error"))?
            {
                return Err((StatusCode::UNAUTHORIZED, "invalid credentials"));
            }

            Ok(Authenticated(user))
        })
    }
}

/// Hash a plaintext password using Argon2 + random salt.
pub fn hash_password(plain: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|_| anyhow::anyhow!("Failed to hash password"))?
        .to_string();
    Ok(hash)
}

/// Verify a plaintext password against a stored Argon2 hash.
pub fn verify_password(hash: &str, plain: &str) -> anyhow::Result<bool> {
    let parsed = PasswordHash::new(hash)
        .map_err(|_| anyhow::anyhow!("Failed to instantiate PasswordHash"))?;
    Ok(Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok())
}
