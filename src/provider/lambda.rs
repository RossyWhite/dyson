use futures::TryStreamExt;
use std::collections::HashSet;
use std::sync::Arc;

use aws_sdk_lambda::types::{FunctionConfiguration, PackageType};

use tokio::task::JoinSet;
use tokio_stream::StreamExt;

use crate::provider::{EcrImageId, ImageProvider, ImageProviderError};
use crate::utils::try_join_set_to_stream;

/// An ECR image source from lambda functions
struct LambdaImageProvider {
    client: Arc<aws_sdk_lambda::Client>,
}

#[async_trait::async_trait]
impl ImageProvider for LambdaImageProvider {
    async fn list_images(&self) -> Result<HashSet<EcrImageId>, ImageProviderError> {
        let functions: Vec<FunctionConfiguration> = self
            .client
            .list_functions()
            .into_paginator()
            .items()
            .send()
            .collect::<Result<Vec<_>, _>>()
            .await?;

        let mut tasks = JoinSet::new();
        functions
            .into_iter()
            .filter(|f| f.package_type() == Some(&PackageType::Image))
            .for_each(|f| {
                let client = self.client.clone();
                tasks.spawn(async move {
                    client
                        .get_function()
                        .set_function_name(f.function_name().map(|s| s.to_owned()))
                        .send()
                        .await
                        .map(|output| {
                            output
                                .code()
                                .and_then(|code| code.image_uri())
                                .and_then(EcrImageId::from_image_uri_opt)
                        })
                        .map_err(ImageProviderError::from)
                });
            });

        try_join_set_to_stream(tasks)
            .try_fold(HashSet::new(), |mut acc, cur| async {
                cur.map(|cur| acc.insert(cur));
                Ok(acc)
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a() {
        let config = aws_config::load_from_env().await;
        let client = aws_sdk_lambda::Client::new(&config);
        let source = LambdaImageProvider {
            client: Arc::new(client),
        };
        let images = source.list_images().await.unwrap();
        println!("images: {:?}", images);
    }
}
