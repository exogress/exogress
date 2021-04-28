use crate::entities::AccessKeyId;
use jsonwebtoken::{Algorithm, TokenData, Validation};
use p256::{
    elliptic_curve::rand_core::OsRng,
    pkcs8::{ToPrivateKey, ToPublicKey},
    PublicKey,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum JwtError {
    #[error("base58 decoding error: {_0}")]
    Base58(#[from] bs58::decode::Error),

    #[error("JWT error: {_0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),

    #[error("PEM error: {_0}")]
    Pem(#[from] pem::PemError),

    #[error("ASN.1 decode error: {_0}")]
    Asn1(#[from] simple_asn1::ASN1DecodeErr),

    #[error("no public key")]
    NoPublicKey,

    #[error("bad legacy public key: {_0}")]
    BadLegacyPublicKey(#[from] p256::elliptic_curve::Error),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub iss: String,
}

pub struct AccessKeyPair {
    secret_access_key: String,
    public_key_pem: String,
}

pub fn generate_access_key_pair() -> AccessKeyPair {
    let secret_key = p256::SecretKey::random(OsRng::default());
    let public_key = secret_key.public_key();

    let secret_access_key = bs58::encode(secret_key.to_bytes())
        .with_alphabet(bs58::Alphabet::FLICKR)
        .into_string();
    let public_key_pem = public_key.to_public_key_pem();

    AccessKeyPair {
        secret_access_key,
        public_key_pem,
    }
}

pub fn generate_jwt_token(
    secret_access_key: &str,
    access_key_id: &AccessKeyId,
) -> anyhow::Result<String> {
    let claims = Claims {
        iss: access_key_id.to_string(),
    };

    let is_legacy = secret_access_key.len() > 100;

    let secret_key_bytes = bs58::decode(secret_access_key)
        .with_alphabet(bs58::Alphabet::FLICKR)
        .into_vec()?;

    let jwt_encoding_key = if is_legacy {
        jsonwebtoken::EncodingKey::from_ec_der(secret_key_bytes.as_ref())
    } else {
        let secret_key = p256::SecretKey::from_bytes(secret_key_bytes)?;
        jsonwebtoken::EncodingKey::from_ec_der(secret_key.to_pkcs8_der().as_ref())
    };

    Ok(jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::ES256),
        &claims,
        &jwt_encoding_key,
    )?)
}

fn extract_public_key(mut asn1: Vec<simple_asn1::ASN1Block>) -> Result<PublicKey, JwtError> {
    if let Some(simple_asn1::ASN1Block::Sequence(_, entries)) = asn1.pop() {
        if let Some(public_key) = entries.get(3) {
            if let simple_asn1::ASN1Block::Explicit(_, _, _, bit_string) = public_key {
                if let simple_asn1::ASN1Block::BitString(_, _, value) = bit_string.as_ref() {
                    return Ok(p256::PublicKey::from_sec1_bytes(value)?);
                }
            }
        }
    }

    Err(JwtError::NoPublicKey)
}

pub fn validate_jwt_token(
    public_key_pem: &str,
    access_key_id: &AccessKeyId,
    jwt_token: &str,
) -> Result<TokenData<Claims>, JwtError> {
    let res = if public_key_pem.contains("BEGIN EC PRIVATE") {
        let der = pem::parse(public_key_pem)?.contents;
        let decoded = simple_asn1::from_der(der.as_ref())?;
        let buf = extract_public_key(decoded)?.to_public_key_pem();
        let jwt_decoding_key = jsonwebtoken::DecodingKey::from_ec_pem(buf.as_ref())?;

        jsonwebtoken::decode::<Claims>(
            &jwt_token,
            &jwt_decoding_key,
            &Validation {
                iss: Some(access_key_id.to_string()),
                algorithms: vec![Algorithm::ES256],
                validate_exp: false,
                ..Default::default()
            },
        )?
    } else {
        let jwt_decoding_key = jsonwebtoken::DecodingKey::from_ec_pem(public_key_pem.as_ref())?;

        jsonwebtoken::decode::<Claims>(
            &jwt_token,
            &jwt_decoding_key,
            &Validation {
                iss: Some(access_key_id.to_string()),
                algorithms: vec![Algorithm::ES256],
                validate_exp: false,
                ..Default::default()
            },
        )?
    };

    assert_eq!(res.claims.iss, access_key_id.to_string());

    Ok(res)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_generation() {
        let AccessKeyPair {
            secret_access_key,
            public_key_pem,
        } = generate_access_key_pair();

        let access_key_id = AccessKeyId::new();
        let jwt = generate_jwt_token(secret_access_key.as_str(), &access_key_id).unwrap();

        let data =
            validate_jwt_token(public_key_pem.as_str(), &access_key_id, jwt.as_str()).unwrap();

        assert_eq!(data.claims.iss, access_key_id.to_string());
    }

    #[test]
    fn test_legacy() {
        let legacy_secret_access_key: &str = "2eoAYVjpjtztomf7mL94fJeZVS5TSkvEDSYB97v1CQxDDyfeg4H4G2mjdbf4kmFmJTo19gSLpfP6SXH2QRjBN8mZU1oA21AYraV76ddRqhVRDMgaQhh9YTsRLNummKN9DV8BJvntmb4e5zjhnWv6FVFRoHGRadExC9Umpb8E9cWDFNXtnUHAP23nV2SjV";
        let legacy_public_key_pem: &str = "-----BEGIN EC PRIVATE KEY-----
MHcCAQEEILwGPRUJpkM4hpE94BW+ftAbmKHQZlMFMkBzSxG4N+mFoAoGCCqGSM49
AwEHoUQDQgAEwohweoKGVumMMLStdxAXKT4AzZ4A4kP/HNn6E4x5FPnl+oEcbg1q
XQ4M346gTs81SuW+zR4G+B5X+282ajrX6Q==
-----END EC PRIVATE KEY-----
";

        let access_key_id = AccessKeyId::new();
        let jwt = generate_jwt_token(legacy_secret_access_key, &access_key_id).unwrap();

        let data = validate_jwt_token(legacy_public_key_pem, &access_key_id, jwt.as_str()).unwrap();

        assert_eq!(data.claims.iss, access_key_id.to_string());
    }
}
