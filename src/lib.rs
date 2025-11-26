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

pub struct AppState {
    pub config: AppConfig,
    pub db: Db,
    pub base_pdns: PowerDnsClient,
    pub sub_pdns: PowerDnsClient,
}

pub type SharedState = Arc<AppState>;
