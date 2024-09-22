use eyre::Context;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Meetings {
    pub meetings: Vec<ListedMeeting>,
    pub next_page_token: Option<String>,
    pub page_count: Option<i64>,
    pub page_number: Option<i64>,
    pub page_size: i64,
    pub total_records: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct ListedMeeting {
    pub agenda: Option<String>,
    pub created_at: String,
    pub duration: Option<i64>,
    pub host_id: String,
    pub id: i64,
    pub start_time: Option<String>,
    pub timezone: Option<String>,
    pub r#type: i64,
    pub uuid: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct UpdateMeetingStatusBody {
    action: String,
}

pub(crate) async fn adios(meeting_id: i64, access_token: &str) -> cja::Result<()> {
    let client = Client::new();
    let url = format!("https://api.zoom.us/v2/meetings/{}/status", meeting_id);
    let body = UpdateMeetingStatusBody {
        action: "end".to_string(),
    };
    let resp = client
        .put(url)
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await?;

    if resp.status().is_success() {
        Ok(())
    } else {
        let status = resp.status();
        let text = resp.text().await?;
        Err(eyre::eyre!("Failed to end meeting: {status} {text}"))
    }
}

impl ListedMeeting {
    pub(crate) async fn adios(&self, access_token: &str) -> cja::Result<()> {
        adios(self.id, access_token).await
    }

    pub(crate) fn created_at(&self) -> cja::Result<chrono::NaiveDateTime> {
        chrono::NaiveDateTime::parse_from_str(&self.created_at, "%Y-%m-%dT%H:%M:%SZ")
            .context("Could not parse created at timestamp")
    }

    pub(crate) fn live_duration(&self) -> cja::Result<i64> {
        if self.r#type == 4 {
            return Err(eyre::eyre!(
                "Could not determine the live duration of a Personal Meeting Room Meeting because the API created_at is the first time the meeting was ever used and no start time is presented"
            ));
        }
        if self.r#type != 1 {
            return Err(eyre::eyre!("Meeting type {} is not supported", self.r#type));
        }
        let created_at = self.created_at()?;
        let now = chrono::Utc::now().naive_utc();
        let duration = now - created_at;
        Ok(duration.num_seconds())
    }

    pub(crate) async fn get_full_meeting(&self, access_token: &str) -> cja::Result<Meeting> {
        let client = Client::new();
        let url = format!("https://api.zoom.us/v2/meetings/{}", self.id);
        let resp = client.get(url).bearer_auth(access_token).send().await?;
        let resp_text = resp.text().await?;
        dbg!(&resp_text);
        Ok(serde_json::from_str(&resp_text)?)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct Meeting {
    pub id: i64,
    pub r#type: i64,
    pub start_time: Option<chrono::NaiveDateTime>,
    pub duration: Option<i64>,
    pub occurrences: Option<Vec<MeetingOccurrence>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct MeetingOccurrence {
    pub occurrence_id: String,
    pub start_time: chrono::NaiveDateTime,
    pub duration: i64,
}

pub(crate) enum MeetingType {
    Live,
    Scheduled,
}

impl MeetingType {
    fn query_param(&self) -> &str {
        match self {
            MeetingType::Live => "live",
            MeetingType::Scheduled => "scheduled",
        }
    }
}

pub(crate) async fn get_meetings(
    access_token: &str,
    meeting_type: MeetingType,
) -> cja::Result<Meetings> {
    let client = Client::new();
    let resp = client
        .get("https://api.zoom.us/v2/users/me/meetings")
        .query(&[("type", meeting_type.query_param())])
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    let resp_text = resp.text().await?;
    dbg!(&resp_text);

    Ok(serde_json::from_str(&resp_text)?)
}
