//! Public-facing API handlers for signup, authentication, and discovery.

use crate::config::AppConfig;
use crate::db::user_repo;
use crate::error::AppError;
use crate::powerdns::types::{PdnsRecord, PdnsRrset, PdnsZoneCreate};
use crate::validation::validate_subdomain_name;
use crate::{SharedState, auth::hash_password};
use axum::{Extension, Json, http::header, response::IntoResponse};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Error as SqlxError;
use std::collections::BTreeSet;

/// Payload for creating a brand-new delegated subdomain.
#[derive(Deserialize)]
pub struct SignupRequest {
    pub subdomain: String,
    pub password: String,
}

/// Create a user account and delegate the requested subdomain if available.
pub async fn signup(
    Extension(state): Extension<SharedState>,
    Json(req): Json<SignupRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    // 1) validate subdomain syntax
    crate::validation::validate_subdomain_name(&req.subdomain)
        .map_err(|e| (axum::http::StatusCode::BAD_REQUEST, e.to_string()))?;

    if state.config.is_disallowed_subdomain(&req.subdomain) {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "requested subdomain is reserved".into(),
        ));
    }

    // 2) check if exists
    if user_repo::exists(&state.db, &req.subdomain)
        .await
        .map_err(internal)?
    {
        return Err((axum::http::StatusCode::CONFLICT, "already exists".into()));
    }

    if dns_label_occupied(&state, &req.subdomain)
        .await
        .map_err(internal)?
    {
        return Err((axum::http::StatusCode::CONFLICT, "already exists".into()));
    }

    if state.config.internal_ns.is_empty() {
        return Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "no internal nameservers configured".into(),
        ));
    }

    let hash = hash_password(&req.password).map_err(internal)?;

    // 3) prepare PDNS zone & NS
    let zone_name = state.config.user_zone_name(&req.subdomain);
    let parent_zone = state.config.parent_zone_name();

    // create zone in sub-PDNS
    let z = PdnsZoneCreate {
        name: zone_name.clone(),
        kind: "Native".into(),
        nameservers: state.config.internal_ns.clone(),
    };
    state.sub_pdns.create_zone(&z).await.map_err(internal)?;

    let sub_zone_rrsets = vec![
        build_apex_ns_rrset(&state.config, &zone_name),
        build_apex_soa_rrset(&state.config, &zone_name),
    ];

    if let Err(err) = state
        .sub_pdns
        .patch_rrsets(&zone_name, &sub_zone_rrsets)
        .await
    {
        cleanup_partial_signup(&state, &parent_zone, &zone_name).await;
        return Err(internal(err));
    }

    // 4) create NS delegation in base-PDNS
    if let Err(err) = state
        .base_pdns
        .patch_rrsets(
            &parent_zone,
            &[build_apex_ns_rrset(&state.config, &zone_name)],
        )
        .await
    {
        cleanup_partial_signup(&state, &parent_zone, &zone_name).await;
        return Err(internal(err));
    }

    // 5) insert into DB
    if let Err(err) = user_repo::insert(&state.db, &req.subdomain, &hash).await {
        cleanup_partial_signup(&state, &parent_zone, &zone_name).await;
        if is_unique_violation(&err) {
            return Err((axum::http::StatusCode::CONFLICT, "already exists".into()));
        }
        return Err(internal(err));
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}

pub(crate) fn internal<E: std::fmt::Debug + std::fmt::Display>(e: E) -> (axum::http::StatusCode, String) {
    tracing::error!("{e:?}");
    (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

/// Credentials used to authenticate an existing subdomain owner.
#[derive(Deserialize)]
pub struct SigninRequest {
    pub subdomain: String,
    pub password: String,
}

/// Authenticate a user against the stored password hash.
pub async fn signin(
    Extension(state): Extension<SharedState>,
    Json(req): Json<SigninRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    use crate::auth::verify_password;
    use crate::db::user_repo;

    let user = user_repo::find_by_subdomain(&state.db, &req.subdomain)
        .await
        .map_err(internal)?
        .ok_or((
            axum::http::StatusCode::UNAUTHORIZED,
            "invalid credentials".into(),
        ))?;

    if !verify_password(&user.password_hash, &req.password).map_err(internal)? {
        return Err((
            axum::http::StatusCode::UNAUTHORIZED,
            "invalid credentials".into(),
        ));
    }

    user_repo::update_last_login(&state.db, user.id)
        .await
        .map_err(internal)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Response indicating whether a requested label may be registered.
#[derive(Serialize)]
pub struct CheckSubdomainResponse {
    available: bool,
}

/// Validate syntax, reservation list, database, and DNS occupancy for a label.
pub async fn check_subdomain(
    Extension(state): Extension<SharedState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<CheckSubdomainResponse>, AppError> {
    let Some(name) = params.get("name") else {
        return Err(AppError::bad_request("missing 'name' parameter"));
    };

    validate_subdomain_name(name).map_err(|e| AppError::BadRequest(e.to_string()))?;

    if state.config.is_disallowed_subdomain(name) {
        return Err(AppError::bad_request("requested subdomain is reserved"));
    }

    let exists = user_repo::exists(&state.db, name)
        .await
        .map_err(AppError::internal)?;

    let dns_exists = dns_label_occupied(&state, name)
        .await
        .map_err(AppError::internal_anyhow)?;

    Ok(Json(CheckSubdomainResponse {
        available: !(exists || dns_exists),
    }))
}

fn is_unique_violation(err: &SqlxError) -> bool {
    match err {
        SqlxError::Database(db_err) => db_err.message().contains("UNIQUE"),
        _ => false,
    }
}

/// Best-effort cleanup if any step of signup fails after DNS writes.
async fn cleanup_partial_signup(state: &SharedState, parent_zone: &str, zone_name: &str) {
    let delete_rrset = PdnsRrset {
        name: zone_name.to_string(),
        rrtype: "NS".into(),
        ttl: 300,
        changetype: Some("DELETE".into()),
        records: Vec::new(),
        comments: Vec::new(),
    };

    let _ = state
        .base_pdns
        .patch_rrsets(parent_zone, &[delete_rrset])
        .await;
    let _ = state.sub_pdns.delete_zone(zone_name).await;
}

/// Public description of the base domain the service manages.
#[derive(Serialize)]
pub struct AboutResponse {
    pub base_domain: String,
}

/// Return the base domain so clients can build FQDNs locally.
pub async fn about(
    Extension(state): Extension<SharedState>,
) -> Result<Json<AboutResponse>, (axum::http::StatusCode, String)> {
    Ok(Json(AboutResponse {
        base_domain: state.config.base_domain_root().to_string(),
    }))
}

/// Grouping of delegated label -> NS targets used on the landing page.
#[derive(Serialize)]
pub struct SubdomainListResponse {
    pub name: String,
    pub records: Vec<String>,
}

/// SOA response for the parent zone shown to unauthenticated users.
#[derive(Serialize)]
pub struct ParentSoaResponse {
    pub soa: String,
}

/// Enumerate all NS delegations under the parent zone.
pub async fn list_ns_records(
    Extension(state): Extension<SharedState>,
) -> Result<Json<Vec<SubdomainListResponse>>, (axum::http::StatusCode, String)> {
    use std::collections::BTreeMap;

    let parent_zone = state.config.parent_zone_name();
    let zone = state
        .base_pdns
        .get_zone(&parent_zone)
        .await
        .map_err(internal)?;

    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if let Some(rrsets) = zone.rrsets {
        for rr in rrsets
            .into_iter()
            .filter(|rr| rr.rrtype.eq_ignore_ascii_case("NS"))
        {
            let entry = map.entry(rr.name).or_default();
            entry.extend(rr.records.into_iter().map(|rec| rec.content));
        }
    }

    let mut grouped = Vec::with_capacity(map.len());
    for (name, records) in map {
        grouped.push(SubdomainListResponse { name, records });
    }

    Ok(Json(grouped))
}

/// Return the parent zone's SOA record so clients can copy/paste it.
pub async fn parent_zone_soa(
    Extension(state): Extension<SharedState>,
) -> Result<Json<ParentSoaResponse>, (axum::http::StatusCode, String)> {
    let parent_zone = state.config.parent_zone_name();
    let zone = state
        .base_pdns
        .get_zone(&parent_zone)
        .await
        .map_err(internal)?;

    if let Some(rrsets) = zone.rrsets {
        for rr in rrsets {
            if rr
                .rrtype
                .eq_ignore_ascii_case("SOA")
                && normalize_dns_name(&rr.name) == normalize_dns_name(&parent_zone)
            {
                if let Some(record) = rr.records.into_iter().next() {
                    return Ok(Json(ParentSoaResponse { soa: record.content }));
                }
            }
        }
    }

    Err((
        axum::http::StatusCode::NOT_FOUND,
        "SOA record not found".into(),
    ))
}

/// Prometheus metrics endpoint exporting subdomain counts.
pub async fn metrics(
    Extension(state): Extension<SharedState>,
) -> Result<impl IntoResponse, (axum::http::StatusCode, String)> {
    let parent_zone = state.config.parent_zone_name();
    let zone = state
        .base_pdns
        .get_zone(&parent_zone)
        .await
        .map_err(internal)?;

    let mut subdomains: BTreeSet<String> = BTreeSet::new();
    if let Some(rrsets) = zone.rrsets {
        for rr in rrsets.into_iter().filter(|rr| rr.rrtype.eq_ignore_ascii_case("NS")) {
            let owner = normalize_dns_name(&rr.name);
            if owner == normalize_dns_name(&parent_zone) {
                continue;
            }
            subdomains.insert(owner);
        }
    }

    let body = format!("satsuki_subdomains_total {}\n", subdomains.len());
    Ok((
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        body,
    ))
}

const NS_TTL: u32 = 300;
const SOA_TTL: u32 = 3600;
const SOA_REFRESH: u32 = 7200;
const SOA_RETRY: u32 = 900;
const SOA_EXPIRE: u32 = 1_209_600;
const SOA_MINIMUM: u32 = 300;

/// Helper to construct the canonical NS RRset for a user zone.
fn build_apex_ns_rrset(config: &AppConfig, zone_name: &str) -> PdnsRrset {
    PdnsRrset {
        name: zone_name.to_string(),
        rrtype: "NS".into(),
        ttl: NS_TTL,
        changetype: Some("REPLACE".into()),
        records: config
            .internal_ns
            .iter()
            .map(|ns| PdnsRecord {
                content: ns.clone(),
                disabled: false,
            })
            .collect(),
        comments: Vec::new(),
    }
}

/// Inspect PowerDNS to determine if the label already has any RRsets.
async fn dns_label_occupied(state: &SharedState, subdomain: &str) -> anyhow::Result<bool> {
    let parent_zone = state.config.parent_zone_name();
    let desired = normalize_dns_name(&state.config.user_zone_name(subdomain));
    let zone = state.base_pdns.get_zone(&parent_zone).await?;

    if let Some(rrsets) = zone.rrsets {
        for rr in rrsets {
            if normalize_dns_name(&rr.name) == desired {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

/// Normalize a DNS name by trimming its trailing dot and lowercasing.
fn normalize_dns_name(name: &str) -> String {
    name.trim_end_matches('.').to_ascii_lowercase()
}

/// Helper to build the authoritative SOA RRset for a user zone.
fn build_apex_soa_rrset(config: &AppConfig, zone_name: &str) -> PdnsRrset {
    let mname = config.internal_main_ns.clone();
    let contact = config.internal_contact.clone();
    let serial = Utc::now().format("%Y%m%d01").to_string();

    let content = format!(
        "{} {} {} {} {} {} {}",
        mname, contact, serial, SOA_REFRESH, SOA_RETRY, SOA_EXPIRE, SOA_MINIMUM
    );

    PdnsRrset {
        name: zone_name.to_string(),
        rrtype: "SOA".into(),
        ttl: SOA_TTL,
        changetype: Some("REPLACE".into()),
        records: vec![PdnsRecord {
            content,
            disabled: false,
        }],
        comments: Vec::new(),
    }
}
