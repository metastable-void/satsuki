// src/api/dns.rs
use axum::{Json, Extension};
use serde::{Deserialize, Serialize};
use crate::{SharedState, auth::Authenticated};
use crate::powerdns::types::{PdnsRrset, PdnsRecord};
use super::public::internal;

#[derive(Serialize, Deserialize)]
pub struct RecordDto {
    pub name: String,   // relative or FQDN, your choice
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
    let zone_name = format!("{}.{}.", user.subdomain, state.config.base_domain);

    let zone = state.sub_pdns.get_zone(&zone_name).await.map_err(internal)?;
    let mut records = Vec::new();

    if let Some(rrsets) = zone.rrsets {
        for rr in rrsets {
            // skip apex NS; keep these under NS-mode control
            if rr.rrtype == "NS" && rr.name == zone_name {
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
    let zone_name = format!("{}.{}.", user.subdomain, state.config.base_domain);

    // transform DTOs into rrsets grouped by (name, type)
    use std::collections::BTreeMap;
    let mut map: BTreeMap<(String, String), Vec<RecordDto>> = BTreeMap::new();
    for r in req.records {
        let key = (r.name.clone(), r.rrtype.clone());
        map.entry(key).or_default().push(r);
    }

    let mut rrsets = Vec::new();
    for ((name, rrtype), recs) in map {
        let pdns_records = recs.into_iter()
            .map(|r| PdnsRecord { content: r.content, disabled: false })
            .collect();

        rrsets.push(PdnsRrset {
            name,
            rrtype,
            ttl: 300, // or choose per-record; for now constant
            changetype: Some("REPLACE".into()),
            records: pdns_records,
        });
    }

    state.sub_pdns
        .patch_rrsets(&zone_name, &rrsets)
        .await
        .map_err(internal)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}
