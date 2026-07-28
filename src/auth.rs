use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};

use axum::{
    Json,
    extract::{ConnectInfo, Extension, FromRequestParts, State},
    http::{HeaderMap, HeaderValue, header, request::Parts},
    response::IntoResponse,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use cookie::{Cookie, SameSite};
use rand::TryRng;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::{
    AppState,
    db::entities::session,
    domain::{
        now,
        users::{PublicUser, Role, UserRepository, verify_password},
    },
    error::AppError,
};

const SESSION_COOKIE: &str = "lightbws_session";
const CSRF_COOKIE: &str = "lightbws_csrf";
const SESSION_TTL_SECONDS: i64 = 12 * 60 * 60;

#[derive(Clone)]
pub struct AuthenticatedSession {
    pub user_id: Uuid,
    pub csrf_digest: String,
    pub csrf_token: String,
    pub session_digest: String,
    pub user: PublicUser,
}

pub struct MutationSession(pub AuthenticatedSession);

#[derive(Deserialize)]
pub struct LoginInput {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResponse {
    pub user: PublicUser,
    pub csrf_token: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthConfig {
    pub initialized: bool,
    pub local_enabled: bool,
}

pub async fn config(State(state): State<AppState>) -> Result<Json<AuthConfig>, AppError> {
    use crate::db::entities::user;
    Ok(Json(AuthConfig {
        initialized: user::Entity::find()
            .one(state.db.connection())
            .await?
            .is_some(),
        local_enabled: true,
    }))
}

pub async fn login(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    Json(input): Json<LoginInput>,
) -> Result<impl IntoResponse, AppError> {
    let rate_key = login_rate_key(&input.username, peer.as_ref().map(|peer| peer.0.0));
    check_login_rate(&state, &rate_key).await?;
    let repository = UserRepository::new(state.db.clone());
    let model = match repository.find_login(&input.username).await {
        Ok(model) => model,
        Err(_) => {
            record_login_failure(&state, &rate_key).await;
            return Err(AppError::Unauthorized);
        }
    };
    let hash = model.password_hash.clone();
    let password = input.password;
    let valid = tokio::task::spawn_blocking(move || verify_password(&hash, &password))
        .await
        .map_err(AppError::internal)?;
    if !valid || model.disabled {
        record_login_failure(&state, &rate_key).await;
        return Err(AppError::Unauthorized);
    }
    clear_login_failures(&state, &rate_key).await;
    let user = repository.record_login(model).await?;
    let (token, csrf_token) = create_session(&state, user.id).await?;
    let cookie = Cookie::build((SESSION_COOKIE, token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(state.cookie_secure)
        .max_age(cookie::time::Duration::seconds(SESSION_TTL_SECONDS))
        .build();
    let mut response_headers = HeaderMap::new();
    response_headers.append(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie.to_string()).map_err(AppError::internal)?,
    );
    let csrf_cookie = Cookie::build((CSRF_COOKIE, csrf_token.clone()))
        .path("/")
        .http_only(false)
        .same_site(SameSite::Strict)
        .secure(state.cookie_secure)
        .max_age(cookie::time::Duration::seconds(SESSION_TTL_SECONDS))
        .build();
    response_headers.append(
        header::SET_COOKIE,
        HeaderValue::from_str(&csrf_cookie.to_string()).map_err(AppError::internal)?,
    );
    Ok((response_headers, Json(SessionResponse { user, csrf_token })))
}

pub async fn current(session: AuthenticatedSession) -> Json<SessionResponse> {
    Json(SessionResponse {
        user: session.user,
        csrf_token: session.csrf_token,
    })
}

pub async fn logout(
    State(state): State<AppState>,
    mutation: MutationSession,
) -> Result<impl IntoResponse, AppError> {
    session::Entity::delete_by_id(mutation.0.session_digest)
        .exec(state.db.connection())
        .await?;
    let expired = Cookie::build((SESSION_COOKIE, ""))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(cookie::time::Duration::ZERO)
        .build();
    let mut headers = HeaderMap::new();
    headers.append(
        header::SET_COOKIE,
        HeaderValue::from_str(&expired.to_string()).map_err(AppError::internal)?,
    );
    let csrf_expired = Cookie::build((CSRF_COOKIE, ""))
        .path("/")
        .same_site(SameSite::Strict)
        .max_age(cookie::time::Duration::ZERO)
        .build();
    headers.append(
        header::SET_COOKIE,
        HeaderValue::from_str(&csrf_expired.to_string()).map_err(AppError::internal)?,
    );
    Ok((headers, axum::http::StatusCode::NO_CONTENT))
}

pub async fn revoke_user_sessions(state: &AppState, user_id: Uuid) -> Result<(), AppError> {
    session::Entity::delete_many()
        .filter(session::Column::UserId.eq(user_id.to_string()))
        .exec(state.db.connection())
        .await?;
    Ok(())
}

pub fn require_admin(user: &PublicUser) -> Result<(), AppError> {
    if user.role == Role::Admin && !user.disabled {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

impl FromRequestParts<AppState> for AuthenticatedSession {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = cookie_value(&parts.headers, SESSION_COOKIE).ok_or(AppError::Unauthorized)?;
        let session_digest = digest(&token);
        let model = session::Entity::find_by_id(&session_digest)
            .one(state.db.connection())
            .await?
            .ok_or(AppError::Unauthorized)?;
        if model.expires_at <= now() {
            session::Entity::delete_by_id(&session_digest)
                .exec(state.db.connection())
                .await?;
            return Err(AppError::Unauthorized);
        }
        let user_id = Uuid::parse_str(&model.user_id).map_err(AppError::internal)?;
        let user = UserRepository::new(state.db.clone()).get(user_id).await?;
        if user.disabled {
            return Err(AppError::Unauthorized);
        }
        let csrf_token = cookie_value(&parts.headers, CSRF_COOKIE).ok_or(AppError::Unauthorized)?;
        if digest(&csrf_token)
            .as_bytes()
            .ct_eq(model.csrf_digest.as_bytes())
            .unwrap_u8()
            != 1
        {
            return Err(AppError::Unauthorized);
        }
        Ok(Self {
            user_id,
            csrf_digest: model.csrf_digest,
            csrf_token,
            session_digest,
            user,
        })
    }
}

impl FromRequestParts<AppState> for MutationSession {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let session = AuthenticatedSession::from_request_parts(parts, state).await?;
        let supplied = parts
            .headers
            .get("x-csrf-token")
            .and_then(|value| value.to_str().ok())
            .ok_or(AppError::Csrf)?;
        if digest(supplied)
            .as_bytes()
            .ct_eq(session.csrf_digest.as_bytes())
            .unwrap_u8()
            != 1
        {
            return Err(AppError::Csrf);
        }
        Ok(Self(session))
    }
}

async fn create_session(state: &AppState, user_id: Uuid) -> Result<(String, String), AppError> {
    session::Entity::delete_many()
        .filter(session::Column::ExpiresAt.lte(now()))
        .exec(state.db.connection())
        .await?;
    let token = random_token()?;
    let csrf_token = random_token()?;
    session::ActiveModel {
        id: Set(digest(&token)),
        user_id: Set(user_id.to_string()),
        csrf_digest: Set(digest(&csrf_token)),
        expires_at: Set(now() + SESSION_TTL_SECONDS),
        created_at: Set(now()),
    }
    .insert(state.db.connection())
    .await?;
    Ok((token, csrf_token))
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == name).then(|| value.to_owned())
        })
}

fn random_token() -> Result<String, AppError> {
    let mut bytes = [0_u8; 32];
    rand::rng()
        .try_fill_bytes(&mut bytes)
        .map_err(AppError::internal)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

fn login_rate_key(username: &str, peer: Option<SocketAddr>) -> String {
    format!(
        "{}|{}",
        username.trim().to_lowercase(),
        peer.map_or_else(|| "unknown".into(), |peer| peer.ip().to_string())
    )
}

async fn check_login_rate(state: &AppState, key: &str) -> Result<(), AppError> {
    let mut failures = state.login_failures.lock().await;
    let cutoff = Instant::now() - Duration::from_secs(60);
    failures.retain(|_, attempts| {
        while attempts.front().is_some_and(|time| *time < cutoff) {
            attempts.pop_front();
        }
        !attempts.is_empty()
    });
    if failures
        .get(key)
        .is_some_and(|attempts| attempts.len() >= 8)
    {
        return Err(AppError::RateLimited);
    }
    Ok(())
}

async fn record_login_failure(state: &AppState, key: &str) {
    let mut failures = state.login_failures.lock().await;
    if failures.len() >= 4096
        && !failures.contains_key(key)
        && let Some(oldest) = failures
            .iter()
            .min_by_key(|(_, attempts)| attempts.front().copied())
            .map(|(key, _)| key.clone())
    {
        failures.remove(&oldest);
    }
    let attempts = failures.entry(key.to_owned()).or_default();
    if attempts.len() < 8 {
        attempts.push_back(Instant::now());
    }
}

async fn clear_login_failures(state: &AppState, key: &str) {
    state.login_failures.lock().await.remove(key);
}
