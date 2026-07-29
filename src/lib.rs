pub mod api;
pub mod auth;
pub mod config;
pub mod crypto;
pub mod db;
pub mod domain;
pub mod error;
mod sdk_crypto;
pub mod sdk_models;
pub mod web;

use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::Instant,
};

use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{HeaderName, HeaderValue},
    routing::get,
};
use tokio::sync::{Mutex, Semaphore};
use tower_http::{
    catch_panic::CatchPanicLayer, compression::CompressionLayer, limit::RequestBodyLimitLayer,
    set_header::SetResponseHeaderLayer, trace::TraceLayer,
};

use crate::crypto::MasterKey;
use crate::{config::Config, db::Database};

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub cookie_secure: bool,
    pub(crate) login_failures: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
    pub backup_permits: Arc<Semaphore>,
    pub master_key: MasterKey,
    pub allow_plaintext_backups: bool,
}

impl AppState {
    pub fn new(db: Database, config: &Config) -> Self {
        Self {
            db,
            cookie_secure: config.cookie_secure,
            login_failures: Arc::new(Mutex::new(HashMap::new())),
            backup_permits: Arc::new(Semaphore::new(1)),
            master_key: MasterKey::random().expect("operating system random source"),
            allow_plaintext_backups: config.allow_plaintext_backups,
        }
    }

    pub fn with_master_key(mut self, master_key: MasterKey) -> Self {
        self.master_key = master_key;
        self
    }
}

pub fn create_app(state: AppState) -> Router {
    let csp = HeaderValue::from_static(
        "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; font-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'; form-action 'self'",
    );
    let limited_routes = Router::new()
        .nest("/api/v1", api::routes())
        .merge(api::sdk::routes())
        .merge(api::access::sdk_routes())
        .route("/health", get(api::sdk::health))
        .route("/help", get(api::sdk::help))
        .fallback(web::serve)
        .layer(RequestBodyLimitLayer::new(3 * 1024 * 1024));

    limited_routes
        .nest(
            "/api/v1/transfer",
            // Automatic archives contain an inner Base64 envelope and the JSON API
            // Base64-encodes the file again. 128 MiB covers a 64 MiB logical dump.
            api::transfer::routes().layer(DefaultBodyLimit::max(128 * 1024 * 1024)),
        )
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::CONTENT_SECURITY_POLICY,
            csp,
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
        ))
        .layer(CompressionLayer::new())
        .layer(CatchPanicLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
