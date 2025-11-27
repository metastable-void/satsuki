//! serde-compatible types mirroring PowerDNS JSON payloads.
use serde::{Deserialize, Serialize};

/// Zone representation returned by the PDNS API.
#[derive(Debug, Serialize, Deserialize)]
pub struct PdnsZone {
    pub id: String,   // "/api/.../zones/example.com."
    pub name: String, // "example.com."
    #[serde(rename = "type", default)]
    pub zone_type: Option<String>, // "Zone"
    pub kind: String, // "Native", etc.
    pub rrsets: Option<Vec<PdnsRrset>>,
}

/// RRset representation for records inside a zone.
#[derive(Debug, Serialize, Deserialize)]
pub struct PdnsRrset {
    pub name: String, // "www.example.com."
    #[serde(rename = "type")]
    pub rrtype: String, // "A", "NS", ...
    pub ttl: u32,
    pub changetype: Option<String>, // "REPLACE" / "DELETE" when patching
    pub records: Vec<PdnsRecord>,
    #[serde(default)]
    pub comments: Vec<PdnsComment>,
}

/// Individual record content/flags stored inside an RRset.
#[derive(Debug, Serialize, Deserialize)]
pub struct PdnsRecord {
    pub content: String, // "192.0.2.1" or "ns1.example.net."
    #[serde(default)]
    pub disabled: bool,
}

/// Metadata comment attached to an RRset.
#[derive(Debug, Serialize, Deserialize)]
pub struct PdnsComment {
    pub content: String,
    pub account: String,
    pub modified_at: String,
}

/// Payload accepted by PDNS when creating a zone.
#[derive(Debug, Serialize, Deserialize)]
pub struct PdnsZoneCreate {
    pub name: String,             // "sub.base.example.com."
    pub kind: String,             // "Native"
    pub nameservers: Vec<String>, // ["ns1.example.net.", "ns2.example.net."]
}
