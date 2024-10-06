use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Form,
};
use chrono::Utc;
use cja::{app_state::AppState as _, server::session::DBSession};
use maud::html;
use reqwest::StatusCode;
use serde::{Deserialize, Deserializer, Serialize};
use tower_cookies::Cookies;

mod webhooks;

use crate::{
    db::{DBMeeting, DBUser},
    zoom::{adios, get_meetings, MeetingType},
    AppState,
};

pub fn routes(app_state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/", get(home))
        .route("/live_meetings", get(live_meetings)) // This hits the Zoom API and gets the current meetings
        .route("/meetings", get(meetings)) // This is the list of meetings that have been scheduled
        .route("/meetings/:meeting_id", get(meeting))
        .route("/meetings/:meeting_id", post(edit_meeting))
        .route("/meetings/end", get(end_meeting))
        .route("/oauth/zoom", get(zoom_oauth))
        .route("/webhooks/zoom", post(webhooks::zoom_webhook))
        .route("/settings", get(settings))
        .route("/settings/edit", get(edit_settings))
        .route("/settings/edit", post(update_settings))
        .with_state(app_state)
}

#[derive(Debug, Deserialize, Clone)]
struct EndMeetingParams {
    meeting_id: String,
}

async fn end_meeting(
    State(state): State<AppState>,
    session: DBSession,
    Query(params): Query<EndMeetingParams>,
) -> Result<impl IntoResponse, Response> {
    let user = sqlx::query_as!(
        DBUser,
        "SELECT * FROM users WHERE user_id = $1",
        session.user_id,
    )
    .fetch_one(state.db())
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch user: {e:?}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch user").into_response()
    })?;

    let meeting_id = params.meeting_id.parse::<i64>().map_err(|e| {
        tracing::error!("Failed to parse meeting id: {e:?}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to parse meeting id",
        )
            .into_response()
    })?;

    adios(meeting_id, &user.access_token).await.map_err(|e| {
        tracing::error!("Failed to end meeting: {e:?}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to end meeting").into_response()
    })?;

    Ok(Redirect::temporary("/meetings").into_response())
}

async fn live_meetings(
    State(state): State<AppState>,
    session: DBSession,
) -> Result<impl IntoResponse, Response> {
    let user = sqlx::query_as!(
        DBUser,
        "SELECT * FROM users WHERE user_id = $1",
        session.user_id,
    )
    .fetch_one(state.db())
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch user: {e:?}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch user").into_response()
    })?;

    let access_token = user.access_token(&state).await.map_err(|e| {
        tracing::error!("Failed to get access token: {e:?}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to get access token",
        )
            .into_response()
    })?;

    let meetings = get_meetings(&access_token, MeetingType::Live)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get meetings: {e:?}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get meetings").into_response()
        })?;

    Ok(html! {
        h1 { "Meetings" }

        p {
            "You are logged in as " (user.display_name)
        }

        p {
          "Total meetings: " (meetings.total_records)
        }

        ul {
          @for meeting in meetings.meetings {
            li {
              (format!("{meeting:?}"))
              (meeting.live_duration().unwrap_or(-1))
            }
          }
        }
    })
}

async fn meetings(
    State(state): State<AppState>,
    session: DBSession,
) -> Result<impl IntoResponse, Response> {
    let meetings = sqlx::query_as!(
        DBMeeting,
        "SELECT * FROM meetings WHERE user_id = $1",
        session.user_id,
    )
    .fetch_all(state.db())
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch meetings: {e:?}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch meetings",
        )
            .into_response()
    })?;

    let (current_meetings, ended_meetings): (Vec<_>, Vec<_>) =
        meetings.iter().partition(|m| !m.is_ended());

    Ok(html! {
        h1 { "Meetings" }

        h2 { "Current Meetings" }
        ul {
          @for meeting in current_meetings {
            li {
              a href=(format!("/meetings/{}", meeting.meeting_id)) { (format!("{meeting:?}")) }
            }
          }
        }

        h2 { "Ended Meetings" }
        ul {
          @for meeting in ended_meetings {
            li {
              a href=(format!("/meetings/{}", meeting.meeting_id)) { (format!("{meeting:?}")) }
            }
          }
        }
    })
}

async fn meeting(
    State(state): State<AppState>,
    session: DBSession,
    Path(meeting_id): Path<String>,
) -> Result<impl IntoResponse, Response> {
    let user = sqlx::query_as!(
        DBUser,
        "SELECT * FROM users WHERE user_id = $1",
        session.user_id,
    )
    .fetch_one(state.db())
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch user: {e:?}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch user").into_response()
    })?;

    let meeting_id = cja::uuid::Uuid::parse_str(&meeting_id).map_err(|e| {
        tracing::error!("Failed to parse meeting id: {e:?}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to parse meeting id",
        )
            .into_response()
    })?;

    let meeting = sqlx::query_as!(
        DBMeeting,
        "SELECT * FROM meetings WHERE meeting_id = $1 and user_id = $2",
        meeting_id,
        session.user_id,
    )
    .fetch_one(state.db())
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch meeting: {e:?}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch meeting").into_response()
    })?;

    let minutes_remaining = if !meeting.is_ended() {
        meeting.fetch_minutes_remaining(&state).await.ok()
    } else {
        None
    };

    Ok(html! {
        h1 { "Meeting" }

        @if let Some(minutes_remaining) = minutes_remaining {
            h2 { "Meeting is still running" }

            @match minutes_remaining {
                0 => p { "Under 1 minutes remaining. Meeting will be ended shortly" },
                1 => p { "1 minute remaining"},
                minutes_remaining => p { (minutes_remaining) " minute(s) remaining"},
            }
        }

        p {
            "Zoom Meeting ID: " (meeting.zoom_id)
        }

        p {
            "Start Time: " (meeting.start_time)
        }

        p {
            "Duration: " (meeting.duration().num_minutes()) " minutes"
        }

        @if let Some(end_time) = meeting.end_time {
            p {
                "End Time: " (end_time.format("%Y-%m-%d %H:%M:%S"))
            }
        } @else {
            p {
                "Meeting is still running"
            }
        }

        @if !meeting.is_ended() {
            @if let Some(max_meeting_length_minutes) = meeting.max_meeting_length_minutes {
                p {
                    "Max Meeting Length: " (max_meeting_length_minutes) " minutes"
                }
            } @else {
                p {
                    "No max meeting length set for meeting"
                }

                @if let Some(user_default_meeting_length) = user.default_meeting_length_minutes {
                    p {
                        "User Default Meeting Length: "  (user_default_meeting_length)  " minutes"
                    }
                } @else {
                    p {
                        "No default meeting length set for user either. Will use App default of " (crate::jobs::end_meeting::DEFAULT_MAX_MEETING_LENGTH_MINUTES)
                    }
                }
            }

            form action=(format!("/meetings/{}", meeting.meeting_id)) method="post" {
                label for="max_meeting_length_minutes" { "Max Meeting Length (minutes)" }
                input type="number" name="max_meeting_length_minutes" value=[meeting.max_meeting_length_minutes] {}

                input type="submit" value="Update" { }
            }
        }

        a href="/meetings" { "Back to Meetings" }
    })
}

#[derive(Debug, Deserialize, Clone)]
struct EditMeetingParams {
    #[serde(deserialize_with = "empty_string_is_none")]
    max_meeting_length_minutes: Option<i32>,
}

async fn edit_meeting(
    State(state): State<AppState>,
    session: DBSession,
    Path(meeting_id): Path<String>,
    Form(params): Form<EditMeetingParams>,
) -> Result<impl IntoResponse, Response> {
    let meeting_id = cja::uuid::Uuid::parse_str(&meeting_id).map_err(|e| {
        tracing::error!("Failed to parse meeting id: {e:?}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to parse meeting id",
        )
            .into_response()
    })?;

    sqlx::query!(
        "UPDATE meetings SET max_meeting_length_minutes = $1 WHERE meeting_id = $2 AND user_id = $3",
        params.max_meeting_length_minutes,
        meeting_id,
        session.user_id,
    )
    .execute(state.db())
    .await
    .map_err(|e| {
        tracing::error!("Failed to update meeting: {e:?}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to update meeting",
        )
            .into_response()
    })?;

    Ok(Redirect::to(&format!("/meetings/{}", meeting_id)).into_response())
}

async fn home(State(state): State<AppState>, session: Option<DBSession>) -> impl IntoResponse {
    let zoom_redirect_uri = state.zoom_redirect_url();
    let client_id = &state.zoom.client_id;
    let zoom_auth_url = format!(
        "https://zoom.us/oauth/authorize?response_type=code&client_id={client_id}&redirect_uri={zoom_redirect_uri}",
    );

    let user = if let Some(session) = session {
        tracing::info!("Session {} found, fetching user", session.session_id);
        sqlx::query_as!(
            DBUser,
            "SELECT * FROM users WHERE user_id = $1",
            session.user_id,
        )
        .fetch_one(state.db())
        .await
        .ok()
    } else {
        None
    };

    html! {
        h1 { "Just Adios" }

        @if let Some(user) = user {
          p {
              "You are logged in as " (user.display_name)
          }

          a href="/meetings" { "Meetings" }
        }

        p {
          "Welcome to Just Adios. This app will end your Zoom meetings for you."
        }

        a href=(zoom_auth_url) { "Authorize with Zoom" }
    }
}

#[derive(Debug, Deserialize, Clone)]
struct ZoomOauthRedirectParams {
    code: String,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ZoomTokenResponse {
    pub(crate) access_token: String,
    pub(crate) api_url: String,
    pub(crate) expires_in: i64,
    pub(crate) refresh_token: String,
    pub(crate) scope: String,
    pub(crate) token_type: String,
}

async fn zoom_oauth(
    State(state): State<AppState>,
    Query(params): Query<ZoomOauthRedirectParams>,
    cookies: Cookies,
) -> Result<Response, Response> {
    let zoom_redirect_uri = state.zoom_redirect_url();
    let client = reqwest::Client::new();
    let access_token_response = client
        .post("https://zoom.us/oauth/token")
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", &params.code),
            ("redirect_uri", &zoom_redirect_uri),
        ])
        .basic_auth(&state.zoom.client_id, Some(&state.zoom.client_secret))
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to get access token: {e:?}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get access token",
            )
                .into_response()
        })?;

    let token_response_text = access_token_response.text().await.map_err(|e| {
        tracing::error!("Failed to get access token: {e:?}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to get access token",
        )
            .into_response()
    })?;

    let token_response: ZoomTokenResponse =
        serde_json::from_str(&token_response_text).map_err(|e| {
            tracing::error!("Failed to parse access token response: {e:?}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to parse access token response",
            )
                .into_response()
        })?;

    let user_response = client
        .get("https://api.zoom.us/v2/users/me")
        .bearer_auth(&token_response.access_token)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to get user info: {e:?}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get user info").into_response()
        })?;

    tracing::info!("User response Status: {:?}", user_response.status());

    let user_info_text = user_response.text().await.map_err(|e| {
        tracing::error!("Failed to get user info: {e:?}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get user info").into_response()
    })?;

    tracing::info!("User info text: {:?}", user_info_text);

    let user_info: ZoomUser = serde_json::from_str(&user_info_text).map_err(|e| {
        tracing::error!("Failed to parse user info: {e:?}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to parse user info",
        )
            .into_response()
    })?;

    tracing::info!("Zoom User info: {user_info:?}");

    let expires_at = Utc::now() + chrono::Duration::seconds(token_response.expires_in);
    let user = sqlx::query_as!(
      DBUser,
      "INSERT INTO users (zoom_id, display_name, access_token, refresh_token, expires_at) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (zoom_id) DO UPDATE SET (display_name, access_token, refresh_token, expires_at, updated_at) = ($2, $3, $4, $5, now()) RETURNING *",
      user_info.id,
      user_info.display_name,
      token_response.access_token,
      token_response.refresh_token,
      expires_at,
    ).fetch_one(state.db()).await.map_err(|e| {
      tracing::error!("Failed to insert user into database: {e:?}");
      (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Failed to insert user into database",
      )
        .into_response()
    })?;

    tracing::info!("User inserted into database: {}", user.user_id);

    DBSession::create(user.user_id, &state, &cookies)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create session: {e:?}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create session",
            )
                .into_response()
        })?;

    Ok(Redirect::temporary("/").into_response())
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ZoomUser {
    id: String,
    display_name: String,
}

async fn settings(
    State(state): State<AppState>,
    session: DBSession,
) -> Result<impl IntoResponse, Response> {
    let user = sqlx::query_as!(
        DBUser,
        "SELECT * FROM users WHERE user_id = $1",
        session.user_id,
    )
    .fetch_one(state.db())
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch user: {e:?}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch user").into_response()
    })?;

    Ok(html! {
        h1 { "Settings" }

        p {
            "You are logged in as " (user.display_name)
        }

        @if let Some(default_meeting_length_minutes) = user.default_meeting_length_minutes {
          p {
            "Default meeting length: " (default_meeting_length_minutes) " minutes"
          }
        } @else {
          p {
            "No default meeting length set"
          }
        }

        a href="/settings/edit" { "Edit Settings" }
    })
}

async fn edit_settings(
    State(state): State<AppState>,
    session: DBSession,
) -> Result<impl IntoResponse, Response> {
    let user = sqlx::query_as!(
        DBUser,
        "SELECT * FROM users WHERE user_id = $1",
        session.user_id,
    )
    .fetch_one(state.db())
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch user: {e:?}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch user").into_response()
    })?;

    Ok(html! {
        h1 { "Edit Settings" }

        p {
            "You are logged in as " (user.display_name)
        }

        form action="/settings/edit" method="post" {
            label for="default_meeting_length_minutes" { "Default Meeting Length (minutes)" }
            input type="number" name="default_meeting_length_minutes" value=[user.default_meeting_length_minutes] {}

            input type="submit" value="Update" { }
        }
    })
}

async fn update_settings(
    State(state): State<AppState>,
    session: DBSession,
    Form(params): Form<EditSettingsParams>,
) -> Result<impl IntoResponse, Response> {
    // let user = sqlx::query_as!(
    //     DBUser,
    //     "SELECT * FROM users WHERE user_id = $1",
    //     session.user_id,
    // )
    // .fetch_one(state.db())
    // .await
    // .map_err(|e| {
    //     tracing::error!("Failed to fetch user: {e:?}");
    //     (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch user").into_response()
    // })?;

    sqlx::query!(
        "UPDATE users SET default_meeting_length_minutes = $1 WHERE user_id = $2",
        params.default_meeting_length_minutes,
        session.user_id,
    )
    .execute(state.db())
    .await
    .map_err(|e| {
        tracing::error!("Failed to update user: {e:?}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to update user").into_response()
    })?;

    Ok(Redirect::to("/settings").into_response())
}

#[derive(Debug, Deserialize, Clone)]
struct EditSettingsParams {
    #[serde(deserialize_with = "empty_string_is_none")]
    default_meeting_length_minutes: Option<i32>,
}

fn empty_string_is_none<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.is_empty() {
        Ok(None)
    } else {
        let parsed = s
            .parse::<i32>()
            .map_err(|e| serde::de::Error::custom(e.to_string()))?;
        Ok(Some(parsed))
    }
}
