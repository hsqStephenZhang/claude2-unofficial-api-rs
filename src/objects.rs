use chrono::{DateTime, Local};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

#[derive(Deserialize, Debug, Clone)]
pub struct Conversation {
    pub uuid: String,
    pub name: String,
    pub summary: String,
    // format: 2023-07-20T11:54:41.108217+00:00
    #[serde(deserialize_with = "from_date_string")]
    pub created_at: DateTime<Local>,
    #[serde(deserialize_with = "from_date_string")]
    pub updated_at: DateTime<Local>,
}

fn from_date_string<'de, D>(deserializer: D) -> Result<DateTime<Local>, D::Error>
where
    D: Deserializer<'de>,
{
    let date_str = String::deserialize(deserializer)?;
    DateTime::parse_from_rfc3339(&date_str)
        .map(|datetime| datetime.with_timezone(&Local))
        .map_err(serde::de::Error::custom)
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
pub struct History {
    pub uuid: String,
    pub name: String,
    pub summary: String,
    #[serde(deserialize_with = "from_date_string")]
    pub created_at: DateTime<Local>,
    #[serde(deserialize_with = "from_date_string")]
    pub updated_at: DateTime<Local>,
    #[serde(rename = "chat_messages")]
    pub chat_messages: Vec<ChatMessage>,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
pub struct ChatMessage {
    pub uuid: String,
    pub text: String,
    pub sender: String,
    pub index: i64,
    #[serde(deserialize_with = "from_date_string")]
    pub created_at: DateTime<Local>,
    #[serde(deserialize_with = "from_date_string")]
    pub updated_at: DateTime<Local>,
    #[serde(rename = "edited_at")]
    pub edited_at: Value,
    #[serde(rename = "chat_feedback")]
    pub chat_feedback: Value,
    pub attachments: Vec<Value>,
}
