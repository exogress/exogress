// https://github.com/durch/rust-s3/blob/45dd3f25a4047186e414e47fcedb4f83e492368e/aws-region/src/region.rs

use crate::entities::ParameterName;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(deny_unknown_fields)]
pub struct S3Bucket {
    pub bucket: ParameterName,
    pub credentials: ParameterName,
}
