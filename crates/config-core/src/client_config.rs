use crate::config::UpstreamDefinition;
use crate::Config;
use exogress_entities::Upstream;

#[derive(Debug, Clone)]
pub struct ClientConfig {
    config: Config,
}

impl From<Config> for ClientConfig {
    fn from(config: Config) -> Self {
        ClientConfig { config }
    }
}

impl AsRef<Config> for ClientConfig {
    fn as_ref(&self) -> &Config {
        &self.config
    }
}

impl ClientConfig {
    pub fn resolve_upstream(&self, upstream: &Upstream) -> Option<UpstreamDefinition> {
        info!(
            "upstreams = {:?}, looking for {:?}",
            self.config.upstreams, upstream
        );
        self.config.upstreams.get(upstream).cloned()
    }
}
