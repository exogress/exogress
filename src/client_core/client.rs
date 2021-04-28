use anyhow::{anyhow, bail};
use futures::{
    channel::{mpsc, oneshot},
    pin_mut, select_biased, FutureExt, SinkExt, StreamExt,
};
use shadow_clone::shadow_clone;
use std::{io, mem, path::PathBuf, time::Duration};
use tokio::{fs::File, io::AsyncReadExt, sync::watch};
use tracing::{debug, error, info, warn};
use trust_dns_resolver::TokioAsyncResolver;
use url::Url;

use crate::{
    config_core::{ClientConfig, Config, UpstreamSocketAddr},
    entities::{
        AccessKeyId, AccountName, LabelName, LabelValue, ProfileName, ProjectName, SmolStr,
        Upstream,
    },
};

use crate::client_core::{signal_client, tunnel};

use crate::signaling::{
    ConfigUpdateResult, TunnelRequest, WsCloudToInstanceMessage, WsInstanceToCloudMessage,
};
use parking_lot::{Mutex, RwLock};
use std::sync::Arc;
use tracing_futures::Instrument;

use crate::{
    access_tokens::generate_jwt_token,
    client_core::{health::UpstreamsHealth, internal_server::internal_server},
    common_utils::backoff::Backoff,
    config_core::DEFAULT_CONFIG_FILE,
};
use dashmap::DashMap;
use derive_builder::Builder;
use hashbrown::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::time::sleep;

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

    #[builder(setter(into))]
    pub profile: Option<ProfileName>,

    #[builder(setter(into), default = "Default::default()")]
    pub labels: HashMap<LabelName, LabelValue>,

    #[builder(setter(into), default = "Default::default()")]
    pub maybe_identity: Option<Vec<u8>>,

    #[builder(setter(into), default = "Default::default()")]
    pub refined_upstream_addrs: HashMap<Upstream, UpstreamSocketAddr>,

    #[builder(setter(into), default = "Default::default()")]
    pub additional_connection_params: HashMap<SmolStr, SmolStr>,
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
        mut reload_config_rx: mpsc::UnboundedReceiver<()>,
        resolver: TokioAsyncResolver,
    ) -> Result<(), anyhow::Error> {
        let project_name: ProjectName = self.project.parse()?;
        let account_name: AccountName = self.account.parse()?;
        let maybe_identity = self.maybe_identity.clone();

        let (health_update_tx, mut health_update_rx) = mpsc::channel(16);

        let instance_id_storage = Arc::new(Mutex::new(None));

        let config_path = PathBuf::from(shellexpand::full(self.config_path.as_str())?.into_owned());
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
            path_segments.push("channel");
        }

        for (k, v) in self.additional_connection_params.iter() {
            url.query_pairs_mut().append_pair(k.as_str(), v.as_str());
        }

        url.query_pairs_mut()
            .append_pair("exogress_version", crate::client_core::VERSION)
            .append_pair("project", self.project.as_ref())
            .append_pair("account", self.account.as_ref())
            .append_pair(
                "labels",
                serde_json::to_string(&self.labels).unwrap().as_str(),
            );

        if let Some(profile) = &self.profile {
            url.query_pairs_mut()
                .append_pair("active_profile", profile.to_string().as_str());
        }

        info!("Cloud endpoint is {}", self.cloud_endpoint);

        let (send_tx, mut send_rx) = mpsc::channel(1);
        let (recv_tx, recv_rx) = mpsc::channel(1);

        let config_tx;
        let config_rx;

        let refined_upstream_addrs = self.refined_upstream_addrs;

        let open = async {
            let mut config = Vec::new();
            File::open(&config_path)
                .await?
                .read_to_end(&mut config)
                .await?;
            let client_config =
                ClientConfig::parse_with_redefined_upstreams(&config, &refined_upstream_addrs)?;

            client_config.validate()?;

            Ok::<ClientConfig, anyhow::Error>(client_config)
        };

        let client_config = match open.await {
            Ok(cfg) => cfg,
            Err(e) => {
                bail!(
                    "error reading config at {}: {}",
                    config_path.to_str().unwrap_or_default(),
                    e
                );
            }
        };

        let profile = self.profile;

        let upstream_health_checkers = UpstreamsHealth::new(
            &client_config,
            health_update_tx,
            &profile,
            tokio::runtime::Handle::current(),
        )?;

        tokio::spawn({
            shadow_clone!(upstream_health_checkers, mut recv_tx);

            async move {
                while let Some(status) = health_update_rx.next().await {
                    let health = upstream_health_checkers.dump_health().await;
                    info!(
                        upstream = %status.upstream,
                        probe = %status.probe,
                        status = %status.status_desc(),
                        "health status updated"
                    );

                    recv_tx
                        .send(
                            serde_json::to_string(&WsInstanceToCloudMessage::HealthState(health))
                                .unwrap(),
                        )
                        .await?;
                }

                Ok::<_, anyhow::Error>(())
            }
            .instrument(tracing::info_span!("health_check"))
        });

        let current_config = Arc::new(RwLock::new(client_config.clone()));

        let (cfg_tx, cfg_rx) = watch::channel(client_config.clone());

        config_tx = cfg_tx;
        config_rx = cfg_rx;

        if self.watch_config {
            shadow_clone!(reload_config_tx, config_path);

            info!("Watching for config changes");

            tokio::spawn({
                shadow_clone!(config_path);

                async move {
                    let mut maybe_curent_buf = None;
                    let mut is_error = false;

                    loop {
                        tokio::time::sleep(Duration::from_secs(1)).await;

                        let file_result = async {
                            let mut v = String::new();
                            File::open(&config_path)
                                .await?
                                .read_to_string(&mut v)
                                .await?;
                            Ok::<_, io::Error>(v)
                        };

                        match file_result.await {
                            Ok(r) => {
                                is_error = false;
                                if let Some(current_buf) = maybe_curent_buf.clone() {
                                    if r != current_buf {
                                        reload_config_tx.unbounded_send(()).unwrap();
                                        maybe_curent_buf = Some(r);
                                    }
                                } else {
                                    maybe_curent_buf = Some(r);
                                }
                            }
                            Err(e) => {
                                if is_error {
                                    warn!("Could not read config file: {}", e);
                                    is_error = true;
                                }
                            }
                        }
                    }
                }
            });
        }

        tokio::spawn({
            shadow_clone!(
                config_path,
                current_config,
                refined_upstream_addrs,
                upstream_health_checkers
            );

            async move {
                let mut last_config = None;

                while let Some(()) = reload_config_rx.next().await {
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
                                    } else if last_config
                                        .as_ref()
                                        .map(|c| serde_yaml::to_string(c).unwrap())
                                        != Some(serde_yaml::to_string(&client_config).unwrap())
                                    {
                                        upstream_health_checkers.sync_probes(&client_config).await;
                                        *current_config.write() = client_config.clone();
                                        config_tx.send(client_config.clone()).unwrap();
                                        last_config = Some(client_config);
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

        let authorization =
            generate_jwt_token(self.secret_access_key.as_str(), &self.access_key_id)?.into();

        error!("authorization = {:?}", authorization);

        let connector_result = tokio::spawn({
            shadow_clone!(
                tunnels,
                resolver,
                current_config,
                instance_id_storage,
                upstream_health_checkers
            );

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
            .instrument(tracing::info_span!("cloud_connector"))
        })
        .fuse();

        let (internal_server_connector, new_conn_rx) = mpsc::channel(1);

        tokio::spawn(internal_server(new_conn_rx, current_config.clone()));

        let tunnel_requests_processor =
            tokio::spawn({
                let access_key_id = self.access_key_id;
                let secret_access_key = self.secret_access_key;
                let gw_tunnels_port = self.gw_tunnels_port;
                let additional_connection_params = self.additional_connection_params;

                async move {
                    while let Some(incoming_msg) = send_rx.next().await {
                        match incoming_msg {
                            WsCloudToInstanceMessage::TunnelRequest(TunnelRequest {
                                                                        hostname,
                                                                        max_tunnels_count,
                                                                    }) => {
                                if !tunnels.contains_key(&hostname) {
                                    for tunnel_index in 0..max_tunnels_count {
                                        let (stop_tunnel_tx, stop_tunnel_rx) = oneshot::channel();

                                        tunnels
                                            .entry(hostname.clone())
                                            .or_default()
                                            .insert(tunnel_index, stop_tunnel_tx);

                                        tokio::spawn({
                                            shadow_clone!(
                                            profile,
                                            account_name,
                                            project_name,
                                            secret_access_key,
                                            gw_tunnels_port,
                                            access_key_id,
                                            instance_id_storage,
                                            hostname,
                                            current_config,
                                            tunnels,
                                            resolver,
                                            mut internal_server_connector,
                                            additional_connection_params
                                        );

                                            {
                                                shadow_clone!(hostname);

                                                async move {
                                                    let connector = async {
                                                        let backoff = Backoff::new(
                                                            Duration::from_millis(100),
                                                            Duration::from_secs(20),
                                                        );

                                                        pin_mut!(backoff);

                                                        let retry = Arc::new(AtomicUsize::new(0));

                                                        loop {
                                                            let backoff_handle = backoff.next().await.unwrap();

                                                            let existence = Arc::new(Mutex::new(()));
                                                            let weak = Arc::downgrade(&existence);
                                                            tokio::spawn({
                                                                let retry = retry.clone();

                                                                async move {
                                                                    sleep(Duration::from_secs(10)).await;
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
                                                                        &profile,
                                                                        &additional_connection_params,
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
                                                                            error!("tunnel error: {}", e);
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

                                                    if let dashmap::mapref::entry::Entry::Occupied(
                                                        mut tunnel_entry,
                                                    ) = tunnels.entry(hostname.clone())
                                                    {
                                                        tunnel_entry.get_mut().remove(&tunnel_index);

                                                        if tunnel_entry.get().is_empty() {
                                                            tunnel_entry.remove_entry();
                                                        }
                                                    }
                                                }
                                            }.instrument(
                                                tracing::info_span!(
                                            "tunnels",
                                            gw = %hostname
                                        ),
                                            )
                                        });
                                    }
                                }
                            }
                            WsCloudToInstanceMessage::ConfigUpdateResult(config_update_result) => {
                                match config_update_result {
                                    ConfigUpdateResult::Error { msg } => {
                                        error!("error updating config: {}. Previous version is still active", msg);
                                    }
                                    ConfigUpdateResult::Ok { base_urls } => {
                                        info!("updated config is now active.");
                                        info!(
                                            "Base URLs now served by the client: {}",
                                            itertools::join(
                                                base_urls.iter().map(|b| format!("https://{}", b)),
                                                ", "
                                            )
                                        );
                                    }
                                }
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
//     use tokio::time::sleep;
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
//         sleep(Duration::from_secs(1)).await;
//
//         stop_tx.stop(());
//
//         bg.await.unwrap();
//     }
// }
