pub mod access;
pub mod admin;
pub mod auth;
pub mod backups;
pub mod sdk;
pub mod transfer;
pub mod web_data;

use axum::Router;

use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::routes())
        .nest("/admin", admin::routes())
        .nest("/backups", backups::routes())
        .nest("/transfer", transfer::routes())
        .merge(access::web_routes())
        .merge(web_data::routes())
}
