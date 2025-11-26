
pub mod powerdns;
pub mod config;
pub mod db;
pub mod api;
pub mod auth;
pub mod validation;
pub mod error;

use powerdns::client::PowerDnsClient;
use db::Db;
use config::AppConfig;

use std::sync::Arc;

pub struct AppState {
    pub config: AppConfig,
    pub db: Db,
    pub base_pdns: PowerDnsClient,
    pub sub_pdns: PowerDnsClient,
}

pub type SharedState = Arc<AppState>;
