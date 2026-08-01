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
    domain::users::{Role, UserRepository},
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

struct Fixture {
    _data: TempDir,
    app: Router,
}

impl Fixture {
    async fn new() -> Self {
        let data = tempfile::tempdir().expect("tempdir");
        let db = Database::connect(&data.path().join("admin.sqlite3"))
            .await
            .expect("database");
        UserRepository::new(db.clone())
            .create("admin", "Administrator", Role::Admin, "test-password-123")
            .await
            .expect("admin");
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
        }
    }

    async fn request(&self, request: Request<Body>) -> axum::response::Response {
        self.app.clone().oneshot(request).await.expect("request")
    }

    async fn login(&self, username: &str, password: &str) -> (String, String) {
        let response = self
            .request(json_request(
                Method::POST,
                "/api/v1/auth/login",
                Some(json!({ "username": username, "password": password })),
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
async fn administrator_manages_users_while_last_admin_and_member_boundaries_hold() {
    let fixture = Fixture::new().await;
    let (cookies, csrf) = fixture.login("admin", "test-password-123").await;
    let created = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/admin/users",
            Some(json!({
                "username": "member",
                "displayName": "Member",
                "role": "user",
                "password": "member-password-123"
            })),
            Some(&cookies),
            Some(&csrf),
        ))
        .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let member_id = response_json(created).await["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let (member_cookie, _) = fixture.login("member", "member-password-123").await;
    let denied = fixture
        .request(json_request(
            Method::GET,
            "/api/v1/admin/users",
            None,
            Some(&member_cookie),
            None,
        ))
        .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    let users = fixture
        .request(json_request(
            Method::GET,
            "/api/v1/admin/users",
            None,
            Some(&cookies),
            None,
        ))
        .await;
    let users = response_json(users).await;
    let admin = users
        .as_array()
        .unwrap()
        .iter()
        .find(|user| user["username"] == "admin")
        .unwrap();
    let last_admin = fixture
        .request(json_request(
            Method::PUT,
            &format!("/api/v1/admin/users/{}", admin["id"].as_str().unwrap()),
            Some(json!({ "displayName": "Administrator", "role": "user", "disabled": false })),
            Some(&cookies),
            Some(&csrf),
        ))
        .await;
    assert_eq!(last_admin.status(), StatusCode::CONFLICT);

    let promoted = fixture
        .request(json_request(
            Method::PUT,
            &format!("/api/v1/admin/users/{member_id}"),
            Some(json!({ "displayName": "Member", "role": "admin", "disabled": false })),
            Some(&cookies),
            Some(&csrf),
        ))
        .await;
    assert_eq!(promoted.status(), StatusCode::OK);

    let self_disable = fixture
        .request(json_request(
            Method::PUT,
            &format!("/api/v1/admin/users/{}", admin["id"].as_str().unwrap()),
            Some(json!({ "displayName": "Administrator", "role": "admin", "disabled": true })),
            Some(&cookies),
            Some(&csrf),
        ))
        .await;
    assert_eq!(self_disable.status(), StatusCode::CONFLICT);

    let unknown_field = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/admin/users",
            Some(json!({
                "username": "unexpected",
                "displayName": "Unexpected",
                "role": "user",
                "password": "member-password-123",
                "disabled": false
            })),
            Some(&cookies),
            Some(&csrf),
        ))
        .await;
    assert_eq!(unknown_field.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let invalid_display_name = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/admin/users",
            Some(json!({
                "username": "invalid-name",
                "displayName": "x".repeat(129),
                "role": "user",
                "password": "member-password-123"
            })),
            Some(&cookies),
            Some(&csrf),
        ))
        .await;
    assert_eq!(
        invalid_display_name.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        response_json(invalid_display_name).await["error"]["code"],
        "VALIDATION"
    );

    let reset = fixture
        .request(json_request(
            Method::PUT,
            &format!("/api/v1/admin/users/{member_id}/password"),
            Some(json!({ "password": "new-member-password-123" })),
            Some(&cookies),
            Some(&csrf),
        ))
        .await;
    assert_eq!(reset.status(), StatusCode::NO_CONTENT);
    fixture.login("member", "new-member-password-123").await;
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
