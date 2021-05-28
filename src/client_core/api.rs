use crate::{
    access_tokens::generate_jwt_token,
    api::{InvalidationRequest, SingleInvalidationRequest},
    entities::{AccessKeyId, AccountName, ProjectName},
};
use anyhow::anyhow;
use http::header::AUTHORIZATION;
use reqwest::Error;
use smol_str::SmolStr;
use tokio::time::Duration;
use url::Url;

#[derive(Debug)]
pub struct ApiClient {
    project: ProjectName,
    account: AccountName,
    access_key_id: AccessKeyId,
    secret_access_key: SmolStr,
    cloud_endpoint: url::Url,
    client: reqwest::Client,
}

impl ApiClient {
    pub fn new(
        project: &ProjectName,
        account: &AccountName,
        access_key_id: &AccessKeyId,
        secret_access_key: &str,
        cloud_endpoint: &Url,
    ) -> anyhow::Result<Self> {
        let client = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .use_rustls_tls()
            .trust_dns(true)
            .build()?;

        Ok(ApiClient {
            project: project.clone(),
            account: account.clone(),
            access_key_id: access_key_id.clone(),
            secret_access_key: secret_access_key.into(),
            cloud_endpoint: cloud_endpoint.clone(),
            client,
        })
    }

    pub async fn invalidate(&self, reqs: &[SingleInvalidationRequest]) -> anyhow::Result<()> {
        let mut url = self.cloud_endpoint.clone();

        url.path_segments_mut()
            .unwrap()
            .push("api")
            .push("v1")
            .push("accounts")
            .push(self.account.as_str())
            .push("projects")
            .push(self.project.as_str())
            .push("invalidate");

        let body = InvalidationRequest {
            groups: reqs.to_vec(),
        };

        let request = self
            .client
            .post(url)
            .json(&body)
            .header(
                AUTHORIZATION,
                format!(
                    "Bearer {}",
                    generate_jwt_token(&self.secret_access_key, &self.access_key_id)?
                ),
            )
            .build()?;

        let res = self.client.execute(request).await?;

        let status = res.status();

        if !status.is_success() {
            let body_res = res.json::<serde_json::Value>().await;

            let mut err = anyhow!("Error while trying to invalidate. Status code: {}", status);

            err = match body_res {
                Ok(body) => err.context(body.to_string()),
                Err(e) => err.context(e.to_string()),
            };

            return Err(err);
        }

        Ok(())
    }
}
