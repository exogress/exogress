use anyhow::anyhow;
use futures::channel::{mpsc, oneshot};
use futures::{pin_mut, select_biased, FutureExt, SinkExt, StreamExt};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use shadow_clone::shadow_clone;
use std::path::PathBuf;
use std::time::Duration;
use std::{io, mem};
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::sync::watch;
use tracing::{debug, error, info, warn};
use trust_dns_resolver::TokioAsyncResolver;
use url::Url;

use crate::config_core::{ClientConfig, Config, UpstreamSocketAddr};
use crate::entities::{
    AccessKeyId, AccountName, LabelName, LabelValue, ProjectName, SmolStr, Upstream,
};

use crate::client_core::{signal_client, tunnel};

use crate::signaling::{TunnelRequest, WsInstanceToCloudMessage};
use parking_lot::{Mutex, RwLock};
use std::sync::Arc;
use tracing_futures::Instrument;

use crate::client_core::health::UpstreamsHealth;
use crate::client_core::internal_server::internal_server;
use crate::common_utils::backoff::Backoff;
use crate::common_utils::jwt::jwt_token;
use crate::config_core::DEFAULT_CONFIG_FILE;
use dashmap::DashMap;
use derive_builder::Builder;
use hashbrown::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::time::delay_for;

pub const DEFAULT_CLOUD_ENDPOINT: &str = "https://app.exogress.com/";

#[derive(Default, Builder, Debug)]
pub struct Client {
    #[builder(setter(into), default = "DEFAULT_CONFIG_FILE.into()")]
    pub config_path: SmolStr,

    #[builder(default = "true")]
    pub watch_config: bool,

    #[builder(setter(into), default = "443")]
    pub gw_tunnels_port: u16,

    #[builder(setter(into))]
    pub access_key_id: AccessKeyId,

    #[builder(setter(into))]
    pub secret_access_key: SmolStr,

    #[builder(setter(into))]
    pub project: SmolStr,

    #[builder(setter(into))]
    pub account: SmolStr,

    #[builder(setter(into), default = "DEFAULT_CLOUD_ENDPOINT.into()")]
    pub cloud_endpoint: SmolStr,

    #[builder(setter(into), default = "Default::default()")]
    pub labels: HashMap<LabelName, LabelValue>,

    #[builder(setter(into), default = "Default::default()")]
    pub maybe_identity: Option<Vec<u8>>,

    #[builder(setter(into), default = "Default::default()")]
    pub refined_upstream_addrs: HashMap<Upstream, UpstreamSocketAddr>,
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

impl Client {
    pub async fn spawn(
        self,
        reload_config_tx: mpsc::UnboundedSender<()>,
        reload_config_rx: mpsc::UnboundedReceiver<()>,
        resolver: TokioAsyncResolver,
    ) -> Result<(), anyhow::Error> {
        let project_name: ProjectName = self.project.parse()?;
        let account_name: AccountName = self.account.parse()?;
        let maybe_identity = self.maybe_identity.clone();

        let (health_update_tx, mut health_update_rx) = mpsc::channel(16);

        let instance_id_storage = Arc::new(Mutex::new(None));

        let config_path = PathBuf::from(shellexpand::full(self.config_path.as_str())?.into_owned());
        info!("Use config at {}", config_path.as_path().display());
        let mut config_dir = config_path
            .parent()
            .expect("Could not config directory path")
            .to_owned();
        if config_dir.as_os_str() == "" {
            config_dir.push(".");
        }

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
        let (recv_tx, recv_rx) = mpsc::channel(1);

        let config_tx;
        let config_rx;

        let refined_upstream_addrs = self.refined_upstream_addrs;

        let mut config = Vec::new();
        File::open(&config_path)
            .await
            .unwrap()
            .read_to_end(&mut config)
            .await
            .unwrap();
        let client_config =
            ClientConfig::parse_with_redefined_upstreams(&config, &refined_upstream_addrs).unwrap();
        client_config.validate()?;
        let upstream_health_checkers = UpstreamsHealth::new(
            &client_config,
            health_update_tx,
            tokio::runtime::Handle::current(),
        )?;

        tokio::spawn({
            shadow_clone!(upstream_health_checkers);
            shadow_clone!(mut recv_tx);

            async move {
                while let Some(status) = health_update_rx.next().await {
                    let health = upstream_health_checkers.dump_health().await;
                    info!("updated health status = {:?}", status);
                    recv_tx
                        .send(
                            serde_json::to_string(&WsInstanceToCloudMessage::HealthState(health))
                                .unwrap(),
                        )
                        .await?;
                }

                Ok::<_, anyhow::Error>(())
            }
        });

        let current_config = Arc::new(RwLock::new(client_config.clone()));

        let (cfg_tx, cfg_rx) = watch::channel(client_config.clone());

        config_tx = cfg_tx;
        config_rx = cfg_rx;

        let mut watcher: RecommendedWatcher;

        if self.watch_config {
            shadow_clone!(reload_config_tx);
            shadow_clone!(config_path);
            shadow_clone!(config_dir);

            info!("Watching for config changes");

            let (re_add_tx, mut re_add_rx) = mpsc::unbounded();

            watcher = Watcher::new_immediate({
                shadow_clone!(config_path);
                shadow_clone!(config_dir);

                move |event: Result<Event, notify::Error>| {
                    let event = event.expect("Error watching for file change");

                    debug!("received fs event: {:?}", event);

                    if !event.paths.iter().any(|path| path == &config_path) {
                        return;
                    }

                    re_add_tx.unbounded_send(()).unwrap();
                }
            })
            .unwrap();

            let _ = watcher.watch(config_dir.clone(), RecursiveMode::NonRecursive);
            let _ = watcher.watch(config_path.clone(), RecursiveMode::NonRecursive);

            tokio::spawn({
                shadow_clone!(config_path);

                async move {
                    loop {
                        let _ = re_add_rx.next().await;
                        let _ = watcher.unwatch(config_dir.clone());
                        let _ = watcher.unwatch(config_path.clone());
                        let _ = watcher.watch(config_dir.clone(), RecursiveMode::NonRecursive);
                        let _ = watcher.watch(config_path.clone(), RecursiveMode::NonRecursive);
                        reload_config_tx.unbounded_send(()).unwrap();
                    }
                }
            });
        }

        tokio::spawn({
            shadow_clone!(config_path);
            shadow_clone!(current_config);
            shadow_clone!(refined_upstream_addrs);
            shadow_clone!(upstream_health_checkers);

            async move {
                let mut last_checksum = None;

                let mut joined_events = reload_config_rx.ready_chunks(30);
                while let Some(event) = joined_events.next().await {
                    if event.is_empty() {
                        continue;
                    }
                    let mut config = Vec::new();
                    let read_file = async {
                        tokio::fs::File::open(&config_path)
                            .await?
                            .read_to_end(&mut config)
                            .await
                    };

                    match read_file.await {
                        Ok(_) => {
                            match ClientConfig::parse_with_redefined_upstreams(
                                config,
                                &refined_upstream_addrs,
                            ) {
                                Ok(client_config) => {
                                    if let Err(err) = client_config.validate() {
                                        error!("Error in config: {}. Changes are not applied", err);
                                    } else {
                                        let current_checksum = client_config.checksum();
                                        if last_checksum != Some(current_checksum) {
                                            upstream_health_checkers
                                                .sync_probes(&client_config)
                                                .await;
                                            *current_config.write() = client_config.clone();
                                            config_tx.broadcast(client_config).unwrap();
                                            info!("New config successfully loaded");
                                            last_checksum = Some(current_checksum);
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("error parsing config file: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error reading config file: `{}`", e);
                        }
                    }
                }
            }
        });

        let tunnels = Arc::new(DashMap::new());

        let authorization = jwt_token(&self.access_key_id, self.secret_access_key.as_str())?.into();

        let connector_result = tokio::spawn({
            shadow_clone!(tunnels);
            shadow_clone!(resolver);
            shadow_clone!(current_config);
            shadow_clone!(instance_id_storage);
            shadow_clone!(upstream_health_checkers);

            signal_client::spawn(
                instance_id_storage,
                current_config,
                config_rx,
                tunnels,
                url,
                send_tx,
                recv_rx,
                upstream_health_checkers,
                authorization,
                Duration::from_millis(50),
                Duration::from_secs(30),
                maybe_identity,
                resolver,
            )
            .instrument(tracing::info_span!("cloud connector"))
        })
        .fuse();

        let (internal_server_connector, new_conn_rx) = mpsc::channel(1);

        tokio::spawn(internal_server(new_conn_rx, current_config.clone()));

        let tunnel_requests_processor = tokio::spawn({
            let access_key_id = self.access_key_id;
            let secret_access_key = self.secret_access_key;
            let gw_tunnels_port = self.gw_tunnels_port;

            async move {
                while let Some(TunnelRequest {
                                   hostname,
                                   max_tunnels_count,
                               }) = send_rx.next().await
                {
                    {
                        if !tunnels.contains_key(&hostname) {
                            for tunnel_index in 0..max_tunnels_count {
                                let (stop_tunnel_tx, stop_tunnel_rx) = oneshot::channel();

                                tunnels
                                    .entry(hostname.clone())
                                    .or_default()
                                    .insert(tunnel_index, stop_tunnel_tx);

                                tokio::spawn({
                                    shadow_clone!(account_name);
                                    shadow_clone!(project_name);
                                    shadow_clone!(secret_access_key);
                                    shadow_clone!(gw_tunnels_port);
                                    shadow_clone!(access_key_id);
                                    shadow_clone!(instance_id_storage);
                                    shadow_clone!(hostname);
                                    shadow_clone!(current_config);
                                    shadow_clone!(tunnels);
                                    shadow_clone!(resolver);
                                    shadow_clone!( mut internal_server_connector);

                                    async move {
                                        let connector = async {
                                            let mut backoff = Backoff::new(
                                                Duration::from_millis(100),
                                                Duration::from_secs(20),
                                            );

                                            let retry = Arc::new(AtomicUsize::new(0));

                                            loop {
                                                info!(
                                                    "try to establish tunnel {} attempt {}",
                                                    tunnel_index,
                                                    retry.load(Ordering::SeqCst)
                                                );
                                                let backoff_handle = backoff.next().await.unwrap();

                                                let existence = Arc::new(Mutex::new(()));
                                                let weak = Arc::downgrade(&existence);
                                                tokio::spawn({
                                                    let retry = retry.clone();

                                                    async move {
                                                        delay_for(Duration::from_secs(10)).await;
                                                        if weak.upgrade().is_some() {
                                                            debug!("Tunnel is ok. Reset backoff");
                                                            backoff_handle.reset();
                                                            retry.store(0, Ordering::SeqCst);
                                                        }
                                                    }
                                                });
                                                {
                                                    let maybe_instance_id =
                                                        *instance_id_storage.lock();
                                                    if let Some(instance_id) = maybe_instance_id {
                                                        let tunnel_spawn_result = tunnel::spawn(
                                                            current_config.clone(),
                                                            account_name.clone(),
                                                            project_name.clone(),
                                                            instance_id,
                                                            access_key_id,
                                                            secret_access_key.clone(),
                                                            hostname.clone(),
                                                            gw_tunnels_port,
                                                            internal_server_connector.clone(),
                                                            resolver.clone(),
                                                        )
                                                            .await;
                                                        match tunnel_spawn_result {
                                                            Ok(true) => {
                                                                // should retry
                                                            }
                                                            Ok(false) => {
                                                                // stop retrying
                                                                break;
                                                            }
                                                            Err(e) => {
                                                                error!("error in tunnel {}. will re-connect", e);
                                                            }
                                                        }
                                                    }
                                                    mem::drop(existence);
                                                }

                                                retry.fetch_add(1, Ordering::SeqCst);
                                            }
                                        };

                                        tokio::select! {
                                        _ = connector => {},
                                        _ = stop_tunnel_rx => {},
                                        }

                                        if let dashmap::mapref::entry::Entry::Occupied(mut tunnel_entry) = tunnels.entry(hostname.clone())
                                        {
                                            info!("tunnel index {} closed", tunnel_index);
                                            tunnel_entry.get_mut().remove(&tunnel_index);

                                            if tunnel_entry.get().is_empty() {
                                                info!(
                                                    "tunnels finally empty. remove whole entry"
                                                );
                                                tunnel_entry.remove_entry();
                                            }
                                        }
                                    }
                                });
                            }
                        } else {
                            warn!("skip request to connect to {}: already connected", hostname);
                        }
                    }
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
            _ = tunnel_requests_processor => {
            }
        }

        Err(anyhow!("unexpected termination"))
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use futures::FutureExt;
//     use std::str::FromStr;
//     use stop_handle::stop_handle;
//     use tokio::runtime::Handle;
//     use tokio::time::delay_for;
//
//     #[tokio::test]
//     async fn test_minimal() {
//         let resolver = TokioAsyncResolver::from_system_conf(Handle::current())
//             .await
//             .unwrap();
//
//         let (stop_tx, stop_wait) = stop_handle();
//
//         let (reload_config_tx, reload_config_rx) = mpsc::unbounded();
//
//         File::crea
//
//         let bg = tokio::spawn(async move {
//             let f = Client::builder()
//                 .access_key_id(AccessKeyId::new())
//                 .secret_access_key("secret_access_key".to_string())
//                 .account("account".to_string())
//                 .project("project".to_string())
//                 .labels(
//                     vec![(
//                         LabelName::from_str("test").unwrap(),
//                         LabelValue::from_str("true").unwrap(),
//                     )]
//                     .into_iter()
//                     .collect::<HashMap<_, _>>(),
//                 )
//                 .build()
//                 .unwrap()
//                 .spawn(reload_config_tx, reload_config_rx, resolver.clone())
//                 .fuse();
//
//             tokio::select! {
//                 _ = f => {},
//                 _ = stop_wait => {},
//             }
//         });
//
//         delay_for(Duration::from_secs(1)).await;
//
//         stop_tx.stop(());
//
//         bg.await.unwrap();
//     }
// }
