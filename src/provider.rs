use std::collections::HashSet;
use std::fmt::Debug;

use aws_smithy_http::result::SdkError;

use crate::image::EcrImageId;

pub mod ecr;
pub mod ecs_service;
pub mod lambda;
pub mod task_definition;

/// ImageProvider is a trait for providing images
#[async_trait::async_trait]
pub trait ImageProvider {
    async fn provide_images(&self) -> Result<HashSet<EcrImageId>, ImageProviderError>;
}

/// An error returned an ImageProvider
#[derive(Debug, thiserror::Error)]
#[error("[ImageProviderError] kind: {:?}, source: {}", self.kind, self.source)]
pub struct ImageProviderError {
    /// The kind of the error.
    kind: ImageProviderErrorKind,
    /// The source of the error.
    source: Box<dyn std::error::Error + Send + Sync>,
}

/// The kind of an ImageProviderError.
#[derive(Debug)]
pub enum ImageProviderErrorKind {
    /// An error caused by AWS SDK.
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
pub trait ImageDeleter {
    /// Delete images given by `images`.
    async fn delete_images(&self, images: &HashSet<EcrImageId>) -> Result<(), ImageDeleterError>;
}

/// An error returned an ImageDeleter
#[derive(Debug, thiserror::Error)]
#[error("[ImageDeleterError] kind: {:?}, source: {}", self.kind, self.source)]
pub struct ImageDeleterError {
    /// The kind of the error.
    kind: ImageDeleterErrorKind,
    /// The source of the error.
    source: Box<dyn std::error::Error + Send + Sync>,
}

/// The kind of an ImageDeleterError.
#[derive(Debug)]
pub enum ImageDeleterErrorKind {
    /// An error caused by AWS SDK.
    SdkError,
}

impl<T> From<SdkError<T>> for ImageDeleterError
where
    T: std::error::Error + Send + Sync + 'static,
{
    fn from(err: SdkError<T>) -> Self {
        Self {
            kind: ImageDeleterErrorKind::SdkError,
            source: Box::new(err),
        }
    }
}

/// ImageRegistry implements ImageProvider and ImageCleaner
pub trait ImageRegistry: ImageProvider + ImageDeleter {}
