use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum CacheStrategy {
    // never cache
    #[serde(rename = "no-cache")]
    NoCache,

    // Etag along with last-modified
    #[serde(rename = "conditional")]
    Conditional,

    // set time to live. expire after that
    #[serde(rename = "expiration")]
    Expiration {
        #[serde(with = "humantime_serde")]
        ttl: Duration,
    },

    // content is never changed
    #[serde(rename = "immutable")]
    Immutable,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_deserialize() {
        assert_eq!(
            CacheStrategy::NoCache,
            serde_yaml::from_str::<CacheStrategy>(
                r#"
---
strategy: no-cache
"#
            )
            .unwrap()
        );

        assert_eq!(
            CacheStrategy::Conditional,
            serde_yaml::from_str::<CacheStrategy>(
                r#"
---
strategy: conditional
"#
            )
            .unwrap()
        );

        assert_eq!(
            CacheStrategy::Immutable,
            serde_yaml::from_str::<CacheStrategy>(
                r#"
---
strategy: immutable
"#
            )
            .unwrap()
        );

        assert_eq!(
            CacheStrategy::Expiration {
                ttl: Duration::from_secs(10)
            },
            serde_yaml::from_str::<CacheStrategy>(
                r#"
---
strategy: expiration
ttl: '10s'
"#
            )
            .unwrap()
        );
    }
}
