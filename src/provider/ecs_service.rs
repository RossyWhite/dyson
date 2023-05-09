use std::collections::HashSet;

use futures::TryStreamExt;
use tokio::task::JoinSet;
use tokio_stream::StreamExt;

use crate::provider::{EcrImageId, ImageProvider, ImageProviderError};
use crate::utils::try_join_set_to_stream;

/// An ECR image provider from ECS services
struct EcsServiceImageProvider {
    /// The AWS SDK client for ECS
    client: aws_sdk_ecs::Client,
}

#[async_trait::async_trait]
impl ImageProvider for EcsServiceImageProvider {
    async fn provide_images(&self) -> Result<HashSet<EcrImageId>, ImageProviderError> {
        let clusters: Vec<String> = self
            .client
            .list_clusters()
            .into_paginator()
            .items()
            .send()
            .collect::<Result<Vec<_>, _>>()
            .await?;

        let mut tasks = JoinSet::new();
        for cluster in clusters {
            let services: Vec<String> = self
                .client
                .list_services()
                .cluster(&cluster)
                .into_paginator()
                .items()
                .send()
                .collect::<Result<Vec<_>, _>>()
                .await?;

            services.chunks(10).for_each(|chunk| {
                let client = self.client.clone();
                let chunk = chunk.to_vec();
                let cluster = cluster.clone();
                tasks.spawn(async move {
                    let tds = client
                        .describe_services()
                        .set_services(Some(chunk))
                        .cluster(cluster)
                        .send()
                        .await?
                        .services()
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|svc| svc.task_definition())
                        .map(|td| td.to_string())
                        .collect::<Vec<_>>();

                    let mut ret: HashSet<EcrImageId> = HashSet::new();
                    for td in tds.into_iter() {
                        let def = &client
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
        }

        try_join_set_to_stream(tasks)
            .try_fold(HashSet::new(), |mut acc, cur| async {
                acc.extend(cur);
                Ok(acc)
            })
            .await
            .map_err(ImageProviderError::from)
    }
}
