use axum::{
    Router,
    routing::{get, post},
};

use crate::{AppState, auth};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/config", get(auth::config))
        .route("/login", post(auth::login))
        .route("/session", get(auth::current))
        .route("/logout", post(auth::logout))
}
