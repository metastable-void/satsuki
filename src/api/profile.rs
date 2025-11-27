//! Authenticated profile endpoints for viewing and updating NS delegation.
use super::public::internal;
use crate::db::user_repo;
use crate::powerdns::types::{PdnsRecord, PdnsRrset};
use crate::validation::validate_fqdn_ascii;
use crate::{
    SharedState,
    auth::{self, Authenticated},
};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

/// Public profile information returned to signed-in users.
#[derive(Serialize)]
pub struct ProfileDto {
    pub subdomain: String,
    pub external_ns: bool,
    pub external_ns1: Option<String>,
    pub external_ns2: Option<String>,
    pub external_ns3: Option<String>,
    pub external_ns4: Option<String>,
    pub external_ns5: Option<String>,
    pub external_ns6: Option<String>,
}

/// Return the caller's profile metadata and NS configuration.
pub async fn get_profile(
    Authenticated(user): Authenticated,
    Extension(_state): Extension<SharedState>,
) -> Result<Json<ProfileDto>, (axum::http::StatusCode, String)> {
    Ok(Json(ProfileDto {
        subdomain: user.subdomain.clone(),
        external_ns: user.external_ns,
        external_ns1: user.external_ns1.clone(),
        external_ns2: user.external_ns2.clone(),
        external_ns3: user.external_ns3.clone(),
        external_ns4: user.external_ns4.clone(),
        external_ns5: user.external_ns5.clone(),
        external_ns6: user.external_ns6.clone(),
    }))
}

/// Switch the caller back to the operator-managed nameservers.
pub async fn set_ns_internal(
    Authenticated(user): Authenticated,
    Extension(state): Extension<SharedState>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let zone_name = state.config.user_zone_name(&user.subdomain);
    let parent_zone = state.config.parent_zone_name();

    let ns_rrset = PdnsRrset {
        name: zone_name.clone(),
        rrtype: "NS".into(),
        ttl: 300,
        changetype: Some("REPLACE".into()),
        records: state
            .config
            .internal_ns
            .iter()
            .map(|ns| PdnsRecord {
                content: ns.clone(),
                disabled: false,
            })
            .collect(),
        comments: Vec::new(),
    };
    state
        .base_pdns
        .patch_rrsets(&parent_zone, &[ns_rrset])
        .await
        .map_err(internal)?;

    user_repo::set_external_ns(
        &state.db, user.id, false, None, None, None, None, None, None,
    )
    .await
    .map_err(internal)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Payload describing the external NS list the user wants to delegate to.
#[derive(Deserialize)]
pub struct SetExternalNsRequest {
    pub ns: Vec<String>, // validate to be FQDNs with trailing dots
}

/// Configure custom nameservers for the caller and persist them in PDNS.
pub async fn set_ns_external(
    Authenticated(user): Authenticated,
    Extension(state): Extension<SharedState>,
    Json(req): Json<SetExternalNsRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    if req.ns.is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "at least one NS required".into(),
        ));
    }

    let zone_name = state.config.user_zone_name(&user.subdomain);
    let parent_zone = state.config.parent_zone_name();

    if req.ns.len() > 6 {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "up to six nameservers supported".into(),
        ));
    }

    let mut validated_ns = Vec::with_capacity(req.ns.len());
    for ns in req.ns {
        if !ns.ends_with('.') {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                "nameservers must end with '.'".into(),
            ));
        }
        validate_fqdn_ascii(&ns)
            .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;
        validated_ns.push(ns);
    }

    let ns_rrset = PdnsRrset {
        name: zone_name.clone(),
        rrtype: "NS".into(),
        ttl: 300,
        changetype: Some("REPLACE".into()),
        records: validated_ns
            .iter()
            .map(|ns| PdnsRecord {
                content: ns.clone(),
                disabled: false,
            })
            .collect(),
        comments: Vec::new(),
    };
    state
        .base_pdns
        .patch_rrsets(&parent_zone, &[ns_rrset])
        .await
        .map_err(internal)?;

    let ns1 = validated_ns.get(0).cloned();
    let ns2 = validated_ns.get(1).cloned();
    let ns3 = validated_ns.get(2).cloned();
    let ns4 = validated_ns.get(3).cloned();
    let ns5 = validated_ns.get(4).cloned();
    let ns6 = validated_ns.get(5).cloned();

    user_repo::set_external_ns(&state.db, user.id, true, ns1, ns2, ns3, ns4, ns5, ns6)
        .await
        .map_err(internal)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Request body for updating the user's password.
#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// Change the caller's password after verifying the current secret.
pub async fn change_password(
    Authenticated(user): Authenticated,
    Extension(state): Extension<SharedState>,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    if req.new_password.trim().len() < 8 {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "new password must be at least 8 characters".into(),
        ));
    }

    let valid_current = auth::verify_password(&user.password_hash, &req.current_password)
        .map_err(internal)?;

    if !valid_current {
        return Err((
            axum::http::StatusCode::UNAUTHORIZED,
            "current password is incorrect".into(),
        ));
    }

    let new_hash = auth::hash_password(&req.new_password).map_err(internal)?;
    user_repo::update_password(&state.db, user.id, &new_hash)
        .await
        .map_err(internal)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}
