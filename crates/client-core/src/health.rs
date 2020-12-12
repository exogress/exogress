//! Upstream healthchecks

use core::mem;
use dashmap::DashMap;
use exogress_config_core::{ClientConfig, Probe, UpstreamDefinition};
use exogress_entities::{HealthCheckProbeName, Upstream};
use exogress_signaling::{ProbeHealthStatus, UnhealthyReason};
use futures::channel::{mpsc, oneshot};
use futures::SinkExt;
use hashbrown::{HashMap, HashSet};
use http::{Method, Request, Response, StatusCode};
use hyper::client::HttpConnector;
use hyper::{Body, Error};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Handle;
use tracing::Level;
use tracing_futures::Instrument;
use url::Url;

pub struct HealthCheckProbeInner {
    probe: Probe,
    upstream: Upstream,
    probe_url: Url,

    status: ProbeHealthStatus,
    probe_name: HealthCheckProbeName,
}

#[derive(Debug)]
pub struct ProbeStatusUpdate {
    pub upstream: Upstream,
    pub probe: HealthCheckProbeName,
    pub status: Option<ProbeHealthStatus>,
}

pub struct HealthCheckProbe {
    inner: Arc<Mutex<HealthCheckProbeInner>>,
    update_tx: mpsc::Sender<ProbeStatusUpdate>,
    handle: Handle,
    stop_tx: oneshot::Sender<()>,
}

pub async fn start_checker(
    probe_inner: Arc<Mutex<HealthCheckProbeInner>>,
    update_tx: mpsc::Sender<ProbeStatusUpdate>,
    stop_rx: oneshot::Receiver<()>,
    hyper_client: hyper::Client<HttpConnector>,
) {
    let locked = probe_inner.lock();

    let mut interval = tokio::time::interval(locked.probe.target.period);
    let target = locked.probe.target.clone();
    let url = locked.probe_url.clone();
    mem::drop(locked);

    let mut update_tx = update_tx.clone();

    let probe_inner = probe_inner.clone();

    let upstream = probe_inner.lock().upstream.clone();
    let probe_name = probe_inner.lock().probe_name.clone();

    tokio::spawn(
        {
            shadow_clone!(upstream);
            shadow_clone!(probe_name);

            async move {
                let check = {
                    shadow_clone!(mut update_tx);
                    shadow_clone!(upstream);

                    async move {
                        loop {
                            interval.tick().await;
                            let mut health_request = Request::builder()
                                .uri(url.as_str())
                                .method(Method::GET)
                                .body(Body::empty())
                                .unwrap();

                            let was_status = probe_inner.lock().status.clone();

                            let res = tokio::time::timeout(
                                target.timeout,
                                hyper_client.request(health_request),
                            )
                            .await;

                            {
                                let mut probe_locked = probe_inner.lock();
                                match res {
                                    Ok(Ok(res)) => {
                                        if !res.status().is_success() {
                                            probe_locked.status = ProbeHealthStatus::Unhealthy {
                                                reason: UnhealthyReason::BadStatus {
                                                    status: res.status(),
                                                },
                                            };
                                        } else {
                                            probe_locked.status = ProbeHealthStatus::Healthy;
                                        }
                                    }
                                    Ok(Err(e)) => {
                                        probe_locked.status = ProbeHealthStatus::Unhealthy {
                                            reason: UnhealthyReason::RequestError {
                                                err: e.to_string(),
                                            },
                                        };
                                    }
                                    Err(_) => {
                                        probe_locked.status = ProbeHealthStatus::Unhealthy {
                                            reason: UnhealthyReason::Timeout,
                                        };
                                    }
                                }
                            }

                            let new_status = probe_inner.lock().status.clone();

                            if was_status != new_status {
                                update_tx
                                    .send(ProbeStatusUpdate {
                                        upstream: upstream.clone(),
                                        probe: probe_name.clone(),
                                        status: Some(new_status),
                                    })
                                    .await?;
                            }
                        }

                        Ok::<_, mpsc::SendError>(())
                    }
                };

                tokio::select! {
                    r = check => {
                        error!("healthchecker unexpectedly stopped: {:?}", r);
                        r?;
                    },
                    _ = stop_rx => {},
                }

                Ok::<_, mpsc::SendError>(())
            }
        }
        .instrument(tracing::info_span!(
            "healthcheck",
            upstream = upstream.as_str(),
            probe_name = probe_name.as_str()
        )),
    );
}

impl HealthCheckProbe {
    pub fn new(
        probe_name: HealthCheckProbeName,
        probe: Probe,
        upstream: Upstream,
        upstream_definition: UpstreamDefinition,
        update_tx: mpsc::Sender<ProbeStatusUpdate>,
        handle: Handle,
    ) -> Result<Self, url::ParseError> {
        let (stop_tx, stop_rx) = oneshot::channel();

        let mut probe_url: Url = format!(
            "http://{}:{}{}",
            upstream_definition.get_host(),
            upstream_definition.addr.port,
            probe.target.path
        )
        .parse()?;

        let probe = HealthCheckProbe {
            inner: Arc::new(Mutex::new(HealthCheckProbeInner {
                probe,
                upstream,
                probe_url,
                status: ProbeHealthStatus::default(),
                probe_name,
            })),
            update_tx,
            stop_tx,
            handle: handle.clone(),
        };

        handle.spawn(start_checker(
            probe.inner.clone(),
            probe.update_tx.clone(),
            stop_rx,
            hyper::Client::new(),
        ));

        Ok(probe)
    }

    pub fn update(
        &mut self,
        probe: Probe,
        upstream_definition: UpstreamDefinition,
    ) -> Result<(), url::ParseError> {
        let (stop_tx, stop_rx) = oneshot::channel();

        let mut probe_url: Url = format!(
            "http://{}:{}{}",
            upstream_definition.get_host(),
            upstream_definition.addr.port,
            probe.target.path
        )
        .parse()?;
        {
            let mut locked = self.inner.lock();

            if locked.probe == probe && locked.probe_url == probe_url {
                return Ok(());
            }

            locked.probe = probe;
            locked.probe_url = probe_url;
        }
        self.stop_tx = stop_tx;

        self.handle.spawn(start_checker(
            self.inner.clone(),
            self.update_tx.clone(),
            stop_rx,
            hyper::Client::new(),
        ));

        Ok(())
    }
}

#[derive(Clone)]
pub struct UpstreamsHealth {
    inner: Arc<Mutex<HashMap<Upstream, HashMap<HealthCheckProbeName, HealthCheckProbe>>>>,
    update_tx: mpsc::Sender<ProbeStatusUpdate>,
    handle: Handle,
}

impl UpstreamsHealth {
    pub fn dump_health(
        &self,
    ) -> HashMap<Upstream, HashMap<HealthCheckProbeName, ProbeHealthStatus>> {
        self.inner
            .lock()
            .iter()
            .map(|(upstream, probes)| {
                (
                    upstream.clone(),
                    probes
                        .iter()
                        .map(|(probe, health)| (probe.clone(), health.inner.lock().status.clone()))
                        .collect::<HashMap<_, _>>(),
                )
            })
            .collect::<HashMap<_, _>>()
    }

    pub fn new(
        config: &ClientConfig,
        update_tx: mpsc::Sender<ProbeStatusUpdate>,
        handle: Handle,
    ) -> Result<Self, url::ParseError> {
        let mut storage =
            HashMap::<Upstream, HashMap<HealthCheckProbeName, HealthCheckProbe>>::new();

        for (upstream, upstream_definition) in &config.upstreams {
            let entry = storage.entry(upstream.clone()).or_default();
            for (probe_name, probe) in &upstream_definition.health_checks {
                entry.insert(
                    probe_name.clone(),
                    HealthCheckProbe::new(
                        probe_name.clone(),
                        probe.clone(),
                        upstream.clone(),
                        upstream_definition.clone(),
                        update_tx.clone(),
                        handle.clone(),
                    )?,
                );
            }
        }

        Ok(UpstreamsHealth {
            inner: Arc::new(Mutex::new(storage)),
            update_tx,
            handle,
        })
    }

    pub async fn sync_probes(&self, config: &ClientConfig) {
        let mut update_tx = self.update_tx.clone();

        let locked = &mut *self.inner.lock();
        let new_upstreams: HashSet<_> = config.upstreams.keys().cloned().collect();
        let existing_upstreams: HashSet<_> = locked.keys().cloned().collect();

        let span = span!(Level::INFO, "healthcheck config");
        let _enter = span.enter();

        for to_delete_upstream in existing_upstreams.difference(&new_upstreams) {
            let removed_probes = locked.remove(to_delete_upstream).unwrap();
            for (probe_name, probe) in removed_probes.into_iter() {
                let _ = update_tx
                    .send(ProbeStatusUpdate {
                        upstream: to_delete_upstream.clone(),
                        probe: probe_name,
                        status: None,
                    })
                    .await;
            }
        }

        for to_create_upstream in new_upstreams.difference(&existing_upstreams) {
            locked.insert(to_create_upstream.clone(), Default::default());
        }

        for (upstream_name, existing_probes) in locked {
            let span = span!(Level::INFO, "", upstream = upstream_name.as_str());
            let _enter = span.enter();

            let new_upstream_probe_names: HashSet<_> = config
                .upstreams
                .get(upstream_name)
                .unwrap()
                .health_checks
                .keys()
                .cloned()
                .collect();
            let existing_probe_names: HashSet<_> = existing_probes.keys().cloned().collect();

            for to_delete_probe in existing_probe_names.difference(&new_upstream_probe_names) {
                let span = span!(Level::INFO, "", probe = to_delete_probe.as_str());
                let _enter = span.enter();

                existing_probes.remove(to_delete_probe);

                let _ = update_tx
                    .send(ProbeStatusUpdate {
                        upstream: upstream_name.clone(),
                        probe: to_delete_probe.clone(),
                        status: None,
                    })
                    .await;
            }

            for to_create_probe in new_upstream_probe_names.difference(&existing_probe_names) {
                let span = span!(Level::INFO, "", probe = to_create_probe.as_str());
                let _enter = span.enter();

                let upstream = config.upstreams.get(upstream_name).unwrap();
                let probe = upstream.health_checks.get(to_create_probe).unwrap();
                match HealthCheckProbe::new(
                    to_create_probe.clone(),
                    probe.clone(),
                    upstream_name.clone(),
                    upstream.clone(),
                    update_tx.clone(),
                    self.handle.clone(),
                ) {
                    Ok(r) => {
                        existing_probes.insert(to_create_probe.clone(), r);
                    }
                    Err(e) => {
                        error!("failed to create probe: {}", e);
                    }
                }
            }

            for to_modify_probe in new_upstream_probe_names.intersection(&existing_probe_names) {
                let span = span!(Level::INFO, "", probe = to_modify_probe.as_str());
                let _enter = span.enter();

                let upstream = config.upstreams.get(upstream_name).unwrap();
                let probe = upstream.health_checks.get(to_modify_probe).unwrap();
                if let Err(e) = existing_probes
                    .get_mut(to_modify_probe)
                    .unwrap()
                    .update(probe.clone(), upstream.clone())
                {
                    error!("failed to modify probe. error: {}", e);
                }
            }
        }
    }
}
