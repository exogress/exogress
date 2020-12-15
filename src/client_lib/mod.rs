use std::thread;

use tokio::runtime::{Handle, Runtime};
use trust_dns_resolver::TokioAsyncResolver;

use crate::client_core::Client;
use crate::entities::AccessKeyId;
use tracing::Level;

pub fn spawn(
    access_key_id: String,
    secret_access_key: String,
    account: String,
    project: String,
) -> Result<(), anyhow::Error> {
    let res = thread::spawn(|| {
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(Level::INFO)
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("no global subscriber has been set");

        let mut rt = Runtime::new().unwrap();

        rt.block_on(async move {
            let resolver = TokioAsyncResolver::from_system_conf(Handle::current()).await?;

            Ok::<_, anyhow::Error>(
                Client::builder()
                    .access_key_id(
                        access_key_id
                            .parse::<AccessKeyId>()
                            .map_err(|e| anyhow::Error::msg(e.to_string()))?,
                    )
                    .secret_access_key(secret_access_key)
                    .account(account)
                    .project(project)
                    .build()
                    .map_err(anyhow::Error::msg)?
                    .spawn(resolver)
                    .await?,
            )
        })
    })
    .join();

    match res {
        Err(e) => {
            if let Some(e) = e.downcast_ref::<&'static str>() {
                Err(anyhow::Error::msg(e.to_string()))
            } else {
                Err(anyhow::Error::msg("panic"))
            }
        }
        Ok(r) => Ok(r?),
    }
}
