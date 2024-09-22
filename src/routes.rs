use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use chrono::{DateTime, Utc};
use cja::{app_state::AppState as _, server::session::DBSession, uuid::Uuid};
use maud::html;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tower_cookies::Cookies;

use crate::AppState;

pub fn routes(app_state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/", get(home))
        .route("/oauth/zoom", get(zoom_oauth))
        .with_state(app_state)
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
            User,
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
struct ZoomTokenResponse {
    access_token: String,
    api_url: String,
    expires_in: i64,
    refresh_token: String,
    scope: String,
    token_type: String,
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
      User,
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

    // let session = sqlx::query_as!(
    //     DBSession,
    //     "INSERT INTO sessions (user_id) VALUES ($1) RETURNING *",
    //     user.user_id,
    // )
    // .fetch_one(state.db())
    // .await
    // .map_err(|e| {
    //     tracing::error!("Failed to insert session into database: {e:?}");
    //     (
    //         StatusCode::INTERNAL_SERVER_ERROR,
    //         "Failed to insert session into database",
    //     )
    //         .into_response()
    // })?;

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

struct User {
    user_id: Uuid,
    zoom_id: String,
    display_name: String,
    access_token: String,
    refresh_token: String,
    expires_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}
