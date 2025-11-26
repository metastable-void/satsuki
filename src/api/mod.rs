pub mod dns;
pub mod profile;
pub mod public;

use crate::SharedState;
use axum::{
    Extension, Router,
    routing::{get, post},
};
use tower_http::cors::{Any, CorsLayer};

pub fn create_router(state: SharedState) -> Router {
    use crate::api::{dns, profile, public};

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

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
        .layer(cors)
        .layer(Extension(state))
}
