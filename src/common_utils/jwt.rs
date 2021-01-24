use crate::entities::AccessKeyId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    iss: String,
}

#[derive(Debug, thiserror::Error)]
pub enum JwtError {
    #[error("base58 decoding error: {_0}")]
    Base58(#[from] bs58::decode::Error),

    #[error("JWT error: {_0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
}

fn secret_access_key_private_key(
    secret_access_key: &str,
) -> Result<jsonwebtoken::EncodingKey, JwtError> {
    let der = bs58::decode(secret_access_key)
        .with_alphabet(bs58::Alphabet::FLICKR)
        .into_vec()?;
    Ok(jsonwebtoken::EncodingKey::from_ec_der(der.as_ref()))
}

pub fn jwt_token(access_key_id: &AccessKeyId, secret_access_key: &str) -> Result<String, JwtError> {
    let claims = Claims {
        iss: access_key_id.to_string(),
    };

    let jwt_encoding_key = secret_access_key_private_key(secret_access_key)?;

    Ok(jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::ES256),
        &claims,
        &jwt_encoding_key,
    )?)
}
