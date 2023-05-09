use std::collections::HashSet;

use aws_sdk_lambda::types::{FunctionConfiguration, PackageType};
use futures::TryStreamExt;
use tokio::task::JoinSet;
use tokio_stream::StreamExt;

use crate::provider::{EcrImageId, ImageProvider, ImageProviderError};
use crate::utils::try_join_set_to_stream;

/// An ECR image provider from lambda functions
pub struct LambdaImageProvider {
    /// The AWS SDK client for Lambda
    client: aws_sdk_lambda::Client,
}

impl LambdaImageProvider {
    pub fn from_conf(conf: &aws_config::SdkConfig) -> LambdaImageProvider {
        let client = aws_sdk_lambda::Client::new(conf);
        Self { client }
    }
}

#[async_trait::async_trait]
impl ImageProvider for LambdaImageProvider {
    async fn provide_images(&self) -> Result<HashSet<EcrImageId>, ImageProviderError> {
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
