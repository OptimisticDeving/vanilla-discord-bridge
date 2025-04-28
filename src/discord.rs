use std::{borrow::Cow, sync::Arc};
use tokio::spawn;

use tracing::warn;
use twilight_http::Client;
use twilight_model::id::{Id, marker::WebhookMarker};

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
