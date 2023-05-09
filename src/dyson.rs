use std::collections::HashSet;
use std::sync::Arc;

use futures::future::try_join_all;

use crate::config::DysonConfig;
use crate::image::EcrImageId;
use crate::provider::ecr::EcrImageRegistry;
use crate::provider::ecs_service::EcsServiceImageProvider;
use crate::provider::lambda::LambdaImageProvider;
use crate::provider::task_definition::TaskDefinitionProvider;
use crate::provider::{ImageDeleterError, ImageProvider, ImageProviderError, ImageRegistry};

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
        let registry = Arc::new(EcrImageRegistry::from_conf(&conf.registry).await);

        let mut scan_targets = Vec::<Arc<dyn ImageProvider>>::new();

        for scan in &conf.scans {
            let c = &aws_config::from_env()
                .profile_name(&scan.profile_name)
                .load()
                .await;
            scan_targets.push(Arc::new(LambdaImageProvider::from_conf(c)));
            scan_targets.push(Arc::new(EcsServiceImageProvider::from_conf(c)));
            scan_targets.push(Arc::new(TaskDefinitionProvider::from_conf(c)));
        }

        Self {
            registry,
            scan_targets,
        }
    }

    /// Dry-run the cleaner
    pub async fn plan(&self, _output: impl std::io::Write) -> Result<(), DysonError> {
        let targets = self.aggregate_images().await?;
        println!("targets: {:?}", targets);
        Ok(())
    }

    /// Apply the cleaner
    pub async fn apply(&self, _output: impl std::io::Write) -> Result<(), DysonError> {
        let targets = self.aggregate_images().await?;
        println!("targets: {:?}", targets);
        self.delete_images(&targets).await?;
        Ok(())
    }

    /// aggregate images from sources
    async fn aggregate_images(&self) -> Result<HashSet<EcrImageId>, DysonError> {
        let includes = self.registry.provide_images().await?;

        let excludes = try_join_all(self.scan_targets.iter().map(|s| s.provide_images()))
            .await?
            .into_iter()
            .fold(HashSet::new(), |mut a, i| {
                a.extend(i);
                a
            });

        Ok(&includes - &excludes)
    }

    /// delete images from registry
    async fn delete_images(&self, images: &HashSet<EcrImageId>) -> Result<(), DysonError> {
        self.registry.delete_images(images).await?;
        Ok(())
    }
}

/// An error returned an ImageProvider
#[derive(Debug, thiserror::Error)]
#[error("[DysonError] kind: {:?}, source: {}", self.kind, self.source)]
pub struct DysonError {
    /// The kind of the error.
    kind: DysonErrorKind,
    /// The source of the error.
    source: Box<dyn std::error::Error + Send + Sync>,
}

/// The kind of an DysonError.
#[derive(Debug)]
pub enum DysonErrorKind {
    /// An error caused by aggregation.
    AggregationError,
    /// An error caused by deletion.
    DeletionError,
}

impl From<ImageProviderError> for DysonError {
    fn from(err: ImageProviderError) -> Self {
        Self {
            kind: DysonErrorKind::AggregationError,
            source: Box::new(err),
        }
    }
}

impl From<ImageDeleterError> for DysonError {
    fn from(err: ImageDeleterError) -> Self {
        Self {
            kind: DysonErrorKind::DeletionError,
            source: Box::new(err),
        }
    }
}

#[cfg(test)]
mod tests {
    
    use crate::provider::ImageDeleter;

    use super::*;

    #[tokio::test]
    async fn aggregation() {
        struct MockProvider {
            images: HashSet<EcrImageId>,
        }

        #[async_trait::async_trait]
        impl ImageProvider for MockProvider {
            async fn provide_images(&self) -> Result<HashSet<EcrImageId>, ImageProviderError> {
                Ok(self.images.clone())
            }
        }

        #[async_trait::async_trait]
        impl ImageDeleter for MockProvider {
            async fn delete_images(
                &self,
                _images: &HashSet<EcrImageId>,
            ) -> Result<(), ImageDeleterError> {
                Ok(())
            }
        }

        impl ImageRegistry for MockProvider {}

        #[derive(Debug)]
        struct TestCase {
            name: String,
            registry: HashSet<EcrImageId>,
            scanned: Vec<HashSet<EcrImageId>>,
            expected: HashSet<EcrImageId>,
        }

        let cases = vec![
            TestCase {
                name: "scanned images is empty".to_string(),
                registry: HashSet::from([EcrImageId::default_with_tag("test")]),
                scanned: vec![],
                expected: HashSet::from([EcrImageId::default_with_tag("test")]),
            },
            TestCase {
                name: "registry is empty".to_string(),
                registry: HashSet::new(),
                scanned: vec![HashSet::from([EcrImageId::default_with_tag("test")])],
                expected: HashSet::new(),
            },
            TestCase {
                name: "only difference".to_string(),
                registry: HashSet::from([
                    EcrImageId::default_with_tag("test"),
                    EcrImageId::default_with_tag("test2"),
                    EcrImageId::default_with_tag("test3"),
                    EcrImageId::default_with_tag("test4"),
                    EcrImageId::default_with_tag("test5"),
                ]),
                scanned: vec![
                    HashSet::from([
                        EcrImageId::default_with_tag("test"),
                        EcrImageId::default_with_tag("test2"),
                    ]),
                    HashSet::from([EcrImageId::default_with_tag("test3")]),
                ],
                expected: HashSet::from([
                    EcrImageId::default_with_tag("test4"),
                    EcrImageId::default_with_tag("test5"),
                ]),
            },
        ];

        for case in cases {
            let registry = Arc::new(MockProvider {
                images: case.registry,
            });

            let scan_targets = case
                .scanned
                .into_iter()
                .map(|s| {
                    let p = MockProvider { images: s };
                    Arc::new(p) as Arc<dyn ImageProvider>
                })
                .collect();
            let dyson = Dyson {
                registry,
                scan_targets,
            };

            let res = dyson.aggregate_images().await.unwrap();
            assert_eq!(res, case.expected, "{}", case.name);
        }
    }
}
