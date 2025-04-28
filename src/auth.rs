use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use tracing::warn;

use crate::{State, has_header_and_matches};

pub struct Authorized;

impl FromRequestParts<State> for Authorized {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &State) -> Result<Self, Self::Rejection> {
        if has_header_and_matches(parts, "User-Agent", |value| {
            !value.starts_with("Minecraft server")
        }) {
            warn!("invalid user agent in request");
            return Err(StatusCode::IM_A_TEAPOT);
        }

        if has_header_and_matches(parts, "Authorization", |value| {
            value != state.expected_auth_header.as_ref()
        }) {
            warn!("invalid authorization in request");
            return Err(StatusCode::IM_A_TEAPOT);
        }

        Ok(Authorized)
    }
}
