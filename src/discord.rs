use anyhow::Result;
use regex::Regex;
use std::{borrow::Cow, sync::Arc};
use tokio::{spawn, sync::mpsc::UnboundedSender};
use twilight_gateway::{Event, EventTypeFlags, Intents, Shard, ShardId, StreamExt};
use uuid::{Uuid, fmt::Simple};

use tracing::warn;
use twilight_http::Client;
use twilight_model::id::{
    Id,
    marker::{ChannelMarker, WebhookMarker},
};

use crate::{AppState, content::escape_minecraft};

#[inline]
pub fn schedule_send_discord(
    state: &AppState,
    sender: Cow<'static, str>,
    sender_id: Option<Uuid>,
    content: String,
) {
    spawn(send_discord(
        state.client.clone(),
        state.webhook_id.clone(),
        state.webhook_token.clone(),
        state.discord_username_regex.clone(),
        state.formatting_regex.clone(),
        state.embed_url.clone(),
        sender,
        sender_id,
        content,
    ));
}

#[inline]
async fn send_discord(
    client: Arc<Client>,
    webhook_id: Id<WebhookMarker>,
    webhook_token: Arc<str>,
    discord_username_regex: Arc<Regex>,
    formatting_regex: Arc<Regex>,
    embed_url: bool,
    sender_name: Cow<'static, str>,
    sender_id: Option<Uuid>,
    content: String,
) {
    let mut escaped_formatting = formatting_regex.replace_all(&content, "\\$1");

    if !embed_url {
        escaped_formatting = Cow::Owned(escaped_formatting.replace(":", "\\:"))
    }

    let username = discord_username_regex.replace_all(sender_name.as_ref(), "$1¡$3");

    let mut message_builder = client
        .execute_webhook(webhook_id, &webhook_token)
        .content(&escaped_formatting)
        .username(&username);

    let avatar_url = sender_id.map(|id| {
        format!(
            "https://minotar.net/helm/{}",
            Simple::from_uuid(id).to_string()
        )
    });

    if let Some(avatar_url) = avatar_url.as_ref() {
        message_builder = message_builder.avatar_url(avatar_url);
    }

    match message_builder.await {
        Ok(_) => {}
        Err(e) => warn!(?e, "failure sending message to webhook"),
    }
}

#[derive(Debug)]
pub struct IncomingDiscordMessage {
    pub username: String,
    pub content: String,
}

#[inline]
fn escape_for_component(inp: &str) -> String {
    inp.replace("\\", "\\\\").replace("\"", "\\\"")
}

impl IncomingDiscordMessage {
    #[inline]
    pub fn create_command(self, tellraw_prefix: &str) -> String {
        format!(
            r#"{tellraw_prefix} "<{}> {}"
"#,
            escape_minecraft(&escape_for_component(&self.username)),
            escape_minecraft(&escape_for_component(&self.content))
        )
    }
}

#[inline]
pub async fn read_discord(
    token: String,
    channel_id: Id<ChannelMarker>,
    webhook_id: Id<WebhookMarker>,
    discord_message_sender: UnboundedSender<IncomingDiscordMessage>,
) -> Result<()> {
    let mut shard = Shard::new(
        ShardId::ONE,
        token,
        Intents::MESSAGE_CONTENT | Intents::GUILD_MESSAGES,
    );

    while let Some(event) = shard.next_event(EventTypeFlags::MESSAGE_CREATE).await {
        let Ok(event) = event else {
            warn!(source = ?event.unwrap_err(), "event error");
            continue;
        };

        match event {
            Event::MessageCreate(event) => {
                if event.channel_id != channel_id
                    || event.webhook_id.is_some_and(|id| id == webhook_id)
                    || event.content.is_empty()
                {
                    continue;
                }

                let escaped_name = escape_minecraft(&event.author.name);
                discord_message_sender.send(IncomingDiscordMessage {
                    username: if event.author.bot {
                        format!("[BOT] {escaped_name}")
                    } else {
                        escaped_name
                    },
                    content: escape_minecraft(&event.content),
                })?;
            }
            _ => continue,
        }
    }

    Ok(())
}
