use axum::{
    Json, Router,
    body::Body,
    http::{HeaderValue, Response, StatusCode, header},
    routing::post,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    auth::{MutationSession, require_admin},
    domain::transfer::{
        ImportSummary, decrypt_portable, dump_database, encrypt_portable, import_database,
    },
    error::AppError,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportInput {
    passphrase: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportInput {
    passphrase: String,
    data_base64: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportResponse {
    imported: ImportSummary,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/export", post(export))
        .route("/import", post(import))
}

async fn export(
    axum::extract::State(state): axum::extract::State<AppState>,
    session: MutationSession,
    Json(input): Json<ExportInput>,
) -> Result<Response<Body>, AppError> {
    require_admin(&session.0.user)?;
    let payload = encrypt_portable(&input.passphrase, &dump_database(&state.db).await?)?;
    let filename = format!(
        "lightbws-{}.lightbws",
        time::OffsetDateTime::now_utc().date()
    );
    let mut response = Response::new(Body::from(payload));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/vnd.lightbws.backup"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            .map_err(AppError::internal)?,
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn import(
    axum::extract::State(state): axum::extract::State<AppState>,
    session: MutationSession,
    Json(input): Json<ImportInput>,
) -> Result<Json<ImportResponse>, AppError> {
    require_admin(&session.0.user)?;
    let envelope = STANDARD
        .decode(input.data_base64)
        .map_err(|_| AppError::Validation("invalid import encoding".into()))?;
    let plaintext = decrypt_portable(&input.passphrase, &envelope)?;
    let imported = import_database(&state.db, &plaintext).await?;
    Ok(Json(ImportResponse { imported }))
}
