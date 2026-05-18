use std::{collections::HashMap, sync::Arc};

use anyhow::{Context as _, Result};
use kodik_shiki::KodikApiResponse;
use mpv_client::{Handle, mpv_handle};
use reqwest::Client;
use tokio::runtime::{Builder, Runtime};

use crate::config::Config;

pub struct PluginState<'a> {
    mpv: &'a mut Handle,
    client: Client,
    runtime: Runtime,
    config: Config,
    kodik_videos: HashMap<String, KodikApiResponse>,
}

impl PluginState<'_> {
    pub fn new(handle: *mut mpv_handle) -> Result<Self> {
        let mpv = Handle::from_ptr(handle);
        let config = Config::load(mpv)?;

        let client = Client::builder()
            .cookie_store(true)
            .gzip(true)
            .brotli(true)
            .zstd(true)
            .deflate(true)
            .cookie_provider(Arc::new(config.load_cookies()?))
            .build()?;

        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .context("failed to create tokio runtime")?;

        let kodik_videos = HashMap::new();

        Ok(Self {
            mpv,
            client,
            runtime,
            config,
            kodik_videos,
        })
    }

    pub const fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    pub const fn client(&self) -> &Client {
        &self.client
    }

    pub const fn mpv_mut(&mut self) -> &mut Handle {
        &mut *self.mpv
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
}
