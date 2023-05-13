use std::error::Error;

use crate::config::SlackNotifierConfig;

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
    body: String,
}

impl Message {
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
        }
    }
}

pub struct SlackNotifier {
    webhook_url: String,
    username: Option<String>,
    channel: Option<String>,
}

impl SlackNotifier {
    pub fn new(config: &SlackNotifierConfig) -> Self {
        Self {
            webhook_url: config.webhook_url.clone(),
            username: config.username.clone(),
            channel: config.channel.clone(),
        }
    }
}

#[async_trait::async_trait]
impl Notifier for SlackNotifier {
    async fn notify(&self, message: Message) -> Result<(), NotificationError> {
        todo!()
    }
}
