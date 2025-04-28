mod auth;
mod legacy;

use std::{borrow::Cow, iter::once, sync::Arc};

use anyhow::Result;
use auth::Authorized;
use axum::{
    Json, Router,
    http::{header::AUTHORIZATION, request::Parts},
    routing::post,
    serve,
};
use base64::{Engine, prelude::BASE64_STANDARD};
use legacy::{JoinOrLeaveEvent, LegacyChat, LegacyChatResponse};
use serde::Deserialize;
use tokio::{main, net::TcpListener};
use tower_http::{sensitive_headers::SetSensitiveHeadersLayer, trace::TraceLayer};
use tracing::info;
use tracing_subscriber::fmt;

#[inline]
const fn default_bind_address() -> Cow<'static, str> {
    Cow::Borrowed("[::]:8080")
}

#[derive(Debug, Deserialize)]
struct Config {
    pub api_key: String,
    #[serde(default = "default_bind_address")]
    pub bind_address: Cow<'static, str>,
}

#[derive(Debug, Clone)]
#[repr(transparent)]
struct State {
    expected_auth_header: Arc<str>,
}

#[main]
async fn main() -> Result<()> {
    fmt().init();

    let config: Config = serde_env::from_env()?;
    let state = State {
        expected_auth_header: format!("Basic {}", BASE64_STANDARD.encode(&config.api_key)).into(),
    };

    let app = Router::new()
        .layer(SetSensitiveHeadersLayer::new(once(AUTHORIZATION)))
        .layer(TraceLayer::new_for_http())
        .route("/v1/chatx", post(chat))
        .route("/v1/join", post(join))
        .route("/v1/leave", post(leave))
        .with_state(state);

    let listener = TcpListener::bind(config.bind_address.as_ref()).await?;
    serve(listener, app).await?;
    Ok(())
}

#[inline]
fn has_header_and_matches<P: FnOnce(&str) -> bool>(
    parts: &Parts,
    header_name: &str,
    value_predicate: P,
) -> bool {
    parts
        .headers
        .get(header_name)
        .and_then(|header| header.to_str().ok())
        .is_some_and(value_predicate)
}

const PASS_THROUGH_RESPONSE: LegacyChatResponse = LegacyChatResponse { pass_through: true };

#[inline]
async fn chat(
    _authorized: Authorized,
    Json(chat): Json<LegacyChat>,
) -> Json<&'static LegacyChatResponse> {
    info!(
        "chat from {} ({}): {}",
        chat.profile.user_display_name, chat.profile.user_id, chat.text
    );

    Json(&PASS_THROUGH_RESPONSE)
}

#[inline]
async fn join(_authorized: Authorized, Json(join): Json<JoinOrLeaveEvent>) {
    info!(
        "join from {} ({})",
        join.profile.user_display_name, join.profile.user_id
    )
}

#[inline]
async fn leave(_authorized: Authorized, Json(leave): Json<JoinOrLeaveEvent>) {
    info!(
        "leave from {} ({})",
        leave.profile.user_display_name, leave.profile.user_id
    )
}
