use crate::config_core::parametrized::mime_types::MimeTypes;
use crate::config_core::parametrized::Container;
use serde::{Deserialize, Serialize};

// #[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
// #[serde(deny_unknown_fields)]
// pub struct ImagePostProcessing {
//     pub webp: WebpPostProcessing,
// }
//
// #[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
// #[serde(deny_unknown_fields)]
// pub struct WebpPostProcessing {
//     enabled: bool,
// }

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Encoding {
    #[serde(rename = "mime-types")]
    pub mime_types: Container<MimeTypes>,

    #[serde(default = "default_compression")]
    pub brotli: bool,

    #[serde(default = "default_compression")]
    pub gzip: bool,

    #[serde(default = "default_compression")]
    pub deflate: bool,

    #[serde(default = "default_compression_min_size")]
    pub min_size: u32,
}

fn default_compressible_mime_types() -> Container<MimeTypes> {
    Container::Parameter("compressible-mime-types".parse().unwrap())
}

fn default_compression() -> bool {
    true
}

fn default_compression_min_size() -> u32 {
    100
}

#[derive(Serialize, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PostProcessing {
    // pub image: ImagePostProcessing,
    pub encoding: Encoding,
}

// impl Default for ImagePostProcessing {
//     fn default() -> Self {
//         Self {
//             image: true,
//             encoding: false,
//         }
//     }
// }

impl Default for Encoding {
    fn default() -> Self {
        Self {
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
            encoding: Default::default(),
        }
    }
}
