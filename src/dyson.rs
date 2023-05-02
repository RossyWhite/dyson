use std::collections::HashSet;
use std::sync::Arc;

use futures::future::try_join_all;

use crate::config::DysonConfig;
use crate::image::EcrImageId;
use crate::provider::ecr::EcrImageProvider;
use crate::provider::{ImageProvider, ImageRegistry};

/// dyson App
pub struct Dyson {
    /// registry is the source of truth of images
    registry: Arc<dyn ImageRegistry>,
    /// scan targets are the targets to scan for images
    scan_targets: Vec<Arc<dyn ImageProvider>>,
}

impl Dyson {
    /// Create a new dyson cleaner
    pub async fn new(conf: &DysonConfig) -> Self {
        let registry = Arc::new(EcrImageProvider::from_conf(&conf.registry).await);
        let scan_targets: Vec<_> = conf.scans.iter().map(|_s| todo!()).collect::<Vec<_>>();

        Self {
            registry,
            scan_targets,
        }
    }

    /// Dry-run the cleaner
    pub async fn plan(
        &self,
        _output: impl std::io::Write,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let targets = self.aggregate_images().await?;
        println!("targets: {:?}", targets);
        Ok(())
    }

    /// Apply the cleaner
    pub async fn apply(
        &self,
        _output: impl std::io::Write,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let targets = self.aggregate_images().await?;
        println!("targets: {:?}", targets);
        self.registry.delete_images(&targets).await?;
        Ok(())
    }

    /// aggregate images from sources
    async fn aggregate_images(&self) -> Result<HashSet<EcrImageId>, Box<dyn std::error::Error>> {
        let includes = self.registry.list_images().await?;

        let excludes = try_join_all(self.scan_targets.iter().map(|s| s.list_images()))
            .await?
            .into_iter()
            .fold(HashSet::new(), |mut a, i| {
                a.extend(i);
                a
            });

        Ok(&includes - &excludes)
    }
}
