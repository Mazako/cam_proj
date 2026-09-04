use std::time::Duration as StdDuration;

use axum::{
    extract::{Form, Request, State},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use subtle::ConstantTimeEq;
use time::Duration;
use tokio::time::sleep;
use tower_sessions::{Expiry, MemoryStore, Session, SessionManagerLayer};
use uuid::Uuid;

use crate::{app_state::AppState, views};

pub(crate) const AUTHENTICATED_KEY: &str = "authenticated";
const CSRF_TOKEN_KEY: &str = "csrf_token";
const SESSION_COOKIE_NAME: &str = "camwatch-session";
const INVALID_LOGIN_DELAY: StdDuration = StdDuration::from_millis(250);

#[derive(Deserialize)]
pub(crate) struct LoginForm {
    login: String,
    password: String,
    csrf_token: String,
}

#[derive(Deserialize)]
pub(crate) struct LogoutForm {
    csrf_token: String,
}

pub(crate) fn session_layer(expiry: Duration) -> SessionManagerLayer<MemoryStore> {
    SessionManagerLayer::new(MemoryStore::default())
        .with_name(SESSION_COOKIE_NAME)
        .with_http_only(true)
        .with_same_site(tower_sessions::cookie::SameSite::Strict)
        .with_secure(false)
        .with_path("/")
        .with_expiry(Expiry::OnInactivity(expiry))
        .with_always_save(true)
}

pub(crate) async fn require_auth(request: Request, next: Next) -> Response {
    let Some(session) = request.extensions().get::<Session>().cloned() else {
        return views::internal_error_response();
    };

    match session.get::<bool>(AUTHENTICATED_KEY).await {
        Ok(Some(true)) => next.run(request).await,
        Ok(_) => Redirect::to("/login").into_response(),
        Err(_) => {
            tracing::error!("failed to read authentication session");
            views::internal_error_response()
        }
    }
}

pub(crate) async fn login_page(session: Session) -> Response {
    match session.get::<bool>(AUTHENTICATED_KEY).await {
        Ok(Some(true)) => return Redirect::to("/cameras").into_response(),
        Ok(_) => {}
        Err(_) => {
            tracing::error!("failed to read authentication session");
            return views::internal_error_response();
        }
    }

    let csrf_token = match csrf_token(&session).await {
        Ok(token) => token,
        Err(_) => return views::internal_error_response(),
    };
    views::login_page_response(csrf_token, false)
}

pub(crate) async fn login(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<LoginForm>,
) -> Response {
    if !valid_csrf(&session, &form.csrf_token).await {
        return views::forbidden_response();
    }

    if !state.auth.verify(&form.login, &form.password) {
        sleep(INVALID_LOGIN_DELAY).await;
        let token = match csrf_token(&session).await {
            Ok(token) => token,
            Err(_) => return views::internal_error_response(),
        };
        return views::login_page_response(token, true);
    }

    if session.cycle_id().await.is_err() {
        tracing::error!("failed to rotate authentication session");
        return views::internal_error_response();
    }
    if session.insert(AUTHENTICATED_KEY, true).await.is_err() {
        tracing::error!("failed to save authentication session");
        return views::internal_error_response();
    }

    Redirect::to("/cameras").into_response()
}

pub(crate) async fn logout(session: Session, Form(form): Form<LogoutForm>) -> Response {
    if !valid_csrf(&session, &form.csrf_token).await {
        return views::forbidden_response();
    }

    if session.flush().await.is_err() {
        tracing::error!("failed to delete authentication session");
        return views::internal_error_response();
    }

    Redirect::to("/login").into_response()
}

pub(crate) async fn protected_home(session: Session) -> Response {
    let csrf_token = match csrf_token(&session).await {
        Ok(token) => token,
        Err(_) => return views::internal_error_response(),
    };
    views::home_page_response(csrf_token)
}

pub(crate) async fn csrf_token(
    session: &Session,
) -> Result<String, tower_sessions::session::Error> {
    if let Some(token) = session.get::<String>(CSRF_TOKEN_KEY).await? {
        return Ok(token);
    }

    let token = Uuid::now_v7().to_string();
    session.insert(CSRF_TOKEN_KEY, &token).await?;
    Ok(token)
}

async fn valid_csrf(session: &Session, provided: &str) -> bool {
    let Ok(Some(expected)) = session.get::<String>(CSRF_TOKEN_KEY).await else {
        return false;
    };
    expected.as_bytes().ct_eq(provided.as_bytes()).into()
}
