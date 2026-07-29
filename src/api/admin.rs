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
        audit::{AuditActor, AuditRepository},
        groups::{Group, GroupRepository},
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

#[derive(Deserialize)]
struct GroupInput {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroupMembersInput {
    member_ids: Vec<Uuid>,
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
        .route("/groups", get(list_groups).post(create_group))
        .route(
            "/groups/{id}",
            axum::routing::put(update_group).delete(delete_group),
        )
        .route(
            "/groups/{id}/members",
            get(get_group_members).put(replace_group_members),
        )
}

async fn list_groups(
    State(state): State<AppState>,
    session: crate::auth::AuthenticatedSession,
) -> Result<Json<Vec<Group>>, AppError> {
    require_admin(&session.user)?;
    Ok(Json(GroupRepository::new(state.db).list().await?))
}

async fn create_group(
    State(state): State<AppState>,
    mutation: MutationSession,
    Json(input): Json<GroupInput>,
) -> Result<(StatusCode, Json<Group>), AppError> {
    require_admin(&mutation.0.user)?;
    let group = GroupRepository::new(state.db.clone())
        .create(&input.name)
        .await?;
    record_admin_event(&state, &mutation, "group.create", "group", group.id).await?;
    Ok((StatusCode::CREATED, Json(group)))
}

async fn update_group(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
    Json(input): Json<GroupInput>,
) -> Result<Json<Group>, AppError> {
    require_admin(&mutation.0.user)?;
    let group = GroupRepository::new(state.db.clone())
        .update(id, &input.name)
        .await?;
    record_admin_event(&state, &mutation, "group.update", "group", id).await?;
    Ok(Json(group))
}

async fn delete_group(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
) -> Result<StatusCode, AppError> {
    require_admin(&mutation.0.user)?;
    GroupRepository::new(state.db.clone()).delete(id).await?;
    record_admin_event(&state, &mutation, "group.delete", "group", id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_group_members(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: crate::auth::AuthenticatedSession,
) -> Result<Json<Group>, AppError> {
    require_admin(&session.user)?;
    Ok(Json(GroupRepository::new(state.db).get(id).await?))
}

async fn replace_group_members(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
    Json(input): Json<GroupMembersInput>,
) -> Result<Json<Group>, AppError> {
    require_admin(&mutation.0.user)?;
    let group = GroupRepository::new(state.db.clone())
        .replace_members(id, &input.member_ids)
        .await?;
    record_admin_event(&state, &mutation, "group.members.replace", "group", id).await?;
    Ok(Json(group))
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
    let user = UserRepository::new(state.db.clone())
        .create(
            &input.username,
            &input.display_name,
            input.role,
            &input.password,
        )
        .await?;
    record_admin_event(&state, &mutation, "user.create", "user", user.id).await?;
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
    record_admin_event(&state, &mutation, "user.update", "user", id).await?;
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
    record_admin_event(&state, &mutation, "user.password.reset", "user", id).await?;
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
    let account = MachineRepository::new(state.db.clone())
        .issue(&input.name, mutation.0.user_id)
        .await?;
    record_admin_event(
        &state,
        &mutation,
        "machine.create",
        "machine",
        account.account.id,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(account)))
}

async fn revoke_machine(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
) -> Result<Json<MachineAccount>, AppError> {
    require_admin(&mutation.0.user)?;
    let machine = MachineRepository::new(state.db.clone())
        .set_revoked(id, true)
        .await?;
    record_admin_event(&state, &mutation, "machine.revoke", "machine", id).await?;
    Ok(Json(machine))
}

async fn restore_machine(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
) -> Result<Json<MachineAccount>, AppError> {
    require_admin(&mutation.0.user)?;
    let machine = MachineRepository::new(state.db.clone())
        .set_revoked(id, false)
        .await?;
    record_admin_event(&state, &mutation, "machine.restore", "machine", id).await?;
    Ok(Json(machine))
}

async fn delete_machine(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
) -> Result<StatusCode, AppError> {
    require_admin(&mutation.0.user)?;
    MachineRepository::new(state.db.clone()).delete(id).await?;
    record_admin_event(&state, &mutation, "machine.delete", "machine", id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn record_admin_event(
    state: &AppState,
    mutation: &MutationSession,
    action: &str,
    resource_kind: &str,
    resource_id: Uuid,
) -> Result<(), AppError> {
    AuditRepository::new(state.db.clone())
        .record(
            AuditActor::User(mutation.0.user_id),
            action,
            resource_kind,
            Some(resource_id),
            "changed",
        )
        .await
}
