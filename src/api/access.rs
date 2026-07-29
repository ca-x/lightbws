use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState,
    api::sdk::SdkAuth,
    auth::{AuthenticatedSession, MutationSession, require_admin},
    domain::{
        ORGANIZATION_ID,
        access::{
            AccessPolicyInput, AccessPolicyView, AccessRepository, GrantInput, MachineAccessInput,
            MachineAccessView, NamedGrant,
        },
        audit::{AuditActor, AuditEvent, AuditRepository, AuditSettings, UpdateAuditSettings},
        groups::GroupRepository,
        machines::MachineRepository,
        projects::ProjectRepository,
        users::UserRepository,
    },
    error::AppError,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OfficialGrant {
    grantee_id: Uuid,
    read: bool,
    write: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PeoplePolicyRequest {
    user_access_policy_requests: Option<Vec<OfficialGrant>>,
    group_access_policy_requests: Option<Vec<OfficialGrant>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceAccountPolicyRequest {
    service_account_access_policy_requests: Option<Vec<OfficialGrant>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrantedProject {
    granted_id: Uuid,
    read: bool,
    write: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrantedProjectsRequest {
    project_granted_policy_requests: Option<Vec<GrantedProject>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UserPolicy {
    object: &'static str,
    read: bool,
    write: bool,
    organization_user_id: Uuid,
    organization_user_name: String,
    current_user: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GroupPolicy {
    object: &'static str,
    read: bool,
    write: bool,
    group_id: Uuid,
    group_name: String,
    current_user_in_group: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MachinePolicy {
    object: &'static str,
    read: bool,
    write: bool,
    service_account_id: Uuid,
    service_account_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PeoplePolicyResponse {
    object: &'static str,
    user_access_policies: Vec<UserPolicy>,
    group_access_policies: Vec<GroupPolicy>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MachinePolicyResponse {
    object: &'static str,
    service_account_access_policies: Vec<MachinePolicy>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SecretPolicyResponse {
    object: &'static str,
    user_access_policies: Vec<UserPolicy>,
    group_access_policies: Vec<GroupPolicy>,
    service_account_access_policies: Vec<MachinePolicy>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PotentialGrantee {
    object: &'static str,
    id: Uuid,
    name: String,
    r#type: &'static str,
    email: Option<String>,
    current_user_in_group: bool,
    current_user: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PotentialGranteeList {
    object: &'static str,
    data: Vec<PotentialGrantee>,
    continuation_token: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GrantedProjectPolicy {
    object: &'static str,
    access_policy: GrantedProjectAccess,
    has_permission: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GrantedProjectAccess {
    object: &'static str,
    read: bool,
    write: bool,
    granted_project_id: Uuid,
    granted_project_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GrantedProjectsResponse {
    object: &'static str,
    granted_project_policies: Vec<GrantedProjectPolicy>,
}

pub fn web_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/{id}/access",
            get(web_project_access).put(web_replace_project_access),
        )
        .route(
            "/secrets/{id}/access",
            get(web_secret_access).put(web_replace_secret_access),
        )
        .route(
            "/machines/{id}/access",
            get(web_machine_access).put(web_replace_machine_access),
        )
        .route("/audit", get(web_audit))
        .route("/audit", axum::routing::delete(web_clear_audit))
        .route(
            "/audit/settings",
            get(web_audit_settings).put(web_update_audit_settings),
        )
}

pub fn sdk_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/organizations/{id}/access-policies/people/potential-grantees",
            get(potential_people),
        )
        .route(
            "/api/organizations/{id}/access-policies/projects/potential-grantees",
            get(potential_projects),
        )
        .route(
            "/api/organizations/{id}/access-policies/service-accounts/potential-grantees",
            get(potential_machines),
        )
        .route(
            "/api/projects/{id}/access-policies/people",
            get(project_people).put(put_project_people),
        )
        .route(
            "/api/projects/{id}/access-policies/service-accounts",
            get(project_machines).put(put_project_machines),
        )
        .route("/api/secrets/{id}/access-policies", get(secret_access))
        .route(
            "/api/service-accounts/{id}/granted-policies",
            get(machine_projects).put(put_machine_projects),
        )
        .route(
            "/api/service-accounts/{id}/access-policies/people",
            get(machine_people).put(put_machine_people),
        )
}

async fn web_project_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: AuthenticatedSession,
) -> Result<Json<AccessPolicyView>, AppError> {
    require_admin(&session.user)?;
    Ok(Json(
        AccessRepository::new(state.db).project_view(id).await?,
    ))
}

async fn web_replace_project_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
    Json(input): Json<AccessPolicyInput>,
) -> Result<Json<AccessPolicyView>, AppError> {
    require_admin(&mutation.0.user)?;
    let view = AccessRepository::new(state.db.clone())
        .replace_project(id, &input)
        .await?;
    AuditRepository::new(state.db)
        .record(
            AuditActor::User(mutation.0.user_id),
            "policy.replace",
            "project",
            Some(id),
            "changed",
        )
        .await?;
    Ok(Json(view))
}

async fn web_secret_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: AuthenticatedSession,
) -> Result<Json<AccessPolicyView>, AppError> {
    require_admin(&session.user)?;
    Ok(Json(AccessRepository::new(state.db).secret_view(id).await?))
}

async fn web_replace_secret_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
    Json(input): Json<AccessPolicyInput>,
) -> Result<Json<AccessPolicyView>, AppError> {
    require_admin(&mutation.0.user)?;
    let view = AccessRepository::new(state.db.clone())
        .replace_secret(id, &input)
        .await?;
    AuditRepository::new(state.db)
        .record(
            AuditActor::User(mutation.0.user_id),
            "policy.replace",
            "secret",
            Some(id),
            "changed",
        )
        .await?;
    Ok(Json(view))
}

async fn web_audit(
    State(state): State<AppState>,
    session: AuthenticatedSession,
) -> Result<Json<Vec<AuditEvent>>, AppError> {
    require_admin(&session.user)?;
    Ok(Json(AuditRepository::new(state.db).list().await?))
}

async fn web_audit_settings(
    State(state): State<AppState>,
    session: AuthenticatedSession,
) -> Result<Json<AuditSettings>, AppError> {
    require_admin(&session.user)?;
    Ok(Json(AuditRepository::new(state.db).settings().await?))
}

async fn web_update_audit_settings(
    State(state): State<AppState>,
    mutation: MutationSession,
    Json(input): Json<UpdateAuditSettings>,
) -> Result<Json<AuditSettings>, AppError> {
    require_admin(&mutation.0.user)?;
    Ok(Json(
        AuditRepository::new(state.db)
            .update_settings(input)
            .await?,
    ))
}

async fn web_clear_audit(
    State(state): State<AppState>,
    mutation: MutationSession,
) -> Result<Json<serde_json::Value>, AppError> {
    require_admin(&mutation.0.user)?;
    let deleted = AuditRepository::new(state.db).clear().await?;
    Ok(Json(serde_json::json!({ "deleted": deleted })))
}

async fn web_machine_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: AuthenticatedSession,
) -> Result<Json<MachineAccessView>, AppError> {
    require_admin(&session.user)?;
    Ok(Json(
        AccessRepository::new(state.db)
            .machine_access_view(id)
            .await?,
    ))
}

async fn web_replace_machine_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mutation: MutationSession,
    Json(input): Json<MachineAccessInput>,
) -> Result<Json<MachineAccessView>, AppError> {
    require_admin(&mutation.0.user)?;
    let view = AccessRepository::new(state.db.clone())
        .replace_machine_access(id, &input)
        .await?;
    AuditRepository::new(state.db)
        .record(
            AuditActor::User(mutation.0.user_id),
            "policy.replace",
            "machine",
            Some(id),
            "changed",
        )
        .await?;
    Ok(Json(view))
}

fn require_sdk_manager(auth: &SdkAuth) -> Result<(), AppError> {
    auth.machine
        .compatibility_account
        .then_some(())
        .ok_or(AppError::Forbidden)
}

fn require_org(id: Uuid) -> Result<(), AppError> {
    (id.to_string() == ORGANIZATION_ID)
        .then_some(())
        .ok_or(AppError::Forbidden)
}

async fn project_people(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: SdkAuth,
) -> Result<Json<PeoplePolicyResponse>, AppError> {
    require_sdk_manager(&auth)?;
    Ok(Json(people_response(
        "projectPeopleAccessPolicies",
        AccessRepository::new(state.db).project_view(id).await?,
    )))
}

async fn put_project_people(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: SdkAuth,
    Json(input): Json<PeoplePolicyRequest>,
) -> Result<Json<PeoplePolicyResponse>, AppError> {
    require_sdk_manager(&auth)?;
    let repository = AccessRepository::new(state.db.clone());
    let current = repository.project_view(id).await?;
    let policy = AccessPolicyInput {
        users: official_grants(input.user_access_policy_requests),
        groups: official_grants(input.group_access_policy_requests),
        machines: named_inputs(&current.machines),
    };
    Ok(Json(people_response(
        "projectPeopleAccessPolicies",
        repository.replace_project(id, &policy).await?,
    )))
}

async fn project_machines(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: SdkAuth,
) -> Result<Json<MachinePolicyResponse>, AppError> {
    require_sdk_manager(&auth)?;
    Ok(Json(machine_response(
        "projectServiceAccountsAccessPolicies",
        AccessRepository::new(state.db).project_view(id).await?,
    )))
}

async fn put_project_machines(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: SdkAuth,
    Json(input): Json<ServiceAccountPolicyRequest>,
) -> Result<Json<MachinePolicyResponse>, AppError> {
    require_sdk_manager(&auth)?;
    let repository = AccessRepository::new(state.db.clone());
    let current = repository.project_view(id).await?;
    let policy = AccessPolicyInput {
        users: named_inputs(&current.users),
        groups: named_inputs(&current.groups),
        machines: official_grants(input.service_account_access_policy_requests),
    };
    Ok(Json(machine_response(
        "projectServiceAccountsAccessPolicies",
        repository.replace_project(id, &policy).await?,
    )))
}

async fn secret_access(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: SdkAuth,
) -> Result<Json<SecretPolicyResponse>, AppError> {
    require_sdk_manager(&auth)?;
    Ok(Json(secret_response(
        AccessRepository::new(state.db).secret_view(id).await?,
    )))
}

async fn machine_projects(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: SdkAuth,
) -> Result<Json<GrantedProjectsResponse>, AppError> {
    if auth.machine.id != id {
        require_sdk_manager(&auth)?;
    }
    Ok(Json(granted_projects(
        AccessRepository::new(state.db)
            .machine_granted_projects(id)
            .await?,
    )))
}

async fn put_machine_projects(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: SdkAuth,
    Json(input): Json<GrantedProjectsRequest>,
) -> Result<Json<GrantedProjectsResponse>, AppError> {
    require_sdk_manager(&auth)?;
    let grants = input
        .project_granted_policy_requests
        .unwrap_or_default()
        .into_iter()
        .map(|grant| GrantInput {
            grantee_id: grant.granted_id,
            read: grant.read,
            write: grant.write,
        })
        .collect::<Vec<_>>();
    Ok(Json(granted_projects(
        AccessRepository::new(state.db)
            .replace_machine_projects(id, &grants)
            .await?,
    )))
}

async fn machine_people(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: SdkAuth,
) -> Result<Json<PeoplePolicyResponse>, AppError> {
    require_sdk_manager(&auth)?;
    Ok(Json(people_response(
        "serviceAccountPeopleAccessPolicies",
        AccessRepository::new(state.db)
            .machine_people_view(id)
            .await?,
    )))
}

async fn put_machine_people(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: SdkAuth,
    Json(input): Json<PeoplePolicyRequest>,
) -> Result<Json<PeoplePolicyResponse>, AppError> {
    require_sdk_manager(&auth)?;
    let view = AccessRepository::new(state.db)
        .replace_machine_people(
            id,
            &official_grants(input.user_access_policy_requests),
            &official_grants(input.group_access_policy_requests),
        )
        .await?;
    Ok(Json(people_response(
        "serviceAccountPeopleAccessPolicies",
        view,
    )))
}

async fn potential_people(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: SdkAuth,
) -> Result<Json<PotentialGranteeList>, AppError> {
    require_org(id)?;
    require_sdk_manager(&auth)?;
    let mut data = UserRepository::new(state.db.clone())
        .list()
        .await?
        .into_iter()
        .map(|user| PotentialGrantee {
            object: "potentialGrantee",
            id: user.id,
            name: user.display_name,
            r#type: "User",
            email: Some(user.username),
            current_user_in_group: false,
            current_user: false,
        })
        .collect::<Vec<_>>();
    data.extend(
        GroupRepository::new(state.db)
            .list()
            .await?
            .into_iter()
            .map(|group| PotentialGrantee {
                object: "potentialGrantee",
                id: group.id,
                name: group.name,
                r#type: "Group",
                email: None,
                current_user_in_group: false,
                current_user: false,
            }),
    );
    Ok(Json(PotentialGranteeList {
        object: "list",
        data,
        continuation_token: None,
    }))
}

async fn potential_projects(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: SdkAuth,
) -> Result<Json<PotentialGranteeList>, AppError> {
    require_org(id)?;
    require_sdk_manager(&auth)?;
    let data = ProjectRepository::new(state.db)
        .list(false)
        .await?
        .into_iter()
        .map(|project| {
            Ok(PotentialGrantee {
                object: "potentialGrantee",
                id: Uuid::parse_str(&project.id).map_err(AppError::internal)?,
                name: project
                    .name_plain
                    .or(project.name_cipher)
                    .unwrap_or_else(|| "Project".into()),
                r#type: "Project",
                email: None,
                current_user_in_group: false,
                current_user: false,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    Ok(Json(PotentialGranteeList {
        object: "list",
        data,
        continuation_token: None,
    }))
}

async fn potential_machines(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: SdkAuth,
) -> Result<Json<PotentialGranteeList>, AppError> {
    require_org(id)?;
    require_sdk_manager(&auth)?;
    let data = MachineRepository::new(state.db)
        .list()
        .await?
        .into_iter()
        .map(|machine| PotentialGrantee {
            object: "potentialGrantee",
            id: machine.id,
            name: machine.name,
            r#type: "ServiceAccount",
            email: None,
            current_user_in_group: false,
            current_user: false,
        })
        .collect();
    Ok(Json(PotentialGranteeList {
        object: "list",
        data,
        continuation_token: None,
    }))
}

fn official_grants(values: Option<Vec<OfficialGrant>>) -> Vec<GrantInput> {
    values
        .unwrap_or_default()
        .into_iter()
        .map(|grant| GrantInput {
            grantee_id: grant.grantee_id,
            read: grant.read,
            write: grant.write,
        })
        .collect()
}
fn named_inputs(values: &[NamedGrant]) -> Vec<GrantInput> {
    values
        .iter()
        .map(|grant| GrantInput {
            grantee_id: grant.grantee_id,
            read: grant.read,
            write: grant.write,
        })
        .collect()
}
fn people_response(object: &'static str, view: AccessPolicyView) -> PeoplePolicyResponse {
    PeoplePolicyResponse {
        object,
        user_access_policies: view
            .users
            .into_iter()
            .map(|grant| UserPolicy {
                object: "userAccessPolicy",
                read: grant.read,
                write: grant.write,
                organization_user_id: grant.grantee_id,
                organization_user_name: grant.name,
                current_user: false,
            })
            .collect(),
        group_access_policies: view
            .groups
            .into_iter()
            .map(|grant| GroupPolicy {
                object: "groupAccessPolicy",
                read: grant.read,
                write: grant.write,
                group_id: grant.grantee_id,
                group_name: grant.name,
                current_user_in_group: false,
            })
            .collect(),
    }
}
fn machine_response(object: &'static str, view: AccessPolicyView) -> MachinePolicyResponse {
    MachinePolicyResponse {
        object,
        service_account_access_policies: view
            .machines
            .into_iter()
            .map(|grant| MachinePolicy {
                object: "serviceAccountAccessPolicy",
                read: grant.read,
                write: grant.write,
                service_account_id: grant.grantee_id,
                service_account_name: grant.name,
            })
            .collect(),
    }
}
fn secret_response(view: AccessPolicyView) -> SecretPolicyResponse {
    let people = people_response("secretAccessPolicies", view.clone());
    let machines = machine_response("secretAccessPolicies", view);
    SecretPolicyResponse {
        object: "secretAccessPolicies",
        user_access_policies: people.user_access_policies,
        group_access_policies: people.group_access_policies,
        service_account_access_policies: machines.service_account_access_policies,
    }
}
fn granted_projects(values: Vec<NamedGrant>) -> GrantedProjectsResponse {
    GrantedProjectsResponse {
        object: "serviceAccountGrantedPolicies",
        granted_project_policies: values
            .into_iter()
            .map(|grant| GrantedProjectPolicy {
                object: "grantedProjectAccessPolicyPermissionDetails",
                access_policy: GrantedProjectAccess {
                    object: "grantedProjectAccessPolicy",
                    read: grant.read,
                    write: grant.write,
                    granted_project_id: grant.grantee_id,
                    granted_project_name: grant.name,
                },
                has_permission: true,
            })
            .collect(),
    }
}
