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
        let db = Database::connect(&data.path().join("auth.sqlite3"))
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
        let state = AppState::new(db, &config);
        Self {
            _data: data,
            app: create_app(state),
        }
    }

    async fn request(&self, request: Request<Body>) -> axum::response::Response {
        self.app.clone().oneshot(request).await.expect("request")
    }

    async fn login(&self) -> (String, String) {
        let response = self
            .request(json_request(
                Method::POST,
                "/api/v1/auth/login",
                json!({ "username": "admin", "password": "test-password-123" }),
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
async fn login_sets_hardened_cookie_and_csrf_protects_mutations() {
    let fixture = Fixture::new().await;
    let response = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/auth/login",
            json!({ "username": "admin", "password": "test-password-123" }),
            None,
            None,
        ))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let set_cookie = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("SameSite=Lax"));
    let body = response_json(response).await;
    assert!(
        body["csrfToken"]
            .as_str()
            .is_some_and(|value| value.len() > 32)
    );

    let (cookies, csrf) = fixture.login().await;
    let rejected = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/projects",
            json!({ "name": "Denied" }),
            Some(&cookies),
            None,
        ))
        .await;
    assert_eq!(rejected.status(), StatusCode::FORBIDDEN);
    let accepted = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/projects",
            json!({ "name": "Accepted" }),
            Some(&cookies),
            Some(&csrf),
        ))
        .await;
    assert_eq!(accepted.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn repeated_invalid_logins_are_rate_limited_without_account_disclosure() {
    let fixture = Fixture::new().await;
    for index in 0..8 {
        let response = fixture
            .request(json_request(
                Method::POST,
                "/api/v1/auth/login",
                json!({ "username": format!("missing-{index}"), "password": "wrong-password" }),
                None,
                None,
            ))
            .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(
            !response_json(response)
                .await
                .to_string()
                .contains("missing")
        );
    }
    let unaffected = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/auth/login",
            json!({ "username": "admin", "password": "test-password-123" }),
            None,
            None,
        ))
        .await;
    assert_eq!(unaffected.status(), StatusCode::OK);

    for _ in 0..8 {
        let response = fixture
            .request(json_request(
                Method::POST,
                "/api/v1/auth/login",
                json!({ "username": "admin", "password": "wrong-password" }),
                None,
                None,
            ))
            .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let limited = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/auth/login",
            json!({ "username": " ADMIN ", "password": "test-password-123" }),
            None,
            None,
        ))
        .await;
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(limited.headers()[header::RETRY_AFTER], "60");
}

fn json_request(
    method: Method,
    uri: &str,
    body: Value,
    cookies: Option<&str>,
    csrf: Option<&str>,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(cookies) = cookies {
        builder = builder.header(header::COOKIE, cookies);
    }
    if let Some(csrf) = csrf {
        builder = builder.header("x-csrf-token", csrf);
    }
    builder.body(Body::from(body.to_string())).unwrap()
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
