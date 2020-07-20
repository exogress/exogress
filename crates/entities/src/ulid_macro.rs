use crate::ULID_LEN;

#[derive(thiserror::Error, Debug)]
pub enum UlidIdentifierParseError {
    #[error("Bad length for ULID. Expected {} bytes", ULID_LEN)]
    BadLen,

    #[error("string identifier error: `{0}`")]
    Ulid(ulid::DecodeError),
}

#[macro_export]
macro_rules! ulid_type {
    ($x:ident) => {
        #[derive(
            Debug,
            Clone,
            Copy,
            $crate::serde::Serialize,
            $crate::serde::Deserialize,
            Hash,
            Eq,
            PartialEq,
            Ord,
            PartialOrd,
        )]
        #[serde(transparent)]
        pub struct $x {
            inner: $crate::Ulid,
        }

        impl $x {
            pub fn new() -> Self {
                Default::default()
            }
        }

        impl Default for $x {
            fn default() -> Self {
                use $crate::rand::Rng;

                let mut rng = $crate::rand::thread_rng();
                $x {
                    inner: $crate::Ulid(rng.gen()),
                }
            }
        }

        impl std::fmt::Display for $x {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.inner)
            }
        }

        impl From<$crate::Ulid> for $x {
            fn from(n: $crate::Ulid) -> Self {
                $x { inner: n }
            }
        }

        impl From<u128> for $x {
            fn from(n: u128) -> Self {
                $x {
                    inner: $crate::Ulid(n),
                }
            }
        }

        impl From<$x> for u128 {
            fn from(v: $x) -> Self {
                v.inner.into()
            }
        }

        impl std::str::FromStr for $x {
            type Err = $crate::ulid::DecodeError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok($x { inner: s.parse()? })
            }
        }

        impl std::convert::TryFrom<$crate::bytes::Bytes> for $x {
            type Error = $crate::UlidIdentifierParseError;

            fn try_from(mut value: $crate::bytes::Bytes) -> Result<Self, Self::Error> {
                use $crate::bytes::buf::Buf;

                if value.len() != ulid::ULID_LEN {
                    return Err($crate::UlidIdentifierParseError::BadLen);
                }

                Ok(value.get_u128().into())
            }
        }

        impl From<$x> for $crate::bytes::Bytes {
            fn from(v: $x) -> Self {
                use $crate::bytes::buf::BufMut;

                let mut b = $crate::bytes::BytesMut::new();
                b.put_u128(v.inner.into());

                b.freeze()
            }
        }
    };
}

#[cfg(test)]
mod test {
    use bytes::Bytes;
    use std::convert::TryFrom;
    use std::str::FromStr;

    ulid_type!(TestUlid);

    #[test]
    pub fn parse_string() {
        TestUlid::from_str("asd").err().unwrap();
        TestUlid::from_str("01EDTVB8EBWRZZMSJYPNNQD1DC").unwrap();
    }

    #[test]
    pub fn parse_bytes() {
        TestUlid::try_from(Bytes::from_static("asd".as_bytes()))
            .err()
            .unwrap();
        TestUlid::try_from(Bytes::from_static("01EDTYSASBD3Q06MB47P1ZCC37".as_bytes())).unwrap();
    }

    #[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
    struct Container {
        id: TestUlid,
    }

    #[test]
    pub fn parse_serde() {
        serde_json::from_str::<Container>(r#"{"id": "asd"}"#)
            .err()
            .unwrap();
        serde_json::from_str::<Container>(r#"{"id": "01EDTVHB17YZTCD4YJKFXDQV7E"}"#).unwrap();
    }
}
