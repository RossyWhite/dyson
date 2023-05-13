use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use aws_sdk_ecr::types::ImageIdentifier;
use futures::future::try_join_all;
use futures::TryFutureExt;

use crate::config::DysonConfig;
use crate::image::{EcrImageId, ImagesSummary};
use crate::notifier::{Message, Notifier, SlackNotifier};
use crate::provider::ecr::EcrImageRegistry;
use crate::provider::ecs_service::EcsServiceImageProvider;
use crate::provider::lambda::LambdaImageProvider;
use crate::provider::task_definition::TaskDefinitionProvider;
use crate::provider::{ImageProvider, ImageRegistry};

/// dyson App
pub struct Dyson {
    /// registry is the source of truth of images
    registry: Arc<dyn ImageRegistry>,
    /// scan targets are the targets to scan for images
    scan_targets: Vec<Arc<dyn ImageProvider>>,
    /// notifier to notify the result
    notifier: Option<Box<dyn Notifier>>,
}

impl Dyson {
    /// Create a new dyson cleaner
    pub async fn new(conf: &DysonConfig) -> Result<Self, DysonError> {
        let registry = Arc::new(
            EcrImageRegistry::from_conf(&conf.registry)
                .await
                .map_err(DysonError::initialization_error)?,
        );

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

        let notifier = conf
            .notifier
            .as_ref()
            .map(|conf| Box::new(SlackNotifier::new(&conf.slack)) as Box<dyn Notifier>);

        Ok(Self {
            registry,
            scan_targets,
            notifier,
        })
    }

    /// List target images
    pub async fn list_target_images(&self) -> Result<ImagesSummary, DysonError> {
        let targets = self.aggregate_target_images().await?;
        let summarized = self.summarize_tags_per_repo(&targets).await;
        Ok(summarized)
    }

    /// aggregate images from sources
    async fn aggregate_target_images(&self) -> Result<HashSet<EcrImageId>, DysonError> {
        let includes = self
            .registry
            .provide_images()
            .await
            .map_err(DysonError::aggregation_error)?;

        let excludes = try_join_all(self.scan_targets.iter().map(|s| s.provide_images()))
            .await
            .map_err(DysonError::aggregation_error)?
            .into_iter()
            .fold(HashSet::new(), |mut a, i| {
                a.extend(i);
                a
            });

        Ok(&includes - &excludes)
    }

    /// summarize images per repository
    async fn summarize_tags_per_repo(&self, images: &HashSet<EcrImageId>) -> ImagesSummary {
        images.iter().fold(HashMap::new(), |mut acc, image| {
            let r = image.repository_name.clone();
            let id = ImageIdentifier::builder()
                .image_tag(&image.image_tag)
                .build();
            acc.entry(r).or_insert_with(Vec::new).push(id);
            acc
        })
    }

    /// delete images from registry
    pub async fn delete_images(&self, images: ImagesSummary) -> Result<(), DysonError> {
        self.registry
            .delete_images(images)
            .await
            .map_err(DysonError::deletion_error)?;
        Ok(())
    }

    pub async fn notify_result(&self, title: &str, body: &str) -> Result<(), DysonError> {
        let Some(notifier) = &self.notifier else { return Ok(()); };
        Ok(notifier
            .notify(Message::new(title, body))
            .await
            .map_err(DysonError::notification_error)?)
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
    /// An error caused by initialization.
    Initialization,
    /// An error caused by aggregation.
    Aggregation,
    /// An error caused by deletion.
    Deletion,
    /// An error caused by notification.
    Notification,
}

impl DysonError {
    pub fn initialization_error<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            kind: DysonErrorKind::Initialization,
            source: Box::new(err),
        }
    }

    pub fn aggregation_error<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            kind: DysonErrorKind::Aggregation,
            source: Box::new(err),
        }
    }

    pub fn deletion_error<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            kind: DysonErrorKind::Deletion,
            source: Box::new(err),
        }
    }

    pub fn notification_error<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            kind: DysonErrorKind::Notification,
            source: Box::new(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::provider::{ImageDeleter, ImageDeleterError, ImageProviderError};

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
            async fn delete_images(&self, _images: ImagesSummary) -> Result<(), ImageDeleterError> {
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

            let res = dyson.aggregate_target_images().await.unwrap();
            assert_eq!(res, case.expected, "{}", case.name);
        }
    }
}
