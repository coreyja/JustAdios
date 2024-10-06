use axum::{
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use eyre::eyre;
use hmac::{KeyInit as _, Mac, SimpleHmac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::{
    db::{DBMeeting, DBUser},
    AppState,
};

#[derive(Debug, Deserialize, Clone, Serialize)]
pub(crate) struct ZoomWebhookBody {
    event: String,
    payload: serde_json::Value,
}

enum ZoomWebhookEvent {
    MeetingStarted(MeetingStartedPayload),
    MeetingEnded(MeetingEndedPayload),
    ParticipantJoined(ParticipantJoinedPayload),
    ParticipantLeft(ParticipantLeftPayload),
}

impl TryFrom<ZoomWebhookBody> for ZoomWebhookEvent {
    type Error = cja::color_eyre::Report;

    fn try_from(body: ZoomWebhookBody) -> Result<Self, Self::Error> {
        match body.event.as_str() {
            "meeting.started" => {
                Ok(serde_json::from_value(body.payload).map(Self::MeetingStarted)?)
            }
            "meeting.ended" => Ok(serde_json::from_value(body.payload).map(Self::MeetingEnded)?),
            "meeting.participant_joined" => {
                Ok(serde_json::from_value(body.payload).map(Self::ParticipantJoined)?)
            }
            "meeting.participant_left" => {
                Ok(serde_json::from_value(body.payload).map(Self::ParticipantLeft)?)
            }
            _ => Err(eyre!("Unknown event type")),
        }
    }
}

pub(crate) trait ProcessZoomWebhook {
    async fn process(self, state: &AppState) -> Result<(), Response>;
}

impl ProcessZoomWebhook for ZoomWebhookEvent {
    async fn process(self, state: &AppState) -> Result<(), Response> {
        match self {
            Self::MeetingStarted(payload) => payload.process(state).await,
            Self::MeetingEnded(payload) => payload.process(state).await,
            Self::ParticipantJoined(payload) => payload.process(state).await,
            Self::ParticipantLeft(payload) => payload.process(state).await,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct MeetingStartedPayload {
    account_id: String,
    object: MeetingDetails,
}

impl ProcessZoomWebhook for MeetingStartedPayload {
    async fn process(self, state: &AppState) -> Result<(), Response> {
        let user = sqlx::query_as!(
            DBUser,
            "SELECT * FROM users WHERE zoom_id = $1",
            self.object.host_id
        )
        .fetch_one(&state.db)
        .await
        .map_err(|_| (axum::http::StatusCode::BAD_REQUEST, "User not found").into_response())?;

        let meeting = sqlx::query_as!(
            DBMeeting,
            "INSERT INTO meetings (user_id, zoom_id, zoom_uuid, start_time, topic) VALUES ($1, $2, $3, $4, $5) RETURNING *",
            user.user_id,
            self.object.id,
            self.object.uuid,
            self.object.start_time,
            self.object.topic
        ) 
        .fetch_one(&state.db)
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::BAD_REQUEST,
                format!("DB Error: {}", e).into_response(),
            )
                .into_response()
        })?;

        tracing::info!("Meeting created: {:?}", meeting);

        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct MeetingDetails {
    duration: i64,
    host_id: String,
    id: String,
    start_time: chrono::DateTime<chrono::Utc>,
    end_time: Option<chrono::DateTime<chrono::Utc>>,
    topic: String,
    r#type: i64,
    uuid: String,
}

#[derive(Serialize, Deserialize)]
struct MeetingEndedPayload {
    account_id: String,
    object: MeetingDetails,
}

impl ProcessZoomWebhook for MeetingEndedPayload {
    async fn process(self, state: &AppState) -> Result<(), Response> {
        let meeting = sqlx::query_as!(
            DBMeeting,
            "UPDATE meetings SET end_time = $1 WHERE zoom_uuid = $2 RETURNING *",
            self.object.end_time,
            self.object.uuid
        )
        .fetch_one(&state.db)
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::BAD_REQUEST,
                format!("DB Error: {}", e).into_response(),
            )
                .into_response()
        })?;

        tracing::info!("Meeting updated: {:?}", meeting);

        Ok(())
    }
}

fn verify_zoom_signature(
    secret_token: &str,
    headers: &HeaderMap,
    body: &str,
) -> Result<(), Response> {
    let zoom_timestamp = headers.get("x-zm-request-timestamp").unwrap();
    let message = format!("v0:{}:{}", zoom_timestamp.to_str().unwrap(), body);

    let mut mac = SimpleHmac::<Sha256>::new_from_slice(secret_token.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(message.as_bytes());

    let result = mac.finalize();
    let code_bytes = result.into_bytes().to_vec();
    let code = hex::encode(code_bytes);
    let signature = format!("v0={}", code);

    let zoom_signature = headers.get("x-zm-signature").unwrap();

    if zoom_signature != &signature {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "Invalid zoom webhook signature",
        )
            .into_response());
    }

    Ok(())
}

#[axum_macros::debug_handler]
pub(crate) async fn zoom_webhook(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<(), Response> {
    verify_zoom_signature(&app_state.zoom.secret_token, &headers, &body)?;

    let body = serde_json::from_str::<ZoomWebhookBody>(&body).unwrap();
    tracing::info!("Processing zoom webhook event: {:?}", body.event);

    let event = ZoomWebhookEvent::try_from(body.clone()).map_err(|e| {
        tracing::error!("Invalid zoom webhook body: {:?}", body);
        (
            axum::http::StatusCode::BAD_REQUEST,
            format!("Invalid zoom webhook body: {}", e),
        )
            .into_response()
    })?;

    event.process(&app_state).await
}

#[derive(Serialize, Deserialize)]
struct ParticipantJoined {
    email: String,
    id: String,
    join_time: String,
    participant_user_id: String,
    participant_uuid: String,
    user_id: String,
    user_name: String,
}

#[derive(Serialize, Deserialize)]
struct ParticipantJoinedPayloadInner {
    id: String,
    participant: ParticipantJoined,
    start_time: String,
    timezone: String,
    topic: String,
    r#type: i64,
    uuid: String,
}

#[derive(Serialize, Deserialize)]
struct ParticipantJoinedPayload {
    account_id: String,
    object: ParticipantJoinedPayloadInner,
}

impl ProcessZoomWebhook for ParticipantJoinedPayload {
    async fn process(self, _state: &AppState) -> Result<(), Response> {
        tracing::info!("Participant joined -- No-Oping for now");
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct ParticipantLeft {
    email: String,
    id: String,
    leave_reason: String,
    leave_time: String,
    participant_user_id: String,
    participant_uuid: String,
    registrant_id: String,
    user_id: String,
    user_name: String,
}
#[derive(Serialize, Deserialize)]
struct ParticipantLeftPayloadInner {
    id: String,
    participant: ParticipantLeft,
    start_time: String,
    timezone: String,
    topic: String,
    r#type: i64,
    uuid: String,
}

#[derive(Serialize, Deserialize)]
struct ParticipantLeftPayload {
    account_id: String,
    object: ParticipantLeftPayloadInner,
}

impl ProcessZoomWebhook for ParticipantLeftPayload {
    async fn process(self, _state: &AppState) -> Result<(), Response> {
        tracing::info!("Participant left -- No-Oping for now");
        Ok(())
    }
}
