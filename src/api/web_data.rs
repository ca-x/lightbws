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
    auth::{AuthenticatedSession, MutationSession, require_admin},
    domain::{
        access::AccessRepository,
        audit::{AuditActor, AuditRepository},
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
    project_id: Uuid,
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
    session: AuthenticatedSession,
) -> Result<Json<Overview>, AppError> {
    let access = AccessRepository::new(state.db.clone());
    let mut projects = 0;
    for project in ProjectRepository::new(state.db.clone()).list(false).await? {
        let id = Uuid::parse_str(&project.id).map_err(AppError::internal)?;
        if access
            .user_project(session.user_id, session.user.role, id)
            .await?
            .read
        {
            projects += 1;
        }
    }
    let secrets_repository = SecretRepository::new(state.db);
    let mut secrets = 0;
    let mut trash = 0;
    for secret in secrets_repository.list(true, None).await? {
        if access
            .user_secret(session.user_id, session.user.role, &secret)
            .await?
            .read
        {
            if secret.deleted_at.is_some() {
                trash += 1;
            } else {
                secrets += 1;
            }
        }
    }
    Ok(Json(Overview {
        projects,
        secrets,
        trash,
    }))
}

async fn list_projects(
    State(state): State<AppState>,
    session: AuthenticatedSession,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<WebProject>>, AppError> {
    let access = AccessRepository::new(state.db.clone());
    let mut projects = Vec::new();
    for project in ProjectRepository::new(state.db).list(query.trash).await? {
        if (!query.trash && project.deleted_at.is_some())
            || (query.trash && project.deleted_at.is_none())
        {
            continue;
        }
        let id = Uuid::parse_str(&project.id).map_err(AppError::internal)?;
        let permissions = access
            .user_project(session.user_id, session.user.role, id)
            .await?;
        if permissions.read {
            let mut view = WebProject::try_from(project)?;
            view.permissions = permissions;
            projects.push(view);
        }
    }
    Ok(Json(projects))
}

async fn create_project(
    State(state): State<AppState>,
    mutation: MutationSession,
    Json(input): Json<ProjectInput>,
) -> Result<(StatusCode, Json<WebProject>), AppError> {
    require_admin(&mutation.0.user)?;
    let project = ProjectRepository::new(state.db.clone())
        .create_plain(&input.name)
        .await?;
    record_user_event(
        &state,
        mutation.0.user_id,
        "project.create",
        "project",
        project.id,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(project)))
}

async fn update_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
    Json(input): Json<ProjectInput>,
) -> Result<Json<WebProject>, AppError> {
    require_admin(&mutation.0.user)?;
    let project = ProjectRepository::new(state.db.clone())
        .update_plain(id, &input.name)
        .await?;
    record_user_event(&state, mutation.0.user_id, "project.update", "project", id).await?;
    Ok(Json(project))
}

async fn trash_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
) -> Result<StatusCode, AppError> {
    require_admin(&mutation.0.user)?;
    ProjectRepository::new(state.db.clone())
        .set_deleted(&[id], true)
        .await?;
    record_user_event(&state, mutation.0.user_id, "project.trash", "project", id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn restore_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
) -> Result<StatusCode, AppError> {
    require_admin(&mutation.0.user)?;
    ProjectRepository::new(state.db.clone())
        .set_deleted(&[id], false)
        .await?;
    record_user_event(&state, mutation.0.user_id, "project.restore", "project", id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn purge_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
) -> Result<StatusCode, AppError> {
    require_admin(&mutation.0.user)?;
    ProjectRepository::new(state.db.clone()).purge(id).await?;
    record_user_event(&state, mutation.0.user_id, "project.purge", "project", id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_secrets(
    State(state): State<AppState>,
    session: AuthenticatedSession,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<WebSecret>>, AppError> {
    let access = AccessRepository::new(state.db.clone());
    let mut secrets = Vec::new();
    for secret in SecretRepository::new(state.db)
        .list(query.trash, query.project_id)
        .await?
    {
        if (!query.trash && secret.deleted_at.is_some())
            || (query.trash && secret.deleted_at.is_none())
        {
            continue;
        }
        let permissions = access
            .user_secret(session.user_id, session.user.role, &secret)
            .await?;
        if permissions.read {
            let mut view = WebSecret::try_from(secret)?;
            view.permissions = permissions;
            secrets.push(view);
        }
    }
    Ok(Json(secrets))
}

async fn get_secret(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: AuthenticatedSession,
) -> Result<Json<WebSecret>, AppError> {
    let model = SecretRepository::new(state.db.clone()).get(id).await?;
    let permissions = AccessRepository::new(state.db.clone())
        .user_secret(session.user_id, session.user.role, &model)
        .await?;
    permissions.require_read()?;
    let mut view = WebSecret::try_from(model)?;
    view.permissions = permissions;
    record_user_event(&state, session.user_id, "secret.read", "secret", id).await?;
    Ok(Json(view))
}

async fn create_secret(
    State(state): State<AppState>,
    mutation: MutationSession,
    Json(input): Json<SecretInput>,
) -> Result<(StatusCode, Json<WebSecret>), AppError> {
    let permissions = AccessRepository::new(state.db.clone())
        .user_project(mutation.0.user_id, mutation.0.user.role, input.project_id)
        .await?;
    permissions.require_write()?;
    let mut secret = SecretRepository::new(state.db.clone())
        .create_plain(&input.key, &input.value, &input.note, input.project_id)
        .await?;
    secret.permissions = permissions;
    record_user_event(
        &state,
        mutation.0.user_id,
        "secret.create",
        "secret",
        secret.id,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(secret)))
}

async fn update_secret(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
    Json(input): Json<SecretInput>,
) -> Result<Json<WebSecret>, AppError> {
    let repository = SecretRepository::new(state.db.clone());
    let existing = repository.get(id).await?;
    let access = AccessRepository::new(state.db.clone());
    access
        .user_secret(mutation.0.user_id, mutation.0.user.role, &existing)
        .await?
        .require_write()?;
    let target = if existing.project_id == input.project_id.to_string() {
        access
            .user_secret(mutation.0.user_id, mutation.0.user.role, &existing)
            .await?
    } else {
        let target = access
            .user_project(mutation.0.user_id, mutation.0.user.role, input.project_id)
            .await?;
        target.require_write()?;
        target
    };
    let mut secret = repository
        .update_plain(id, &input.key, &input.value, &input.note, input.project_id)
        .await?;
    secret.permissions = target;
    record_user_event(&state, mutation.0.user_id, "secret.update", "secret", id).await?;
    Ok(Json(secret))
}

async fn trash_secret(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
) -> Result<StatusCode, AppError> {
    require_secret_write(&state, &mutation, id).await?;
    SecretRepository::new(state.db.clone())
        .set_deleted(&[id], true)
        .await?;
    record_user_event(&state, mutation.0.user_id, "secret.trash", "secret", id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn restore_secret(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
) -> Result<StatusCode, AppError> {
    require_secret_write(&state, &mutation, id).await?;
    SecretRepository::new(state.db.clone())
        .set_deleted(&[id], false)
        .await?;
    record_user_event(&state, mutation.0.user_id, "secret.restore", "secret", id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn purge_secret(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
) -> Result<StatusCode, AppError> {
    require_secret_write(&state, &mutation, id).await?;
    SecretRepository::new(state.db.clone()).purge(id).await?;
    record_user_event(&state, mutation.0.user_id, "secret.purge", "secret", id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn require_secret_write(
    state: &AppState,
    mutation: &MutationSession,
    id: Uuid,
) -> Result<(), AppError> {
    let model = SecretRepository::new(state.db.clone()).get(id).await?;
    AccessRepository::new(state.db.clone())
        .user_secret(mutation.0.user_id, mutation.0.user.role, &model)
        .await?
        .require_write()
}

async fn record_user_event(
    state: &AppState,
    user_id: Uuid,
    action: &str,
    resource_kind: &str,
    resource_id: Uuid,
) -> Result<(), AppError> {
    AuditRepository::new(state.db.clone())
        .record(
            AuditActor::User(user_id),
            action,
            resource_kind,
            Some(resource_id),
            "allowed",
        )
        .await
}
