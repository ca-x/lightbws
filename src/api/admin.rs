use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    AppState,
    auth::{MutationSession, require_admin, revoke_user_sessions},
    domain::{
        machines::{IssuedMachineAccount, MachineAccount, MachineRepository},
        users::{PublicUser, Role, UserRepository},
    },
    error::AppError,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateUserInput {
    username: String,
    display_name: String,
    role: Role,
    password: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateUserInput {
    display_name: String,
    role: Role,
    disabled: bool,
}

#[derive(Deserialize)]
struct PasswordInput {
    password: String,
}

#[derive(Deserialize)]
struct CreateMachineInput {
    name: String,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/users", get(list_users).post(create_user))
        .route("/users/{id}", axum::routing::put(update_user))
        .route("/users/{id}/password", axum::routing::put(reset_password))
        .route("/machines", get(list_machines).post(create_machine))
        .route("/machines/{id}/revoke", axum::routing::put(revoke_machine))
        .route(
            "/machines/{id}/restore",
            axum::routing::put(restore_machine),
        )
        .route("/machines/{id}", axum::routing::delete(delete_machine))
}

async fn list_users(
    State(state): State<AppState>,
    mutation: crate::auth::AuthenticatedSession,
) -> Result<Json<Vec<PublicUser>>, AppError> {
    require_admin(&mutation.user)?;
    Ok(Json(UserRepository::new(state.db).list().await?))
}

async fn create_user(
    State(state): State<AppState>,
    mutation: MutationSession,
    Json(input): Json<CreateUserInput>,
) -> Result<(StatusCode, Json<PublicUser>), AppError> {
    require_admin(&mutation.0.user)?;
    let user = UserRepository::new(state.db)
        .create(
            &input.username,
            &input.display_name,
            input.role,
            &input.password,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(user)))
}

async fn update_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
    Json(input): Json<UpdateUserInput>,
) -> Result<Json<PublicUser>, AppError> {
    require_admin(&mutation.0.user)?;
    let updated = UserRepository::new(state.db.clone())
        .update(id, &input.display_name, input.role, input.disabled)
        .await?;
    if input.disabled {
        revoke_user_sessions(&state, id).await?;
    }
    Ok(Json(updated))
}

async fn reset_password(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
    Json(input): Json<PasswordInput>,
) -> Result<StatusCode, AppError> {
    require_admin(&mutation.0.user)?;
    UserRepository::new(state.db.clone())
        .reset_password(id, &input.password)
        .await?;
    revoke_user_sessions(&state, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_machines(
    State(state): State<AppState>,
    session: crate::auth::AuthenticatedSession,
) -> Result<Json<Vec<MachineAccount>>, AppError> {
    require_admin(&session.user)?;
    Ok(Json(MachineRepository::new(state.db).list().await?))
}

async fn create_machine(
    State(state): State<AppState>,
    mutation: MutationSession,
    Json(input): Json<CreateMachineInput>,
) -> Result<(StatusCode, Json<IssuedMachineAccount>), AppError> {
    require_admin(&mutation.0.user)?;
    let account = MachineRepository::new(state.db)
        .issue(&input.name, mutation.0.user_id)
        .await?;
    Ok((StatusCode::CREATED, Json(account)))
}

async fn revoke_machine(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
) -> Result<Json<MachineAccount>, AppError> {
    require_admin(&mutation.0.user)?;
    Ok(Json(
        MachineRepository::new(state.db)
            .set_revoked(id, true)
            .await?,
    ))
}

async fn restore_machine(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
) -> Result<Json<MachineAccount>, AppError> {
    require_admin(&mutation.0.user)?;
    Ok(Json(
        MachineRepository::new(state.db)
            .set_revoked(id, false)
            .await?,
    ))
}

async fn delete_machine(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
) -> Result<StatusCode, AppError> {
    require_admin(&mutation.0.user)?;
    MachineRepository::new(state.db).delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
