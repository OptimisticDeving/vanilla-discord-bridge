use anyhow::Result;
use regex::Regex;
use std::{borrow::Cow, sync::Arc};
use tokio::{spawn, sync::mpsc::UnboundedSender};
use twilight_gateway::{Event, EventTypeFlags, Intents, Shard, ShardId, StreamExt};

use tracing::warn;
use twilight_http::Client;
use twilight_model::id::{
    Id,
    marker::{ChannelMarker, WebhookMarker},
};

use crate::{AppState, content::escape_minecraft};

#[inline]
pub fn schedule_send_discord(state: &AppState, sender: Cow<'static, str>, content: String) {
    spawn(send_discord(
        state.client.clone(),
        state.webhook_id.clone(),
        state.webhook_token.clone(),
        state.discord_username_regex.clone(),
        state.formatting_regex.clone(),
        sender,
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
    sender: Cow<'static, str>,
    content: String,
) {
    match client
        .execute_webhook(webhook_id, &webhook_token)
        .content(formatting_regex.replace_all(&content, "\\$1").as_ref())
        .username(
            discord_username_regex
                .replace_all(sender.as_ref(), "$1¡$3")
                .as_ref(),
        )
        .await
    {
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
    pub fn create_command(self) -> String {
        format!(
            r#"tellraw @a "<{}> {}"
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

    todo!()
}
