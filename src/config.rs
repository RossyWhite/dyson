use std::fmt::Debug;

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct DysonConfig {
    /// The registry config
    pub registry: RegistryConfig,
    /// The scan configs
    pub scans: Vec<ScanConfig>,
}

/// The registry to delete
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct RegistryConfig {
    /// The name of the config
    pub name: Option<String>,
    /// The AWS profile to use
    pub profile_name: String,
    /// The repository filters
    pub filters: Vec<RepositoryFiltersConfig>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct RepositoryFiltersConfig {
    /// The repository pattern to apply this option to
    pub pattern: String,
    /// The number of days after which to extract images
    pub days_after: u64,
    /// The tag patterns to ignore
    pub ignore_tag_patterns: Vec<String>,
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
                filters: vec![RepositoryFiltersConfig {
                    pattern: "*".to_string(),
                    days_after: 30,
                    ignore_tag_patterns: vec!["latest".to_string()],
                }],
            },
            scans: vec![ScanConfig {
                name: Some("scan-target".to_string()),
                profile_name: "profile2".to_string(),
            }],
        }
    }
}
