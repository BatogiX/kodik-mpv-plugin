use std::{collections::HashMap, sync::Arc};

use anyhow::{Context as _, Result};
use kodik_shiki::KodikApiResponse;
use mpv_client::Handle;
use reqwest::{Client, cookie::Jar};
use tokio::runtime::{Builder, Runtime};

use crate::{config::Config, events::MetaData};

pub struct PluginState {
    client: Client,
    runtime: Runtime,
    config: Config,
    kodik_videos: HashMap<String, KodikApiResponse>,
    metadata: HashMap<String, MetaData>,
    jar: Arc<Jar>,
}

impl PluginState {
    pub fn new(mpv: &mut Handle) -> Result<Self> {
        let config = Config::load(mpv)?;
        let jar = Arc::new(config.load_cookies()?);

        let client = Client::builder()
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
