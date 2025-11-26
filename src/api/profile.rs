// src/api/profile.rs
use super::public::internal;
use crate::db::user_repo;
use crate::powerdns::types::{PdnsRecord, PdnsRrset};
use crate::validation::validate_fqdn_ascii;
use crate::{SharedState, auth::Authenticated};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

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
