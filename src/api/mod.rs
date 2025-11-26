
pub mod public;
pub mod dns;
pub mod profile;

use axum::{Router, routing::{get, post}, Extension};
use crate::SharedState;

pub fn create_router(state: SharedState) -> Router {
    use crate::api::{public, dns, profile};

    Router::new()
        // public
        .route("/api/signup", post(public::signup))
        .route("/api/signin", post(public::signin))
        .route("/api/subdomain/check", get(public::check_subdomain))
        // authenticated
        .route("/api/zone", get(dns::get_zone).put(dns::put_zone))
        .route("/api/ns-mode/internal", post(profile::set_ns_internal))
        .route("/api/ns-mode/external", post(profile::set_ns_external))
        .route("/api/profile", get(profile::get_profile))
        .layer(Extension(state))
}
