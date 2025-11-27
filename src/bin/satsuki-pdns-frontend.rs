use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use anyhow::{Context, Result, bail};
use axum::{
    Router,
    body::Body,
    extract::OriginalUri,
    http::{Method, Response, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use clap::Parser;
use rust_embed::RustEmbed;
use satsuki::{
    AppState, SharedState, api, config::AppConfig, db, powerdns::client::PowerDnsClient,
};
use tokio::{net::TcpListener, signal};
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(author, version, about, rename_all = "kebab-case")]
struct Cli {
    /// Base domain (e.g. example.com)
    #[arg(long, value_name = "DOMAIN")]
    base_domain: String,
    /// Path to the SQLite database file
    #[arg(long, value_name = "PATH")]
    db_path: PathBuf,
    /// Listen address for the HTTP server
    #[arg(long, value_name = "ADDR", default_value = "0.0.0.0:8080")]
    listen: SocketAddr,
    /// Base PowerDNS API URL
    #[arg(long, value_name = "URL")]
    base_pdns_url: String,
    /// Base PowerDNS API key
    #[arg(long, value_name = "KEY")]
    base_pdns_key: String,
    /// Base PowerDNS server ID
    #[arg(long, value_name = "ID", default_value = "localhost")]
    base_pdns_server_id: String,
    /// Subdomain PowerDNS API URL
    #[arg(long, value_name = "URL")]
    sub_pdns_url: String,
    /// Subdomain PowerDNS API key
    #[arg(long, value_name = "KEY")]
    sub_pdns_key: String,
    /// Subdomain PowerDNS server ID
    #[arg(long, value_name = "ID", default_value = "localhost")]
    sub_pdns_server_id: String,
    /// Internal nameserver FQDN (repeat for multiple values)
    #[arg(long = "internal-ns", value_name = "FQDN", required = true)]
    internal_ns: Vec<String>,
    /// Override for SOA mname value (defaults to first internal NS)
    #[arg(long, value_name = "FQDN")]
    internal_main_ns: Option<String>,
    /// Override for SOA rname/contact (defaults to hostmaster.<base-domain>.)
    #[arg(long, value_name = "FQDN")]
    internal_contact: Option<String>,
    /// Additional reserved subdomain labels
    #[arg(long = "disallow-subdomain", value_name = "LABEL")]
    disallow_subdomain: Vec<String>,
}

#[derive(RustEmbed)]
#[folder = "dist"]
struct EmbeddedDist;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let cli = Cli::parse();
    let config = build_app_config(&cli)?;
    let state = init_shared_state(&cli, config).await?;

    let spa_routes = get(frontend_handler).head(frontend_handler);
    let app = Router::new()
        .merge(api::create_router(state))
        .route("/", spa_routes.clone())
        .route("/{*path}", spa_routes);

    let listener = TcpListener::bind(cli.listen)
        .await
        .with_context(|| format!("failed to bind to {}", cli.listen))?;

    info!("listening on http://{}", listener.local_addr()?);

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server exited with error")?;

    Ok(())
}

async fn init_shared_state(cli: &Cli, config: AppConfig) -> Result<SharedState> {
    if let Some(parent) = cli.db_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create db directory {}", parent.display()))?;
    }

    let db = db::init_db(&cli.db_path).await?;
    let base_pdns = PowerDnsClient::new(
        &cli.base_pdns_url,
        &cli.base_pdns_key,
        &cli.base_pdns_server_id,
    );
    let sub_pdns = PowerDnsClient::new(
        &cli.sub_pdns_url,
        &cli.sub_pdns_key,
        &cli.sub_pdns_server_id,
    );

    Ok(Arc::new(AppState {
        config,
        db,
        base_pdns,
        sub_pdns,
    }))
}

fn build_app_config(cli: &Cli) -> Result<AppConfig> {
    if cli.internal_ns.is_empty() {
        bail!("at least one --internal-ns value is required");
    }

    let internal_ns = cli
        .internal_ns
        .iter()
        .map(|ns| normalize_fqdn(ns).with_context(|| format!("invalid internal-ns value '{ns}'")))
        .collect::<Result<Vec<_>>>()?;

    let internal_main_ns = match &cli.internal_main_ns {
        Some(value) => {
            normalize_fqdn(value).with_context(|| format!("invalid internal-main-ns '{value}'"))?
        }
        None => internal_ns
            .first()
            .cloned()
            .expect("internal_ns already validated"),
    };

    let default_contact = format!("hostmaster.{}", cli.base_domain.trim_end_matches('.'));
    let internal_contact_source = cli
        .internal_contact
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or(&default_contact);
    let internal_contact = normalize_fqdn(internal_contact_source)
        .with_context(|| format!("invalid internal-contact '{}'", internal_contact_source))?;

    let disallowed_subdomains = cli
        .disallow_subdomain
        .iter()
        .map(|label| label.trim().to_ascii_lowercase())
        .filter(|label| !label.is_empty())
        .collect();

    Ok(AppConfig {
        base_domain: cli.base_domain.trim_end_matches('.').to_string(),
        internal_ns,
        internal_main_ns,
        internal_contact,
        disallowed_subdomains,
    })
}

fn normalize_fqdn(input: &str) -> Result<String> {
    let trimmed = input.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        bail!("FQDN cannot be empty");
    }
    Ok(format!("{}.", trimmed))
}

async fn shutdown_signal() {
    if let Err(err) = signal::ctrl_c().await {
        error!("failed to install CTRL+C handler: {err}");
    }
    info!("shutdown signal received");
}

async fn frontend_handler(method: Method, OriginalUri(uri): OriginalUri) -> impl IntoResponse {
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }

    let path = uri.path().trim_start_matches('/');
    if path.contains("..") {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let candidate = if path.is_empty() { "index.html" } else { path };
    if let Some(resp) = embedded_response(candidate, &method) {
        return resp;
    }
    if let Some(resp) = embedded_response("index.html", &method) {
        return resp;
    }

    StatusCode::NOT_FOUND.into_response()
}

fn embedded_response(path: &str, method: &Method) -> Option<Response<Body>> {
    let asset = EmbeddedDist::get(path)?;
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from(asset.data.into_owned())
    };
    let mime = mime_guess::from_path(path).first_or_octet_stream();

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(
            header::CACHE_CONTROL,
            if path == "index.html" {
                "no-cache"
            } else {
                "public, max-age=31536000, immutable"
            },
        )
        .header(
            header::CONTENT_SECURITY_POLICY,
            "default-src 'self'; base-uri 'self'; frame-ancestors 'none'; form-action 'self'; \
             script-src 'self'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; \
             img-src 'self' data:; font-src 'self' https://fonts.gstatic.com; connect-src 'self'; \
             object-src 'none'; upgrade-insecure-requests",
        )
        .header(header::REFERRER_POLICY, "no-referrer")
        .body(body)
        .ok()
}

fn init_tracing() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,tower_http=info".into());
    tracing_subscriber::fmt().with_env_filter(env_filter).init();
}
