use std::{collections::HashMap, sync::Arc};

use anyhow::{Context as _, Result};
use kodik_parser::KodikApiResponse;
use mpv_client::Handle;
use reqwest::{Certificate, Client, cookie::Jar};
use tokio::runtime::{Builder, Runtime};

use crate::{config::Config, events::MetaData, mpv_ext::MpvExt};

pub struct PluginState {
    client: Client,
    runtime: Runtime,
    config: Config,
    kodik_videos: HashMap<String, KodikApiResponse>,
    metadata: HashMap<String, MetaData>,
    jar: Arc<Jar>,
}

impl PluginState {
    pub fn new(mp: &Handle) -> Result<Self> {
        let mut config: Config = mp.read_options();

        if let Some(cookies) = config.cookies() {
            let cookies_str = cookies.to_string_lossy();
            if cookies_str.starts_with('~') {
                config.set_cookies(Some(mp.expand_path(cookies_str)?));
            }
        }

        let jar = Arc::new(config.load_cookies()?);

        let certs = webpki_root_certs::TLS_SERVER_ROOT_CERTS
            .iter()
            .filter_map(|cert_der| Certificate::from_der(cert_der.as_ref()).ok());

        let client = Client::builder()
            .tls_certs_only(certs)
            .cookie_store(true)
            .gzip(true)
            .brotli(true)
            .zstd(true)
            .deflate(true)
            .cookie_provider(Arc::clone(&jar))
            .build()?;

        let runtime = Builder::new_multi_thread()
            .enable_all()
            .thread_name("kodik")
            .build()
            .context("failed to create tokio runtime")?;

        let kodik_videos = HashMap::new();
        let metadata = HashMap::new();

        Ok(Self {
            client,
            runtime,
            config,
            kodik_videos,
            metadata,
            jar,
        })
    }

    pub const fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    pub const fn client(&self) -> &Client {
        &self.client
    }

    pub const fn config(&self) -> &Config {
        &self.config
    }

    pub const fn kodik_videos_mut(&mut self) -> &mut HashMap<String, KodikApiResponse> {
        &mut self.kodik_videos
    }

    pub const fn kodik_videos(&self) -> &HashMap<String, KodikApiResponse> {
        &self.kodik_videos
    }

    pub fn jar(&self) -> &Jar {
        &self.jar
    }

    pub const fn metadata_mut(&mut self) -> &mut HashMap<String, MetaData> {
        &mut self.metadata
    }

    pub const fn metadata(&self) -> &HashMap<String, MetaData> {
        &self.metadata
    }

    pub const fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }
}
