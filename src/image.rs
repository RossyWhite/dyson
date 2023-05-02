use std::fmt::Debug;

use once_cell::sync::OnceCell;
use regex::Regex;

/// An image identifier in ECR
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub struct EcrImageId {
    /// The AWS account ID associated with the registry containing the image.
    pub registry_id: String,
    /// The AWS region associated with the registry containing the image.
    pub region: String,
    /// The name of the image's repository.
    pub repository_name: String,
    /// The tag used for the image.
    pub image_tag: String,
}

impl EcrImageId {
    pub fn new(
        registry_id: impl Into<String>,
        region: impl Into<String>,
        repository_name: impl Into<String>,
        image_tag: impl Into<String>,
    ) -> Self {
        Self {
            registry_id: registry_id.into(),
            region: region.into(),
            repository_name: repository_name.into(),
            image_tag: image_tag.into(),
        }
    }

    /// Parse an image URI into an EcrImage
    pub fn from_image_uri_opt(uri: &str) -> Option<Self> {
        let pattern = {
            static RE: OnceCell<Regex> = OnceCell::new();
            RE.get_or_init(|| Regex::new(
                r"^(?P<registry_id>\d{12})\.dkr\.ecr\.(?P<region>[a-z0-9-]+)\.amazonaws.com/(?P<repository_name>[^:]+):(?P<image_tag>[^:]+)$"
            ).unwrap())
        };

        pattern.captures(uri).map(|caps| Self {
            registry_id: caps.name("registry_id").unwrap().as_str().to_owned(),
            region: caps.name("region").unwrap().as_str().to_owned(),
            repository_name: caps.name("repository_name").unwrap().as_str().to_owned(),
            image_tag: caps.name("image_tag").unwrap().as_str().to_owned(),
        })
    }
}

/// An image in ECR
#[derive(Debug, Clone)]
pub struct EcrImageDetail {
    /// image identifier
    pub id: EcrImageId,
    /// the date and time which the image was pushed to the repository
    pub image_pushed_at: aws_smithy_types::DateTime,
}

impl EcrImageDetail {
    pub fn new(
        registry_id: impl Into<String>,
        region: impl Into<String>,
        repository_name: impl Into<String>,
        image_tag: impl Into<String>,
        image_pushed_at: aws_smithy_types::DateTime,
    ) -> Self {
        Self {
            id: EcrImageId::new(registry_id, region, repository_name, image_tag),
            image_pushed_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_image_uri_opt() {
        let cases = vec![
            ("public.ecr.aws/nginx/nginx:stable", None),
            ("nginx:latest", None),
            ("gcr.io/google-containers/nginx:latest", None),
            (
                "123456789012.dkr.ecr.us-east-1.amazonaws.com/A/b:latest",
                Some(EcrImageId {
                    registry_id: "123456789012".to_string(),
                    region: "us-east-1".to_string(),
                    repository_name: "A/b".to_string(),
                    image_tag: "latest".to_string(),
                }),
            ),
        ];

        for (input, expected) in cases {
            assert_eq!(EcrImageId::from_image_uri_opt(input), expected);
        }
    }
}
