use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use lightbws::{
    AppState,
    config::Config,
    create_app,
    db::{Database, entities::machine_session},
    domain::{
        ORGANIZATION_ID,
        machines::{MachineRepository, UPSTREAM_CLIENT_ID, UPSTREAM_CLIENT_SECRET},
        projects::ProjectRepository,
        users::{Role, UserRepository},
    },
};
use sea_orm::EntityTrait;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

struct Fixture {
    _data: TempDir,
    app: Router,
    db: Database,
}

impl Fixture {
    async fn new() -> Self {
        let data = tempfile::tempdir().expect("tempdir");
        let db = Database::connect(&data.path().join("sdk.sqlite3"))
            .await
            .expect("database");
        let admin = UserRepository::new(db.clone())
            .create("admin", "Administrator", Role::Admin, "test-password-123")
            .await
            .expect("admin");
        MachineRepository::new(db.clone())
            .ensure_compatibility_account(admin.id)
            .await
            .expect("compatibility account");
        let config = Config {
            bind: "127.0.0.1:0".parse().unwrap(),
            data_dir: data.path().into(),
            bootstrap_admin: None,
            cookie_secure: false,
            upstream_compatibility_account: false,
            master_key: None,
            allow_plaintext_backups: false,
        };
        Self {
            _data: data,
            app: create_app(AppState::new(db.clone(), &config)),
            db,
        }
    }

    async fn request(&self, request: Request<Body>) -> axum::response::Response {
        self.app.clone().oneshot(request).await.expect("request")
    }

    async fn bearer(&self) -> String {
        self.bearer_for(UPSTREAM_CLIENT_ID, UPSTREAM_CLIENT_SECRET)
            .await
    }

    async fn bearer_for(&self, client_id: &str, client_secret: &str) -> String {
        let response = self
            .request(
                Request::builder()
                    .method(Method::POST)
                    .uri("/identity/connect/token")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(format!(
                        "grant_type=client_credentials&scope=api.secrets&client_id={client_id}&client_secret={client_secret}"
                    )))
                    .unwrap(),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        response_json(response).await["access_token"]
            .as_str()
            .expect("access token")
            .to_owned()
    }
}

#[tokio::test]
async fn sdk_access_policies_scope_regular_machines_and_support_direct_secret_access() {
    let fixture = Fixture::new().await;
    let compatibility_bearer = fixture.bearer().await;
    let first_project =
        create_sdk_project(&fixture, &compatibility_bearer, "2.first-project").await;
    let second_project =
        create_sdk_project(&fixture, &compatibility_bearer, "2.second-project").await;
    let first_secret = create_sdk_secret(
        &fixture,
        &compatibility_bearer,
        first_project,
        "2.first-key",
    )
    .await;
    let second_secret = create_sdk_secret(
        &fixture,
        &compatibility_bearer,
        second_project,
        "2.second-key",
    )
    .await;
    let admin = UserRepository::new(fixture.db.clone())
        .list()
        .await
        .expect("users")
        .into_iter()
        .find(|user| user.role == Role::Admin)
        .expect("admin");
    let issued = MachineRepository::new(fixture.db.clone())
        .issue("scoped-sdk", admin.id)
        .await
        .expect("machine");
    let client_secret = issued
        .access_token
        .split('.')
        .nth(2)
        .and_then(|part| part.split(':').next())
        .expect("client secret")
        .to_owned();

    let grants = fixture
        .request(json_request(
            Method::PUT,
            &format!(
                "/api/service-accounts/{}/granted-policies",
                issued.account.id
            ),
            Some(json!({
                "projectGrantedPolicyRequests": [{
                    "grantedId": first_project,
                    "read": true,
                    "write": false
                }]
            })),
            Some(&compatibility_bearer),
        ))
        .await;
    assert_eq!(grants.status(), StatusCode::OK);
    assert_eq!(
        response_json(grants).await["grantedProjectPolicies"][0]["accessPolicy"]["read"],
        true
    );

    let scoped_bearer = fixture
        .bearer_for(&issued.account.client_id.to_string(), &client_secret)
        .await;
    let projects = fixture
        .request(json_request(
            Method::GET,
            &format!("/api/organizations/{ORGANIZATION_ID}/projects"),
            None,
            Some(&scoped_bearer),
        ))
        .await;
    let projects = response_json(projects).await;
    assert_eq!(projects["data"].as_array().unwrap().len(), 1);
    assert_eq!(projects["data"][0]["id"], first_project.to_string());

    let readable = fixture
        .request(json_request(
            Method::GET,
            &format!("/api/secrets/{first_secret}"),
            None,
            Some(&scoped_bearer),
        ))
        .await;
    assert_eq!(readable.status(), StatusCode::OK);
    let hidden = fixture
        .request(json_request(
            Method::GET,
            &format!("/api/secrets/{second_secret}"),
            None,
            Some(&scoped_bearer),
        ))
        .await;
    assert_eq!(hidden.status(), StatusCode::FORBIDDEN);
    let read_only_write = fixture
        .request(json_request(
            Method::PUT,
            &format!("/api/secrets/{first_secret}"),
            Some(json!({
                "key": "2.denied",
                "value": "2.denied",
                "note": "2.denied",
                "projectIds": [first_project]
            })),
            Some(&scoped_bearer),
        ))
        .await;
    assert_eq!(read_only_write.status(), StatusCode::FORBIDDEN);

    let direct_policy = fixture
        .request(json_request(
            Method::PUT,
            &format!("/api/secrets/{second_secret}"),
            Some(json!({
                "key": "2.second-key",
                "value": "2.second-value",
                "note": "2.second-note",
                "projectIds": [second_project],
                "accessPoliciesRequests": {
                    "userAccessPolicyRequests": [],
                    "groupAccessPolicyRequests": [],
                    "serviceAccountAccessPolicyRequests": [{
                        "granteeId": issued.account.id,
                        "read": true,
                        "write": true
                    }]
                }
            })),
            Some(&compatibility_bearer),
        ))
        .await;
    assert_eq!(direct_policy.status(), StatusCode::OK);
    let policies = fixture
        .request(json_request(
            Method::GET,
            &format!("/api/secrets/{second_secret}/access-policies"),
            None,
            Some(&compatibility_bearer),
        ))
        .await;
    let policies = response_json(policies).await;
    assert_eq!(
        policies["serviceAccountAccessPolicies"][0]["serviceAccountId"],
        issued.account.id.to_string()
    );

    let direct_read = fixture
        .request(json_request(
            Method::GET,
            &format!("/api/secrets/{second_secret}"),
            None,
            Some(&scoped_bearer),
        ))
        .await;
    assert_eq!(direct_read.status(), StatusCode::OK);
    let direct_write = fixture
        .request(json_request(
            Method::PUT,
            &format!("/api/secrets/{second_secret}"),
            Some(json!({
                "key": "2.direct-updated",
                "value": "2.direct-updated",
                "note": "2.direct-updated",
                "projectIds": [second_project]
            })),
            Some(&scoped_bearer),
        ))
        .await;
    assert_eq!(direct_write.status(), StatusCode::OK);

    let synced = fixture
        .request(json_request(
            Method::GET,
            &format!("/api/organizations/{ORGANIZATION_ID}/secrets/sync"),
            None,
            Some(&scoped_bearer),
        ))
        .await;
    let synced = response_json(synced).await;
    assert_eq!(synced["secrets"]["data"].as_array().unwrap().len(), 2);

    ProjectRepository::new(fixture.db.clone())
        .purge(second_project)
        .await
        .expect("purge project without deleting its secrets");
    let unassigned = fixture
        .request(json_request(
            Method::GET,
            &format!("/api/secrets/{second_secret}"),
            None,
            Some(&scoped_bearer),
        ))
        .await;
    assert_eq!(unassigned.status(), StatusCode::OK);
    assert!(
        response_json(unassigned).await["projects"]
            .as_array()
            .expect("secret projects")
            .is_empty()
    );

    let identifiers = fixture
        .request(json_request(
            Method::GET,
            &format!("/api/organizations/{ORGANIZATION_ID}/secrets"),
            None,
            Some(&scoped_bearer),
        ))
        .await;
    let identifiers = response_json(identifiers).await;
    let unassigned_identifier = identifiers["secrets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|secret| secret["id"] == second_secret.to_string())
        .expect("unassigned secret remains listed");
    assert!(
        unassigned_identifier["projects"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let updated_unassigned = fixture
        .request(json_request(
            Method::PUT,
            &format!("/api/secrets/{second_secret}"),
            Some(json!({
                "key": "2.unassigned-updated",
                "value": "2.unassigned-updated",
                "note": "2.unassigned-updated",
                "projectIds": null
            })),
            Some(&scoped_bearer),
        ))
        .await;
    assert_eq!(updated_unassigned.status(), StatusCode::OK);
    assert!(
        response_json(updated_unassigned).await["projects"]
            .as_array()
            .expect("secret projects")
            .is_empty()
    );
}

async fn create_sdk_project(fixture: &Fixture, bearer: &str, name: &str) -> Uuid {
    let response = fixture
        .request(json_request(
            Method::POST,
            &format!("/api/organizations/{ORGANIZATION_ID}/projects"),
            Some(json!({ "name": name })),
            Some(bearer),
        ))
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    Uuid::parse_str(response_json(response).await["id"].as_str().unwrap()).unwrap()
}

async fn create_sdk_secret(fixture: &Fixture, bearer: &str, project_id: Uuid, key: &str) -> Uuid {
    let response = fixture
        .request(json_request(
            Method::POST,
            &format!("/api/organizations/{ORGANIZATION_ID}/secrets"),
            Some(json!({
                "key": key,
                "value": "2.value",
                "note": "2.note",
                "projectIds": [project_id]
            })),
            Some(bearer),
        ))
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    Uuid::parse_str(response_json(response).await["id"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn sdk_rejects_invalid_bearer_and_persists_project_secret_round_trip() {
    let fixture = Fixture::new().await;
    let rejected = fixture
        .request(json_request(
            Method::GET,
            &format!("/api/organizations/{ORGANIZATION_ID}/projects"),
            None,
            Some("invalid"),
        ))
        .await;
    assert_eq!(rejected.status(), StatusCode::UNAUTHORIZED);

    let bearer = fixture.bearer().await;
    assert_eq!(bearer.split('.').count(), 3);
    let second_bearer = fixture.bearer().await;
    assert_ne!(bearer, second_bearer);
    let sessions = machine_session::Entity::find()
        .all(fixture.db.connection())
        .await
        .expect("machine sessions");
    assert_eq!(sessions.len(), 2);
    assert!(sessions.iter().all(|session| session.id != bearer));

    let repository = MachineRepository::new(fixture.db.clone());
    let machine = repository
        .list()
        .await
        .expect("machines")
        .into_iter()
        .find(|machine| machine.client_id.to_string() == UPSTREAM_CLIENT_ID)
        .expect("compatibility machine");
    repository
        .set_revoked(machine.id, true)
        .await
        .expect("revoke machine");
    let revoked = fixture
        .request(json_request(
            Method::GET,
            &format!("/api/organizations/{ORGANIZATION_ID}/projects"),
            None,
            Some(&bearer),
        ))
        .await;
    assert_eq!(revoked.status(), StatusCode::UNAUTHORIZED);
    repository
        .set_revoked(machine.id, false)
        .await
        .expect("restore machine");
    let restored_old_token = fixture
        .request(json_request(
            Method::GET,
            &format!("/api/organizations/{ORGANIZATION_ID}/projects"),
            None,
            Some(&bearer),
        ))
        .await;
    assert_eq!(restored_old_token.status(), StatusCode::OK);
    let bearer = fixture.bearer().await;
    let project = fixture
        .request(json_request(
            Method::POST,
            &format!("/api/organizations/{ORGANIZATION_ID}/projects"),
            Some(json!({ "name": "2.project-ciphertext" })),
            Some(&bearer),
        ))
        .await;
    assert_eq!(project.status(), StatusCode::CREATED);
    let project = response_json(project).await;
    assert_eq!(project["organizationId"], ORGANIZATION_ID);
    assert_eq!(project["name"], "2.project-ciphertext");
    assert!(project["creationDate"].is_string());
    let project_id = project["id"].as_str().unwrap();

    let secret = fixture
        .request(json_request(
            Method::POST,
            &format!("/api/organizations/{ORGANIZATION_ID}/secrets"),
            Some(json!({
                "key": "2.key-ciphertext",
                "value": "2.value-ciphertext",
                "note": "2.note-ciphertext",
                "projectIds": [project_id]
            })),
            Some(&bearer),
        ))
        .await;
    assert_eq!(secret.status(), StatusCode::CREATED);
    let secret = response_json(secret).await;
    assert_eq!(secret["key"], "2.key-ciphertext");
    assert_eq!(secret["projects"][0]["id"], project_id);
    assert!(secret.get("projectId").is_none());
    let secret_id = secret["id"].as_str().unwrap();
    let first_revision = secret["revisionDate"].as_str().unwrap().to_owned();

    let updated = fixture
        .request(json_request(
            Method::PUT,
            &format!("/api/secrets/{secret_id}"),
            Some(json!({
                "key": "2.key-ciphertext-updated",
                "value": "2.value-ciphertext-updated",
                "note": "2.note-ciphertext-updated",
                "projectIds": [project_id],
                "valueChanged": true
            })),
            Some(&bearer),
        ))
        .await;
    assert_eq!(updated.status(), StatusCode::OK);
    let updated = response_json(updated).await;
    assert_ne!(updated["revisionDate"], first_revision);
    let last_synced_date = updated["revisionDate"].as_str().unwrap().to_owned();

    let fetched = fixture
        .request(json_request(
            Method::GET,
            &format!("/api/secrets/{secret_id}"),
            None,
            Some(&bearer),
        ))
        .await;
    assert_eq!(fetched.status(), StatusCode::OK);
    assert_eq!(
        response_json(fetched).await["value"],
        "2.value-ciphertext-updated"
    );

    let identifiers = fixture
        .request(json_request(
            Method::GET,
            &format!("/api/projects/{project_id}/secrets"),
            None,
            Some(&bearer),
        ))
        .await;
    let identifiers = response_json(identifiers).await;
    assert_eq!(identifiers["secrets"].as_array().unwrap().len(), 1);
    assert_eq!(identifiers["secrets"][0]["projects"][0]["id"], project_id);

    let by_ids = fixture
        .request(json_request(
            Method::POST,
            "/api/secrets/get-by-ids",
            Some(json!({ "ids": [secret_id] })),
            Some(&bearer),
        ))
        .await;
    assert_eq!(
        response_json(by_ids).await["data"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let sync = fixture
        .request(json_request(
            Method::GET,
            &format!("/api/organizations/{ORGANIZATION_ID}/secrets/sync"),
            None,
            Some(&bearer),
        ))
        .await;
    let sync = response_json(sync).await;
    assert_eq!(sync["hasChanges"], true);
    assert_eq!(sync["secrets"]["data"].as_array().unwrap().len(), 1);

    let deleted_secret = fixture
        .request(json_request(
            Method::POST,
            "/api/secrets/delete",
            Some(json!([secret_id])),
            Some(&bearer),
        ))
        .await;
    assert_eq!(
        response_json(deleted_secret).await["data"][0]["error"],
        Value::Null
    );
    let deletion_sync = fixture
        .request(json_request(
            Method::GET,
            &format!(
                "/api/organizations/{ORGANIZATION_ID}/secrets/sync?lastSyncedDate={last_synced_date}"
            ),
            None,
            Some(&bearer),
        ))
        .await;
    let deletion_sync = response_json(deletion_sync).await;
    assert_eq!(deletion_sync["hasChanges"], true);
    assert!(
        deletion_sync["secrets"]["data"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let deleted_project = fixture
        .request(json_request(
            Method::POST,
            "/api/projects/delete",
            Some(json!([project_id])),
            Some(&bearer),
        ))
        .await;
    assert_eq!(
        response_json(deleted_project).await["data"][0]["error"],
        Value::Null
    );
}

fn json_request(
    method: Method,
    uri: &str,
    body: Option<Value>,
    bearer: Option<&str>,
) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }
    if let Some(bearer) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {bearer}"));
    }
    builder
        .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
        .unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    serde_json::from_slice(&body).expect("json")
}
