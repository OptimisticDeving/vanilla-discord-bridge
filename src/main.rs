mod auth;
mod discord;
mod legacy;

use std::{borrow::Cow, iter::once, sync::Arc};

use anyhow::Result;
use auth::Authorized;
use axum::{
    Json, Router,
    extract::State,
    http::{header::AUTHORIZATION, request::Parts},
    routing::post,
    serve,
};
use base64::{Engine, prelude::BASE64_STANDARD};
use discord::schedule_send_discord;
use legacy::{JoinOrLeaveEvent, LegacyChat, LegacyChatResponse};
use serde::Deserialize;
use tokio::{main, net::TcpListener};
use tower_http::{sensitive_headers::SetSensitiveHeadersLayer, trace::TraceLayer};
use tracing::info;
use tracing_subscriber::fmt;
use twilight_http::Client;
use twilight_model::{
    channel::message::AllowedMentions,
    id::{Id, marker::WebhookMarker},
};

#[inline]
const fn default_bind_address() -> Cow<'static, str> {
    Cow::Borrowed("[::]:8080")
}

#[derive(Debug, Deserialize)]
struct Config {
    api_key: String,
    #[serde(default = "default_bind_address")]
    bind_address: Cow<'static, str>,
    webhook_id: u64,
    webhook_token: String,
}

#[derive(Debug, Clone)]
struct AppState {
    client: Arc<Client>,
    expected_auth_header: Arc<str>,
    webhook_id: Id<WebhookMarker>,
    webhook_token: Arc<str>,
}

#[main]
async fn main() -> Result<()> {
    fmt().init();

    let config: Config = serde_env::from_env()?;
    let state = AppState {
        client: Client::builder()
            .default_allowed_mentions(AllowedMentions {
                parse: Vec::new(),
                replied_user: false,
                roles: Vec::new(),
                users: Vec::new(),
            })
            .build()
            .into(),
        expected_auth_header: format!("Basic {}", BASE64_STANDARD.encode(&config.api_key)).into(),
        webhook_id: Id::new(config.webhook_id),
        webhook_token: config.webhook_token.into(),
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
    State(state): State<AppState>,
    _authorized: Authorized,
    Json(chat): Json<LegacyChat>,
) -> Json<&'static LegacyChatResponse> {
    info!(
        "chat from {} ({}): {}",
        chat.profile.user_display_name, chat.profile.user_id, chat.text
    );

    schedule_send_discord(&state, chat.profile.user_display_name.into(), chat.text);

    Json(&PASS_THROUGH_RESPONSE)
}

#[inline]
async fn join(
    State(state): State<AppState>,
    _authorized: Authorized,
    Json(join): Json<JoinOrLeaveEvent>,
) {
    info!(
        "join from {} ({})",
        join.profile.user_display_name, join.profile.user_id
    );

    schedule_send_discord(
        &state,
        "System".into(),
        format!("{} joined the game", join.profile.user_display_name),
    );
}

#[inline]
async fn leave(
    State(state): State<AppState>,
    _authorized: Authorized,
    Json(leave): Json<JoinOrLeaveEvent>,
) {
    info!(
        "leave from {} ({})",
        leave.profile.user_display_name, leave.profile.user_id
    );

    schedule_send_discord(
        &state,
        "System".into(),
        format!("{} left the game", leave.profile.user_display_name),
    );
}
