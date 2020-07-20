use std::thread;

use tokio::runtime::{Handle, Runtime};
use trust_dns_resolver::TokioAsyncResolver;

use exogress_client_core::Client;
use exogress_entities::{tracing, ClientId, InstanceId};
use tracing::Level;

pub fn spawn(client_id: String, client_secret: String, account: String, project: String) {
    thread::spawn(|| {
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(Level::INFO)
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("no global subscriber has been set");

        let mut rt = Runtime::new().unwrap();

        rt.block_on(async move {
            let resolver = TokioAsyncResolver::from_system_conf(Handle::current())
                .await
                .unwrap();

            Client::builder()
                .instance_id(InstanceId::new())
                .client_id(client_id.parse::<ClientId>().unwrap())
                .client_secret(client_secret)
                .account(account)
                .project(project)
                .build()
                .unwrap()
                .spawn(resolver)
                .await
        })
        .unwrap();
    });
}
