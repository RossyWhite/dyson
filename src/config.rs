use std::fmt::Debug;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct DysonConfig {
    /// The registry config
    pub registry: RegistryConfig,
    /// The scan configs
    pub scans: Vec<ScanConfig>,
    /// The notifier config
    pub notifier: Option<NotifierConfig>,
}

/// The registry to delete
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct RegistryConfig {
    /// The name of the config
    pub name: Option<String>,
    /// The AWS profile to use
    pub profile_name: String,
    /// The repository exclude patterns
    pub excludes: Option<Vec<String>>,
    /// The repository filters
    pub filters: Option<Vec<RepositoryFilterConfig>>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct RepositoryFilterConfig {
    /// The repository pattern to apply this option to
    pub pattern: String,
    /// The number of days after which to extract images
    pub days_after: Option<u64>,
    /// The tag patterns to ignore
    pub ignore_tag_patterns: Option<Vec<String>>,
}

/// The notifier config
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct NotifierConfig {
    /// The slack config
    pub slack: SlackNotifierConfig,
}

/// The slack notifier config
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct SlackNotifierConfig {
    /// The slack webhook url
    pub webhook_url: String,
    /// The slack username
    pub username: Option<String>,
    /// The slack channel
    pub channel: Option<String>,
    /// The slack icon url
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
}

/// Scan Target
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ScanConfig {
    /// The name of the scan target
    pub name: Option<String>,
    /// The AWS profile to use
    pub profile_name: String,
}

impl DysonConfig {
    /// Load a config from a file
    pub fn load_path(
        path: impl AsRef<std::path::Path>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let f = std::fs::File::open(&path)?;
        Ok(serde_yaml::from_reader(&f)?)
    }

    /// Example configuration
    pub fn example_config() -> Self {
        Self {
            registry: RegistryConfig {
                name: Some("my-registry".to_string()),
                profile_name: "profile1".to_string(),
                excludes: Some(vec!["exclude/*".to_string()]),
                filters: Some(vec![RepositoryFilterConfig {
                    pattern: "*".to_string(),
                    days_after: Some(30),
                    ignore_tag_patterns: Some(vec!["latest".to_string()]),
                }]),
            },
            scans: vec![ScanConfig {
                name: Some("scan-target".to_string()),
                profile_name: "profile2".to_string(),
            }],

            notifier: Some(NotifierConfig {
                slack: SlackNotifierConfig {
                    webhook_url: "https://hooks.slack.com/services/xxx/yyy/zzz".to_string(),
                    username: Some("dyson-bot".to_string()),
                    channel: Some("random".to_string()),
                    icon_url: None,
                },
            }),
        }
    }
}
