use axum::{
    Form, Json, Router,
    extract::{FromRequestParts, Path, Query, State},
    http::{HeaderMap, StatusCode, request::Parts},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, TransactionTrait};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    AppState,
    db::entities::{sdk_sync_state, secret},
    domain::{
        ORGANIZATION_ID,
        machines::{MachineRepository, SDK_SESSION_TTL_SECONDS},
        projects::ProjectRepository,
        secrets::SecretRepository,
    },
    error::AppError,
    sdk_models::{
        CreateProjectRequest, CreateSecretRequest, DataResponse, DeleteResult, GetByIdsRequest,
        IdentifierProject, SdkProject, SdkSecret, SecretIdentifier, SecretsListResponse,
        SyncResponse, UpdateSecretRequest,
    },
};

const ENCRYPTED_PAYLOAD: &str = concat!(
    "2.E9fE8+M/VWMfhhim1KlCbQ==|eLsHR484S/tJbIkM6spnG/HP65tj9A6Tba7kAAvUp+rYuQmGLixiOCfMsqt5OvBctDfvvr/Aes",
    "Bu7cZimPLyOEhqEAjn52jF0eaI38XZfeOG2VJl0LOf60Wkfh3ryAMvfvLj3G4ZCNYU8sNgoC2+IQ==|lNApuCQ4Pyakfo/wwuuajWNaEX/2MW8/3rjXB/V7n+k="
);

#[derive(Debug, Deserialize)]
struct TokenRequest {
    grant_type: String,
    client_id: Option<String>,
    client_secret: Option<String>,
    scope: Option<String>,
}

#[derive(Debug, Serialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    refresh_token: Option<String>,
    token_type: &'static str,
    scope: &'static str,
    encrypted_payload: &'static str,
}

#[derive(Debug, Deserialize)]
struct SyncQuery {
    #[serde(rename = "lastSyncedDate")]
    last_synced_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProjectQuery {
    #[serde(rename = "projectId")]
    project_id: Option<Uuid>,
}

struct SdkAuth;

#[derive(Debug, Deserialize)]
struct JwtSessionClaims {
    sub: String,
}

impl FromRequestParts<AppState> for SdkAuth {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let bearer = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or(AppError::Unauthorized)?;
        let session_token = session_token_from_bearer(bearer);
        MachineRepository::new(state.db.clone())
            .authenticate_session(&session_token)
            .await?;
        Ok(Self)
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/identity/connect/token", post(token))
        .route("/api/projects/{id}", get(get_project).put(update_project))
        .route(
            "/api/organizations/{org_id}/projects",
            get(list_projects).post(create_project),
        )
        .route("/api/projects/delete", post(delete_projects))
        .route("/api/secrets/{id}", get(get_secret).put(update_secret))
        .route(
            "/api/organizations/{org_id}/secrets",
            get(list_secrets).post(create_secret),
        )
        .route(
            "/api/projects/{project_id}/secrets",
            get(list_project_secrets),
        )
        .route("/api/secrets/get-by-ids", post(get_secrets_by_ids))
        .route(
            "/api/organizations/{org_id}/secrets/sync",
            get(sync_secrets),
        )
        .route("/api/secrets/delete", post(delete_secrets))
        .route("/echo", post(echo))
}

async fn token(
    State(state): State<AppState>,
    Form(payload): Form<TokenRequest>,
) -> Result<Json<TokenResponse>, AppError> {
    if payload.grant_type != "client_credentials"
        || payload
            .scope
            .as_deref()
            .is_some_and(|scope| !scope.contains("api.secrets"))
    {
        return Err(AppError::Unauthorized);
    }
    let repository = MachineRepository::new(state.db);
    let machine = repository
        .authenticate(
            payload.client_id.as_deref().ok_or(AppError::Unauthorized)?,
            payload
                .client_secret
                .as_deref()
                .ok_or(AppError::Unauthorized)?,
        )
        .await?;
    let session_token = repository.create_session(machine.id).await?;
    Ok(Json(TokenResponse {
        access_token: session_jwt(&session_token, machine.client_id),
        expires_in: u64::try_from(SDK_SESSION_TTL_SECONDS).expect("positive SDK session TTL"),
        refresh_token: None,
        token_type: "Bearer",
        scope: "api.secrets",
        encrypted_payload: ENCRYPTED_PAYLOAD,
    }))
}

fn session_jwt(session_token: &str, client_id: Uuid) -> String {
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT"}"#);
    let payload = json!({
        "nbf": crate::domain::now(),
        "exp": crate::domain::now() + SDK_SESSION_TTL_SECONDS,
        "iss": "lightbws",
        "client_id": client_id,
        "sub": session_token,
        "organization": ORGANIZATION_ID,
        "scope": ["api.secrets"],
    });
    let payload = URL_SAFE_NO_PAD.encode(payload.to_string());
    format!("{header}.{payload}.lightbws")
}

fn session_token_from_bearer(bearer: &str) -> String {
    let Some(payload) = bearer.split('.').nth(1) else {
        return bearer.to_owned();
    };
    let Ok(payload) = URL_SAFE_NO_PAD.decode(payload) else {
        return bearer.to_owned();
    };
    serde_json::from_slice::<JwtSessionClaims>(&payload)
        .map(|claims| claims.sub)
        .unwrap_or_else(|_| bearer.to_owned())
}

async fn list_projects(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
    _auth: SdkAuth,
) -> Result<Json<DataResponse<Vec<SdkProject>>>, AppError> {
    require_org(org_id)?;
    let data = ProjectRepository::new(state.db)
        .list(false)
        .await?
        .into_iter()
        .filter(|project| project.name_cipher.is_some())
        .map(SdkProject::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(DataResponse { data }))
}

async fn get_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    _auth: SdkAuth,
) -> Result<Json<SdkProject>, AppError> {
    let model = ProjectRepository::new(state.db).get(id).await?;
    if model.deleted_at.is_some() {
        return Err(AppError::NotFound);
    }
    Ok(Json(SdkProject::try_from(model)?))
}

async fn create_project(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
    _auth: SdkAuth,
    Json(input): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<SdkProject>), AppError> {
    require_org(org_id)?;
    let model = ProjectRepository::new(state.db)
        .create_cipher(input.name)
        .await?;
    Ok((StatusCode::CREATED, Json(SdkProject::try_from(model)?)))
}

async fn update_project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    _auth: SdkAuth,
    Json(input): Json<CreateProjectRequest>,
) -> Result<Json<SdkProject>, AppError> {
    let model = ProjectRepository::new(state.db)
        .update_cipher(id, input.name)
        .await?;
    Ok(Json(SdkProject::try_from(model)?))
}

async fn delete_projects(
    State(state): State<AppState>,
    _auth: SdkAuth,
    Json(ids): Json<Vec<Uuid>>,
) -> Result<Json<DataResponse<Vec<DeleteResult>>>, AppError> {
    let repository = ProjectRepository::new(state.db);
    let mut data = Vec::with_capacity(ids.len());
    for id in ids {
        let error = match repository.get(id).await {
            Ok(_) => repository
                .set_deleted(&[id], true)
                .await
                .err()
                .map(|_| "Delete failed".into()),
            Err(_) => Some("Not found".into()),
        };
        data.push(DeleteResult { id, error });
    }
    Ok(Json(DataResponse { data }))
}

async fn list_secrets(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
    Query(query): Query<ProjectQuery>,
    _auth: SdkAuth,
) -> Result<Json<SecretsListResponse>, AppError> {
    require_org(org_id)?;
    identifiers(&state, query.project_id).await
}

async fn list_project_secrets(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    _auth: SdkAuth,
) -> Result<Json<SecretsListResponse>, AppError> {
    identifiers(&state, Some(project_id)).await
}

async fn identifiers(
    state: &AppState,
    project_id: Option<Uuid>,
) -> Result<Json<SecretsListResponse>, AppError> {
    let repository = SecretRepository::new(state.db.clone());
    let mut secrets = Vec::new();
    for model in repository
        .list(false, project_id)
        .await?
        .into_iter()
        .filter(|model| model.key_cipher.is_some())
    {
        let projects = if let Some(project_id) = model.project_id.as_deref() {
            let project_id = Uuid::parse_str(project_id).map_err(AppError::internal)?;
            let project = ProjectRepository::new(state.db.clone())
                .get(project_id)
                .await?;
            project
                .name_cipher
                .map(|name| {
                    vec![IdentifierProject {
                        id: project_id,
                        name,
                    }]
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        secrets.push(SecretIdentifier {
            id: Uuid::parse_str(&model.id).map_err(AppError::internal)?,
            organization_id: Uuid::parse_str(&model.organization_id).map_err(AppError::internal)?,
            key: model.key_cipher.unwrap_or_default(),
            projects,
        });
    }
    Ok(Json(SecretsListResponse { secrets }))
}

async fn get_secret(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    _auth: SdkAuth,
) -> Result<Json<SdkSecret>, AppError> {
    let model = SecretRepository::new(state.db).get(id).await?;
    if model.deleted_at.is_some() {
        return Err(AppError::NotFound);
    }
    Ok(Json(SdkSecret::try_from(model)?))
}

async fn create_secret(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
    _auth: SdkAuth,
    Json(input): Json<CreateSecretRequest>,
) -> Result<(StatusCode, Json<SdkSecret>), AppError> {
    require_org(org_id)?;
    let model = SecretRepository::new(state.db)
        .create_cipher(
            input.key,
            input.value,
            input.note,
            first_project(input.project_ids),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(SdkSecret::try_from(model)?)))
}

async fn update_secret(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    _auth: SdkAuth,
    Json(input): Json<UpdateSecretRequest>,
) -> Result<Json<SdkSecret>, AppError> {
    let _ = input.value_changed;
    let model = SecretRepository::new(state.db)
        .update_cipher(
            id,
            input.key,
            input.value,
            input.note,
            first_project(input.project_ids),
        )
        .await?;
    Ok(Json(SdkSecret::try_from(model)?))
}

async fn get_secrets_by_ids(
    State(state): State<AppState>,
    _auth: SdkAuth,
    Json(input): Json<GetByIdsRequest>,
) -> Result<Json<DataResponse<Vec<SdkSecret>>>, AppError> {
    let data = SecretRepository::new(state.db)
        .get_many(&input.ids)
        .await?
        .into_iter()
        .filter(|secret| secret.key_cipher.is_some())
        .map(SdkSecret::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(DataResponse { data }))
}

async fn delete_secrets(
    State(state): State<AppState>,
    _auth: SdkAuth,
    Json(ids): Json<Vec<Uuid>>,
) -> Result<Json<DataResponse<Vec<DeleteResult>>>, AppError> {
    let repository = SecretRepository::new(state.db);
    let mut data = Vec::with_capacity(ids.len());
    for id in ids {
        let error = match repository.get(id).await {
            Ok(_) => repository
                .set_deleted(&[id], true)
                .await
                .err()
                .map(|_| "Delete failed".into()),
            Err(_) => Some("Not found".into()),
        };
        data.push(DeleteResult { id, error });
    }
    Ok(Json(DataResponse { data }))
}

async fn sync_secrets(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
    Query(query): Query<SyncQuery>,
    _auth: SdkAuth,
) -> Result<Json<SyncResponse>, AppError> {
    require_org(org_id)?;
    let last = query
        .last_synced_date
        .as_deref()
        .and_then(|value| {
            time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).ok()
        })
        .map(|value| value.unix_timestamp_nanos());
    let transaction = state.db.connection().begin().await?;
    let revision = sdk_sync_state::Entity::find_by_id(ORGANIZATION_ID)
        .one(&transaction)
        .await?
        .ok_or_else(|| AppError::internal(anyhow::anyhow!("SDK state is missing")))?
        .revision_nanos;
    let models = secret::Entity::find()
        .filter(secret::Column::OrganizationId.eq(ORGANIZATION_ID))
        .order_by_desc(secret::Column::RevisionNanos)
        .all(&transaction)
        .await?;
    transaction.commit().await?;
    let has_changes = last.is_none_or(|last| i128::from(revision) > last);
    let secrets = if has_changes {
        Some(DataResponse {
            data: models
                .into_iter()
                .filter(|secret| secret.deleted_at.is_none() && secret.key_cipher.is_some())
                .map(SdkSecret::try_from)
                .collect::<Result<Vec<_>, _>>()?,
        })
    } else {
        None
    };
    Ok(Json(SyncResponse {
        has_changes,
        secrets,
    }))
}

pub async fn health() -> Json<Value> {
    Json(json!({ "status": "healthy", "timestamp": time::OffsetDateTime::now_utc() }))
}

pub async fn help() -> Json<Value> {
    Json(json!({
        "name": "LightBWS",
        "compatibility": "Bitwarden Secrets Manager server",
        "documentation": {
            "officialSdk": "https://github.com/bitwarden/sdk-sm",
            "bws": "https://github.com/bitwarden/sdk-sm/tree/main/crates/bws",
            "fnox": "https://fnox.jdx.dev/providers/bitwarden-sm",
            "secretsManager": "https://bitwarden.com/help/secrets-manager-overview/"
        }
    }))
}

async fn echo(_auth: SdkAuth, headers: HeaderMap, Json(value): Json<Value>) -> Json<Value> {
    let _ = headers;
    Json(value)
}

fn require_org(id: Uuid) -> Result<(), AppError> {
    if id.to_string() == ORGANIZATION_ID {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

fn first_project(ids: Option<Vec<Uuid>>) -> Option<Uuid> {
    ids.and_then(|ids| ids.into_iter().next())
}
