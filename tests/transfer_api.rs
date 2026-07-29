use axum::{
    Router,
    body::{Body, Bytes},
    http::{Method, Request, StatusCode, header},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use http_body_util::BodyExt;
use lightbws::{
    AppState,
    config::Config,
    create_app,
    crypto::MasterKey,
    db::Database,
    domain::{
        projects::ProjectRepository,
        transfer::{BackupScopes, dump_database_scoped, encode_plain_backup, encrypt_backup},
        users::{Role, UserRepository},
    },
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

struct Fixture {
    _data: TempDir,
    app: Router,
    db: Database,
    master_key: MasterKey,
    master_key_text: String,
}

impl Fixture {
    async fn new() -> Self {
        let data = tempfile::tempdir().expect("tempdir");
        let db = Database::connect(&data.path().join("transfer-api.sqlite3"))
            .await
            .expect("database");
        UserRepository::new(db.clone())
            .create("admin", "Administrator", Role::Admin, "test-password-123")
            .await
            .expect("admin");
        ProjectRepository::new(db.clone())
            .create_plain("Production")
            .await
            .expect("project");
        let config = Config {
            bind: "127.0.0.1:0".parse().expect("bind"),
            data_dir: data.path().into(),
            bootstrap_admin: None,
            cookie_secure: false,
            upstream_compatibility_account: false,
            allow_plaintext_backups: false,
            master_key: None,
        };
        let master_key_text = "11".repeat(32);
        let master_key = MasterKey::parse(&master_key_text).expect("master key");
        let app =
            create_app(AppState::new(db.clone(), &config).with_master_key(master_key.clone()));
        Self {
            _data: data,
            app,
            db,
            master_key,
            master_key_text,
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
        (
            cookies,
            body["csrfToken"].as_str().expect("csrf").to_owned(),
        )
    }
}

#[tokio::test]
async fn transfer_routes_detect_credentials_and_gate_plaintext_exports() {
    let fixture = Fixture::new().await;
    let unauthorized = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/transfer/export",
            json!({ "passphrase": "portable-passphrase-123" }),
            None,
            None,
        ))
        .await;
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let (cookies, csrf) = fixture.login().await;
    let exported = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/transfer/export",
            json!({ "passphrase": "portable-passphrase-123" }),
            Some(&cookies),
            Some(&csrf),
        ))
        .await;
    assert_eq!(exported.status(), StatusCode::OK);
    let exported = response_bytes(exported).await;
    assert!(exported.starts_with(b"LBWSX01\0"));
    let imported_portable = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/transfer/import",
            json!({
                "passphrase": "portable-passphrase-123",
                "dataBase64": STANDARD.encode(&exported)
            }),
            Some(&cookies),
            Some(&csrf),
        ))
        .await;
    assert_eq!(imported_portable.status(), StatusCode::OK);

    let oversized = fixture
        .request(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/transfer/import")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookies)
                .header("x-csrf-token", &csrf)
                .body(Body::from(vec![b' '; 128 * 1024 * 1024 + 1]))
                .expect("oversized request"),
        )
        .await;
    assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let plaintext_denied = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/transfer/export",
            json!({ "plaintext": true, "confirmPlaintext": true }),
            Some(&cookies),
            Some(&csrf),
        ))
        .await;
    assert_eq!(plaintext_denied.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let dump = dump_database_scoped(&fixture.db, &fixture.master_key, BackupScopes::default())
        .await
        .expect("dump");
    let automatic = encrypt_backup(&fixture.master_key, &dump).expect("automatic backup");
    let missing_key = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/transfer/import",
            json!({ "dataBase64": STANDARD.encode(&automatic) }),
            Some(&cookies),
            Some(&csrf),
        ))
        .await;
    assert_eq!(missing_key.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let imported = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/transfer/import",
            json!({ "masterKey": &fixture.master_key_text, "dataBase64": STANDARD.encode(&automatic) }),
            Some(&cookies),
            Some(&csrf),
        ))
        .await;
    assert_eq!(imported.status(), StatusCode::OK);

    let plain = encode_plain_backup(&dump).expect("plaintext backup");
    let imported_plain = fixture
        .request(json_request(
            Method::POST,
            "/api/v1/transfer/import",
            json!({ "dataBase64": STANDARD.encode(plain) }),
            Some(&cookies),
            Some(&csrf),
        ))
        .await;
    assert_eq!(imported_plain.status(), StatusCode::OK);
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
    builder.body(Body::from(body.to_string())).expect("request")
}

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&response_bytes(response).await).expect("json")
}

async fn response_bytes(response: axum::response::Response) -> Bytes {
    response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes()
}
