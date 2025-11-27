//! Crate entrypoint wiring together configuration, DB, PowerDNS, and APIs.

pub mod api;
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod powerdns;
pub mod validation;

use config::AppConfig;
use db::Db;
use powerdns::client::PowerDnsClient;

use std::sync::Arc;

/// Complete application dependencies shared across handlers.
pub struct AppState {
    pub config: AppConfig,
    pub db: Db,
    pub base_pdns: PowerDnsClient,
    pub sub_pdns: PowerDnsClient,
}

/// Arc-wrapped version of `AppState` passed into Axum extensions.
pub type SharedState = Arc<AppState>;
