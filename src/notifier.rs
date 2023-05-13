use crate::config::SlackNotifierConfig;
use crate::image::ImagesSummary;

/// An error that can occur during the notification process.
#[derive(Debug, thiserror::Error)]
#[error("[NotificationError] source: {}", self.source)]
pub struct NotificationError {
    /// The source of the error.
    source: Box<dyn std::error::Error + Send + Sync>,
}

/// The notifier trait
#[async_trait::async_trait]
pub trait Notifier {
    async fn notify(&self, message: Message) -> Result<(), NotificationError>;
}

/// A message to send
pub struct Message {
    /// The title of the message
    title: String,
    /// The body of the message
    summary: ImagesSummary,
}

impl Message {
    pub fn new(title: impl Into<String>, summary: ImagesSummary) -> Self {
        Self {
            title: title.into(),
            summary,
        }
    }
}

pub struct SlackNotifier {
    webhook_url: String,
    username: Option<String>,
    channel: Option<String>,
    icon_url: Option<String>,
    http_client: reqwest::Client,
}

impl SlackNotifier {
    pub fn new(config: &SlackNotifierConfig) -> Self {
        Self {
            webhook_url: config.webhook_url.clone(),
            username: config.username.clone(),
            channel: config.channel.clone(),
            icon_url: config.icon_url.clone(),
            http_client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl Notifier for SlackNotifier {
    async fn notify(&self, message: Message) -> Result<(), NotificationError> {
        let result = message.summary.iter().fold(
            String::from("Repo | Count\n----------------\n"),
            |acc, (key, value)| format!("{}{} | {}\n", acc, key, value.len()),
        );

        let mut payload = serde_json::json!({
            "attachments": [
                {
                    "color": "#36a64f",
                    "fields": [
                      {
                        "title": message.title,
                        "value": format!("```{}```", result),
                        "short": false
                      }
                    ]
                }
            ]
        });

        if let Some(username) = &self.username {
            payload["username"] = serde_json::json!(username);
        }

        if let Some(channel) = &self.channel {
            payload["channel"] = serde_json::json!(channel);
        }

        if let Some(icon_url) = &self.icon_url {
            payload["icon_url"] = serde_json::json!(icon_url);
        }

        self.http_client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await
            .map(|_| ())
            .map_err(|err| NotificationError {
                source: Box::new(err),
            })
    }
}
