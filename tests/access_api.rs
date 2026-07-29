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
    db::Database,
    domain::{
        projects::ProjectRepository,
        secrets::SecretRepository,
        users::{Role, UserRepository},
    },
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

struct Fixture {
    _data: TempDir,
    app: Router,
    project_id: String,
    secret_id: String,
    member_id: String,
}

impl Fixture {
    async fn new() -> Self {
        let data = tempfile::tempdir().expect("tempdir");
        let db = Database::connect(&data.path().join("access-api.sqlite3"))
            .await
            .expect("database");
        UserRepository::new(db.clone())
            .create("admin", "Administrator", Role::Admin, "test-password-123")
            .await
            .expect("admin");
        let member = UserRepository::new(db.clone())
            .create("member", "Member", Role::User, "test-password-123")
            .await
            .expect("member");
        let project = ProjectRepository::new(db.clone())
            .create_plain("Application")
            .await
            .expect("project");
        let secret = SecretRepository::new(db.clone())
            .create_plain("DATABASE_URL", "sqlite://data", "runtime", project.id)
            .await
            .expect("secret");
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
            app: create_app(AppState::new(db, &config)),
            project_id: project.id.to_string(),
            secret_id: secret.id.to_string(),
            member_id: member.id.to_string(),
        }
    }

    async fn request(&self, request: Request<Body>) -> axum::response::Response {
        self.app.clone().oneshot(request).await.expect("request")
    }

    async fn login(&self, username: &str) -> (String, String) {
        let response = self
            .request(json_request(
                Method::POST,
                "/api/v1/auth/login",
                Some(json!({ "username": username, "password": "test-password-123" })),
                None,
                None,
            ))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let cookies = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .filter_map(|value| value.split(';').next())
            .collect::<Vec<_>>()
            .join("; ");
        let body = response_json(response).await;
        (cookies, body["csrfToken"].as_str().unwrap().to_owned())
    }
}

#[tokio::test]
async fn web_users_receive_group_and_direct_secret_permissions_without_admin_access() {
    let fixture = Fixture::new().await;
    let (member_cookies, member_csrf) = fixture.login("member").await;
    let empty_projects = fixture
        .request(json_request(
            Method::GET,
            "/api/v1/projects",
            None,
            Some(&member_cookies),
            None,
        ))
        .await;
    assert!(
        response_json(empty_projects)
            .await
            .as_array()
            .unwrap()
            .is_empty()
    );
    let hidden_secret = fixture
        .request(json_request(
            Method::GET,
            &format!("/api/v1/secrets/{}", fixture.secret_id),
            None,
            Some(&member_cookies),
            None,
        ))
        .await;
    assert_eq!(hidden_secret.status(), StatusCode::FORBIDDEN);

    let (admin_cookies, admin_csrf) = fixture.login("admin").await;
    let unassigned = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/secrets",
            Some(json!({
                "key": "GLOBAL_TOKEN",
                "value": "global",
                "note": "not bound to a project",
                "projectId": null
            })),
            Some(&admin_cookies),
            Some(&admin_csrf),
        ))
        .await;
    assert_eq!(unassigned.status(), StatusCode::CREATED);
    let unassigned = response_json(unassigned).await;
    assert_eq!(unassigned["projectId"], Value::Null);
    let unassigned_id = unassigned["id"].as_str().unwrap();
    let unassigned_create_denied = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/secrets",
            Some(json!({
                "key": "DENIED_GLOBAL",
                "value": "denied",
                "note": "",
                "projectId": null
            })),
            Some(&member_cookies),
            Some(&member_csrf),
        ))
        .await;
    assert_eq!(unassigned_create_denied.status(), StatusCode::FORBIDDEN);
    let unassigned_policy = fixture
        .request(json_request(
            Method::PUT,
            &format!("/api/v1/secrets/{unassigned_id}/access"),
            Some(json!({
                "users": [{ "granteeId": fixture.member_id, "read": true, "write": true }],
                "groups": [],
                "machines": []
            })),
            Some(&admin_cookies),
            Some(&admin_csrf),
        ))
        .await;
    assert_eq!(unassigned_policy.status(), StatusCode::OK);
    let unassigned_update = fixture
        .request(json_request(
            Method::PUT,
            &format!("/api/v1/secrets/{unassigned_id}"),
            Some(json!({
                "key": "GLOBAL_TOKEN",
                "value": "updated-global",
                "note": "direct grant",
                "projectId": null
            })),
            Some(&member_cookies),
            Some(&member_csrf),
        ))
        .await;
    assert_eq!(unassigned_update.status(), StatusCode::OK);
    assert_eq!(
        response_json(unassigned_update).await["projectId"],
        Value::Null
    );

    let group = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/admin/groups",
            Some(json!({ "name": "Developers" })),
            Some(&admin_cookies),
            Some(&admin_csrf),
        ))
        .await;
    assert_eq!(group.status(), StatusCode::CREATED);
    let group_id = response_json(group).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let members = fixture
        .request(json_request(
            Method::PUT,
            &format!("/api/v1/admin/groups/{group_id}/members"),
            Some(json!({ "memberIds": [fixture.member_id] })),
            Some(&admin_cookies),
            Some(&admin_csrf),
        ))
        .await;
    assert_eq!(members.status(), StatusCode::OK);
    let policy = fixture
        .request(json_request(
            Method::PUT,
            &format!("/api/v1/projects/{}/access", fixture.project_id),
            Some(json!({
                "users": [],
                "groups": [{ "granteeId": group_id, "read": true, "write": false }],
                "machines": []
            })),
            Some(&admin_cookies),
            Some(&admin_csrf),
        ))
        .await;
    assert_eq!(policy.status(), StatusCode::OK);

    let projects = fixture
        .request(json_request(
            Method::GET,
            "/api/v1/projects",
            None,
            Some(&member_cookies),
            None,
        ))
        .await;
    let projects = response_json(projects).await;
    assert_eq!(projects.as_array().unwrap().len(), 1);
    assert_eq!(projects[0]["permissions"]["read"], true);
    assert_eq!(projects[0]["permissions"]["write"], false);
    let create_denied = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/secrets",
            Some(json!({
                "key": "DENIED",
                "value": "denied",
                "note": "",
                "projectId": fixture.project_id
            })),
            Some(&member_cookies),
            Some(&member_csrf),
        ))
        .await;
    assert_eq!(create_denied.status(), StatusCode::FORBIDDEN);

    let direct = fixture
        .request(json_request(
            Method::PUT,
            &format!("/api/v1/secrets/{}/access", fixture.secret_id),
            Some(json!({
                "users": [{ "granteeId": fixture.member_id, "read": true, "write": true }],
                "groups": [],
                "machines": []
            })),
            Some(&admin_cookies),
            Some(&admin_csrf),
        ))
        .await;
    assert_eq!(direct.status(), StatusCode::OK);
    let update = fixture
        .request(json_request(
            Method::PUT,
            &format!("/api/v1/secrets/{}", fixture.secret_id),
            Some(json!({
                "key": "DATABASE_URL",
                "value": "sqlite://updated",
                "note": "updated",
                "projectId": fixture.project_id
            })),
            Some(&member_cookies),
            Some(&member_csrf),
        ))
        .await;
    assert_eq!(update.status(), StatusCode::OK);
    assert_eq!(response_json(update).await["value"], "sqlite://updated");

    let machine = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/admin/machines",
            Some(json!({ "name": "deploy" })),
            Some(&admin_cookies),
            Some(&admin_csrf),
        ))
        .await;
    assert_eq!(machine.status(), StatusCode::CREATED);
    let machine_id = response_json(machine).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let machine_access = fixture
        .request(json_request(
            Method::PUT,
            &format!("/api/v1/machines/{machine_id}/access"),
            Some(json!({
                "users": [{ "granteeId": fixture.member_id, "read": true, "write": false }],
                "groups": [],
                "projects": [{ "granteeId": fixture.project_id, "read": true, "write": true }]
            })),
            Some(&admin_cookies),
            Some(&admin_csrf),
        ))
        .await;
    assert_eq!(machine_access.status(), StatusCode::OK);
    let machine_access = response_json(machine_access).await;
    assert_eq!(machine_access["users"][0]["granteeId"], fixture.member_id);
    assert_eq!(machine_access["projects"][0]["write"], true);
    let machine_token = fixture
        .request(json_request(
            Method::POST,
            &format!("/api/v1/admin/machines/{machine_id}/tokens"),
            Some(json!({ "name": "CI", "expiresAt": null })),
            Some(&admin_cookies),
            Some(&admin_csrf),
        ))
        .await;
    assert_eq!(machine_token.status(), StatusCode::CREATED);
    let machine_token_id = response_json(machine_token).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let revoked_token = fixture
        .request(json_request(
            Method::PUT,
            &format!("/api/v1/admin/machines/{machine_id}/tokens/{machine_token_id}/revoke"),
            None,
            Some(&admin_cookies),
            Some(&admin_csrf),
        ))
        .await;
    assert_eq!(revoked_token.status(), StatusCode::OK);
    let machine_events = fixture
        .request(json_request(
            Method::GET,
            &format!("/api/v1/admin/machines/{machine_id}/events"),
            None,
            Some(&admin_cookies),
            None,
        ))
        .await;
    assert_eq!(machine_events.status(), StatusCode::OK);
    let machine_events = response_json(machine_events).await;
    assert!(machine_events.as_array().unwrap().iter().any(|event| {
        event["action"] == "machine.token.revoke" && event["resourceId"] == machine_token_id
    }));

    let project_admin_denied = fixture
        .request(json_request(
            Method::PUT,
            &format!("/api/v1/projects/{}", fixture.project_id),
            Some(json!({ "name": "Escalated" })),
            Some(&member_cookies),
            Some(&member_csrf),
        ))
        .await;
    assert_eq!(project_admin_denied.status(), StatusCode::FORBIDDEN);

    let audit = fixture
        .request(json_request(
            Method::GET,
            "/api/v1/audit",
            None,
            Some(&admin_cookies),
            None,
        ))
        .await;
    let audit = response_json(audit).await;
    assert!(
        audit.as_array().unwrap().iter().any(|event| {
            event["action"] == "policy.replace" && event["resourceKind"] == "secret"
        })
    );

    let settings = fixture
        .request(json_request(
            Method::PUT,
            "/api/v1/audit/settings",
            Some(json!({
                "enabled": false,
                "autoCleanupEnabled": true,
                "retentionDays": 30
            })),
            Some(&admin_cookies),
            Some(&admin_csrf),
        ))
        .await;
    assert_eq!(settings.status(), StatusCode::OK);
    let settings = response_json(settings).await;
    assert_eq!(settings["enabled"], false);
    assert_eq!(settings["retentionDays"], 30);

    let clear = fixture
        .request(json_request(
            Method::DELETE,
            "/api/v1/audit",
            None,
            Some(&admin_cookies),
            Some(&admin_csrf),
        ))
        .await;
    assert_eq!(clear.status(), StatusCode::OK);
    assert!(response_json(clear).await["deleted"].as_u64().unwrap() >= 2);
    let cleared = fixture
        .request(json_request(
            Method::GET,
            "/api/v1/audit",
            None,
            Some(&admin_cookies),
            None,
        ))
        .await;
    assert!(response_json(cleared).await.as_array().unwrap().is_empty());
}

fn json_request(
    method: Method,
    uri: &str,
    body: Option<Value>,
    cookies: Option<&str>,
    csrf: Option<&str>,
) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }
    if let Some(cookies) = cookies {
        builder = builder.header(header::COOKIE, cookies);
    }
    if let Some(csrf) = csrf {
        builder = builder.header("x-csrf-token", csrf);
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
