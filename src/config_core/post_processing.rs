use crate::config_core::{
    referenced::{mime_types::MimeTypes, Container},
    refinable::NonExistingSharedEntity,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, JsonSchema)]
pub struct ImagePostProcessing {
    #[serde(default = "default_image_optimizations")]
    pub enabled: bool,

    #[serde(default = "default_image_optimizations")]
    pub png: bool,

    #[serde(default = "default_image_optimizations")]
    pub jpeg: bool,
}

impl Default for ImagePostProcessing {
    fn default() -> Self {
        Self {
            enabled: default_image_optimizations(),
            png: default_image_optimizations(),
            jpeg: default_image_optimizations(),
        }
    }
}

fn default_image_optimizations() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, JsonSchema)]
pub struct Encoding {
    #[serde(default = "default_compression")]
    pub enabled: bool,

    #[serde(rename = "mime-types")]
    pub mime_types: Container<MimeTypes>,

    #[serde(default = "default_compression")]
    pub brotli: bool,

    #[serde(default = "default_compression")]
    pub gzip: bool,

    #[serde(default = "default_compression")]
    pub deflate: bool,

    #[serde(default = "default_compression_min_size", rename = "min-size")]
    pub min_size: u32,
}

fn default_compressible_mime_types() -> Container<MimeTypes, NonExistingSharedEntity> {
    Container::Parameter("compressible-mime-types".parse().unwrap())
}

fn default_compression() -> bool {
    true
}

fn default_compression_min_size() -> u32 {
    100
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq, JsonSchema)]
pub struct PostProcessing {
    #[serde(default, rename = "image-optimization")]
    pub image_optimization: ImagePostProcessing,

    #[serde(default)]
    pub encoding: Encoding,
}

impl Default for Encoding {
    fn default() -> Self {
        Self {
            enabled: default_compression(),
            mime_types: default_compressible_mime_types(),
            brotli: default_compression(),
            gzip: default_compression(),
            deflate: default_compression(),
            min_size: default_compression_min_size(),
        }
    }
}

impl Default for PostProcessing {
    fn default() -> Self {
        Self {
            image_optimization: Default::default(),
            encoding: Default::default(),
        }
    }
}
