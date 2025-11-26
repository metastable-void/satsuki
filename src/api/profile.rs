// src/api/profile.rs
use axum::{Json, Extension};
use serde::{Deserialize, Serialize};
use crate::{SharedState, auth::Authenticated};
use crate::powerdns::types::{PdnsRrset, PdnsRecord};
use crate::db::user_repo;
use super::public::internal;

#[derive(Serialize)]
pub struct ProfileDto {
    pub subdomain: String,
    pub external_ns: bool,
    pub external_ns1: Option<String>,
    pub external_ns2: Option<String>,
}

pub async fn get_profile(
    Authenticated(user): Authenticated,
    Extension(_state): Extension<SharedState>,
) -> Result<Json<ProfileDto>, (axum::http::StatusCode, String)> {
    Ok(Json(ProfileDto {
        subdomain: user.subdomain.clone(),
        external_ns: user.external_ns,
        external_ns1: user.external_ns1.clone(),
        external_ns2: user.external_ns2.clone(),
    }))
}

pub async fn set_ns_internal(
    Authenticated(user): Authenticated,
    Extension(state): Extension<SharedState>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let zone_name = format!("{}.{}.", user.subdomain, state.config.base_domain);

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
        .map_err(internal)?;

    user_repo::set_external_ns(&state.db, user.id, false, None, None, None, None, None, None)
        .await
        .map_err(internal)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct SetExternalNsRequest {
    pub ns: Vec<String>, // validate to be FQDNs with trailing dots
}

pub async fn set_ns_external(
    Authenticated(user): Authenticated,
    Extension(state): Extension<SharedState>,
    Json(req): Json<SetExternalNsRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    if req.ns.is_empty() {
        return Err((axum::http::StatusCode::BAD_REQUEST, "at least one NS required".into()));
    }

    let zone_name = format!("{}.{}.", user.subdomain, state.config.base_domain);

    let ns_rrset = PdnsRrset {
        name: zone_name.clone(),
        rrtype: "NS".into(),
        ttl: 300,
        changetype: Some("REPLACE".into()),
        records: req.ns
            .iter()
            .map(|ns| PdnsRecord { content: ns.clone(), disabled: false })
            .collect(),
    };
    state.base_pdns
        .patch_rrsets(&state.config.base_domain, &[ns_rrset])
        .await
        .map_err(internal)?;

    let ns1 = req.ns.get(0).cloned();
    let ns2 = req.ns.get(1).cloned();
    let ns3 = req.ns.get(2).cloned();
    let ns4 = req.ns.get(3).cloned();
    let ns5 = req.ns.get(4).cloned();
    let ns6 = req.ns.get(5).cloned();

    user_repo::set_external_ns(&state.db, user.id, true, ns1, ns2, ns3, ns4, ns5, ns6)
        .await
        .map_err(internal)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}
