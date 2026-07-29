use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    AppState,
    auth::{AuthenticatedSession, MutationSession, require_admin},
    domain::backups::{
        BackupJob, BackupRepository, BackupTarget, CreateBackupTarget, UpdateBackupTarget,
        run_backup, test_target,
    },
    error::AppError,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/targets", get(list_targets).post(create_target))
        .route(
            "/targets/{id}",
            axum::routing::put(update_target).delete(delete_target),
        )
        .route("/targets/{id}/test", axum::routing::post(test))
        .route("/targets/{id}/run", axum::routing::post(run))
        .route("/jobs", get(list_jobs))
        .route("/capabilities", get(capabilities))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BackupCapabilities {
    plaintext_allowed: bool,
}

async fn capabilities(
    State(state): State<AppState>,
    session: AuthenticatedSession,
) -> Result<Json<BackupCapabilities>, AppError> {
    require_admin(&session.user)?;
    Ok(Json(BackupCapabilities {
        plaintext_allowed: state.allow_plaintext_backups,
    }))
}

async fn list_targets(
    State(state): State<AppState>,
    session: AuthenticatedSession,
) -> Result<Json<Vec<BackupTarget>>, AppError> {
    require_admin(&session.user)?;
    Ok(Json(repository(&state).list_targets().await?))
}

async fn create_target(
    State(state): State<AppState>,
    session: MutationSession,
    Json(input): Json<CreateBackupTarget>,
) -> Result<(StatusCode, Json<BackupTarget>), AppError> {
    require_admin(&session.0.user)?;
    Ok((
        StatusCode::CREATED,
        Json(repository(&state).create_target(input).await?),
    ))
}

async fn update_target(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: MutationSession,
    Json(input): Json<UpdateBackupTarget>,
) -> Result<Json<BackupTarget>, AppError> {
    require_admin(&session.0.user)?;
    Ok(Json(repository(&state).update_target(id, input).await?))
}

async fn delete_target(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: MutationSession,
) -> Result<StatusCode, AppError> {
    require_admin(&session.0.user)?;
    repository(&state).delete_target(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn test(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: MutationSession,
) -> Result<StatusCode, AppError> {
    require_admin(&session.0.user)?;
    test_target(&state, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: MutationSession,
) -> Result<Json<BackupJob>, AppError> {
    require_admin(&session.0.user)?;
    Ok(Json(run_backup(&state, id, "manual").await?))
}

async fn list_jobs(
    State(state): State<AppState>,
    session: AuthenticatedSession,
) -> Result<Json<Vec<BackupJob>>, AppError> {
    require_admin(&session.user)?;
    Ok(Json(repository(&state).list_jobs().await?))
}

fn repository(state: &AppState) -> BackupRepository {
    BackupRepository::for_state(state)
}
