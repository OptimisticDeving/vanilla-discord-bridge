use anyhow::Result;
use std::{borrow::Cow, sync::Arc};
use tokio::{spawn, sync::mpsc::UnboundedSender};
use twilight_gateway::{Event, EventTypeFlags, Intents, Shard, ShardId, StreamExt};

use tracing::warn;
use twilight_http::Client;
use twilight_model::id::{
    Id,
    marker::{ChannelMarker, WebhookMarker},
};

use crate::AppState;

#[inline]
pub fn schedule_send_discord(state: &AppState, sender: Cow<'static, str>, content: String) {
    spawn(send_discord(
        state.client.clone(),
        state.webhook_id.clone(),
        state.webhook_token.clone(),
        sender,
        content,
    ));
}

#[inline]
async fn send_discord(
    client: Arc<Client>,
    webhook_id: Id<WebhookMarker>,
    webhook_token: Arc<str>,
    sender: Cow<'static, str>,
    content: String,
) {
    match client
        .execute_webhook(webhook_id, &webhook_token)
        .content(&content)
        .username(sender.as_ref())
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
            escape_for_component(&self.username),
            escape_for_component(&self.content)
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

                discord_message_sender.send(IncomingDiscordMessage {
                    username: if event.author.bot {
                        format!("[BOT] {}", event.author.name)
                    } else {
                        event.author.name.clone()
                    },
                    content: event.content.clone(),
                })?;
            }
            _ => continue,
        }
    }

    todo!()
}
