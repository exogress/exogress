use bytes::Buf;
use futures::{Stream, StreamExt};
use smol_str::SmolStr;
use std::{
    convert::{TryFrom, TryInto},
    sync::Arc,
};
use tame_gcs::objects::InsertObjectOptional;
use tame_oauth::{gcp::ServiceAccountAccess, Token};
#[derive(Clone)]

pub struct GcsBucketClient {
    gcs_bucket: SmolStr,
    gcs_bucket_location: Location,
    gcs_credentials: Arc<ServiceAccountAccess>,
    gcs_token_storage: Arc<tokio::sync::Mutex<Option<tame_oauth::Token>>>,
    client: reqwest::Client,
}

#[derive(parse_display::Display, parse_display::FromStr, Debug, Clone)]
#[display(style = "snake_case")]
pub enum Location {
    Eu,
    Us,
    Asia,
}

pub const ALL_LOCATIONS: [Location; 3] = [Location::Us, Location::Eu, Location::Asia];

#[derive(Clone)]
pub struct GcsBucketInfo {
    pub name: SmolStr,
    pub location: Location,
}

impl GcsBucketClient {
    pub fn new(
        gcs_bucket: String,
        gcs_bucket_location: Location,
        gcs_credentials_file: String,
    ) -> anyhow::Result<Self> {
        let gcs_service_account = tame_oauth::gcp::ServiceAccountInfo::deserialize(
            std::fs::read_to_string(gcs_credentials_file)?.as_str(),
        )?;
        let gcs_service_account_access =
            tame_oauth::gcp::ServiceAccountAccess::new(gcs_service_account)?;

        Ok(GcsBucketClient {
            gcs_bucket: gcs_bucket.into(),
            gcs_bucket_location,
            gcs_credentials: Arc::new(gcs_service_account_access),
            gcs_token_storage: Arc::new(Default::default()),
            client: reqwest::Client::builder().trust_dns(true).build()?,
        })
    }

    pub fn bucket_info(&self) -> GcsBucketInfo {
        GcsBucketInfo {
            name: self.gcs_bucket.clone(),
            location: Location::Us,
        }
    }

    async fn retrieve_token(&self) -> anyhow::Result<Token> {
        let current_token = self.gcs_token_storage.lock().await.clone();

        let token = match current_token {
            Some(token) if !token.has_expired() => token,
            _ => {
                let token_or_req = self
                    .gcs_credentials
                    .get_token(&[tame_gcs::Scopes::ReadWrite])?;

                let new_token = match token_or_req {
                    tame_oauth::gcp::TokenOrRequest::Token(token) => token,
                    tame_oauth::gcp::TokenOrRequest::Request {
                        request,
                        scope_hash,
                        ..
                    } => {
                        let (parts, body) = request.into_parts();
                        let uri = parts.uri.to_string();

                        // This will always be a POST, but for completeness sake...
                        let builder = match parts.method {
                            http::Method::GET => self.client.get(&uri),
                            http::Method::POST => self.client.post(&uri),
                            http::Method::DELETE => self.client.delete(&uri),
                            http::Method::PUT => self.client.put(&uri),
                            method => unimplemented!("{} not implemented", method),
                        };

                        // Build the full request from the headers and body that were
                        // passed to you, without modifying them.
                        let request = builder.headers(parts.headers).body(body).build().unwrap();

                        let response = self.client.execute(request).await?;

                        let mut builder = http::Response::builder()
                            .status(response.status())
                            .version(response.version());

                        let headers = builder.headers_mut().unwrap();

                        // Unfortunately http doesn't expose a way to just use
                        // an existing HeaderMap, so we have to copy them :(
                        headers.extend(
                            response
                                .headers()
                                .into_iter()
                                .map(|(k, v)| (k.clone(), v.clone())),
                        );

                        let buffer = response.bytes().await.unwrap();
                        let response = builder.body(buffer).unwrap();

                        self.gcs_credentials
                            .parse_token_response(scope_hash, response)?
                    }
                };

                *self.gcs_token_storage.lock().await = Some(new_token.clone());

                new_token
            }
        };

        Ok(token)
    }

    pub async fn upload(
        &self,
        path: String,
        content_length: u64,
        body_stream: impl Stream<Item = Result<impl Buf, warp::Error>> + Send + Sync + 'static,
    ) -> anyhow::Result<()> {
        let token = self.retrieve_token().await?;

        let gcs_bucket = tame_gcs::BucketName::try_from(self.gcs_bucket.to_string())?;
        let object_name = tame_gcs::ObjectName::try_from(path.as_str())?;

        let opts = InsertObjectOptional::default();

        let upload_req = tame_gcs::objects::Object::insert_simple(
            &(&gcs_bucket, &object_name),
            body_stream,
            content_length,
            Some(opts),
        )?;

        let authorization_token: http::HeaderValue = token.try_into()?;

        let upload_req = self
            .client
            .request(upload_req.method().clone(), upload_req.uri().to_string())
            .headers(upload_req.headers().clone())
            .header(http::header::AUTHORIZATION, authorization_token)
            .body(to_reqwest_body(upload_req.into_body()))
            .build()?;

        self.client.execute(upload_req).await?.error_for_status()?;

        Ok(())
    }

    pub async fn download(
        &self,
        path: String,
    ) -> anyhow::Result<impl Stream<Item = Result<impl Buf, reqwest::Error>> + Send + Sync + 'static>
    {
        let token = self.retrieve_token().await?;

        let gcs_bucket = tame_gcs::BucketName::try_from(self.gcs_bucket.to_string())?;
        let object_name = tame_gcs::ObjectName::try_from(path.as_str())?;
        let download_req = tame_gcs::objects::Object::download(&(&gcs_bucket, &object_name), None)?;

        let authorization_token: http::HeaderValue = token.try_into()?;

        let download_req = self
            .client
            .request(
                download_req.method().clone(),
                download_req.uri().to_string(),
            )
            .headers(download_req.headers().clone())
            .header(http::header::AUTHORIZATION, authorization_token)
            .build()?;

        Ok(self
            .client
            .execute(download_req)
            .await?
            .error_for_status()?
            .bytes_stream())
    }

    pub async fn delete(&self, path: String) -> anyhow::Result<()> {
        let token = self.retrieve_token().await?;

        let gcs_bucket = tame_gcs::BucketName::try_from(self.gcs_bucket.to_string())?;
        let object_name = tame_gcs::ObjectName::try_from(path.as_str())?;
        let delete_req = tame_gcs::objects::Object::delete(&(&gcs_bucket, &object_name), None)?;

        let authorization_token: http::HeaderValue = token.try_into()?;

        let download_req = self
            .client
            .request(delete_req.method().clone(), delete_req.uri().to_string())
            .headers(delete_req.headers().clone())
            .header(http::header::AUTHORIZATION, authorization_token)
            .build()?;

        let _res = self
            .client
            .execute(download_req)
            .await?
            .error_for_status()?;

        Ok(())
    }
}

fn to_reqwest_body(
    body_stream: impl Stream<Item = Result<impl Buf, warp::Error>> + Send + Sync + 'static,
) -> reqwest::Body {
    reqwest::Body::wrap_stream(
        body_stream.map(|buf| buf.map(|mut b| b.copy_to_bytes(b.remaining()))),
    )
}
