
use axum::{Json, Extension};
use serde::{Deserialize, Serialize};
use crate::{SharedState, auth::hash_password};
use crate::db::user_repo;
use crate::powerdns::types::{PdnsZoneCreate, PdnsRrset, PdnsRecord};
use crate::error::AppError;
use crate::validation::validate_subdomain_name;

#[derive(Deserialize)]
pub struct SignupRequest {
    pub subdomain: String,
    pub password: String,
}

pub async fn signup(
    Extension(state): Extension<SharedState>,
    Json(req): Json<SignupRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    // 1) validate subdomain syntax
    crate::validation::validate_subdomain_name(&req.subdomain)
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;

    // 2) check if exists
    if user_repo::exists(&state.db, &req.subdomain).await.map_err(internal)? {
        return Err((axum::http::StatusCode::CONFLICT, "already exists".into()));
    }

    // 3) prepare PDNS zone & NS
    let zone_name = format!("{}.{}.", req.subdomain, state.config.base_domain);

    // create zone in sub-PDNS
    let z = PdnsZoneCreate {
        name: zone_name.clone(),
        kind: "Native".into(),
        nameservers: state.config.internal_ns.clone(),
    };
    state.sub_pdns.create_zone(&z).await.map_err(internal)?;

    // 4) create NS delegation in base-PDNS
    let ns_rrset = PdnsRrset {
        name: zone_name.clone(),
        rrtype: "NS".into(),
        ttl: 300,
        changetype: Some("REPLACE".into()),
        records: state.config.internal_ns
            .iter()
            .map(|ns| PdnsRecord { content: ns.clone(), disabled: false })
            .collect(),
    };
    state.base_pdns
        .patch_rrsets(&state.config.base_domain, &[ns_rrset])
        .await
        .map_err(|e| {
            // TODO: attempt rollback of created zone
            internal(e)
        })?;

    // 5) insert into DB
    let hash = hash_password(&req.password).map_err(internal)?;
    user_repo::insert(&state.db, &req.subdomain, &hash).await.map_err(internal)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

pub(crate) fn internal<E: std::fmt::Display>(e: E) -> (axum::http::StatusCode, String) {
    (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

#[derive(Deserialize)]
pub struct SigninRequest {
    pub subdomain: String,
    pub password: String,
}

pub async fn signin(
    Extension(state): Extension<SharedState>,
    Json(req): Json<SigninRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    use crate::auth::verify_password;
    use crate::db::user_repo;

    let user = user_repo::find_by_subdomain(&state.db, &req.subdomain)
        .await
        .map_err(internal)?
        .ok_or((axum::http::StatusCode::UNAUTHORIZED, "invalid credentials".into()))?;

    if !verify_password(&user.password_hash, &req.password).map_err(internal)? {
        return Err((axum::http::StatusCode::UNAUTHORIZED, "invalid credentials".into()));
    }

    // Optional: update last_login_at

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Serialize)]
pub struct CheckSubdomainResponse {
    available: bool,
}

pub async fn check_subdomain(
    Extension(state): Extension<SharedState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<CheckSubdomainResponse>, AppError> {
    let Some(name) = params.get("name") else {
        return Err(AppError::bad_request("missing 'name' parameter"));
    };

    validate_subdomain_name(name).map_err(|e| AppError::BadRequest(e.to_string()))?;

    let exists = user_repo::exists(&state.db, name)
        .await
        .map_err(AppError::internal)?;

    Ok(Json(CheckSubdomainResponse {
        available: !exists,
    }))
}