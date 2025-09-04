mod auth;
mod content;
mod discord;
mod legacy;
mod wrapper;

use std::{borrow::Cow, sync::Arc};

use anyhow::Result;
use auth::Authorized;
use axum::{Json, Router, extract::State, http::request::Parts, routing::post, serve};
use base64::{Engine, prelude::BASE64_STANDARD};
use discord::{read_discord, schedule_send_discord};
use legacy::{JoinOrLeaveEvent, LegacyChat, LegacyChatResponse};
use regex::Regex;
use serde::Deserialize;
use tokio::{
    main,
    net::TcpListener,
    select,
    signal::{
        ctrl_c,
        unix::{SignalKind, signal},
    },
    spawn,
    sync::{mpsc::unbounded_channel, oneshot},
    task::JoinSet,
};
use tracing::{error, info};
use tracing_subscriber::fmt;
use twilight_http::Client;
use twilight_model::{
    channel::message::{AllowedMentions, MentionType},
    id::{
        Id,
        marker::{ChannelMarker, WebhookMarker},
    },
};
use wrapper::launch_wrapper;

#[inline]
const fn default_bind_address() -> Cow<'static, str> {
    Cow::Borrowed("127.0.0.1:8080")
}

#[inline]
const fn default_tellraw_prefix() -> Cow<'static, str> {
    Cow::Borrowed("tellraw @a")
}

#[derive(Debug, Deserialize)]
struct DiscordConfig {
    token: String,
    channel_id: u64,
}

#[derive(Debug, Deserialize)]
struct Config {
    api_key: String,
    #[serde(default = "default_bind_address")]
    bind_address: Cow<'static, str>,
    webhook_id: u64,
    webhook_token: String,
    #[serde(default)]
    discord: Option<DiscordConfig>,
    #[serde(default)]
    allow_everyone_mention: bool,
    #[serde(default)]
    allow_user_mention: bool,
    #[serde(default)]
    allow_role_mention: bool,
    #[serde(default)]
    embed_url: bool,
    #[serde(default = "default_tellraw_prefix")]
    tellraw_prefix: Cow<'static, str>,
}

#[derive(Debug, Clone)]
struct AppState {
    client: Arc<Client>,
    expected_auth_header: Arc<str>,
    webhook_id: Id<WebhookMarker>,
    webhook_token: Arc<str>,
    discord_username_regex: Arc<Regex>,
    formatting_regex: Arc<Regex>,
    embed_url: bool,
}

#[main]
async fn main() -> Result<()> {
    fmt().init();

    let config: Config = serde_env::from_env()?;

    let mut parse_mentions = Vec::new();

    if config.allow_user_mention {
        parse_mentions.push(MentionType::Users);
    }

    if config.allow_role_mention {
        parse_mentions.push(MentionType::Roles);
    }

    if config.allow_everyone_mention {
        parse_mentions.push(MentionType::Everyone);
    }

    let mut client_builder = Client::builder().default_allowed_mentions(AllowedMentions {
        parse: parse_mentions,
        ..Default::default()
    });

    let discord_config = if let Some(discord) = config.discord {
        client_builder = client_builder.token(discord.token.clone());
        Some((discord.token, Id::<ChannelMarker>::new(discord.channel_id)))
    } else {
        None
    };

    let client = Arc::new(client_builder.build());
    let webhook_id = Id::new(config.webhook_id);

    let state = AppState {
        client: client.clone(),
        expected_auth_header: format!("Basic {}", BASE64_STANDARD.encode(&config.api_key)).into(),
        webhook_id,
        webhook_token: config.webhook_token.into(),
        discord_username_regex: Arc::new(Regex::new(r#"(?i)(d)(i)(scord)"#)?),
        formatting_regex: Arc::new(Regex::new(r#"([\\_`*>|-~\[\]()#])"#)?),
        embed_url: config.embed_url,
    };

    let app = Router::new()
        .route("/v1/chatx", post(chat))
        .route("/v1/join", post(join))
        .route("/v1/leave", post(leave))
        .with_state(state);

    let listener = TcpListener::bind(config.bind_address.as_ref()).await?;
    let mut tasks = JoinSet::new();

    tasks.spawn(async { serve(listener, app).await.map_err(anyhow::Error::from) });

    let (discord_message_sender, discord_message_receiver) = unbounded_channel();
    let (death_sender, death_receiver) = oneshot::channel();
    let mut server_launcher = spawn(launch_wrapper(
        discord_message_receiver,
        config.tellraw_prefix.into_owned(),
        death_receiver,
    ));

    if let Some((token, channel_id)) = discord_config {
        tasks.spawn(read_discord(
            token,
            channel_id,
            webhook_id,
            discord_message_sender,
        ));
    }

    let mut sig_term = signal(SignalKind::terminate())?;
    select! {
        res = tasks.join_next() => {
            error!("task failed {:?}", res)
        }
        _ = ctrl_c() => {
            info!("ctrl+c")
        }
        _ = sig_term.recv() => {
            info!("sigint")
        }
        _ = &mut server_launcher => {
            info!("server died");
        }
    }

    let _ = death_sender.send(());
    server_launcher.await??;
    tasks.abort_all();

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
    schedule_send_discord(&state, chat.profile.user_display_name.into(), chat.text);

    Json(&PASS_THROUGH_RESPONSE)
}

#[inline]
async fn join(
    State(state): State<AppState>,
    _authorized: Authorized,
    Json(join): Json<JoinOrLeaveEvent>,
) {
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
    schedule_send_discord(
        &state,
        "System".into(),
        format!("{} left the game", leave.profile.user_display_name),
    );
}
