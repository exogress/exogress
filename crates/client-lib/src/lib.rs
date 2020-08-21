use std::thread;

use tokio::runtime::{Handle, Runtime};
use trust_dns_resolver::TokioAsyncResolver;

use exogress_client_core::Client;
use exogress_entities::{tracing, ClientId, InstanceId};
use tracing::{Level, error, info};

pub fn spawn(client_id: String, client_secret: String, account: String, project: String) -> Result<(), anyhow::Error> {
    let res = thread::spawn(|| {
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(Level::INFO)
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("no global subscriber has been set");

        let mut rt = Runtime::new().unwrap();

        rt.block_on(async move {
            let resolver = TokioAsyncResolver::from_system_conf(Handle::current())
                .await?;

            let res = Client::builder()
                .instance_id(InstanceId::new())
                .client_id(client_id
                    .parse::<ClientId>()
                    .map_err(|e| anyhow::Error::msg(e.to_string()))?)
                .client_secret(client_secret)
                .account(account)
                .project(project)
                .build()
                .map_err(|err_str| anyhow::Error::msg(err_str))?
                .spawn(resolver)
                .await?;

            Ok::<_, anyhow::Error>(res)
        })
    }).join();

    match res {
        Err(e) => {
            if let Some(e) = e.downcast_ref::<&'static str>() {
                return Err(anyhow::Error::msg(e.to_string()));
            } else {
                return Err(anyhow::Error::msg("panic"));
            }
        }
        Ok(r) => Ok(r?)
    }
}
