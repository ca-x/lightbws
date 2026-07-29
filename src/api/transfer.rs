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
    crypto::MasterKey,
    domain::transfer::{
        ArchiveKind, BackupScopes, ImportSummary, decode_plain_backup, decrypt_backup,
        decrypt_portable, dump_database_scoped, encode_plain_backup, encrypt_portable,
        import_database_scoped, inspect_archive,
    },
    error::AppError,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportInput {
    #[serde(default)]
    passphrase: Option<String>,
    #[serde(default)]
    scopes: BackupScopes,
    #[serde(default)]
    plaintext: bool,
    #[serde(default)]
    confirm_plaintext: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportInput {
    #[serde(default)]
    passphrase: Option<String>,
    #[serde(default)]
    master_key: Option<String>,
    data_base64: String,
    #[serde(default)]
    replace: bool,
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
    input.scopes.validate()?;
    if input.plaintext && (!state.allow_plaintext_backups || !input.confirm_plaintext) {
        return Err(AppError::Validation(
            "plaintext export requires server permission and explicit confirmation".into(),
        ));
    }
    let dump = dump_database_scoped(&state.db, &state.master_key, input.scopes).await?;
    let (payload, filename) = if input.plaintext {
        (
            encode_plain_backup(&dump)?,
            format!(
                "lightbws-{}.plain.lightbws",
                time::OffsetDateTime::now_utc().date()
            ),
        )
    } else {
        let passphrase = input
            .passphrase
            .as_deref()
            .ok_or_else(|| AppError::Validation("export passphrase is required".into()))?;
        (
            encrypt_portable(passphrase, &dump)?,
            format!(
                "lightbws-{}.lightbws",
                time::OffsetDateTime::now_utc().date()
            ),
        )
    };
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
    let _restore_permit = if input.replace {
        Some(
            state
                .backup_permits
                .acquire()
                .await
                .map_err(AppError::internal)?,
        )
    } else {
        None
    };
    let envelope = STANDARD
        .decode(input.data_base64)
        .map_err(|_| AppError::Validation("invalid import encoding".into()))?;
    let plaintext =
        match inspect_archive(&envelope)? {
            ArchiveKind::Passphrase => decrypt_portable(
                input
                    .passphrase
                    .as_deref()
                    .ok_or_else(|| AppError::Validation("export passphrase is required".into()))?,
                &envelope,
            )?,
            ArchiveKind::MasterKey => {
                let source_key =
                    MasterKey::parse(input.master_key.as_deref().ok_or_else(|| {
                        AppError::Validation("source master key is required".into())
                    })?)
                    .map_err(|_| AppError::Validation("source master key is invalid".into()))?;
                decrypt_backup(&source_key, &envelope)?
            }
            ArchiveKind::Plaintext => decode_plain_backup(&envelope)?,
        };
    let imported = import_database_scoped(
        &state.db,
        &state.master_key,
        &plaintext,
        input.replace,
        state.allow_plaintext_backups,
    )
    .await?;
    Ok(Json(ImportResponse { imported }))
}
