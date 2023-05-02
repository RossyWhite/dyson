use aws_smithy_http::result::SdkError;
use std::collections::HashSet;
use std::fmt::Debug;

use crate::image::EcrImageId;

pub mod ecr;
pub mod ecs_service;
pub mod lambda;
pub mod task_definition;

/// ImageProvider is a trait for listing images
#[async_trait::async_trait]
pub trait ImageProvider {
    async fn list_images(&self) -> Result<HashSet<EcrImageId>, ImageProviderError>;
}

/// An error returned an ImageProvider
#[derive(Debug, thiserror::Error)]
#[error("[ImageProviderError] kind: {:?}, source: {}", self.kind, self.source)]
pub struct ImageProviderError {
    kind: ImageProviderErrorKind,
    source: Box<dyn std::error::Error + Send + Sync>,
}

#[derive(Debug)]
pub enum ImageProviderErrorKind {
    SdkError,
}

impl<T> From<SdkError<T>> for ImageProviderError
where
    T: std::error::Error + Send + Sync + 'static,
{
    fn from(err: SdkError<T>) -> Self {
        Self {
            kind: ImageProviderErrorKind::SdkError,
            source: Box::new(err),
        }
    }
}

/// ImageCleaner is a trait for deleting images
#[async_trait::async_trait]
pub trait ImageCleaner {
    async fn delete_images(&self, images: &HashSet<EcrImageId>) -> Result<(), ImageCleanerError>;
}

/// An error returned an ImageCleaner
#[derive(Debug, thiserror::Error)]
#[error("[ImageCleanerError] kind: {:?}, source: {}", self.kind, self.source)]
pub struct ImageCleanerError {
    kind: ImageCleanerErrorKind,
    source: Box<dyn std::error::Error + Send + Sync>,
}

#[derive(Debug)]
pub enum ImageCleanerErrorKind {
    SdkError,
}

impl<T> From<SdkError<T>> for ImageCleanerError
where
    T: std::error::Error + Send + Sync + 'static,
{
    fn from(err: SdkError<T>) -> Self {
        Self {
            kind: ImageCleanerErrorKind::SdkError,
            source: Box::new(err),
        }
    }
}

/// ImageRegistry implements ImageProvider and ImageCleaner
pub trait ImageRegistry: ImageProvider + ImageCleaner {}
