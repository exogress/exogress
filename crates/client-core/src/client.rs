use anyhow::anyhow;
use anyhow::Context;
use futures::channel::mpsc;
use futures::{pin_mut, select_biased, FutureExt, StreamExt};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use rand::rngs::SmallRng;
use rand::SeedableRng;
use shadow_clone::shadow_clone;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;
use std::{fs, io};
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::sync::watch;
use trust_dns_resolver::TokioAsyncResolver;
use url::Url;

use exogress_config_core::{ClientConfig, Config};
use exogress_entities::{ClientId, InstanceId, LabelName, LabelValue};

use crate::{signal_client, tunnel};

use exogress_signaling::TunnelRequest;
use notify::event::{CreateKind, ModifyKind, RemoveKind};
use parking_lot::RwLock;
use std::sync::Arc;
use tracing_futures::Instrument;

use crate::internal_server::internal_server;
use exogress_config_core::DEFAULT_CONFIG_FILE;
use hashbrown::HashMap;

pub const DEFAULT_CLOUD_ENDPOINT: &str = "https://app.sexogress.com/";

#[derive(Default, Builder, Debug)]
pub struct Client {
    #[builder(setter(into), default = "DEFAULT_CONFIG_FILE.to_string()")]
    pub config_path: String,

    #[builder(default = "true")]
    pub watch_config: bool,

    #[builder(setter(into))]
    pub client_id: ClientId,

    #[builder(setter(into))]
    pub client_secret: String,

    #[builder(setter(into))]
    pub project: String,

    #[builder(setter(into))]
    pub account: String,

    #[builder(setter(into), default = "DEFAULT_CLOUD_ENDPOINT.to_string()")]
    pub cloud_endpoint: String,

    #[builder(setter(into))]
    pub instance_id: InstanceId,

    #[builder(setter(into), default = "Default::default()")]
    pub labels: HashMap<LabelName, LabelValue>,
}

impl Client {
    pub fn builder() -> ClientBuilder {
        Default::default()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: `{0}`")]
    Io(#[from] io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ClientSecretError {
    #[error("PEM parse error")]
    Jwt(#[from] jsonwebtoken::errors::Error),

    #[error("base58 decoding error")]
    Base58(#[from] bs58::decode::Error),
}

fn client_secret_private_key(
    client_secret: &str,
) -> Result<jsonwebtoken::EncodingKey, ClientSecretError> {
    let pem = pem::Pem {
        tag: "PRIVATE KEY".to_string(),
        contents: bs58::decode(client_secret)
            .with_alphabet(bs58::alphabet::FLICKR)
            .into_vec()?,
    };
    let pem_key = pem::encode(&pem);
    Ok(jsonwebtoken::EncodingKey::from_ec_pem(pem_key.as_ref())?)
}

impl Client {
    pub async fn spawn(self, resolver: TokioAsyncResolver) -> Result<(), anyhow::Error> {
        let jwt_encoding_key = client_secret_private_key(self.client_secret.as_str())
            .context("client_secret error")?;

        let instance_id = self.instance_id;

        let config_path = fs::canonicalize(PathBuf::from(
            shellexpand::full(&self.config_path)?.into_owned(),
        ))?;
        info!("Use config at {}", config_path.as_path().display());

        let mut url = Url::parse(self.cloud_endpoint.as_str()).unwrap();
        if url.scheme() == "https" {
            url.set_scheme("wss").unwrap();
        } else if url.scheme() == "http" {
            url.set_scheme("ws").unwrap();
        }

        {
            let mut path_segments = url.path_segments_mut().unwrap();
            path_segments.push("api");
            path_segments.push("v1");
            path_segments.push("instances");
            path_segments.push(instance_id.to_string().as_str());
            path_segments.push("channel");
        }

        url.set_query(Some(
            format!(
                "project={}&account={}&labels={}",
                self.project,
                self.account,
                urlencoding::encode(serde_json::to_string(&self.labels).unwrap().as_str())
            )
            .as_str(),
        ));

        info!("Will connect signalling channel to {}", url);

        let (send_tx, mut send_rx) = mpsc::channel(1);
        let (_recv_tx, recv_rx) = mpsc::channel(1);

        let config_tx;
        let config_rx;

        let mut config = Vec::new();
        File::open(&config_path)
            .await
            .unwrap()
            .read_to_end(&mut config)
            .await
            .unwrap();
        let client_config = serde_yaml::from_slice::<ClientConfig>(&config).unwrap();
        client_config.validate()?;

        let current_config = Arc::new(RwLock::new(client_config.clone()));

        let (cfg_tx, cfg_rx) = watch::channel(client_config.clone());

        config_tx = cfg_tx;
        config_rx = cfg_rx;

        let mut watcher: RecommendedWatcher;

        if self.watch_config {
            info!("Watching for config changes");

            watcher = Watcher::new_immediate({
                shadow_clone!(config_path);
                shadow_clone!(current_config);

                move |event: Result<Event, notify::Error>| {
                    debug!("received fs event: {:?}", event);

                    let kind = event.expect("Error watching for file change").kind;
                    match kind {
                        EventKind::Modify(ModifyKind::Data(_)) | EventKind::Create(CreateKind::File) => {
                            let mut config = Vec::new();
                            std::fs::File::open(&config_path)
                                .unwrap()
                                .read_to_end(&mut config)
                                .unwrap();
                            match serde_yaml::from_slice::<ClientConfig>(&config) {
                                Ok(client_config) => {
                                    if let Err(err) = client_config.validate() {
                                        error!("Error in config: {}. Changes are not applied", err);
                                    } else {
                                        info!("New config successfully loaded");
                                    }

                                    *current_config.write() = client_config.clone();
                                    config_tx.broadcast(client_config).unwrap();
                                }
                                Err(e) => {
                                    error!("error parsing config file: {}", e);
                                }
                            }
                        }
                        EventKind::Remove(RemoveKind::File) => {
                            warn!("Config file removed. Keep using the latest version until the new one created");
                        }
                        _ => {}
                    }
                }
            }).unwrap();

            watcher
                .watch(config_path, RecursiveMode::NonRecursive)
                .unwrap();
        }

        let connector_result = tokio::spawn({
            shadow_clone!(resolver);
            shadow_clone!(current_config);

            signal_client::spawn(
                current_config,
                config_rx,
                url,
                send_tx,
                recv_rx,
                self.client_id,
                jwt_encoding_key,
                Duration::from_millis(50),
                Duration::from_secs(30),
                resolver,
            )
            .instrument(tracing::info_span!("cloud connector"))
        })
        .fuse();

        let small_rng = SmallRng::from_entropy();

        let (internal_server_connector, new_conn_rx) = mpsc::channel(1);

        tokio::spawn(internal_server(new_conn_rx));

        let tunnel_requests_processor = tokio::spawn(async move {
            while let Some(TunnelRequest {
                hostname,
                max_tunnels_count,
            }) = send_rx.next().await
            {
                for _ in 0..max_tunnels_count {
                    tokio::spawn({
                        shadow_clone!(hostname);
                        shadow_clone!(current_config);
                        shadow_clone!(resolver);
                        shadow_clone!(mut internal_server_connector);
                        shadow_clone!(mut small_rng);

                        async move {
                            tunnel::spawn(
                                current_config.clone(),
                                instance_id,
                                hostname,
                                internal_server_connector,
                                resolver,
                                &mut small_rng,
                            )
                            .await
                            .expect("FIXME: error spawning tunnel");
                        }
                    });
                }
            }
        })
        .fuse();

        pin_mut!(connector_result);
        pin_mut!(tunnel_requests_processor);

        select_biased! {
            res = connector_result => {
                if let Ok(Err(e)) = res {
                    error!("Cloud connector terminated with error: {}", e);
                    return Ok(());
                }
            }
            res = tunnel_requests_processor => {
            }
        }

        Err(anyhow!("unexpected termination"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::FutureExt;
    use std::str::FromStr;
    use stop_handle::stop_handle;
    use tokio::runtime::Handle;
    use tokio::time::delay_for;
    use trust_dns_resolver::config::{ResolverConfig, ResolverOpts};

    #[tokio::test]
    async fn test_minimal() {
        let resolver = TokioAsyncResolver::new(
            ResolverConfig::default(),
            ResolverOpts::default(),
            Handle::current(),
        )
        .await
        .unwrap();

        let (stop_tx, stop_wait) = stop_handle();

        let bg = tokio::spawn(async move {
            let f = Client::builder()
                .instance_id(InstanceId::new())
                .client_id(ClientId::new())
                .client_secret("client_secret".to_string())
                .account("account".to_string())
                .project("project".to_string())
                .labels(
                    vec![(
                        LabelName::from_str("test").unwrap(),
                        LabelValue::from_str("true").unwrap(),
                    )]
                    .into_iter()
                    .collect::<HashMap<_, _>>(),
                )
                .build()
                .unwrap()
                .spawn(resolver.clone())
                .fuse();

            tokio::select! {
                _ = f => {},
                _ = stop_wait => {},
            }
        });

        delay_for(Duration::from_secs(1)).await;

        stop_tx.stop(());

        bg.await.unwrap();
    }
}
