use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use chrono::Utc;
use cja::{app_state::AppState as _, server::session::DBSession};
use maud::html;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tower_cookies::Cookies;

mod webhooks;

use crate::{
    db::DBUser,
    zoom::{adios, get_meetings, MeetingType},
    AppState,
};

pub fn routes(app_state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/", get(home))
        .route("/meetings", get(meetings))
        .route("/meetings/end", get(end_meeting))
        .route("/oauth/zoom", get(zoom_oauth))
        .route("/webhooks/zoom", post(webhooks::zoom_webhook))
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

async fn meetings(
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
