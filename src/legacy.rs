use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct LegacyProfile {
    pub user_display_name: String,
    pub user_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct LegacyChat {
    #[serde(flatten)]
    pub profile: LegacyProfile,
    pub text: String,
}

#[derive(Debug, Serialize)]
#[repr(transparent)]
pub struct LegacyChatResponse {
    #[serde(rename = "response")]
    pub pass_through: bool,
}

#[derive(Debug, Deserialize)]
#[repr(transparent)]
pub struct JoinOrLeaveEvent {
    #[serde(flatten)]
    pub profile: LegacyProfile,
}
