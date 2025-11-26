// src/api/dns.rs
use super::public::internal;
use crate::powerdns::types::{PdnsRecord, PdnsRrset};
use crate::{SharedState, auth::Authenticated};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, btree_map::Entry};

#[derive(Serialize, Deserialize)]
pub struct RecordDto {
    pub name: String, // relative or FQDN, your choice
    pub rrtype: String,
    pub ttl: u32,
    pub content: String,
    pub priority: Option<u16>, // for MX, SRV if you want
}

// GET /api/zone
pub async fn get_zone(
    Authenticated(user): Authenticated,
    Extension(state): Extension<SharedState>,
) -> Result<Json<Vec<RecordDto>>, (axum::http::StatusCode, String)> {
    let zone_name = state.config.user_zone_name(&user.subdomain);

    let zone = state
        .sub_pdns
        .get_zone(&zone_name)
        .await
        .map_err(internal)?;
    let mut records = Vec::new();

    if let Some(rrsets) = zone.rrsets {
        for rr in rrsets {
            // skip apex NS and SOA; keep these under server control
            if rr.rrtype.eq_ignore_ascii_case("SOA") {
                continue;
            }
            if rr.rrtype.eq_ignore_ascii_case("NS") && rr.name.eq_ignore_ascii_case(&zone_name) {
                continue;
            }
            for rec in rr.records {
                records.push(RecordDto {
                    name: rr.name.clone(), // TODO: normalize to relative if desired
                    rrtype: rr.rrtype.clone(),
                    ttl: rr.ttl,
                    content: rec.content,
                    priority: None, // TODO: parse for MX/SRV if you care
                });
            }
        }
    }

    Ok(Json(records))
}

#[derive(Deserialize)]
pub struct ZoneUpdateRequest {
    pub records: Vec<RecordDto>,
}

// PUT /api/zone
pub async fn put_zone(
    Authenticated(user): Authenticated,
    Extension(state): Extension<SharedState>,
    Json(req): Json<ZoneUpdateRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let zone_name = state.config.user_zone_name(&user.subdomain);

    let mut map: BTreeMap<(String, String), (u32, Vec<PdnsRecord>)> = BTreeMap::new();
    for record in req.records {
        if record.ttl == 0 {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                "ttl must be greater than 0".into(),
            ));
        }

        let owner = normalize_owner(&record.name, &zone_name)
            .map_err(|msg| (axum::http::StatusCode::BAD_REQUEST, msg))?;
        let rrtype = record.rrtype.to_uppercase();

        if rrtype == "SOA" {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                "SOA records are managed automatically and cannot be modified".into(),
            ));
        }

        if rrtype == "NS" && owner.eq_ignore_ascii_case(&zone_name) {
            return Err((
                axum::http::StatusCode::BAD_REQUEST,
                "apex NS records must be managed via NS-mode endpoints".into(),
            ));
        }

        match map.entry((owner.clone(), rrtype.clone())) {
            Entry::Vacant(v) => {
                v.insert((
                    record.ttl,
                    vec![PdnsRecord {
                        content: record.content,
                        disabled: false,
                    }],
                ));
            }
            Entry::Occupied(mut o) => {
                let (ttl, records) = o.get_mut();
                if *ttl != record.ttl {
                    return Err((
                        axum::http::StatusCode::BAD_REQUEST,
                        format!("conflicting TTLs for {} {}", owner, rrtype),
                    ));
                }
                records.push(PdnsRecord {
                    content: record.content,
                    disabled: false,
                });
            }
        }
    }

    let mut rrsets = Vec::new();
    for ((name, rrtype), (ttl, records)) in map {
        rrsets.push(PdnsRrset {
            name,
            rrtype,
            ttl,
            changetype: Some("REPLACE".into()),
            records,
            comments: Vec::new(),
        });
    }

    state
        .sub_pdns
        .patch_rrsets(&zone_name, &rrsets)
        .await
        .map_err(internal)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

fn normalize_owner(name: &str, zone_name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed == "@" {
        return Ok(zone_name.to_string());
    }

    if trimmed.ends_with('.') {
        let owner_lower = trimmed.to_ascii_lowercase();
        let zone_lower = zone_name.to_ascii_lowercase();
        if owner_lower == zone_lower {
            return Ok(trimmed.to_string());
        }
        if owner_lower.ends_with(&zone_lower) {
            let prefix_len = owner_lower.len() - zone_lower.len();
            if prefix_len > 0 && owner_lower.as_bytes()[prefix_len - 1] == b'.' {
                return Ok(trimmed.to_string());
            }
        }
        return Err("record name must be within your zone".into());
    }

    Ok(format!("{}.{}", trimmed.trim_end_matches('.'), zone_name))
}
