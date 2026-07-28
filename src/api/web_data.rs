use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState,
    auth::{AuthenticatedSession, MutationSession},
    domain::{
        projects::{ProjectRepository, WebProject},
        secrets::{SecretRepository, WebSecret},
    },
    error::AppError,
};

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    trash: bool,
    #[serde(rename = "projectId")]
    project_id: Option<Uuid>,
}

#[derive(Deserialize)]
struct ProjectInput {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SecretInput {
    key: String,
    value: String,
    #[serde(default)]
    note: String,
    project_id: Option<Uuid>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Overview {
    projects: usize,
    secrets: usize,
    trash: usize,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/overview", get(overview))
        .route("/projects", get(list_projects).post(create_project))
        .route(
            "/projects/{id}",
            axum::routing::put(update_project).delete(trash_project),
        )
        .route(
            "/projects/{id}/restore",
            axum::routing::put(restore_project),
        )
        .route("/projects/{id}/purge", axum::routing::delete(purge_project))
        .route("/secrets", get(list_secrets).post(create_secret))
        .route(
            "/secrets/{id}",
            get(get_secret).put(update_secret).delete(trash_secret),
        )
        .route("/secrets/{id}/restore", axum::routing::put(restore_secret))
        .route("/secrets/{id}/purge", axum::routing::delete(purge_secret))
}

async fn overview(
    State(state): State<AppState>,
    _session: AuthenticatedSession,
) -> Result<Json<Overview>, AppError> {
    let projects = ProjectRepository::new(state.db.clone())
        .list(false)
        .await?
        .len();
    let secrets_repository = SecretRepository::new(state.db);
    let secrets = secrets_repository.list(false, None).await?.len();
    let trash = secrets_repository
        .list(true, None)
        .await?
        .into_iter()
        .filter(|secret| secret.deleted_at.is_some())
        .count();
    Ok(Json(Overview {
        projects,
        secrets,
        trash,
    }))
}

async fn list_projects(
    State(state): State<AppState>,
    _session: AuthenticatedSession,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<WebProject>>, AppError> {
    let projects = ProjectRepository::new(state.db)
        .list(query.trash)
        .await?
        .into_iter()
        .filter(|project| query.trash || project.deleted_at.is_none())
        .filter(|project| !query.trash || project.deleted_at.is_some())
        .map(WebProject::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(projects))
}

async fn create_project(
    State(state): State<AppState>,
    _mutation: MutationSession,
    Json(input): Json<ProjectInput>,
) -> Result<(StatusCode, Json<WebProject>), AppError> {
    Ok((
        StatusCode::CREATED,
        Json(
            ProjectRepository::new(state.db)
                .create_plain(&input.name)
                .await?,
        ),
    ))
}

async fn update_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    _mutation: MutationSession,
    Json(input): Json<ProjectInput>,
) -> Result<Json<WebProject>, AppError> {
    Ok(Json(
        ProjectRepository::new(state.db)
            .update_plain(id, &input.name)
            .await?,
    ))
}

async fn trash_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    _mutation: MutationSession,
) -> Result<StatusCode, AppError> {
    ProjectRepository::new(state.db)
        .set_deleted(&[id], true)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn restore_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    _mutation: MutationSession,
) -> Result<StatusCode, AppError> {
    ProjectRepository::new(state.db)
        .set_deleted(&[id], false)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn purge_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    _mutation: MutationSession,
) -> Result<StatusCode, AppError> {
    ProjectRepository::new(state.db).purge(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_secrets(
    State(state): State<AppState>,
    _session: AuthenticatedSession,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<WebSecret>>, AppError> {
    let secrets = SecretRepository::new(state.db)
        .list(query.trash, query.project_id)
        .await?
        .into_iter()
        .filter(|secret| query.trash || secret.deleted_at.is_none())
        .filter(|secret| !query.trash || secret.deleted_at.is_some())
        .map(WebSecret::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(secrets))
}

async fn get_secret(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    _session: AuthenticatedSession,
) -> Result<Json<WebSecret>, AppError> {
    Ok(Json(WebSecret::try_from(
        SecretRepository::new(state.db).get(id).await?,
    )?))
}

async fn create_secret(
    State(state): State<AppState>,
    _mutation: MutationSession,
    Json(input): Json<SecretInput>,
) -> Result<(StatusCode, Json<WebSecret>), AppError> {
    Ok((
        StatusCode::CREATED,
        Json(
            SecretRepository::new(state.db)
                .create_plain(&input.key, &input.value, &input.note, input.project_id)
                .await?,
        ),
    ))
}

async fn update_secret(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    _mutation: MutationSession,
    Json(input): Json<SecretInput>,
) -> Result<Json<WebSecret>, AppError> {
    Ok(Json(
        SecretRepository::new(state.db)
            .update_plain(id, &input.key, &input.value, &input.note, input.project_id)
            .await?,
    ))
}

async fn trash_secret(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    _mutation: MutationSession,
) -> Result<StatusCode, AppError> {
    SecretRepository::new(state.db)
        .set_deleted(&[id], true)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn restore_secret(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    _mutation: MutationSession,
) -> Result<StatusCode, AppError> {
    SecretRepository::new(state.db)
        .set_deleted(&[id], false)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn purge_secret(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    _mutation: MutationSession,
) -> Result<StatusCode, AppError> {
    SecretRepository::new(state.db).purge(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
