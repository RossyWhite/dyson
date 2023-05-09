use std::collections::HashSet;
use std::sync::Arc;

use aws_sdk_ecs::types::{SortOrder, TaskDefinitionFamilyStatus, TaskDefinitionStatus};
use futures::TryStreamExt;
use tokio::task::JoinSet;
use tokio_stream::StreamExt;

use crate::provider::{EcrImageId, ImageProvider, ImageProviderError};
use crate::utils::try_join_set_to_stream;

/// An ECR image source from Task Definitions
struct TaskDefinitionSource {
    /// The AWS SDK client for ECS
    client: Arc<aws_sdk_ecs::Client>,
    /// The maximum number of results to return in a single family.
    max_result: i32,
}

#[async_trait::async_trait]
impl ImageProvider for TaskDefinitionSource {
    async fn provide_images(&self) -> Result<HashSet<EcrImageId>, ImageProviderError> {
        let families: Vec<String> = self
            .client
            .list_task_definition_families()
            .status(TaskDefinitionFamilyStatus::Active)
            .into_paginator()
            .items()
            .send()
            .collect::<Result<Vec<_>, _>>()
            .await?;

        let mut tasks = JoinSet::new();
        families.into_iter().for_each(|fam| {
            let client = self.client.clone();
            let max_result = self.max_result;
            tasks.spawn(async move {
                let tds = client
                    .list_task_definitions()
                    .family_prefix(&fam)
                    .status(TaskDefinitionStatus::Active)
                    .sort(SortOrder::Desc)
                    .max_results(max_result)
                    .into_paginator()
                    .items()
                    .send()
                    .collect::<Result<Vec<_>, _>>()
                    .await?;

                let mut ret: HashSet<EcrImageId> = HashSet::new();

                for td in tds.iter() {
                    let def = client
                        .describe_task_definition()
                        .task_definition(td)
                        .send()
                        .await?;

                    let cs = def
                        .task_definition()
                        .and_then(|td| td.container_definitions())
                        .unwrap_or_default();

                    let images = cs
                        .iter()
                        .filter_map(|c| c.image())
                        .filter_map(EcrImageId::from_image_uri_opt)
                        .collect::<HashSet<_>>();
                    ret.extend(images);
                }

                Ok::<_, ImageProviderError>(ret)
            });
        });

        try_join_set_to_stream(tasks)
            .try_fold(HashSet::new(), |mut acc, cur| async {
                acc.extend(cur);
                Ok(acc)
            })
            .await
            .map_err(ImageProviderError::from)
    }
}
