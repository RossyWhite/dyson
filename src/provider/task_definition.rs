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
    client: Arc<aws_sdk_ecs::Client>,
    max_result: i32,
}

#[async_trait::async_trait]
impl ImageProvider for TaskDefinitionSource {
    async fn list_images(&self) -> Result<HashSet<EcrImageId>, ImageProviderError> {
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
//
// #[derive(Debug)]
// pub struct TaskDefinitionSourceError {
//     kind: TaskDefinitionSourceErrorKind,
//     source: Box<dyn Error + Send + Sync>,
// }
//
// #[derive(Debug)]
// pub enum TaskDefinitionSourceErrorKind {
//     /// An error caused by AWS SDK.
//     SdkError,
//     /// An error caused by other reasons.
//     Other,
// }
//
// impl Display for TaskDefinitionSourceError {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         write!(f, "kind: {:?}, source: {}", self.kind, self.source)
//     }
// }
//
// impl Error for TaskDefinitionSourceError {
//     fn source(&self) -> Option<&(dyn Error + 'static)> {
//         Some(self.source.as_ref())
//     }
// }
//
// impl TaskDefinitionSourceError {
//     pub fn sdk_error<E>(e: SdkError<E>) -> Self
//     where
//         E: Error + Send + Sync + 'static,
//     {
//         Self {
//             kind: TaskDefinitionSourceErrorKind::SdkError,
//             source: Box::new(e),
//         }
//     }
//
//     pub fn other<E>(e: E) -> Self
//     where
//         E: Error + Send + Sync + 'static,
//     {
//         Self {
//             kind: TaskDefinitionSourceErrorKind::Other,
//             source: Box::new(e),
//         }
//     }
// }
//
// impl From<TaskDefinitionSourceError> for ImageProviderError {
//     fn from(err: TaskDefinitionSourceError) -> ImageProviderError {
//         ImageProviderError::new(err)
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn c() {
        let config = aws_config::load_from_env().await;
        let client = aws_sdk_ecs::Client::new(&config);
        let source = TaskDefinitionSource {
            client: Arc::new(client),
            max_result: 2,
        };
        let images = source.list_images().await.unwrap();
        println!("images: {:?}", images)
    }
}
