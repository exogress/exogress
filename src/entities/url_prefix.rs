use core::fmt;
use url::Url;

use serde::{de, de::Visitor, Deserialize, Deserializer, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct MountPointBaseUrl {
    inner: String,
}

impl MountPointBaseUrl {
    pub fn to_https_url(&self) -> String {
        format!("http://{}", self.inner)
    }
}

impl fmt::Display for MountPointBaseUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum UrlPrefixError {
    #[error("port should not exist")]
    PortFound,

    #[error("fragment should not exist")]
    FragmentFound,

    #[error("auth should not be present")]
    AuthFound,

    #[error("qurty should not be present")]
    QueryFound,

    #[error("host not found")]
    HostNotFound,

    #[error("path root not set")]
    PathRootNotSet,

    #[error("parse error {0}")]
    ParseError(#[from] url::ParseError),

    #[error("malformed")]
    Malformed,
}

impl MountPointBaseUrl {
    pub fn to_url(&self) -> Url {
        Url::parse(format!("http://{}", self.inner).as_str()).unwrap()
    }

    pub fn domain_only(&self) -> MountPointBaseUrl {
        let url = self.to_url();
        MountPointBaseUrl::from_str(format!("{}/", url.host_str().unwrap()).as_str()).unwrap()
    }

    pub fn host(&self) -> String {
        let url = self.to_url();
        url.host_str().unwrap().to_string().into()
    }

    pub fn path(&self) -> std::string::String {
        let url = self.to_url();
        url.path().to_string()
    }

    pub fn is_subpath_of_or_equal(&self, other: &MountPointBaseUrl) -> bool {
        if self.inner.len() > other.inner.len() {
            return false;
        }
        if !other.inner.starts_with(self.inner.as_str()) {
            return false;
        }

        //last symbol
        if other.inner.len() == self.inner.len() {
            return true;
        }

        let cur_char = other.inner.chars().nth(self.inner.len() - 1).unwrap();
        if self.inner.ends_with('/') && cur_char == '/' {
            return true;
        }

        //is next symbol == '\'
        let next_char = other.inner.chars().nth(self.inner.len()).unwrap();
        next_char == '/'
    }
}

impl FromStr for MountPointBaseUrl {
    type Err = UrlPrefixError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let url = Url::parse(format!("http://{}", s).as_str())?;

        if url.port().is_some() {
            return Err(UrlPrefixError::PortFound);
        }

        if url.fragment().is_some() {
            return Err(UrlPrefixError::FragmentFound);
        }

        if url.query().is_some() {
            return Err(UrlPrefixError::QueryFound);
        }

        if !url.has_host() {
            return Err(UrlPrefixError::HostNotFound);
        }

        if url.password().is_some() || !url.username().is_empty() {
            return Err(UrlPrefixError::AuthFound);
        }

        if url.path() == "/" && s.chars().last() != Some('/') {
            return Err(UrlPrefixError::PathRootNotSet);
        }

        let restored = url.to_string()[7..].to_string();
        if restored != s {
            return Err(UrlPrefixError::Malformed);
        }

        let mut inner: String = restored.into();

        if url.path().is_empty() {
            inner.push('/');
        } else if url.path() != "/" {
            inner = inner.trim_end_matches('/').into();
        }

        Ok(MountPointBaseUrl { inner })
    }
}

impl MountPointBaseUrl {
    pub fn as_str(&self) -> &str {
        self.inner.as_str()
    }
}

struct UrlPrefixVisitor;

impl<'de> Visitor<'de> for UrlPrefixVisitor {
    type Value = MountPointBaseUrl;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("URL prefix")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match MountPointBaseUrl::from_str(value) {
            Ok(segment) => Ok(segment),
            Err(e) => Err(de::Error::custom(e)),
        }
    }
}

impl<'de> Deserialize<'de> for MountPointBaseUrl {
    fn deserialize<D>(deserializer: D) -> Result<MountPointBaseUrl, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(UrlPrefixVisitor)
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_parse() {
        MountPointBaseUrl::from_str("asd").err().unwrap();
        MountPointBaseUrl::from_str("link/path").unwrap();
        MountPointBaseUrl::from_str("link.com/").unwrap();
        MountPointBaseUrl::from_str("localhost/").unwrap();
        MountPointBaseUrl::from_str("http://").err().unwrap();
    }

    #[test]
    fn test_deserialize() {
        serde_json::from_str::<MountPointBaseUrl>("\"asd\"")
            .err()
            .unwrap();
        serde_json::from_str::<MountPointBaseUrl>("\"link.com/\"").unwrap();
    }

    #[test]
    fn test_subpath() {
        assert!(MountPointBaseUrl::from_str("host/")
            .unwrap()
            .is_subpath_of_or_equal(&MountPointBaseUrl::from_str("host/d").unwrap()));

        assert!(!MountPointBaseUrl::from_str("host/d")
            .unwrap()
            .is_subpath_of_or_equal(&MountPointBaseUrl::from_str("host/").unwrap()));

        assert!(MountPointBaseUrl::from_str("host/a/b")
            .unwrap()
            .is_subpath_of_or_equal(&MountPointBaseUrl::from_str("host/a/b").unwrap()));

        assert!(MountPointBaseUrl::from_str("host/a/b/")
            .unwrap()
            .is_subpath_of_or_equal(&MountPointBaseUrl::from_str("host/a/b").unwrap()));
        assert!(MountPointBaseUrl::from_str("host/a/b")
            .unwrap()
            .is_subpath_of_or_equal(&MountPointBaseUrl::from_str("host/a/b/").unwrap()));
        assert!(MountPointBaseUrl::from_str("host/a/b")
            .unwrap()
            .is_subpath_of_or_equal(&MountPointBaseUrl::from_str("host/a/b/c").unwrap()));

        assert!(!MountPointBaseUrl::from_str("host/a/b")
            .unwrap()
            .is_subpath_of_or_equal(&MountPointBaseUrl::from_str("host/a/bb/c").unwrap()));

        assert!(!MountPointBaseUrl::from_str("host/a/b")
            .unwrap()
            .is_subpath_of_or_equal(&MountPointBaseUrl::from_str("host/a").unwrap()));
    }
}
