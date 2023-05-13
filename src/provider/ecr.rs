use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use aws_sdk_ecr::types::{DescribeImagesFilter, ImageDetail, Repository, TagStatus};
use futures::TryStreamExt;
use tokio::task::JoinSet;
use tokio_stream::StreamExt;

use crate::config::{RegistryConfig, RepositoryFilterConfig};
use crate::image::{EcrImageDetail, EcrImageId, ImagesSummary};
use crate::provider::{ImageDeleter, ImageDeleterError, ImageRegistry};
use crate::provider::{ImageProvider, ImageProviderError};
use crate::utils::try_join_set_to_stream;

/// An ECR image Registry
pub struct EcrImageRegistry {
    /// The AWS SDK client for ECR
    client: aws_sdk_ecr::Client,
    /// The filter for images
    filter: Arc<ImageFilter>,
    /// The repository excluder
    excluder: Arc<RepositoryExcluder>,
}

impl EcrImageRegistry {
    pub async fn from_conf(conf: &RegistryConfig) -> Result<EcrImageRegistry, ImageProviderError> {
        let client = aws_sdk_ecr::Client::new(
            &aws_config::from_env()
                .profile_name(&conf.profile_name)
                .load()
                .await,
        );

        let filter = Arc::new(ImageFilter::try_new(
            conf.filters.as_ref().unwrap_or(&Vec::new()),
        )?);
        let excluder = Arc::new(RepositoryExcluder::new(
            conf.excludes.as_ref().unwrap_or(&Vec::new()),
        )?);

        Ok(Self {
            client,
            filter,
            excluder,
        })
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

        let now = SystemTime::now();
        let mut tasks = JoinSet::new();
        repos.into_iter().for_each(|r| {
            let client = self.client.clone();
            let filter = self.filter.clone();
            let Some(registry_id) = r.registry_id().map(|s| s.to_owned()) else { return; };
            let Some(repository_name) = r.repository_name().map(|s| s.to_owned()) else { return; };
            let Some(region) = client.conf().region().map(|s| s.to_string()) else { return; };

            // Skip if the repository is excluded
            if self.excluder.is_excluded(&repository_name) {
                return;
            }

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
                        .filter(|img| filter.is_match(img, now))
                        .map(|img| img.id)
                        .collect::<HashSet<_>>();

                    targets.extend(filtered);
                }

                Ok::<_, ImageProviderError>(targets)
            });
        });

        try_join_set_to_stream(tasks)
            .try_fold(HashSet::new(), |mut acc, cur| async {
                let ids = cur.into_iter().collect::<HashSet<_>>();
                acc.extend(ids);
                Ok(acc)
            })
            .await
            .map_err(ImageProviderError::from)
    }
}

#[async_trait::async_trait]
impl ImageDeleter for EcrImageRegistry {
    async fn delete_images(&self, images: &ImagesSummary) -> Result<(), ImageDeleterError> {
        for (repo, ids) in images {
            for chunk in ids.chunks(100).map(|chunk| chunk.to_vec()) {
                self.client
                    .batch_delete_image()
                    .repository_name(repo)
                    .set_image_ids(Some(chunk))
                    .send()
                    .await?;
            }
        }

        Ok(())
    }
}

impl ImageRegistry for EcrImageRegistry {}

/// A filter for deciding whether an image is target or not
#[cfg_attr(test, derive(Debug))]
struct ImageFilter {
    /// Vector of filter items
    filters: Vec<ImageFilterItem>,
}

impl ImageFilter {
    /// Create a new ImageFilter
    fn try_new(conf: &[RepositoryFilterConfig]) -> Result<Self, ImageProviderError> {
        Ok(Self {
            filters: conf
                .iter()
                .map(ImageFilterItem::try_new)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    /// Decide whether the image is target or not
    fn is_match(&self, image: &EcrImageDetail, now: SystemTime) -> bool {
        self.filters.iter().all(|f| f.is_match(image, now))
    }
}

/// a filter item of ImageFilter
#[cfg_attr(test, derive(Debug))]
struct ImageFilterItem {
    /// The glob pattern for repository name
    pattern: glob::Pattern,
    /// the image is target if it is elapsed this days after pushed
    days_after: u64,
    /// The glob patterns for tag to ignore
    ignore_tag_patterns: Vec<glob::Pattern>,
}

impl ImageFilterItem {
    /// Create a new ImageFilterItem
    fn try_new(conf: &RepositoryFilterConfig) -> Result<Self, ImageProviderError> {
        Ok(Self {
            pattern: glob::Pattern::new(conf.pattern.as_str())
                .map_err(ImageProviderError::initialization_error)?,
            days_after: conf.days_after.unwrap_or(0), // by default, all images are target after pushed
            ignore_tag_patterns: conf
                .ignore_tag_patterns
                .as_ref()
                .unwrap_or(&Vec::new())
                .iter()
                .map(|p| {
                    glob::Pattern::new(p.as_str()).map_err(ImageProviderError::initialization_error)
                })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    /// Decide whether the image is target or not
    pub fn is_match(&self, image: &EcrImageDetail, now: SystemTime) -> bool {
        // if repository name not match, that means this image is target (ignore)
        if !self.pattern.matches(image.id.repository_name.as_str()) {
            return true;
        }

        let n_days_before = aws_smithy_types::DateTime::from(
            now - Duration::from_secs(self.days_after * 24 * 60 * 60),
        );

        // if image is pushed is newer than n_days_before, that means this image is not target
        if image.image_pushed_at.as_secs_f64() > n_days_before.as_secs_f64() {
            return false;
        }

        // if image tag matches ignore_tag_pattern, that means this image is not target
        for ignore_tag_pattern in &self.ignore_tag_patterns {
            if ignore_tag_pattern.matches(image.id.image_tag.as_str()) {
                return false;
            }
        }

        true
    }
}

/// A filter for deciding whether a repository is target or not
#[cfg_attr(test, derive(Debug))]
struct RepositoryExcluder {
    patterns: Vec<glob::Pattern>,
}

impl RepositoryExcluder {
    /// Create a new RepositoryExcluder
    fn new(conf: &[String]) -> Result<Self, ImageProviderError> {
        let patterns = conf
            .iter()
            .map(|p| glob::Pattern::new(p))
            .collect::<Result<Vec<_>, _>>()
            .map_err(ImageProviderError::initialization_error)?;

        Ok(Self { patterns })
    }

    /// Decide whether the repository is target or not
    fn is_excluded(&self, repository_name: &str) -> bool {
        self.patterns.iter().any(|p| p.matches(repository_name))
    }
}

#[cfg(test)]
mod tests {
    use aws_smithy_types::date_time::Format;
    use aws_smithy_types::DateTime;
    use glob::Pattern;

    use super::*;

    #[test]
    fn filter_test() {
        #[derive(Debug)]
        struct TestCase {
            name: String,
            excluder: Option<RepositoryExcluder>,
            filter: ImageFilter,
            now: SystemTime,
            input: Vec<EcrImageDetail>,
            expected: HashSet<EcrImageId>,
        }

        let cases = vec![
            TestCase {
                name: "All images will be target if no filters".to_string(),
                excluder: None,
                filter: ImageFilter { filters: vec![] },
                now: SystemTime::UNIX_EPOCH,
                input: vec![EcrImageDetail::new(
                    "registry_id",
                    "region",
                    "repository_name",
                    "image_tag",
                    DateTime::from_str("1970-01-01T00:00:00Z", Format::DateTime).unwrap(),
                )],
                expected: HashSet::from([EcrImageId::new(
                    "registry_id",
                    "region",
                    "repository_name",
                    "image_tag",
                )]),
            },
            TestCase {
                name: "If repository name does not match, image will be target".to_string(),
                excluder: None,
                filter: ImageFilter {
                    filters: vec![
                        ImageFilterItem {
                            pattern: Pattern::new("dummy-*").unwrap(),
                            days_after: 0,
                            ignore_tag_patterns: vec![],
                        }
                    ]
                },
                now: SystemTime::UNIX_EPOCH,
                input: vec![EcrImageDetail::new(
                    "registry_id",
                    "region",
                    "repository_name",
                    "image_tag",
                    DateTime::from_str("1970-01-01T00:00:00Z", Format::DateTime).unwrap(),
                )],
                expected: HashSet::from([EcrImageId::new(
                    "registry_id",
                    "region",
                    "repository_name",
                    "image_tag",
                )]),
            },
            TestCase {
                name: "Repository name is matched, but the image is too new to be deleted".to_string(),
                excluder: None,
                filter: ImageFilter {
                    filters: vec![
                        ImageFilterItem {
                            pattern: Pattern::new("match-*").unwrap(),
                            days_after: 30,
                            ignore_tag_patterns: vec![],
                        }
                    ]
                },
                now: SystemTime::UNIX_EPOCH,
                input: vec![EcrImageDetail::new(
                    "registry_id",
                    "region",
                    "match-2",
                    "image_tag",
                    DateTime::from_str("1969-12-03T00:00:00Z", Format::DateTime).unwrap(),
                )],
                expected: Default::default(),
            },
            TestCase {
                name: "Repository name is matched, and the image is old enough to be deleted".to_string(),
                excluder: None,
                filter: ImageFilter {
                    filters: vec![
                        ImageFilterItem {
                            pattern: Pattern::new("match-*").unwrap(),
                            days_after: 30,
                            ignore_tag_patterns: vec![],
                        }
                    ]
                },
                now: SystemTime::UNIX_EPOCH,
                input: vec![EcrImageDetail::new(
                    "registry_id",
                    "region",
                    "match-2",
                    "image_tag",
                    // UNIX_EPOCH - 31 days. this image is old enough to be deleted
                    DateTime::from_str("1969-12-01T00:00:00Z", Format::DateTime).unwrap(),
                )],
                expected: HashSet::from([EcrImageId::new(
                    "registry_id",
                    "region",
                    "match-2",
                    "image_tag",
                )]),
            },
            TestCase {
                name: "Repository name is matched, and the image is old enough to be deleted, but the tag matches ignore_tag_pattern".to_string(),
                excluder: None,
                filter: ImageFilter {
                    filters: vec![
                        ImageFilterItem {
                            pattern: Pattern::new("match-*").unwrap(),
                            days_after: 30,
                            ignore_tag_patterns: vec![
                                Pattern::new("ignore1-*").unwrap(),
                                Pattern::new("ignore2-*").unwrap(),
                            ],
                        }
                    ]
                },
                now: SystemTime::UNIX_EPOCH,
                input: vec![EcrImageDetail::new(
                    "registry_id",
                    "region",
                    "match-2",
                    // this tag matches ignore_tag_pattern
                    "ignore2-tag",
                    // UNIX_EPOCH - 31 days. this image is old enough to be deleted
                    DateTime::from_str("1969-12-01T00:00:00Z", Format::DateTime).unwrap(),
                )],
                expected: Default::default(),
            },
            TestCase {
                name: "If repository name matches multiple filters, images will be target if all filter matches".to_string(),
                excluder: None,
                filter: ImageFilter {
                    filters: vec![
                        ImageFilterItem {
                            pattern: Pattern::new("match-*").unwrap(),
                            days_after: 50,
                            ignore_tag_patterns: vec![],
                        },
                        ImageFilterItem {
                            pattern: Pattern::new("match-*").unwrap(),
                            days_after: 30,
                            ignore_tag_patterns: vec![],
                        },
                    ]
                },
                now: SystemTime::UNIX_EPOCH,
                input: vec![EcrImageDetail::new(
                    "registry_id",
                    "region",
                    "match-1",
                    // this tag matches ignore_tag_pattern
                    "ignore2-tag",
                    // UNIX_EPOCH - 31 days. this image matches one filter, but not all filters
                    DateTime::from_str("1969-12-01T00:00:00Z", Format::DateTime).unwrap(),
                )],
                expected: Default::default(),
            },
            TestCase {
                name: "Repository is excluded by excluder".to_string(),
                excluder: Some(
                    RepositoryExcluder {
                        patterns: vec![Pattern::new("match-*").unwrap()],
                    }
                ),
                filter: ImageFilter {
                    filters: vec![]
                },
                now: SystemTime::UNIX_EPOCH,
                input: vec![EcrImageDetail::new(
                    "registry_id",
                    "region",
                    "match-2",
                    "image_tag",
                    // UNIX_EPOCH - 31 days. this image is old enough to be deleted
                    DateTime::from_str("1969-12-01T00:00:00Z", Format::DateTime).unwrap(),
                )],
                expected: HashSet::new(),
            },
        ];

        for case in cases {
            let actual = case
                .input
                .into_iter()
                .filter(|image| {
                    if let Some(excluder) = &case.excluder {
                        return !excluder.is_excluded(&image.id.repository_name);
                    }
                    true
                })
                .filter(|image| case.filter.is_match(image, case.now))
                .map(|image| image.id)
                .collect::<HashSet<_>>();

            assert_eq!(actual, case.expected, "{}", case.name);
        }
    }
}
