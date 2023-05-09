use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use aws_sdk_ecr::types::{
    DescribeImagesFilter, ImageDetail, ImageIdentifier, Repository, TagStatus,
};
use futures::TryStreamExt;
use tokio::task::JoinSet;
use tokio_stream::StreamExt;

use crate::config::{RegistryConfig, RepositoryTargetsConfig};
use crate::image::{EcrImageDetail, EcrImageId};
use crate::provider::{ImageDeleter, ImageDeleterError, ImageRegistry};
use crate::provider::{ImageProvider, ImageProviderError};
use crate::utils::try_join_set_to_stream;

/// An ECR image Registry
pub struct EcrImageRegistry {
    /// The AWS SDK client for ECR
    client: aws_sdk_ecr::Client,
    /// The filters for images
    filters: Arc<Vec<ImageFilter>>,
}

impl EcrImageRegistry {
    pub async fn from_conf(conf: &RegistryConfig) -> EcrImageRegistry {
        let client = aws_sdk_ecr::Client::new(
            &aws_config::from_env()
                .profile_name(&conf.profile_name)
                .load()
                .await,
        );

        let filters = Arc::new(
            conf.targets
                .iter()
                .map(ImageFilter::new)
                .collect::<Vec<_>>(),
        );

        Self { client, filters }
    }
}

#[async_trait::async_trait]
impl ImageProvider for EcrImageRegistry {
    async fn provide_images(&self) -> Result<HashSet<EcrImageId>, ImageProviderError> {
        let repos: Vec<Repository> = self
            .client
            .describe_repositories()
            .into_paginator()
            .items()
            .send()
            .collect::<Result<Vec<_>, _>>()
            .await?;

        let mut tasks = JoinSet::new();
        repos.into_iter().for_each(|r| {
            let client = self.client.clone();
            let filters = self.filters.clone();
            let Some(registry_id) = r.registry_id().map(|s| s.to_owned()) else { return; };
            let Some(repository_name) = r.repository_name().map(|s| s.to_owned()) else { return; };
            let Some(region) = client.conf().region().map(|s| s.to_string()) else { return; };

            tasks.spawn(async move {
                let details: Vec<ImageDetail> = client
                    .describe_images()
                    .repository_name(&repository_name)
                    .filter(
                        DescribeImagesFilter::builder()
                            .tag_status(TagStatus::Tagged)
                            .build(),
                    ) // Note: currently only tagged images are supported
                    .into_paginator()
                    .items()
                    .send()
                    .collect::<Result<Vec<_>, _>>()
                    .await?;

                let mut targets = Vec::new();
                for detail in details {
                    let Some(pushed_at) = detail.image_pushed_at().map(|s| s.to_owned()) else { continue; };
                    let Some(tags) = detail.image_tags().map(|s| s.to_owned()) else { continue; };

                    let filtered = tags.iter().map(|t| EcrImageDetail::new(
                        &registry_id,
                        &region,
                        &repository_name,
                        t,
                        pushed_at,
                    ))
                        .filter(|img| {
                            filters.iter().any(|f| f.is_match(img))
                        })
                        .map(|img| img.id)
                        .collect::<HashSet<_>>();

                    targets.extend(filtered);
                }

                Ok::<_, ImageProviderError>(targets)
            });
        });

        try_join_set_to_stream(tasks)
            .try_fold(HashSet::new(), |mut acc, cur| async {
                let ids = cur.into_iter().map(|i| i).collect::<HashSet<_>>();
                acc.extend(ids);
                Ok(acc)
            })
            .await
            .map_err(ImageProviderError::from)
    }
}

#[async_trait::async_trait]
impl ImageDeleter for EcrImageRegistry {
    async fn delete_images(&self, images: &HashSet<EcrImageId>) -> Result<(), ImageDeleterError> {
        let per_repo = images.iter().fold(HashMap::new(), |mut acc, img| {
            let repo = img.repository_name.clone();
            let id = ImageIdentifier::builder().image_tag(&img.image_tag).build();
            acc.entry(repo).or_insert_with(|| Vec::new()).push(id);
            acc
        });

        for (repo, ids) in per_repo {
            self.client
                .batch_delete_image()
                .repository_name(&repo)
                .set_image_ids(Some(ids))
                .send()
                .await?;
        }

        Ok(())
    }
}

impl ImageRegistry for EcrImageRegistry {}

struct ImageFilter {
    pattern: glob::Pattern,
    days_after: u64,
    ignore_tag_patterns: Vec<glob::Pattern>,
}

impl ImageFilter {
    pub fn new(conf: &RepositoryTargetsConfig) -> Self {
        Self {
            pattern: glob::Pattern::new(conf.pattern.as_str()).unwrap(), // todo
            days_after: conf.days_after,
            ignore_tag_patterns: conf
                .ignore_tag_patterns
                .iter()
                .map(|p| glob::Pattern::new(p.as_str()).unwrap())
                .collect::<Vec<_>>(),
        }
    }

    pub fn is_match(&self, image: &EcrImageDetail) -> bool {
        // if not match, that means this image is not target
        if !self.pattern.matches(image.id.repository_name.as_str()) {
            return false;
        }

        let n_days_before = aws_smithy_types::DateTime::from(
            SystemTime::now() - Duration::from_secs(self.days_after * 24 * 60 * 60),
        );

        // if image is pushed after n_days_before, that means this image is not target
        if image.image_pushed_at.as_secs_f64() > n_days_before.as_secs_f64() {
            return false;
        }

        // if image tag matches ignore_tag_pattern, that means this image is not target
        for ignore_tag_pattern in &self.ignore_tag_patterns {
            if ignore_tag_pattern.matches(image.id.image_tag.as_str()) {
                return false;
            }
        }

        return true;
    }
}
